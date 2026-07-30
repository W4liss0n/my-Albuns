use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::OperationPathContext;
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    export_pipeline,
    imaging_processor::{InvocationContext, TauriImagingTransport},
    logging::{LoggingState, log_imaging_failure},
    project_host::ProjectHost,
};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportResult {
    output_path: String,
    width_px: u32,
    height_px: u32,
}

#[tauri::command]
pub(crate) async fn export_spike(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
) -> Result<ExportResult, String> {
    let export_sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("export-{}-{export_sequence}", std::process::id());
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        window_label = window.label(),
        event = "export_started",
    );
    let snapshot = state.render_snapshot(window.label()).inspect_err(|_| {
        log_imaging_failure("export_failed", &request_id, None, "session_lock", None);
    })?;

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Não foi possível preparar o Destino da Exportação: {error}"))?;
    let output_path = output_dir.join(format!(
        "Album-Horizonte_{}_{export_sequence:03}.png",
        std::process::id()
    ));
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .ok_or_else(|| "O snapshot não contém Lâminas.".to_string())?
        .sheet_id
        .clone();
    let sources = state.export_sources(window.label(), &snapshot, &sheet_id)?;
    let dpi = if sources.is_some() { 300 } else { 25 };
    let project_id = safe_log_identifier(&snapshot.project_id).map(str::to_owned);
    let plan = export_pipeline::plan(
        snapshot,
        export_pipeline::ExportOptions::new(
            request_id.clone(),
            output_path,
            sheet_id,
            dpi,
            sources,
        ),
    )
    .map_err(|failure| {
        log_imaging_failure(
            "export_failed",
            &request_id,
            project_id.as_deref(),
            failure.stage.as_str(),
            failure.exit_code,
        );
        failure.message
    })?;
    let mut path_context = OperationPathContext::new();
    for path in plan.required_paths() {
        path_context
            .capture(path)
            .map_err(|error| error.to_string())?;
    }
    let root_bindings = path_context.freeze();

    let started = Instant::now();
    let context = InvocationContext::new(request_id.clone(), project_id.clone());
    let mut transport = TauriImagingTransport::new(&app, &logging);
    let cancellation = AtomicBool::new(false);
    let progress = |progress: export_pipeline::ExportProgress| {
        tracing::debug!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = request_id.as_str(),
            project_id = project_id.as_deref(),
            stage = ?progress.stage,
            completed_units = progress.completed_units,
            total_units = progress.total_units,
            event = "export_progress",
        );
    };
    let published = export_pipeline::execute(
        &mut transport,
        plan,
        &root_bindings,
        &cancellation,
        &progress,
        &context,
    )
    .await
    .map_err(|failure| {
        log_imaging_failure(
            "export_failed",
            &request_id,
            project_id.as_deref(),
            failure.stage.as_str(),
            failure.exit_code,
        );
        failure.message
    })?;
    let completed = published.completion;
    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        process_id = std::process::id(),
        project_id = project_id.as_deref(),
        window_label = window.label(),
        width_px = completed.width_px,
        height_px = completed.height_px,
        dpi = completed.dpi,
        source_count = completed.source_count,
        source_bytes = completed.source_bytes,
        output_bytes = completed.output_bytes,
        output_sha256 = completed.output_sha256.as_str(),
        elapsed_ms,
        event = "export_completed",
    );

    Ok(ExportResult {
        output_path: published
            .output_path
            .to_str()
            .ok_or_else(|| {
                "o caminho da Exportação não pode ser representado pela interface".to_string()
            })?
            .to_owned(),
        width_px: completed.width_px,
        height_px: completed.height_px,
    })
}
