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
    cache_service::CacheNamespaceOwner,
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
    registry: State<'_, CachePreviewRegistry>,
    media_runtime: State<'_, MediaRuntime>,
    media_monitor: State<'_, MediaMonitor>,
    app_paths: State<'_, AppPaths>,
    namespace_owner: State<'_, CacheNamespaceOwner>,
) -> Result<MediaPreview, MediaPreviewCommandError> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let binding = project_host
        .authorized_media_binding(&media_id)
        .map_err(MediaPreviewCommandError::retry_failed)?;
    let monitor = media_monitor.inner().clone();
    let runtime = media_runtime.inner().clone();
    let retry_app = app.clone();
    let retry_app_paths = app_paths.inner().clone();
    let retry_namespace = namespace_owner.namespace().clone();
    let retry_registry = registry.inner().clone();
    let inspection = tauri::async_runtime::spawn_blocking(move || {
        monitor.retry_unavailable(&runtime, &binding, |update| {
            retry_app
                .state::<CacheEngine>()
                .apply_monitor_media_update(
                    &retry_app_paths,
                    &retry_namespace,
                    &retry_registry,
                    update,
                )
                .map(|_| ())
                .map_err(|failure| failure.message)
        })
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
    let state = preview_state(inspection.availability());
    let retained = (state != MediaPreviewState::Ready)
        .then(|| registry.retained_preview(&media_id, state))
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
    namespace_owner: State<'_, CacheNamespaceOwner>,
) -> Result<Option<Vec<MediaPreview>>, MediaPreviewCommandError> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(MediaPreviewCommandError::read_failed());
    }
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
    let namespace = namespace_owner.namespace().clone();
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
    let poll = tauri::async_runtime::spawn_blocking(move || monitor.poll(&runtime, &bindings))
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
        let cache_update = engine
            .apply_demand_media_update(
                &app_paths,
                &namespace,
                registry.inner(),
                &mut demand_revision,
                runtime_update,
            )
            .map_err(|_| MediaPreviewCommandError::read_failed())?;
        if cache_update.retry_required() {
            window
                .emit(
                    LINKED_MEDIA_CHANGED_EVENT,
                    LinkedMediaChanged {
                        media_ids: runtime_update.changed_media_ids().to_vec(),
                    },
                )
                .map_err(|_| MediaPreviewCommandError::read_failed())?;
        }
        if !cache_update.demand_can_resume() {
            return Ok(Some(Vec::new()));
        }
    }
    if !engine.demand_is_current(&demand_revision) {
        return Ok(Some(Vec::new()));
    }
    let mut previews = Vec::with_capacity(ordered_demand.len());
    for media_id in ordered_demand {
        if !engine.demand_is_current(&demand_revision) {
            return Ok(Some(Vec::new()));
        }
        let availability = observations
            .get(media_id.as_str())
            .copied()
            .unwrap_or(MediaAvailability::Unavailable);
        let state = preview_state(availability);
        if state != MediaPreviewState::Ready {
            previews.push(contextual_preview(
                &engine,
                &registry,
                &app_paths,
                &namespace,
                &demand_revision,
                media_id,
                state,
            ));
            continue;
        }
        let binding = catalog_by_id
            .get(media_id.as_str())
            .expect("validated demand remains in the immutable catalog");
        let source = CacheMediaSource::new(
            binding.media_id.clone(),
            binding.kind,
            binding.logical_path.clone(),
        )
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
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
                    media_id,
                    cache_failure_state(),
                ));
                continue;
            }
        };
        let request_id = format!("cache-{}", uuid::Uuid::new_v4().simple());
        let work = CacheWork::new(request_id.clone(), namespace.clone(), source, root_bindings);
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
                    || registry.publish(&app_paths, &namespace, execution.artifact()),
                ) else {
                    return Ok(Some(Vec::new()));
                };
                previews.push(preview.map_err(MediaPreviewCommandError::from)?);
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
                    media_id,
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

fn cache_failure_state() -> MediaPreviewState {
    MediaPreviewState::CacheUnavailable
}

fn contextual_preview(
    engine: &CacheEngine,
    registry: &CachePreviewRegistry,
    app_paths: &AppPaths,
    namespace: &AuthorizedCacheNamespace,
    demand: &cache_engine::CacheDemandRevision,
    media_id: String,
    state: MediaPreviewState,
) -> MediaPreview {
    engine
        .retain_last_known_preview(
            app_paths,
            namespace,
            registry,
            demand,
            media_id.as_str(),
            state,
        )
        .unwrap_or(MediaPreview {
            media_id,
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
        ipc_contract::{MediaPreviewDemand, MediaPreviewState},
        media_runtime::MediaAvailability,
    };

    use super::{cache_failure_state, ordered_demand, preview_state};

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
    fn authoritative_media_availability_maps_exhaustively_without_cache_failures() {
        assert_eq!(
            preview_state(MediaAvailability::Candidate),
            MediaPreviewState::Ready
        );
        assert_eq!(
            preview_state(MediaAvailability::Absent),
            MediaPreviewState::Absent
        );
        assert_eq!(
            preview_state(MediaAvailability::Unavailable),
            MediaPreviewState::Unavailable
        );
        assert_ne!(
            preview_state(MediaAvailability::Unavailable),
            MediaPreviewState::CacheUnavailable,
            "a Cache failure is not an authoritative statement about the Original"
        );
        assert_eq!(cache_failure_state(), MediaPreviewState::CacheUnavailable);
        assert_ne!(
            cache_failure_state(),
            MediaPreviewState::Unavailable,
            "Processor, validation and Cache storage failures do not classify the Original"
        );
    }
}
