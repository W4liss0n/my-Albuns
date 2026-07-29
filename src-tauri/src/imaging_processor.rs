use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingFailureStage};
use myalbuns_logging::{LOG_DIRECTORY_ENV, ProcessRole};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvocationFailureStage {
    ResolveSidecar,
    SpawnSidecar,
    WriteRequest,
    ReadResponse,
    ImagingProcess,
    CacheRecoveryCleanup,
    Processor(ImagingFailureStage),
}

impl InvocationFailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ResolveSidecar => "resolve_sidecar",
            Self::SpawnSidecar => "spawn_sidecar",
            Self::WriteRequest => "write_request",
            Self::ReadResponse => "read_response",
            Self::ImagingProcess => "imaging_process",
            Self::CacheRecoveryCleanup => "cache_recovery_cleanup",
            Self::Processor(stage) => stage.as_str(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct InvocationFailure {
    pub(crate) stage: InvocationFailureStage,
    pub(crate) exit_code: Option<i32>,
    pub(crate) process_id: Option<u32>,
    pub(crate) message: String,
    termination_observed: bool,
}

impl InvocationFailure {
    fn terminated(process_id: u32, exit_code: Option<i32>) -> Self {
        let stage = exit_code
            .and_then(ImagingFailureStage::from_exit_code)
            .map_or(
                InvocationFailureStage::ImagingProcess,
                InvocationFailureStage::Processor,
            );
        Self {
            stage,
            exit_code,
            process_id: Some(process_id),
            message: format!(
                "O Processador de Imagens terminou com o código {:?}.",
                exit_code
            ),
            termination_observed: true,
        }
    }

    pub(crate) fn is_unexpected_termination(&self) -> bool {
        self.termination_observed
            && self
                .exit_code
                .and_then(ImagingFailureStage::from_exit_code)
                .is_none()
    }

    pub(crate) fn cache_recovery_cleanup(failed: &Self, message: impl Into<String>) -> Self {
        Self {
            stage: InvocationFailureStage::CacheRecoveryCleanup,
            exit_code: failed.exit_code,
            process_id: failed.process_id,
            message: message.into(),
            termination_observed: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn unexpected_termination(process_id: u32) -> Self {
        Self::terminated(process_id, Some(-1))
    }

    #[cfg(test)]
    pub(crate) fn deterministic(stage: ImagingFailureStage, process_id: u32) -> Self {
        Self::terminated(process_id, Some(stage.exit_code().into()))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InvocationContext<'a> {
    pub(crate) operation_id: &'a str,
    pub(crate) project_id: Option<&'a str>,
}

pub(crate) async fn invoke_once(
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
            stage: InvocationFailureStage::ResolveSidecar,
            exit_code: None,
            process_id: None,
            message: format!("Processador de Imagens indisponível: {error}"),
            termination_observed: false,
        })?
        .env(LOG_DIRECTORY_ENV, logging.directory());
    let (mut events, mut child) = sidecar.spawn().map_err(|error| InvocationFailure {
        stage: InvocationFailureStage::SpawnSidecar,
        exit_code: None,
        process_id: None,
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
            stage: InvocationFailureStage::WriteRequest,
            exit_code: None,
            process_id: Some(imaging_process_id),
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
            stage: InvocationFailureStage::ReadResponse,
            exit_code,
            process_id: Some(imaging_process_id),
            message: format!("Não foi possível receber a resposta do Processador: {error}"),
            termination_observed: exit_code.is_some(),
        });
    }
    if exit_code != Some(0) {
        return Err(InvocationFailure::terminated(imaging_process_id, exit_code));
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use myalbuns_imaging_protocol::ImagingFailureStage;

    use super::{InvocationFailure, InvocationFailureStage};

    #[test]
    fn processor_exit_codes_remain_typed_at_the_host_boundary() {
        let failure = InvocationFailure::deterministic(ImagingFailureStage::SourceDecode, 4242);

        assert_eq!(
            failure.stage,
            InvocationFailureStage::Processor(ImagingFailureStage::SourceDecode)
        );
        assert_eq!(failure.stage.as_str(), "source_decode");
        assert_eq!(failure.process_id, Some(4242));
        assert!(!failure.is_unexpected_termination());
    }
}
