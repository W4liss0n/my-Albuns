use std::{
    fs::OpenOptions,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{
    AppHandle, Manager, WebviewWindow,
    ipc::{Channel, InvokeResponseBody},
};

use crate::{
    cache_engine::CacheEngine,
    export_attempts::{CancelDisposition, ExportAttempts},
    export_probe_commands::{self, ExportCommandError, ExportEvent, ExportResult},
    imaging_processor::ImagingProcessor,
    logging::LoggingState,
    operation_gate::OperationGate,
    project_host::ProjectHost,
};

pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(20);
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Default)]
pub(crate) struct ExportProbeCapture {
    pub(crate) operation_id: Option<String>,
    pub(crate) progress_stages: Vec<String>,
    pub(crate) cancellation_disposition: Option<CancelDisposition>,
    pub(crate) preparing_claimed: bool,
    pub(crate) failure: Option<String>,
}

#[derive(Debug)]
pub(crate) struct PreparingSnapshot {
    pub(crate) operation_id: String,
    pub(crate) progress_stages: Vec<String>,
}

impl ExportProbeCapture {
    pub(crate) fn operation_id(&self) -> Option<&str> {
        self.operation_id.as_deref()
    }

    pub(crate) fn progress_stages(&self) -> &[String] {
        &self.progress_stages
    }

    pub(crate) fn cancellation_disposition(&self) -> Option<CancelDisposition> {
        self.cancellation_disposition
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
}

pub(crate) fn optional_utf8_environment(name: &str) -> Result<Option<String>, String> {
    std::env::var_os(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| format!("{name} precisa conter texto Unicode válido."))
        })
        .transpose()
}

pub(crate) fn validate_probe_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute()
        || !root.is_dir()
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err("A raiz do probe precisa ser um diretório absoluto existente.".into());
    }
    if !root
        .components()
        .any(|component| component == Component::Normal(".scratch".as_ref()))
    {
        return Err("A raiz do probe precisa permanecer sob .scratch.".into());
    }
    Ok(())
}

pub(crate) fn wait_for_file_blocking(path: &Path, description: &str) -> Result<(), String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        thread::sleep(PROBE_POLL_INTERVAL);
    }
    Err(format!("{description} excedeu o limite do probe"))
}

pub(crate) async fn wait_for_file_async(path: &Path, description: &str) -> Result<(), String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        tokio::time::sleep(PROBE_POLL_INTERVAL).await;
    }
    Err(format!("{description} excedeu o limite do probe"))
}

pub(crate) fn write_json_atomic_new(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "O evento do probe não possui diretório pai.".to_string())?;
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(|error| format!("Não foi possível inspecionar o diretório do probe: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("O diretório de saída do probe é inválido.".into());
    }
    if path.exists() {
        return Err("O evento do probe já existe.".into());
    }

    let json = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Não foi possível serializar o evento do probe: {error}"))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "O nome do evento do probe é inválido.".to_string())?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let write_result = (|| {
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("Não foi possível criar o evento temporário: {error}"))?;
        temporary
            .write_all(&json)
            .and_then(|_| temporary.flush())
            .and_then(|_| temporary.sync_all())
            .map_err(|error| format!("Não foi possível sincronizar o evento: {error}"))?;
        drop(temporary);
        std::fs::rename(&temporary_path, path)
            .map_err(|error| format!("Não foi possível publicar o evento: {error}"))
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    write_result
}

pub(crate) async fn execute_real_export(
    app: &AppHandle,
    window: &WebviewWindow,
    channel: Channel<ExportEvent>,
) -> Result<ExportResult, ExportCommandError> {
    export_probe_commands::export_spike(
        app.clone(),
        window.clone(),
        channel,
        app.state::<ProjectHost>(),
        app.state::<LoggingState>(),
        app.state::<OperationGate>(),
        app.state::<CacheEngine>(),
        app.state::<ImagingProcessor>(),
        app.state::<ExportAttempts>(),
    )
    .await
}

pub(crate) fn observing_channel(capture: Arc<Mutex<ExportProbeCapture>>) -> Channel<ExportEvent> {
    Channel::new(move |body| {
        if let Err(reason) = record_channel_event(body, &capture, false) {
            record_capture_failure(&capture, reason);
        }
        Ok(())
    })
}

