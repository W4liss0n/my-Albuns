use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, OperationPathContext};
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    cache_engine::{self, CacheWork},
    imaging_processor::{InvocationContext, TauriImagingTransport},
    logging::{LoggingState, log_imaging_failure},
    project_host::ProjectHost,
};

static CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const CACHE_PREVIEW_MAX_EDGE_PX: u32 = 1600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MediaPreview {
    media_id: String,
    url: String,
}

#[tauri::command]
pub(crate) async fn prepare_media_previews(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    app_paths: State<'_, AppPaths>,
    logging: State<'_, LoggingState>,
) -> Result<Option<Vec<MediaPreview>>, String> {
    let cache_sequence = CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("cache-{}-{cache_sequence}", std::process::id());
    let projection = state.projection(window.label())?;
    let project_id = projection.state.project_id;
    let cache_paths = app_paths
        .project_cache(&project_id)
        .map_err(|error| error.to_string())?;
    let Some(sources) = state.cache_sources(window.label())? else {
        return Ok(None);
    };
    let mut path_context = OperationPathContext::new();
    path_context
        .capture(cache_paths.root())
        .map_err(|error| error.to_string())?;
    for source in &sources {
        path_context
            .capture(source.source_path())
            .map_err(|error| error.to_string())?;
    }
    let work = CacheWork::new(
        request_id.clone(),
        project_id.clone(),
        cache_paths.clone(),
        sources,
        CACHE_PREVIEW_MAX_EDGE_PX,
        path_context.freeze(),
    );
    let safe_project_id = safe_log_identifier(&project_id);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        media_count = work.sources.len(),
        event = "media_cache_started",
    );
    let started = Instant::now();
    let context = InvocationContext::new(request_id.clone(), safe_project_id);
    let mut transport = TauriImagingTransport::new(&app, &logging);
    let execution = cache_engine::execute(&mut transport, &app_paths, work, &context)
        .await
        .map_err(|failure| {
            log_imaging_failure(
                "media_cache_failed",
                &request_id,
                safe_project_id,
                failure.stage.as_str(),
                failure.exit_code,
            );
            failure.message
        })?;
    let recovered_process_id = execution
        .recovery
        .map(|recovery| recovery.failed_process_id);
    let removed_recovery_temporary_count = execution
        .recovery
        .map_or(0, |recovery| recovery.removed_temporary_count);
    let completed = execution.completion;
    let previews = completed
        .artifacts
        .iter()
        .map(|artifact| -> Result<MediaPreview, String> {
            let preview_path = cache_paths
                .preview_file(&artifact.media_id, &artifact.generation_id)
                .map_err(|error| error.to_string())?;
            Ok(MediaPreview {
                media_id: artifact.media_id.clone(),
                url: app_paths
                    .cache_asset_url(&preview_path)
                    .map_err(|error| error.to_string())?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        generated_count = completed.generated_count,
        reused_count = completed.reused_count,
        source_bytes = completed.source_bytes,
        preview_bytes = completed.preview_bytes,
        recovered_process_id,
        removed_recovery_temporary_count,
        elapsed_ms = started.elapsed().as_millis(),
        event = "media_cache_completed",
    );
    Ok(Some(previews))
}
