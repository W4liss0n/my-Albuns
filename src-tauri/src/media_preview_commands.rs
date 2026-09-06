use futures_util::StreamExt;
use std::collections::{HashMap, HashSet};

use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, ipc::Channel};

use crate::{
    cache_activity_gate::CacheCancellation,
    cache_engine::{
        self, AuthorizedCacheNamespace, CACHE_PROCESSOR_SUSPENDED_MESSAGE, CacheEngine,
        CacheFlightClaim, CacheProcessorStatus, CacheWork,
    },
    cache_previews::{CachePreviewError, CachePreviewRegistry},
    cache_service::ActiveCacheNamespace,
    image_processing::{ImageProcessingBatch, execute_owned_cache},
    imaging_processor::ImagingProcessor,
    ipc_contract::{
        CacheProcessorState, CacheProcessorWarning, LinkedMediaChanged, MediaPreview,
        MediaPreviewCommandError, MediaPreviewCommandErrorCode, MediaPreviewDemand,
        MediaPreviewState,
    },
    logging::LoggingState,
    media_runtime::{MediaAvailability, MediaMonitor, MediaRuntime},
    path_io,
    product_runtime::{
        CACHE_PROCESSOR_WARNING_EVENT, LINKED_MEDIA_CHANGED_EVENT, PROJECT_WINDOW_LABEL,
        refresh_project_photos_for_media_update,
    },
    project_host::ProjectHost,
};