pub(crate) fn record_channel_event(
    body: InvokeResponseBody,
    capture: &Arc<Mutex<ExportProbeCapture>>,
    claim_preparing: bool,
) -> Result<Option<PreparingSnapshot>, String> {
    let InvokeResponseBody::Json(json) = body else {
        return Err("o Channel da Exportação enviou um payload não JSON".into());
    };
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|error| format!("o Channel enviou JSON inválido: {error}"))?;
    let event = value
        .get("event")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "o Channel não informou event".to_string())?;
    let data = value
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "o Channel não informou data".to_string())?;
    let operation_id = data
        .get("operationId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "o Channel não informou operationId".to_string())?;
    let mut capture = capture
        .lock()
        .expect("the Export probe capture remains available");
    match event {
        "started" => {
            if capture.operation_id.is_some() {
                return Err("o Channel informou started mais de uma vez".into());
            }
            capture.operation_id = Some(operation_id.to_owned());
            Ok(None)
        }
        "progress" => {
            if capture.operation_id.as_deref() != Some(operation_id) {
                return Err("o progresso não pertence à Exportação iniciada".into());
            }
            let stage = data
                .get("stage")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "o progresso não informou stage".to_string())?;
            if !matches!(
                stage,
                "preparing"
                    | "loading_sources"
                    | "composing"
                    | "encoding_output"
                    | "verifying"
                    | "publishing"
                    | "completed"
            ) {
                return Err(format!("o Channel informou um stage desconhecido: {stage}"));
            }
            capture.progress_stages.push(stage.to_owned());
            if claim_preparing && stage == "preparing" && !capture.preparing_claimed {
                capture.preparing_claimed = true;
                return Ok(Some(PreparingSnapshot {
                    operation_id: operation_id.to_owned(),
                    progress_stages: capture.progress_stages.clone(),
                }));
            }
            Ok(None)
        }
        other => Err(format!(
            "o Channel informou um evento desconhecido: {other}"
        )),
    }
}

pub(crate) fn capture_snapshot(capture: &Arc<Mutex<ExportProbeCapture>>) -> ExportProbeCapture {
    capture
        .lock()
        .expect("the Export probe capture remains available")
        .clone()
}

pub(crate) fn record_capture_failure(capture: &Arc<Mutex<ExportProbeCapture>>, reason: String) {
    let mut capture = capture
        .lock()
        .expect("the Export probe capture remains available");
    if capture.failure.is_none() {
        capture.failure = Some(reason);
    }
}

pub(crate) fn verify_and_remove_output(result: &ExportResult) -> Result<u64, String> {
    let output_path = PathBuf::from(result.output_path());
    let expected_root = std::env::temp_dir().join("MyAlbuns").join("spike");
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "A saída do probe possui nome inválido.".to_string())?;
    if !output_path.is_absolute()
        || output_path.extension().and_then(|value| value.to_str()) != Some("png")
        || !file_name.starts_with("Album-Horizonte_")
    {
        return Err("A saída do probe não pertence ao namespace esperado.".into());
    }
    let output_parent = output_path
        .parent()
        .ok_or_else(|| "A saída do probe não possui diretório pai.".to_string())?
        .canonicalize()
        .map_err(|error| format!("Não foi possível validar o Destino do probe: {error}"))?;
    let expected_root = expected_root
        .canonicalize()
        .map_err(|error| format!("Não foi possível validar a raiz do Destino: {error}"))?;
    if output_parent != expected_root {
        return Err("A saída do probe está fora de temp/MyAlbuns/spike.".into());
    }
    let metadata = std::fs::symlink_metadata(&output_path)
        .map_err(|error| format!("Não foi possível inspecionar a saída do probe: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("A saída publicada do probe não é um arquivo regular.".into());
    }
    let output_bytes = metadata.len();
    std::fs::remove_file(&output_path)
        .map_err(|error| format!("Não foi possível remover a saída do probe: {error}"))?;
    if output_bytes == 0 {
        return Err("A saída publicada do probe está vazia.".into());
    }
    Ok(output_bytes)
}
