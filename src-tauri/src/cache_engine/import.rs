//! Candidate generations are owned by an import attempt until Core assigns IDs.
use super::*;
use myalbuns_imaging_protocol::{PhotoImportCandidate, PhotoImportSourceId};

struct PreparedImportGeneration {
    candidate: PhotoImportCandidate,
    source: MediaObservation,
    generation: CacheReusableGeneration,
    preview_sha256: [u8; 32],
}

pub(crate) struct CacheImportStage {
    namespace: AuthorizedCacheNamespace,
    attempt_id: String,
    storage: PreparedCacheStorage,
    cleanup: HashSet<PathBuf>,
    prepared: Vec<PreparedImportGeneration>,
    active_imports: Arc<AtomicUsize>,
    cleanup_deferred: bool,
}

impl CacheEngine {
    pub(crate) fn begin_import_stage(
        &self,
        app_paths: &AppPaths,
        namespace: AuthorizedCacheNamespace,
        attempt_id: String,
        candidates: &[PhotoImportCandidate],
    ) -> Result<CacheImportStage, String> {
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|error| error.to_string())?;
        let mut cleanup = HashSet::new();
        for candidate in candidates {
            for format in [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png] {
                cleanup.insert(
                    namespace
                        .paths()
                        .import_preview_file(
                            &attempt_id,
                            candidate.source_id.as_str(),
                            &candidate.generation_id,
                            format,
                        )
                        .map_err(|error| error.to_string())?,
                );
            }
        }
        self.active_imports.fetch_add(1, Ordering::AcqRel);
        Ok(CacheImportStage {
            namespace,
            attempt_id,
            storage,
            cleanup,
            prepared: Vec::new(),
            active_imports: Arc::clone(&self.active_imports),
            cleanup_deferred: false,
        })
    }

    pub(crate) fn publish_import_stage(
        &self,
        stage: &mut CacheImportStage,
        bindings: &[MediaBinding],
        root_bindings: &RootBindingPlan,
        observation_generation: u64,
    ) -> Vec<(PhotoImportSourceId, String)> {
        let _guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let paths = stage.namespace.paths();
        // Reload at the commit boundary: planning/inspection never authorizes
        // publication against an old or externally replaced index snapshot.
        let mut index =
            CacheIndex::read_or_empty(&stage.storage, paths, stage.namespace.project_id());
        let by_path = bindings
            .iter()
            .map(|binding| ((binding.kind, binding.logical_path.as_path()), binding))
            .collect::<HashMap<_, _>>();
        let mut problems = Vec::new();
        let mut receipts = Vec::new();
        let mut obsolete = Vec::new();
        let mut published_paths = Vec::new();
        let mut adopted_ids = Vec::new();
        for prepared in std::mem::take(&mut stage.prepared) {
            let source_id = prepared.candidate.source_id.clone();
            let result = (|| -> Result<(), String> {
                let binding = by_path
                    .get(&(prepared.source.kind, prepared.candidate.path()))
                    .ok_or("A imagem não pertence mais ao Projeto.")?;
                let current = MediaResolver.observe_in_plan(root_bindings, binding);
                if !prepared.source.same_source(&current)
                    || !current.matches_fingerprint(&prepared.generation.fingerprint)
                {
                    return Err("O Original mudou antes da publicação da prévia.".into());
                }
                let candidate_path = paths
                    .import_preview_file(
                        &stage.attempt_id,
                        source_id.as_str(),
                        &prepared.generation.generation_id,
                        prepared.generation.format,
                    )
                    .map_err(|error| error.to_string())?;
                if preview_digest(&stage.storage, &candidate_path, &prepared.generation, false)?
                    != prepared.preview_sha256
                {
                    return Err("A prévia candidata mudou antes da publicação.".into());
                }
                let artifact = artifact_for_media(&binding.media_id, &prepared.generation);
                let target = paths
                    .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
                    .map_err(|error| error.to_string())?;
                stage
                    .storage
                    .relocate_generation(&candidate_path, &target)
                    .map_err(|error| error.to_string())?;
                // Until the index commits, both the old and new names belong to
                // this guard and are discarded on every failure/early return.
                stage.cleanup.insert(target.clone());
                let source = CacheMediaSource::new(
                    binding.media_id.clone(),
                    binding.kind,
                    binding.logical_path.clone(),
                )?;
                let entry = cache_metadata_entry(paths, &source, &artifact)
                    .map_err(|error| error.message)?;
                if let Some(previous) = index.upsert(entry)
                    && let Ok(path) = entry_path(paths, &previous)
                    && path != target
                {
                    obsolete.push(path);
                }
                receipts.push((
                    (
                        stage.namespace.project_id().to_owned(),
                        binding.media_id.clone(),
                    ),
                    PreparedCacheGeneration {
                        source: current,
                        artifact,
                        preview_sha256: prepared.preview_sha256,
                    },
                ));
                published_paths.push(target);
                adopted_ids.push(source_id.clone());
                Ok(())
            })();
            if let Err(reason) = result {
                problems.push((source_id, reason));
            }
        }
        if !receipts.is_empty() {
            match index.commit(&stage.storage, paths, stage.namespace.project_id()) {
                Ok(_) => {
                    for path in published_paths {
                        stage.cleanup.remove(&path);
                    }
                    // A delayed notification predating this adoption cannot
                    // invalidate its successor generation after publication.
                    let mut generations = self
                        .applied_observation_generations
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for (key, _) in &receipts {
                        let applied = generations.entry(key.clone()).or_default();
                        *applied = (*applied).max(observation_generation);
                    }
                    drop(generations);
                    self.prepared
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .extend(receipts);
                    for path in obsolete {
                        if let Err(error) = stage.storage.remove_existing_file(&path) {
                            tracing::warn!(target: "myalbuns.desktop", error = %error, event = "cache_superseded_generation_cleanup_failed");
                        }
                    }
                }
                Err(error) => problems.extend(
                    adopted_ids
                        .into_iter()
                        .map(|id| (id, error.message.clone())),
                ),
            }
        }
        problems
    }
}

