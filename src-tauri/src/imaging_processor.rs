use std::path::Path;

use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingFailureStage};
use myalbuns_logging::{LOG_DIRECTORY_ENV, ProcessRole};
use myalbuns_paths::{AppPaths, CachePathPlan};
use tauri::AppHandle;
use tauri_plugin_shell::{ShellExt, process::CommandEvent};

use crate::logging::LoggingState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImagingOperation {
    Cache,
    Export,
}

impl ImagingOperation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::Export => "export",
        }
    }
}

#[derive(Debug)]
pub(crate) struct InvocationFailure {
    pub(crate) stage: &'static str,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message: String,
    termination_observed: bool,
}

impl InvocationFailure {
    fn terminated(exit_code: Option<i32>) -> Self {
        let stage = exit_code
            .and_then(ImagingFailureStage::from_exit_code)
            .map_or("imaging_process", ImagingFailureStage::as_str);
        Self {
            stage,
            exit_code,
            message: format!(
                "O Processador de Imagens terminou com o código {:?}.",
                exit_code
            ),
            termination_observed: true,
        }
    }

    fn is_unexpected_termination(&self) -> bool {
        self.termination_observed
            && self
                .exit_code
                .and_then(ImagingFailureStage::from_exit_code)
                .is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryAction {
    Restart,
    Fail,
}

fn recovery_action(
    operation: ImagingOperation,
    attempt: u8,
    failure: &InvocationFailure,
) -> RecoveryAction {
    if operation == ImagingOperation::Cache && attempt == 1 && failure.is_unexpected_termination() {
        RecoveryAction::Restart
    } else {
        RecoveryAction::Fail
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InvocationContext<'a> {
    pub(crate) operation_id: &'a str,
    pub(crate) project_id: Option<&'a str>,
}

pub(crate) async fn invoke_cache_with_restart(
    app: &AppHandle,
    logging: &LoggingState,
    app_paths: &AppPaths,
    cache_paths: &CachePathPlan,
    payload: &[u8],
    context: InvocationContext<'_>,
) -> Result<Vec<u8>, InvocationFailure> {
    let mut attempt = 1;
    loop {
        match invoke_once(
            app,
            logging,
            payload,
            context,
            ImagingOperation::Cache,
            attempt,
        )
        .await
        {
            Ok(stdout) => {
                if attempt > 1 {
                    tracing::info!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id,
                        project_id = context.project_id,
                        attempts = attempt,
                        event = "imaging_processor_restart_completed",
                    );
                }
                return Ok(stdout);
            }
            Err(failure)
                if recovery_action(ImagingOperation::Cache, attempt, &failure)
                    == RecoveryAction::Restart =>
            {
                let removed_temporary_count = app_paths
                    .discard_project_cache_temporaries(cache_paths)
                    .map_err(|error| InvocationFailure {
                        stage: "cache_recovery_cleanup",
                        exit_code: failure.exit_code,
                        message: format!(
                            "Não foi possível descartar o item incompleto do Cache: {error}"
                        ),
                        termination_observed: false,
                    })?;
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    protocol_version = IMAGING_PROTOCOL_VERSION,
                    operation_id = context.operation_id,
                    project_id = context.project_id,
                    failed_attempt = attempt,
                    exit_code = failure.exit_code,
                    removed_temporary_count,
                    event = "imaging_processor_restart_started",
                );
                attempt += 1;
            }
            Err(failure) => {
                if attempt > 1 && failure.is_unexpected_termination() {
                    tracing::error!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id,
                        project_id = context.project_id,
                        attempts = attempt,
                        exit_code = failure.exit_code,
                        event = "imaging_processor_restart_exhausted",
                    );
                }
                return Err(failure);
            }
        }
    }
}

pub(crate) async fn invoke_export_once(
    app: &AppHandle,
    logging: &LoggingState,
    payload: &[u8],
    temporary_output_path: &Path,
    context: InvocationContext<'_>,
) -> Result<Vec<u8>, InvocationFailure> {
    match invoke_once(app, logging, payload, context, ImagingOperation::Export, 1).await {
        Ok(stdout) => Ok(stdout),
        Err(failure) => {
            match discard_incomplete_export(temporary_output_path) {
                Ok(removed) => tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    protocol_version = IMAGING_PROTOCOL_VERSION,
                    operation_id = context.operation_id,
                    project_id = context.project_id,
                    removed,
                    event = "incomplete_export_discarded",
                ),
                Err(error) => tracing::error!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    protocol_version = IMAGING_PROTOCOL_VERSION,
                    operation_id = context.operation_id,
                    project_id = context.project_id,
                    event = "incomplete_export_cleanup_failed",
                    reason = error.as_str(),
                ),
            }
            Err(failure)
        }
    }
}

