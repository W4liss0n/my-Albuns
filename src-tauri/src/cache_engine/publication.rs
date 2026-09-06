//! An operation owns its planning snapshot and unpublished generations. The
//! publication boundary reloads the index under the Engine's transition gate.
use super::*;

pub(crate) struct PendingCachePublication {
    pub(super) storage: PreparedCacheStorage,
    pub(super) request: CacheRequest,
    pub(super) work: CacheWork,
    pub(super) execution: CacheExecution,
    pub(super) observed_source: MediaObservation,
    pub(super) preview_sha256: [u8; 32],
    pub(super) cancellation: CacheCancellation,
    pub(super) published: bool,
}

impl Drop for PendingCachePublication {
    fn drop(&mut self) {
        if !self.published
            && let Err(error) = discard_candidate_generation(&self.storage, &self.request)
        {
            tracing::warn!(target: "myalbuns.desktop", error = %error.message,
                event = "cache_candidate_cleanup_failed");
        }
    }
}

impl CacheEngine {
    pub(crate) fn plan_works(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        roots: &RootBindingPlan,
        sources: impl IntoIterator<Item = CacheMediaSource>,
    ) -> Result<Vec<CacheWork>, CacheFailure> {
        let sources = sources.into_iter().collect::<Vec<_>>();
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        let _guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::Plan,
                    format!("Não foi possível ler o índice do Cache: {error}"),
                )
            })?;
        let index = Arc::new(CacheIndex::read_or_empty(
            &storage,
            namespace.paths(),
            namespace.project_id(),
        ));
        Ok(sources
            .into_iter()
            .map(|source| {
                let mut work = CacheWork::new(
                    format!("cache-{}", uuid::Uuid::new_v4().simple()),
                    namespace.clone(),
                    source,
                    roots.clone(),
                );
                work.planned_index = Some(Arc::clone(&index));
                work
            })
            .collect())
    }

    /// Callers keep their flight owners until this returns and hold an activity
    /// permit, so causal mutations cannot overtake the commit or its waiters.
    pub(crate) fn publish_prepared_batch(
        &self,
        mut pending: Vec<PendingCachePublication>,
    ) -> Vec<FlightResult> {
        let Some(first) = pending.first() else {
            return Vec::new();
        };
        let _guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let namespace = first.work.namespace.clone();
        let mut index =
            CacheIndex::read_or_empty(&first.storage, namespace.paths(), namespace.project_id());
        let mut superseded = Vec::new();
        let mut accepted = Vec::new();
        let mut results = Vec::with_capacity(pending.len());
        for (position, item) in pending.iter().enumerate() {
            let result = (|| {
                if item.work.namespace.project_id() != namespace.project_id()
                    || item.work.namespace.paths() != namespace.paths()
                {
                    return Err(CacheFailure::new(
                        CacheFailureStage::PublishIndex,
                        "O lote de Cache pertence a outro namespace.",
                    ));
                }
                if item.cancellation.flag().load(Ordering::Acquire) {
                    return Err(cancelled_before_publication());
                }
                let current = MediaResolver.observe_in_plan(
                    &item.work.root_bindings,
                    &cache_source_binding(&item.work.source),
                );
                let artifact = item.execution.artifact();
                if !item.observed_source.same_source(&current)
                    || !current.matches_fingerprint(&artifact.fingerprint)
                {
                    return Err(CacheFailure::new(
                        CacheFailureStage::VerifyArtifacts,
                        "O Original mudou antes da publicação da prévia.",
                    ));
                }
                // Complete validation already decoded this reduced representation.
                // A digest now proves that the exact validated bytes still exist.
                let generation = CacheReusableGeneration::new(
                    artifact.generation_id.clone(),
                    CacheArtifactProperties::new(
                        artifact.format,
                        artifact.width_px,
                        artifact.height_px,
                        artifact.preview_bytes,
                        artifact.exif_orientation,
                        artifact.source_page_count,
                        artifact.basic_color_profile,
                    ),
                    artifact.fingerprint.clone(),
                )
                .map_err(invalid_artifact)?;
                let path = namespace
                    .paths()
                    .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
                    .map_err(|error| invalid_artifact(error.to_string()))?;
                if import::preview_digest(&item.storage, &path, &generation, false)
                    .map_err(invalid_artifact)?
                    != item.preview_sha256
                {
                    return Err(invalid_artifact(
                        "a prévia mudou antes da publicação".into(),
                    ));
                }
                let entry = cache_metadata_entry(namespace.paths(), &item.work.source, artifact)?;
                if let Some(previous) = index.upsert(entry)
                    && let Ok(previous_path) = entry_path(namespace.paths(), &previous)
                    && previous_path != path
                {
                    superseded.push(previous_path);
                }
                accepted.push((position, current));
                Ok(item.execution.clone())
            })();
            results.push(result);
        }
        if accepted.is_empty() {
            return results;
        }
        match index.commit(
            &pending[0].storage,
            namespace.paths(),
            namespace.project_id(),
        ) {
            Ok(metadata) => {
                let flights = self
                    .flights
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let mut prepared = self
                    .prepared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (position, source) in accepted {
                    let item = &mut pending[position];
                    item.published = true;
                    if flights
                        .get(&item.work.flight_key())
                        .is_some_and(|flight| flight.required_by_processing.load(Ordering::Acquire))
                    {
                        prepared.insert(
                            (
                                namespace.project_id().to_owned(),
                                item.work.source.media_id().to_owned(),
                            ),
                            PreparedCacheGeneration {
                                source,
                                artifact: item.execution.artifact().clone(),
                                preview_sha256: item.preview_sha256,
                            },
                        );
                    }
                }
                drop(prepared);
                drop(flights);
                for path in superseded {
                    if let Err(error) = pending[0].storage.remove_existing_file(&path) {
                        tracing::warn!(target: "myalbuns.desktop", error = %error,
                            event = "cache_superseded_generation_cleanup_failed");
                    }
                }
                if self.can_sweep_after_publication()
                    && let Err(error) = sweep_unreferenced_generations(
                        &pending[0].storage,
                        namespace.paths(),
                        &metadata,
                    )
                {
                    tracing::warn!(target: "myalbuns.desktop", error = %error.message,
                        event = "cache_generation_sweep_failed");
                }
            }
            Err(failure) => {
                for (position, _) in accepted {
                    results[position] = Err(failure.clone());
                }
            }
        }
        results
    }
}
