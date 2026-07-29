mod logging;
mod project_host;
mod topology_spike;

use std::sync::atomic::{AtomicU64, Ordering};

use myalbuns_core::{EditorProjection, ExportResult, ProjectIntent};
use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingRequest, ImagingResponse};
use myalbuns_logging::{LOG_DIRECTORY_ENV, ProcessRole, safe_log_identifier};
use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_shell::{ShellExt, process::CommandEvent};

use logging::{LoggingState, frontend_log};
use project_host::ProjectHost;
use topology_spike::TopologySpike;

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
fn project_state(
    operation_id: String,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    logging::validate_optional_identifier("operationId", Some(&operation_id))?;
    let projection = state.projection(window.label())?;
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = operation_id.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_state_read",
    );
    Ok(projection)
}

#[tauri::command]
fn apply_project_intent(
    intent: ProjectIntent,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let intent_kind = match &intent {
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::FillLeftmostPlaceholder { .. } => "fill_leftmost_placeholder",
    };
    let projection = state.apply(window.label(), intent).inspect_err(|_| {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            window_label = window.label(),
            intent = intent_kind,
            event = "project_intent_rejected",
        );
    })?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        intent = intent_kind,
        event = "project_intent_applied",
    );
    Ok(projection)
}

#[tauri::command]
fn undo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.undo(window.label())?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_undo_completed",
    );
    Ok(projection)
}

#[tauri::command]
fn redo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.redo(window.label())?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_redo_completed",
    );
    Ok(projection)
}

#[tauri::command]
async fn export_spike(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
) -> Result<ExportResult, String> {
    let export_sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("export-{export_sequence}");
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        window_label = window.label(),
        event = "export_started",
    );
    let snapshot = state
        .render_snapshot(window.label())
        .inspect_err(|_| log_export_failure(&request_id, None, "session_lock", None))?;

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    let output_path = output_dir.join(format!(
        "Album-Horizonte_{}_{export_sequence:03}.png",
        std::process::id()
    ));
    let request = ImagingRequest::new(request_id.clone(), output_path.clone(), snapshot);
    let project_id = safe_log_identifier(&request.snapshot.project_id);
    std::fs::create_dir_all(&output_dir).map_err(|error| {
        log_export_failure(&request_id, project_id, "prepare_output", None);
        format!("Não foi possível preparar a Exportação: {error}")
    })?;
    let mut payload = serde_json::to_vec(&request).map_err(|error| {
        log_export_failure(&request_id, project_id, "serialize_snapshot", None);
        format!("Não foi possível preparar o snapshot: {error}")
    })?;
    payload.push(b'\n');

    let sidecar = app
        .shell()
        .sidecar("myalbuns-imaging")
        .map_err(|error| {
            log_export_failure(&request_id, project_id, "resolve_sidecar", None);
            format!("Processador de Imagens indisponível: {error}")
        })?
        .env(LOG_DIRECTORY_ENV, logging.directory());
    let (mut events, mut child) = sidecar.spawn().map_err(|error| {
        log_export_failure(&request_id, project_id, "spawn_sidecar", None);
        format!("Não foi possível iniciar o Processador de Imagens: {error}")
    })?;
    child.write(&payload).map_err(|error| {
        log_export_failure(&request_id, project_id, "write_request", None);
        format!("Não foi possível enviar o snapshot: {error}")
    })?;

    let mut stdout = Vec::new();
    let mut exit_code = None;
    while let Some(event) = events.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.extend(bytes),
            CommandEvent::Stderr(bytes) => {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    protocol_version = IMAGING_PROTOCOL_VERSION,
                    operation_id = request_id.as_str(),
                    project_id,
                    byte_count = bytes.len(),
                    event = "imaging_stderr_received",
                );
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
                break;
            }
            _ => {}
        }
    }

    if exit_code != Some(0) {
        log_export_failure(&request_id, project_id, "imaging_process", exit_code);
        return Err(format!(
            "O Processador de Imagens terminou com o código {:?}.",
            exit_code
        ));
    }
    let response: ImagingResponse = serde_json::from_slice(&stdout).map_err(|error| {
        log_export_failure(&request_id, project_id, "decode_response", None);
        format!("Resposta inválida do Processador de Imagens: {error}")
    })?;
    let Some((width_px, height_px)) = response.completed_dimensions_for(&request_id) else {
        log_export_failure(&request_id, project_id, "validate_response", None);
        return Err("O Processador de Imagens devolveu uma resposta inesperada.".into());
    };
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id,
        width_px,
        height_px,
        event = "export_completed",
    );

    Ok(ExportResult {
        output_path: output_path.to_string_lossy().into_owned(),
        width_px,
        height_px,
    })
}

fn log_export_failure(
    operation_id: &str,
    project_id: Option<&str>,
    stage: &str,
    exit_code: Option<i32>,
) {
    tracing::error!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id,
        project_id,
        stage,
        exit_code,
        event = "export_failed",
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let topology = TopologySpike::from_environment()
        .unwrap_or_else(|error| panic!("configuração inválida do spike de topologia: {error}"));
    let project_host = topology.project_host();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .setup(move |app| {
            let main_window = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("a janela principal não foi criada"))?;
            main_window.set_title(topology.primary_title())?;
            if let Some(secondary) = topology.secondary_window() {
                WebviewWindowBuilder::new(
                    app,
                    secondary.label,
                    WebviewUrl::App("index.html".into()),
                )
                .title(secondary.title.as_str())
                .inner_size(1440.0, 900.0)
                .min_inner_size(1080.0, 720.0)
                .resizable(true)
                .build()?;
            }

            let app_paths = AppPaths::discover()?;
            logging::initialize(app, &app_paths);
            app.manage(app_paths);
            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                process_id = std::process::id(),
                topology = topology.label(),
                session_count = topology.session_count(),
                event = "topology_host_started",
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            frontend_log,
            project_state,
            apply_project_intent,
            undo_project,
            redo_project,
            export_spike
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o MyAlbuns");
}