fn discard_incomplete_export(path: &Path) -> Result<bool, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "não foi possível inspecionar o temporário da Exportação: {error}"
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("o temporário da Exportação não é um arquivo regular".into());
    }
    std::fs::remove_file(path)
        .map_err(|error| format!("não foi possível remover a Exportação incompleta: {error}"))?;
    Ok(true)
}

async fn invoke_once(
    app: &AppHandle,
    logging: &LoggingState,
    payload: &[u8],
    context: InvocationContext<'_>,
    operation: ImagingOperation,
    attempt: u8,
) -> Result<Vec<u8>, InvocationFailure> {
    let sidecar = app
        .shell()
        .sidecar("myalbuns-imaging")
        .map_err(|error| InvocationFailure {
            stage: "resolve_sidecar",
            exit_code: None,
            message: format!("Processador de Imagens indisponível: {error}"),
            termination_observed: false,
        })?
        .env(LOG_DIRECTORY_ENV, logging.directory());
    let (mut events, mut child) = sidecar.spawn().map_err(|error| InvocationFailure {
        stage: "spawn_sidecar",
        exit_code: None,
        message: format!("Não foi possível iniciar o Processador de Imagens: {error}"),
        termination_observed: false,
    })?;
    let imaging_process_id = child.pid();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = context.operation_id,
        project_id = context.project_id,
        operation = operation.as_str(),
        attempt,
        imaging_process_id,
        event = "imaging_process_spawned",
    );
    if let Err(error) = child.write(payload) {
        let _ = child.kill();
        return Err(InvocationFailure {
            stage: "write_request",
            exit_code: None,
            message: format!("Não foi possível enviar a solicitação: {error}"),
            termination_observed: false,
        });
    }

    let mut stdout = Vec::new();
    let mut exit_code = None;
    let mut stream_error = None;
    while let Some(event) = events.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.extend(bytes),
            CommandEvent::Stderr(bytes) => {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    protocol_version = IMAGING_PROTOCOL_VERSION,
                    operation_id = context.operation_id,
                    project_id = context.project_id,
                    byte_count = bytes.len(),
                    event = "imaging_stderr_received",
                );
            }
            CommandEvent::Error(error) => stream_error = Some(error),
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
                break;
            }
            _ => {}
        }
    }

    if let Some(error) = stream_error {
        return Err(InvocationFailure {
            stage: "read_response",
            exit_code,
            message: format!("Não foi possível receber a resposta do Processador: {error}"),
            termination_observed: exit_code.is_some(),
        });
    }
    if exit_code != Some(0) {
        return Err(InvocationFailure::terminated(exit_code));
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::{
        ImagingOperation, InvocationFailure, RecoveryAction, discard_incomplete_export,
        recovery_action,
    };

    #[test]
    fn cache_restarts_once_only_after_an_unexpected_termination() {
        let crash = InvocationFailure::terminated(Some(-1));

        assert_eq!(
            recovery_action(ImagingOperation::Cache, 1, &crash),
            RecoveryAction::Restart
        );
        assert_eq!(
            recovery_action(ImagingOperation::Cache, 2, &crash),
            RecoveryAction::Fail
        );
    }

    #[test]
    fn deterministic_cache_failures_are_not_retried() {
        let failure = InvocationFailure::terminated(Some(
            myalbuns_imaging_protocol::ImagingFailureStage::CacheProcessing
                .exit_code()
                .into(),
        ));

        assert_eq!(
            recovery_action(ImagingOperation::Cache, 1, &failure),
            RecoveryAction::Fail
        );
    }

    #[test]
    fn export_never_restarts_automatically() {
        let crash = InvocationFailure::terminated(Some(-1));

        assert_eq!(
            recovery_action(ImagingOperation::Export, 1, &crash),
            RecoveryAction::Fail
        );
    }

    #[test]
    fn export_failure_cleanup_preserves_the_previous_published_output() {
        let directory = tempfile::tempdir().expect("temporary Export destination");
        let published = directory.path().join("Album_001.png");
        let temporary = directory.path().join(".Album_001.png.export-01.tmp");
        std::fs::write(&published, b"previous export").expect("the previous Export is writable");
        std::fs::write(&temporary, b"incomplete export")
            .expect("the incomplete Export is writable");

        assert!(
            discard_incomplete_export(&temporary).expect("the exact incomplete file is discarded")
        );
        assert_eq!(
            std::fs::read(published).expect("the previous Export remains"),
            b"previous export"
        );
        assert!(!temporary.exists());
    }
}
