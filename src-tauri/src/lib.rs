mod logging;

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

use myalbuns_core::{EditorProjection, ExportResult, ProjectCore, ProjectIntent, ProjectSession};
use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingRequest, ImagingResponse};
use myalbuns_logging::{LOG_DIRECTORY_ENV, ProcessRole, safe_log_identifier};
use tauri::{AppHandle, State};
use tauri_plugin_shell::{ShellExt, process::CommandEvent};

use logging::{LoggingState, frontend_log};

struct AppState {
    session: Mutex<ProjectSession>,
}

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
fn project_state(
    operation_id: String,
    state: State<'_, AppState>,
) -> Result<EditorProjection, String> {
    logging::validate_optional_identifier("operationId", Some(&operation_id))?;
    let session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    let projection = project(&session);
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = operation_id.as_str(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_state_read",
    );
    Ok(projection)
}

#[tauri::command]
fn apply_project_intent(
    intent: ProjectIntent,
    state: State<'_, AppState>,
) -> Result<EditorProjection, String> {
    let intent_kind = match &intent {
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::FillLeftmostPlaceholder { .. } => "fill_leftmost_placeholder",
    };
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    if let Err(error) = session.apply(intent) {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            intent = intent_kind,
            event = "project_intent_rejected",
        );
        return Err(error.to_string());
    }
    let projection = project(&session);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        intent = intent_kind,
        event = "project_intent_applied",
    );
    Ok(projection)
}

#[tauri::command]
fn undo_project(state: State<'_, AppState>) -> Result<EditorProjection, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    session.undo();
    let projection = project(&session);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_undo_completed",
    );
    Ok(projection)
}

#[tauri::command]
fn redo_project(state: State<'_, AppState>) -> Result<EditorProjection, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    session.redo();
    let projection = project(&session);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_redo_completed",
    );
    Ok(projection)
}

#[tauri::command]
async fn export_spike(
    app: AppHandle,
    state: State<'_, AppState>,
    logging: State<'_, LoggingState>,
) -> Result<ExportResult, String> {
    let request_id = format!("export-{}", EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed));
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        event = "export_started",
    );
    let snapshot = {
        let session = state.session.lock().map_err(|_| {
            log_export_failure(&request_id, None, "session_lock", None);
            "A Sessão do Projeto ficou indisponível.".to_string()
        })?;
        session.render_snapshot()
    };

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    let output_path = output_dir.join("Album-Horizonte_001.png");
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

fn project(session: &ProjectSession) -> EditorProjection {
    EditorProjection {
        state: session.state(),
        composition: session.composition_plan(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            logging::initialize(app);
            Ok(())
        })
        .manage(AppState {
            session: Mutex::new(ProjectCore::open_sample_project(12)),
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