impl From<CachePreviewError> for MediaPreviewCommandError {
    fn from(error: CachePreviewError) -> Self {
        let code = match error {
            CachePreviewError::Unavailable => MediaPreviewCommandErrorCode::Unavailable,
            CachePreviewError::InvalidDerivedArtifact => {
                MediaPreviewCommandErrorCode::UnsupportedImage
            }
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

impl MediaPreviewCommandError {
    fn read_failed() -> Self {
        Self {
            code: MediaPreviewCommandErrorCode::ReadFailed,
            message: "Não foi possível preparar as representações reduzidas do Projeto.".into(),
        }
    }

    fn retry_failed(error: impl std::fmt::Display) -> Self {
        Self {
            code: MediaPreviewCommandErrorCode::ReadFailed,
            message: format!("Não foi possível tentar novamente a inspeção da mídia: {error}"),
        }
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn retry_unavailable_media(
    media_id: String,
    on_progress: Channel<crate::ipc_contract::ImageProcessingProgress>,
    window: WebviewWindow,
    app: AppHandle,
    project_host: State<'_, ProjectHost>,
    engine: State<'_, CacheEngine>,
    registry: State<'_, CachePreviewRegistry>,
    media_runtime: State<'_, MediaRuntime>,
    media_monitor: State<'_, MediaMonitor>,
    namespace_owner: State<'_, ActiveCacheNamespace>,
) -> Result<MediaPreview, MediaPreviewCommandError> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let mut processing = ImageProcessingBatch::new(1, |progress| {
        let _ = on_progress.send(progress);
    });
    let _causal_cache_permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let catalog = project_host
        .authorized_media_catalog()
        .map_err(MediaPreviewCommandError::retry_failed)?;
    let retry_namespace = namespace_owner.namespace();
    if catalog.project_id != retry_namespace.project_id() {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let binding = catalog
        .bindings
        .into_iter()
        .find(|binding| binding.media_id == media_id)
        .ok_or_else(MediaPreviewCommandError::read_failed)?;
    let source_path = binding.logical_path.clone();
    let retry_binding = binding.clone();
    let monitor = media_monitor.inner().clone();
    let runtime = media_runtime.inner().clone();
    let retry_app = app.clone();
    let retry_registry = registry.inner().clone();
    let retry_host = project_host.inner().clone();
    let (inspection, refreshed_photo_ids) = tauri::async_runtime::spawn_blocking(move || {
        let inspection = monitor.retry_unavailable(&runtime, &binding, |update| {
            retry_app.state::<CacheEngine>().apply_monitor_media_update(
                &retry_namespace,
                &retry_registry,
                update,
            );
        })?;
        let refreshed_photo_ids = refresh_project_photos_for_media_update(
            &retry_host,
            std::slice::from_ref(&binding),
            inspection.update(),
        );
        Ok::<_, crate::media_runtime::MediaRetryError>((inspection, refreshed_photo_ids))
    })
    .await
    .map_err(MediaPreviewCommandError::retry_failed)?
    .map_err(MediaPreviewCommandError::retry_failed)?;
    tracing::info!(
        target: "myalbuns.desktop",
        media_id,
        changed = !inspection.update().changed_media_ids().is_empty(),
        invalidated = !inspection.update().invalidated_media_ids().is_empty(),
        event = "linked_media_retry_adopted",
    );
    if let Some(change) =
        linked_media_change_for_update(inspection.update(), &refreshed_photo_ids, true)
    {
        window
            .emit(LINKED_MEDIA_CHANGED_EVENT, change)
            .map_err(|_| MediaPreviewCommandError::read_failed())?;
    }
    let state = preview_state(inspection.availability());
    drop(_causal_cache_permit);
    if state == MediaPreviewState::Ready {
        processing.prepare(&app, &retry_binding).await;
    } else {
        processing.complete(Some(crate::ipc_contract::ImageProcessingProblem {
            file_name: source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            reason: match state {
                MediaPreviewState::Absent => "O arquivo da imagem não foi encontrado. Use Religar para escolher sua nova localização.",
                _ => "A origem da imagem continua indisponível. Reconecte o local e tente novamente.",
            }.into(),
        }));
    }
    let retained = (state != MediaPreviewState::Ready)
        .then(|| registry.retained_preview(&media_id, &source_path, state))
        .flatten();
    Ok(retained.unwrap_or(MediaPreview {
        media_id,
        state,
        url: None,
    }))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn prepare_media_previews(
    demand: MediaPreviewDemand,
    on_preview: Channel<MediaPreview>,
    window: WebviewWindow,
    app: AppHandle,
    project_host: State<'_, ProjectHost>,
    engine: State<'_, CacheEngine>,
    registry: State<'_, CachePreviewRegistry>,
    media_runtime: State<'_, MediaRuntime>,
    media_monitor: State<'_, MediaMonitor>,
    processor: State<'_, ImagingProcessor>,
    logging: State<'_, LoggingState>,
    app_paths: State<'_, AppPaths>,
    namespace_owner: State<'_, ActiveCacheNamespace>,
) -> Result<Option<Vec<MediaPreview>>, MediaPreviewCommandError> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let causal_cache_permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let catalog = project_host
        .authorized_media_catalog()
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
    let ordered_demand = ordered_demand(&demand);
    let catalog_by_id = catalog
        .bindings
        .iter()
        .map(|binding| (binding.media_id.as_str(), binding))
        .collect::<HashMap<_, _>>();
    if ordered_demand
        .iter()
        .any(|media_id| !catalog_by_id.contains_key(media_id.as_str()))
    {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let namespace = namespace_owner.namespace();
    if catalog.project_id != namespace.project_id() {
        return Err(MediaPreviewCommandError::read_failed());
    }
    engine.retain_prepared_catalog(&catalog.project_id, &catalog.bindings);
    let mut demand_revision = engine.reconcile_preview_demand(
        registry.inner(),
        namespace.project_id(),
        demand.revision,
        ordered_demand.iter().map(String::as_str),
    );
    if ordered_demand.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if !engine.demand_is_current(&demand_revision) {
        return Ok(Some(Vec::new()));
    }

    let monitor = media_monitor.inner().clone();
    let runtime = media_runtime.inner().clone();
    let bindings = catalog.bindings.clone();
    let demand_host = project_host.inner().clone();
    let (poll, refreshed_photo_ids) = tauri::async_runtime::spawn_blocking(move || {
        let poll = monitor.poll(&runtime, &bindings);
        let refreshed_photo_ids = poll
            .update()
            .map(|update| refresh_project_photos_for_media_update(&demand_host, &bindings, update))
            .unwrap_or_default();
        (poll, refreshed_photo_ids)
    })
    .await
    .map_err(|_| MediaPreviewCommandError::read_failed())?;
    let runtime_update = poll.update().cloned();
    let observations = poll
        .confirmed_observation()
        .map(|proposal| {
            proposal
                .observations()
                .iter()
                .map(|observation| (observation.media_id.as_str(), observation.availability))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    if let Some(runtime_update) = runtime_update.as_ref()
        && (!runtime_update.changed_media_ids().is_empty()
            || !runtime_update.invalidated_media_ids().is_empty())
    {
        let cache_update = engine.apply_demand_media_update(
            &namespace,
            registry.inner(),
            &mut demand_revision,
            runtime_update,
        );
        if let Some(change) = linked_media_change_for_update(
            runtime_update,
            &refreshed_photo_ids,
            cache_update.retry_required(),
        ) {
            window
                .emit(LINKED_MEDIA_CHANGED_EVENT, change)
                .map_err(|_| MediaPreviewCommandError::read_failed())?;
        }
        if !cache_update.demand_can_resume() {
            return Ok(Some(Vec::new()));
        }
    }
    if !engine.demand_is_current(&demand_revision) {
        return Ok(Some(Vec::new()));
    }
    drop(causal_cache_permit);
    let mut previews = Vec::with_capacity(ordered_demand.len());
    let mut publish_preview = |preview: MediaPreview| {
        // A detached frontend must not turn a prepared cache artifact into a failure.
        let _ = on_preview.send(preview.clone());
        previews.push(preview);
    };
    let preparation = DemandedPreviewPreparation {
        app: &app,
        window: &window,
        engine: &engine,
        registry: &registry,
        processor: &processor,
        logging: &logging,
        app_paths: &app_paths,
        namespace: &namespace,
        demand_revision: &demand_revision,
    };
    let pending = futures_util::stream::iter(ordered_demand)
        .map(|media_id| {
            let binding = catalog_by_id[media_id.as_str()];
            let state = projected_preview_state(observations.get(media_id.as_str()).copied());
            preparation.prepare(binding, state)
        })
        .buffer_unordered(crate::imaging_processor::IMAGE_PROCESSING_CONCURRENCY);
    tokio::pin!(pending);
    let mut failure = None;
    // Drain active jobs even if the demand changes, so their cancellation and
    // exact process termination finish before their writer slots are reused.
    while let Some(result) = pending.next().await {
        match result {
            Ok(Some(preview)) if engine.demand_is_current(&demand_revision) => {
                publish_preview(preview)
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
            _ => {}
        }
    }
    if let Some(failure) = failure {
        return Err(failure);
    }
    if !engine.demand_is_current(&demand_revision) {
        return Ok(Some(Vec::new()));
    }
    Ok(Some(previews))
}

struct DemandedPreviewPreparation<'a> {
    app: &'a AppHandle,
    window: &'a WebviewWindow,
    engine: &'a CacheEngine,
    registry: &'a CachePreviewRegistry,
    processor: &'a ImagingProcessor,
    logging: &'a LoggingState,
    app_paths: &'a AppPaths,
    namespace: &'a AuthorizedCacheNamespace,
    demand_revision: &'a cache_engine::CacheDemandRevision,
}

impl DemandedPreviewPreparation<'_> {
    async fn prepare(
        &self,
        binding: &crate::media_runtime::MediaBinding,
        state: Option<MediaPreviewState>,
    ) -> Result<Option<MediaPreview>, MediaPreviewCommandError> {
        let Self {
            app,
            window,
            engine,
            registry,
            processor,
            logging,
            app_paths,
            namespace,
            demand_revision,
        } = *self;
        if !engine.demand_is_current(demand_revision) {
            return Ok(None);
        }
        let Some(state) = state else {
            return Ok(None);
        };
        let media_id = binding.media_id.as_str();
        let source = CacheMediaSource::new(
            binding.media_id.clone(),
            binding.kind,
            binding.logical_path.clone(),
        )
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
        if state != MediaPreviewState::Ready {
            return Ok(Some(contextual_preview(
                engine,
                registry,
                app_paths,
                namespace,
                demand_revision,
                &source,
                state,
            )));
        }
        // The stable Monitor has already revoked changed source bindings. A still
        // resident preview needs no new processor request or derived-file decode.
        if let Some(preview) = engine
            .commit_preview_if_demanded(demand_revision, media_id, || {
                registry.retained_preview(media_id, source.source_path(), state)
            })
            .flatten()
        {
            return Ok(Some(preview));
        }
        if let Some(preview) = engine.publish_prepared_if_demanded(
            app_paths,
            namespace,
            registry,
            demand_revision,
            &source,
        ) {
            return Ok(Some(preview));
        }
        let root_bindings = match path_io::capture_root_bindings(vec![
            namespace.paths().root().to_path_buf(),
            source.source_path().to_path_buf(),
        ])
        .await
        {
            Ok(root_bindings) => root_bindings,
            Err(_) => {
                return Ok(Some(contextual_preview(
                    engine,
                    registry,
                    app_paths,
                    namespace,
                    demand_revision,
                    &source,
                    cache_failure_state(),
                )));
            }
        };
        let request_id = format!("cache-{}", uuid::Uuid::new_v4().simple());
        let work = CacheWork::new(
            request_id.clone(),
            namespace.clone(),
            source.clone(),
            root_bindings,
        );
        let Some(claim) = engine.claim_demanded(demand_revision, &work) else {
            return Ok(None);
        };
        let preview_publication_authority = claim.preview_publication_authority();
        let execution = match claim {
            CacheFlightClaim::Waiter(waiter) => waiter.wait().await,
            CacheFlightClaim::Owner(owner) => {
                let cancellation = owner.cancellation();
                let result = execute_owned_cache(
                    app,
                    logging,
                    app_paths,
                    engine,
                    processor,
                    work,
                    cancellation,
                )
                .await;
                owner.complete(result)
            }
        };
        match execution {
            Ok(execution) => {
                if !engine.demand_is_current(demand_revision) {
                    return Ok(None);
                }
                if let Some(recovery) = execution.recovery {
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        failed_process_id = recovery.failed_process_id,
                        removed_temporary_count = recovery.removed_temporary_count,
                        media_id,
                        event = "cache_processor_recovered",
                    );
                }
                let Some(preview) = engine.commit_claimed_preview_if_demanded(
                    demand_revision,
                    &preview_publication_authority,
                    || {
                        registry.publish(
                            app_paths,
                            namespace,
                            execution.artifact(),
                            source.source_path(),
                        )
                    },
                ) else {
                    return Ok(None);
                };
                Ok(Some(cache_publication_or_context(
                    preview,
                    media_id,
                    || {
                        contextual_preview(
                            engine,
                            registry,
                            app_paths,
                            namespace,
                            demand_revision,
                            &source,
                            cache_failure_state(),
                        )
                    },
                )))
            }
            Err(failure) => {
                if engine.processor_status() == CacheProcessorStatus::Suspended
                    && let Err(error) = window.emit(
                        CACHE_PROCESSOR_WARNING_EVENT,
                        CacheProcessorWarning {
                            state: CacheProcessorState::Suspended,
                            message: CACHE_PROCESSOR_SUSPENDED_MESSAGE.into(),
                        },
                    )
                {
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "cache_processor_warning_emit_failed",
                    );
                }
                tracing::warn!(
                    target: "myalbuns.desktop",
                    stage = ?failure.stage,
                    exit_code = failure.exit_code,
                    message = failure.message,
                    media_id,
                    event = "cache_media_unavailable",
                );
                Ok(Some(contextual_preview(
                    engine,
                    registry,
                    app_paths,
                    namespace,
                    demand_revision,
                    &source,
                    cache_failure_state(),
                )))
            }
        }
    }
}

fn preview_state(availability: MediaAvailability) -> MediaPreviewState {
    match availability {
        MediaAvailability::Candidate => MediaPreviewState::Ready,
        MediaAvailability::Absent => MediaPreviewState::Absent,
        MediaAvailability::Unavailable => MediaPreviewState::Unavailable,
    }
}

fn linked_media_change_for_update(
    update: &crate::media_runtime::MediaRuntimeUpdate,
    refreshed_photo_ids: &[String],
    refresh_all_changed_media_ids: bool,
) -> Option<LinkedMediaChanged> {
    let media_ids = if refresh_all_changed_media_ids {
        update.changed_media_ids().to_vec()
    } else {
        refreshed_photo_ids.to_vec()
    };
    (!media_ids.is_empty()).then_some(LinkedMediaChanged { media_ids })
}

fn projected_preview_state(availability: Option<MediaAvailability>) -> Option<MediaPreviewState> {
    availability.map(preview_state)
}

fn cache_failure_state() -> MediaPreviewState {
    MediaPreviewState::CacheUnavailable
}

fn cache_publication_or_context(
    publication: Result<MediaPreview, CachePreviewError>,
    media_id: &str,
    context: impl FnOnce() -> MediaPreview,
) -> MediaPreview {
    match publication {
        Ok(preview) => preview,
        Err(error) => {
            tracing::warn!(
                target: "myalbuns.desktop",
                media_id,
                error = %error,
                event = "cache_preview_publication_failed",
            );
            context()
        }
    }
}

fn contextual_preview(
    engine: &CacheEngine,
    registry: &CachePreviewRegistry,
    app_paths: &AppPaths,
    namespace: &AuthorizedCacheNamespace,
    demand: &cache_engine::CacheDemandRevision,
    source: &CacheMediaSource,
    state: MediaPreviewState,
) -> MediaPreview {
    engine
        .retain_last_known_preview(app_paths, namespace, registry, demand, source, state)
        .unwrap_or(MediaPreview {
            media_id: source.media_id().to_owned(),
            state,
            url: None,
        })
}

fn ordered_demand(demand: &MediaPreviewDemand) -> Vec<String> {
    let mut seen = HashSet::new();
    demand
        .visible_media_ids
        .iter()
        .chain(&demand.preload_media_ids)
        .filter(|media_id| seen.insert(media_id.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        cache_previews::CachePreviewError,
        ipc_contract::{MediaPreviewDemand, MediaPreviewState},
        media_runtime::{MediaAvailability, MediaRuntimeUpdate},
    };

    use super::{
        cache_failure_state, cache_publication_or_context, linked_media_change_for_update,
        ordered_demand, projected_preview_state,
    };

    #[test]
    fn visible_media_precedes_preload_and_equivalent_demands_are_grouped() {
        let demand = MediaPreviewDemand {
            revision: 1,
            visible_media_ids: vec!["photo-visible".into(), "shared".into()],
            preload_media_ids: vec!["shared".into(), "photo-preload".into()],
        };

        assert_eq!(
            ordered_demand(&demand),
            ["photo-visible", "shared", "photo-preload"]
        );
    }

    #[test]
    fn only_authoritative_media_availability_is_projected_to_the_product() {
        assert_eq!(
            projected_preview_state(None),
            None,
            "an unconsolidated first sample cannot authorize a source recovery action"
        );
        assert_eq!(
            projected_preview_state(Some(MediaAvailability::Candidate)),
            Some(MediaPreviewState::Ready)
        );
        assert_eq!(
            projected_preview_state(Some(MediaAvailability::Absent)),
            Some(MediaPreviewState::Absent)
        );
        assert_eq!(
            projected_preview_state(Some(MediaAvailability::Unavailable)),
            Some(MediaPreviewState::Unavailable)
        );
        assert_ne!(
            projected_preview_state(Some(MediaAvailability::Unavailable)),
            Some(MediaPreviewState::CacheUnavailable),
            "a Cache failure is not an authoritative statement about the Original"
        );
        assert_eq!(cache_failure_state(), MediaPreviewState::CacheUnavailable);
        assert_ne!(
            cache_failure_state(),
            MediaPreviewState::Unavailable,
            "Processor, validation and Cache storage failures do not classify the Original"
        );
    }

    #[test]
    fn registry_publication_failure_becomes_cache_unavailable_without_source_retry() {
        let preview = cache_publication_or_context(
            Err(CachePreviewError::InvalidDerivedArtifact),
            "photo-a",
            || crate::ipc_contract::MediaPreview {
                media_id: "photo-a".into(),
                state: cache_failure_state(),
                url: None,
            },
        );

        assert_eq!(preview.media_id, "photo-a");
        assert_eq!(preview.state, MediaPreviewState::CacheUnavailable);
        assert_ne!(preview.state, MediaPreviewState::Unavailable);
        assert!(preview.url.is_none());
    }

    #[test]
    fn adopted_photo_refreshes_projection_and_retry_refreshes_changed_media() {
        let changed = MediaRuntimeUpdate::for_test_preserving_previews(
            17,
            vec!["photo-a".into(), "photo-b".into()],
        );

        assert_eq!(
            linked_media_change_for_update(&changed, &["photo-a".into()], false)
                .expect("an adopted change refreshes the WebView projection")
                .media_ids,
            ["photo-a"]
        );
        assert!(
            linked_media_change_for_update(&changed, &[], false).is_none(),
            "a change that did not rehydrate the Project does not reload its Projection"
        );
        assert_eq!(
            linked_media_change_for_update(&changed, &[], true)
                .expect("an explicit or causal retry refreshes every changed medium")
                .media_ids,
            ["photo-a", "photo-b"]
        );
        assert!(
            linked_media_change_for_update(&MediaRuntimeUpdate::default(), &[], false).is_none(),
            "an unchanged observation does not schedule redundant Project loads"
        );
    }
}
