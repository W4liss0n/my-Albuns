use std::{future::Future, pin::Pin};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingFailureStage, ImagingResponse,
    decode_response, encode_command,
};
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
    EncodeRequest,
    ResolveSidecar,
    SpawnSidecar,
    WriteRequest,
    ReadResponse,
    DecodeResponse,
    ImagingProcess,
    CacheRecoveryCleanup,
    Processor(ImagingFailureStage),
}

impl InvocationFailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::EncodeRequest => "encode_request",
            Self::ResolveSidecar => "resolve_sidecar",
            Self::SpawnSidecar => "spawn_sidecar",
            Self::WriteRequest => "write_request",
            Self::ReadResponse => "read_response",
            Self::DecodeResponse => "decode_response",
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
    pub(crate) fn at_stage(
        stage: InvocationFailureStage,
        process_id: Option<u32>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            exit_code: None,
            process_id,
            message: message.into(),
            termination_observed: false,
        }
    }

    pub(crate) fn terminated(process_id: u32, exit_code: Option<i32>) -> Self {
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

#[derive(Clone, Debug)]
pub(crate) struct InvocationContext {
    pub(crate) operation_id: String,
    pub(crate) project_id: Option<String>,
}

impl InvocationContext {
    pub(crate) fn new(
        operation_id: impl Into<String>,
        project_id: Option<impl Into<String>>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            project_id: project_id.map(Into::into),
        }
    }
}

pub(crate) type InvocationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ImagingResponse, InvocationFailure>> + Send + 'a>>;

/// Typed boundary shared by the production sidecar adapter and recovery tests.
pub(crate) trait ImagingTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
    ) -> InvocationFuture<'a>;
}

pub(crate) struct TauriImagingTransport<'a> {
    app: &'a AppHandle,
    logging: &'a LoggingState,
}

impl<'a> TauriImagingTransport<'a> {
    pub(crate) fn new(app: &'a AppHandle, logging: &'a LoggingState) -> Self {
        Self { app, logging }
    }
}

impl ImagingTransport for TauriImagingTransport<'_> {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
    ) -> InvocationFuture<'a> {
        Box::pin(invoke_once(
            self.app,
            self.logging,
            command,
            context,
            operation,
            attempt,
        ))
    }
}

async fn invoke_once(
    app: &AppHandle,
    logging: &LoggingState,
    command: &ImagingCommand,
    context: &InvocationContext,
    operation: ImagingOperation,
    attempt: u8,
) -> Result<ImagingResponse, InvocationFailure> {
    let payload = encode_command(command).map_err(|error| {
        InvocationFailure::at_stage(
            InvocationFailureStage::EncodeRequest,
            None,
            format!("Não foi possível preparar a solicitação: {error}"),
        )
    })?;
    let sidecar = app
        .shell()
        .sidecar("myalbuns-imaging")
        .map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::ResolveSidecar,
                None,
                format!("Processador de Imagens indisponível: {error}"),
            )
        })?
        .env(LOG_DIRECTORY_ENV, logging.directory());
    let (mut events, mut child) = sidecar.spawn().map_err(|error| {
        InvocationFailure::at_stage(
            InvocationFailureStage::SpawnSidecar,
            None,
            format!("Não foi possível iniciar o Processador de Imagens: {error}"),
        )
    })?;
    let imaging_process_id = child.pid();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = context.operation_id.as_str(),
        project_id = context.project_id.as_deref(),
        operation = operation.as_str(),
        attempt,
        imaging_process_id,
        event = "imaging_process_spawned",
    );
    if let Err(error) = child.write(&payload) {
        let _ = child.kill();
        return Err(InvocationFailure::at_stage(
            InvocationFailureStage::WriteRequest,
            Some(imaging_process_id),
            format!("Não foi possível enviar a solicitação: {error}"),
        ));
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
                    operation_id = context.operation_id.as_str(),
                    project_id = context.project_id.as_deref(),
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
    complete_invocation(imaging_process_id, exit_code, &stdout)
}

pub(crate) fn complete_invocation(
    process_id: u32,
    exit_code: Option<i32>,
    stdout: &[u8],
) -> Result<ImagingResponse, InvocationFailure> {
    if exit_code != Some(0) {
        return Err(InvocationFailure::terminated(process_id, exit_code));
    }
    decode_response(stdout).map_err(|error| {
        InvocationFailure::at_stage(
            InvocationFailureStage::DecodeResponse,
            Some(process_id),
            format!("Resposta inválida do Processador de Imagens: {error}"),
        )
    })
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