impl CacheImportStage {
    pub(crate) fn discard_sources(&mut self, paths: &HashSet<PathBuf>) {
        self.prepared
            .retain(|prepared| !paths.contains(prepared.candidate.path()));
    }

    pub(crate) fn defer_cleanup_until_restart(&mut self) {
        // An unconfirmed child keeps its durable writer claim. Keep the sweep
        // exclusion as well; namespace recovery cleans up after its death.
        self.cleanup_deferred = true;
    }

    pub(crate) fn record(
        &mut self,
        candidate: PhotoImportCandidate,
        source: MediaObservation,
        generation: CacheReusableGeneration,
    ) -> Result<(), String> {
        if candidate.path() != source.logical_path()
            || candidate.generation_id != generation.generation_id
            || !source.matches_fingerprint(&generation.fingerprint)
        {
            return Err("A preparação não corresponde à fonte importada.".into());
        }
        let path = self
            .namespace
            .paths()
            .import_preview_file(
                &self.attempt_id,
                candidate.source_id.as_str(),
                &generation.generation_id,
                generation.format,
            )
            .map_err(|error| error.to_string())?;
        let preview_sha256 = preview_digest(&self.storage, &path, &generation, true)?;
        self.prepared.push(PreparedImportGeneration {
            candidate,
            source,
            generation,
            preview_sha256,
        });
        Ok(())
    }
}

pub(super) fn preview_digest(
    storage: &PreparedCacheStorage,
    path: &Path,
    generation: &CacheReusableGeneration,
    decode: bool,
) -> Result<[u8; 32], String> {
    let file = storage
        .open_existing_file(path)
        .map_err(|error| error.to_string())?
        .ok_or("A prévia candidata não foi encontrada.")?;
    let payload = crate::opaque_image_protocol::read_image(file, true)
        .map_err(|_| "Não foi possível ler a prévia candidata.")?;
    if payload.source_bytes != generation.preview_bytes
        || payload.body.len() as u64 != generation.preview_bytes
    {
        return Err("A prévia candidata mudou de tamanho.".into());
    }
    if decode {
        validate_cache_preview(
            std::io::Cursor::new(&payload.body),
            CachePreviewSpec {
                format: generation.format,
                width_px: generation.width_px,
                height_px: generation.height_px,
                bytes: generation.preview_bytes,
            },
        )?;
    }
    Ok(Sha256::digest(&payload.body).into())
}

fn artifact_for_media(media_id: &str, generation: &CacheReusableGeneration) -> CacheArtifact {
    CacheArtifact {
        media_id: media_id.to_owned(),
        generation_id: generation.generation_id.clone(),
        width_px: generation.width_px,
        height_px: generation.height_px,
        preview_bytes: generation.preview_bytes,
        format: generation.format,
        exif_orientation: generation.exif_orientation,
        source_page_count: generation.source_page_count,
        basic_color_profile: generation.basic_color_profile,
        fingerprint: generation.fingerprint.clone(),
    }
}

impl Drop for CacheImportStage {
    fn drop(&mut self) {
        if self.cleanup_deferred {
            return;
        }
        for path in &self.cleanup {
            if let Err(error) = self.storage.remove_existing_file(path) {
                tracing::warn!(target: "myalbuns.desktop", error = %error, event = "import_candidate_cleanup_failed");
            }
        }
        self.active_imports.fetch_sub(1, Ordering::AcqRel);
    }
}
