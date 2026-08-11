use std::collections::{HashMap, HashSet};

use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::OperationPathContext;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        self, AuthorizedCacheNamespace, CacheEngine, CacheFailure, CacheFailureStage,
        CacheFlightClaim, CacheWork,
    },
    cache_previews::{CachePreviewError, CachePreviewRegistry},
    imaging_processor::{
        ImagingProcessor, InvocationContext, InvocationFailureStage, TauriImagingTransport,
    },
    ipc_contract::{
        MediaPreview, MediaPreviewCommandError, MediaPreviewCommandErrorCode, MediaPreviewDemand,
        MediaPreviewState,
    },
    logging::LoggingState,
    media_runtime::{MediaAvailability, MediaMonitor, MediaRuntime},
    product_runtime::PROJECT_WINDOW_LABEL,
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
    app_paths: State<'_, myalbuns_paths::AppPaths>,
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
    let namespace = AuthorizedCacheNamespace::mount(&app_paths, &catalog.authority)
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
    let demand_revision = engine.reconcile_demand(
        namespace.project_id(),
        demand.revision,
        ordered_demand.iter().map(String::as_str),
    );
    registry.invalidate_media(
        demand_revision
            .retired_media_ids()
            .iter()
            .map(String::as_str),
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
    let invalidated_media_ids = poll
        .update()
        .map(|update| update.invalidated_media_ids().to_vec())
        .unwrap_or_default();
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
    if !engine.demand_is_current(&demand_revision) {
        return Ok(Some(Vec::new()));
    }
    if !invalidated_media_ids.is_empty() {
        engine
            .invalidate_media(
                &app_paths,
                &namespace,
                invalidated_media_ids.iter().map(String::as_str),
            )
            .map_err(|_| MediaPreviewCommandError::read_failed())?;
        registry.invalidate_media(invalidated_media_ids.iter().map(String::as_str));
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
        if availability != MediaAvailability::Candidate {
            previews.push(MediaPreview {
                media_id,
                state: match availability {
                    MediaAvailability::Absent => MediaPreviewState::Absent,
                    MediaAvailability::Candidate => unreachable!(),
                    MediaAvailability::Unavailable => MediaPreviewState::Unavailable,
                },
                url: None,
            });
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
        let mut path_context = OperationPathContext::new();
        if path_context.capture(namespace.paths().root()).is_err()
            || path_context.capture(source.source_path()).is_err()
        {
            previews.push(MediaPreview {
                media_id,
                state: MediaPreviewState::Unavailable,
                url: None,
            });
            continue;
        }
        let request_id = format!("cache-{}", uuid::Uuid::new_v4().simple());
        let work = CacheWork::new(
            request_id.clone(),
            namespace.clone(),
            source,
            path_context.freeze(),
        );
        let Some(claim) = engine.claim_demanded(&demand_revision, &work) else {
            return Ok(Some(Vec::new()));
        };
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
                previews.push(
                    registry
                        .publish(&app_paths, &namespace, execution.artifact())
                        .map_err(MediaPreviewCommandError::from)?,
                );
            }
            Err(failure) => {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    stage = ?failure.stage,
                    exit_code = failure.exit_code,
                    message = failure.message,
                    media_id,
                    event = "cache_media_unavailable",
                );
                previews.push(MediaPreview {
                    media_id,
                    state: MediaPreviewState::Unavailable,
                    url: None,
                });
            }
        }
    }
    Ok(Some(previews))
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
    use crate::ipc_contract::MediaPreviewDemand;

    use super::ordered_demand;

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
}
