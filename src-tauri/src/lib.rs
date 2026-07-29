use std::sync::Mutex;

use myalbuns_core::{EditorProjection, ExportResult, ProjectCore, ProjectIntent, ProjectSession};
use myalbuns_imaging_protocol::{ImagingRequest, ImagingResponse};
use tauri::{AppHandle, State};
use tauri_plugin_shell::{ShellExt, process::CommandEvent};

struct AppState {
    session: Mutex<ProjectSession>,
}

#[tauri::command]
fn project_state(state: State<'_, AppState>) -> Result<EditorProjection, String> {
    let session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    Ok(project(&session))
}

#[tauri::command]
fn apply_project_intent(
    intent: ProjectIntent,
    state: State<'_, AppState>,
) -> Result<EditorProjection, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    session.apply(intent).map_err(|error| error.to_string())?;
    Ok(project(&session))
}

#[tauri::command]
fn undo_project(state: State<'_, AppState>) -> Result<EditorProjection, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    session.undo();
    Ok(project(&session))
}

#[tauri::command]
fn redo_project(state: State<'_, AppState>) -> Result<EditorProjection, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
    session.redo();
    Ok(project(&session))
}

#[tauri::command]
async fn export_spike(app: AppHandle, state: State<'_, AppState>) -> Result<ExportResult, String> {
    let snapshot = {
        let session = state
            .session
            .lock()
            .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())?;
        session.render_snapshot()
    };

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Não foi possível preparar a Exportação: {error}"))?;
    let output_path = output_dir.join("Album-Horizonte_001.png");
    let request_id = format!("export-revision-{}", snapshot.revision);
    let request = ImagingRequest::new(request_id.clone(), output_path.clone(), snapshot);
    let mut payload = serde_json::to_vec(&request)
        .map_err(|error| format!("Não foi possível preparar o snapshot: {error}"))?;
    payload.push(b'\n');

    let sidecar = app
        .shell()
        .sidecar("myalbuns-imaging")
        .map_err(|error| format!("Processador de Imagens indisponível: {error}"))?;
    let (mut events, mut child) = sidecar
        .spawn()
        .map_err(|error| format!("Não foi possível iniciar o Processador de Imagens: {error}"))?;
    child
        .write(&payload)
        .map_err(|error| format!("Não foi possível enviar o snapshot: {error}"))?;

    let mut stdout = Vec::new();
    let mut exit_code = None;
    while let Some(event) = events.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.extend(bytes),
            CommandEvent::Stderr(bytes) => {
                eprintln!("myalbuns-imaging: {}", String::from_utf8_lossy(&bytes));
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
                break;
            }
            _ => {}
        }
    }

    if exit_code != Some(0) {
        return Err(format!(
            "O Processador de Imagens terminou com o código {:?}.",
            exit_code
        ));
    }
    let response: ImagingResponse = serde_json::from_slice(&stdout)
        .map_err(|error| format!("Resposta inválida do Processador de Imagens: {error}"))?;
    let Some((width_px, height_px)) = response.completed_dimensions_for(&request_id) else {
        return Err("O Processador de Imagens devolveu uma resposta inesperada.".into());
    };

    Ok(ExportResult {
        output_path: output_path.to_string_lossy().into_owned(),
        width_px,
        height_px,
    })
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
        .manage(AppState {
            session: Mutex::new(ProjectCore::open_sample_project(12)),
        })
        .invoke_handler(tauri::generate_handler![
            project_state,
            apply_project_intent,
            undo_project,
            redo_project,
            export_spike
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o MyAlbuns");
}
