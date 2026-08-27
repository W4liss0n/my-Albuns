use std::collections::{HashMap, HashSet};

use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        self, AuthorizedCacheNamespace, CACHE_PROCESSOR_SUSPENDED_MESSAGE, CacheEngine,
        CacheFailure, CacheFailureStage, CacheFlightClaim, CacheProcessorStatus, CacheWork,
    },
    cache_previews::{CachePreviewError, CachePreviewRegistry},
    cache_service::ActiveCacheNamespace,
    imaging_processor::{
        ImagingProcessor, InvocationContext, InvocationFailureStage, TauriImagingTransport,
    },
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
    for media_id in ordered_demand {
        if !engine.demand_is_current(&demand_revision) {
            return Ok(Some(Vec::new()));
        }
        let Some(state) = projected_preview_state(observations.get(media_id.as_str()).copied())
        else {
            continue;
        };
        let binding = catalog_by_id
            .get(media_id.as_str())
            .expect("validated demand remains in the immutable catalog");
        let source = CacheMediaSource::new(
            binding.media_id.clone(),
            binding.kind,
            binding.logical_path.clone(),
        )
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
        if state != MediaPreviewState::Ready {
            previews.push(contextual_preview(
                &engine,
                &registry,
                &app_paths,
                &namespace,
                &demand_revision,
                &source,
                state,
            ));
            continue;
        }
        let root_bindings = match path_io::capture_root_bindings(vec![
            namespace.paths().root().to_path_buf(),
            source.source_path().to_path_buf(),
        ])
        .await
        {
            Ok(root_bindings) => root_bindings,
            Err(_) => {
                previews.push(contextual_preview(
                    &engine,
                    &registry,
                    &app_paths,
                    &namespace,
                    &demand_revision,
                    &source,
                    cache_failure_state(),
                ));
                continue;
            }
        };
        let request_id = format!("cache-{}", uuid::Uuid::new_v4().simple());
        let work = CacheWork::new(
            request_id.clone(),
            namespace.clone(),
            source.clone(),
            root_bindings,
        );
        let Some(claim) = engine.claim_demanded(&demand_revision, &work) else {
            return Ok(Some(Vec::new()));
        };
        let preview_publication_authority = claim.preview_publication_authority();
        let execution = match claim {
            CacheFlightClaim::Waiter(waiter) => waiter.wait().await,
            CacheFlightClaim::Owner(owner) => {
                let cancellation = owner.cancellation();
                let result = execute_owned_cache(
                    &app,
                    &logging,
                    &app_paths,
                    &engine,
                    &processor,
                    work,
                    cancellation,
                )
                .await;
                owner.complete(result)
            }
        };
        match execution {
            Ok(execution) => {
                if !engine.demand_is_current(&demand_revision) {
                    return Ok(Some(Vec::new()));
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
                    &demand_revision,
                    &preview_publication_authority,
                    || {
                        registry.publish(
                            &app_paths,
                            &namespace,
                            execution.artifact(),
                            source.source_path(),
                        )
                    },
                ) else {
                    return Ok(Some(Vec::new()));
                };
                previews.push(cache_publication_or_context(
                    preview,
                    media_id.as_str(),
                    || {
                        contextual_preview(
                            &engine,
                            &registry,
                            &app_paths,
                            &namespace,
                            &demand_revision,
                            &source,
                            cache_failure_state(),
                        )
                    },
                ));
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
                previews.push(contextual_preview(
                    &engine,
                    &registry,
                    &app_paths,
                    &namespace,
                    &demand_revision,
                    &source,
                    cache_failure_state(),
                ));
            }
        }
    }
    Ok(Some(previews))
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

#[allow(clippy::too_many_arguments)]
async fn execute_owned_cache(
    app: &AppHandle,
    logging: &LoggingState,
    app_paths: &myalbuns_paths::AppPaths,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    work: CacheWork,
    cancellation: CacheCancellation,
) -> Result<cache_engine::CacheExecution, CacheFailure> {
    loop {
        match cancellation.reason() {
            Some(CacheCancellationReason::Obsolete) => {
                return Err(CacheFailure::new(
                    CacheFailureStage::Cancelled,
                    "A demanda de Cache ficou obsoleta.",
                ));
            }
            Some(CacheCancellationReason::Paused) if !cancellation.resume_after_pause() => {
                return Err(CacheFailure::new(
                    CacheFailureStage::Cancelled,
                    "A demanda de Cache não pôde ser retomada.",
                ));
            }
            Some(CacheCancellationReason::Paused) | None => {}
        }
        let permit = engine.begin_cancellable_work(cancellation.clone()).await;
        if cancellation.reason() == Some(CacheCancellationReason::Paused) {
            drop(permit);
            continue;
        }
        if cancellation.reason() == Some(CacheCancellationReason::Obsolete) {
            drop(permit);
            return Err(CacheFailure::new(
                CacheFailureStage::Cancelled,
                "A demanda de Cache ficou obsoleta.",
            ));
        }
        let reservation = processor.reserve().await.map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::Processor(InvocationFailureStage::ResolveSidecar),
                error.to_string(),
            )
        })?;
        if cancellation
            .flag()
            .load(std::sync::atomic::Ordering::Acquire)
        {
            drop(reservation);
            drop(permit);
            continue;
        }
        let context = InvocationContext::new(
            work.request_id.clone(),
            Some(work.namespace.project_id().to_owned()),
        );
        let mut transport = TauriImagingTransport::new(app, logging, &reservation);
        let result = engine
            .execute(
                &mut transport,
                app_paths,
                work.clone(),
                &context,
                &cancellation,
            )
            .await;
        drop(reservation);
        drop(permit);
        if result.as_ref().is_err_and(|failure| {
            matches!(
                failure.stage,
                CacheFailureStage::Cancelled
                    | CacheFailureStage::Processor(InvocationFailureStage::Cancelled)
            ) && cancellation.reason() == Some(CacheCancellationReason::Paused)
        }) {
            continue;
        }
        return result;
    }
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
