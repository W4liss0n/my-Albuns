use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingEvent, ImagingFailureStage, ImagingProgress,
    ImagingResponse, decode_event, decode_event_stream, encode_command,
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
    Cancelled,
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
            Self::Cancelled => "cancelled",
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

#[derive(Debug)]
pub(crate) struct OperationFailure<Stage> {
    pub(crate) stage: Stage,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message: String,
}

impl<Stage> OperationFailure<Stage> {
    pub(crate) fn new(stage: Stage, message: impl Into<String>) -> Self {
        Self {
            stage,
            exit_code: None,
            message: message.into(),
        }
    }

    pub(crate) fn from_invocation(
        failure: InvocationFailure,
        map_stage: impl FnOnce(InvocationFailureStage) -> Stage,
    ) -> Self {
        Self {
            stage: map_stage(failure.stage),
            exit_code: failure.exit_code,
            message: failure.message,
        }
    }
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

    pub(crate) fn cancelled(process_id: u32) -> Self {
        Self {
            stage: InvocationFailureStage::Cancelled,
            exit_code: None,
            process_id: Some(process_id),
            message: "A operação do Processador de Imagens foi cancelada.".into(),
            termination_observed: false,
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.stage == InvocationFailureStage::Cancelled
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

#[derive(Clone, Copy)]
pub(crate) struct InvocationControl<'a> {
    cancellation: Option<&'a AtomicBool>,
    progress: Option<&'a (dyn Fn(ImagingProgress) + Send + Sync)>,
}

impl<'a> InvocationControl<'a> {
    pub(crate) const fn uncontrolled() -> Self {
        Self {
            cancellation: None,
            progress: None,
        }
    }

    pub(crate) const fn controlled(
        cancellation: &'a AtomicBool,
        progress: &'a (dyn Fn(ImagingProgress) + Send + Sync),
    ) -> Self {
        Self {
            cancellation: Some(cancellation),
            progress: Some(progress),
        }
    }

    pub(crate) fn is_cancelled(self) -> bool {
        self.cancellation
            .is_some_and(|cancellation| cancellation.load(Ordering::Acquire))
    }

    pub(crate) fn report(self, progress: ImagingProgress) {
        if let Some(callback) = self.progress {
            callback(progress);
        }
    }
}

/// Typed boundary shared by the production sidecar adapter and recovery tests.
pub(crate) trait ImagingTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        control: InvocationControl<'a>,
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
        control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        Box::pin(invoke_once(
            self.app,
            self.logging,
            command,
            context,
            operation,
            attempt,
            control,
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
    control: InvocationControl<'_>,
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
        .env(LOG_DIRECTORY_ENV, logging.directory())
        .set_raw_out(true);
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
        wait_for_termination(&mut events).await;
        return Err(InvocationFailure::at_stage(
            InvocationFailureStage::WriteRequest,
            Some(imaging_process_id),
            format!("Não foi possível enviar a solicitação: {error}"),
        ));
    }

    let mut stdout = Vec::new();
    let mut pending_line = Vec::new();
    let mut response_seen = false;
    let mut exit_code = None;
    let mut stream_error = None;
    loop {
        if control.is_cancelled() {
            let kill_error = child.kill().err();
            wait_for_termination(&mut events).await;
            let mut failure = InvocationFailure::cancelled(imaging_process_id);
            if let Some(error) = kill_error {
                failure.message =
                    format!("A operação foi cancelada, mas o encerramento falhou: {error}");
            }
            return Err(failure);
        }
        let event = match tokio::time::timeout(Duration::from_millis(20), events.recv()).await {
            Ok(event) => event,
            Err(_) => continue,
        };
        let Some(event) = event else {
            break;
        };
        match event {
            CommandEvent::Stdout(bytes) => {
                stdout.extend_from_slice(&bytes);
                pending_line.extend_from_slice(&bytes);
                if let Err(error) = report_complete_event_lines(
                    &mut pending_line,
                    &mut response_seen,
                    &context.operation_id,
                    control,
                ) {
                    let _ = child.kill();
                    wait_for_termination(&mut events).await;
                    return Err(InvocationFailure::at_stage(
                        InvocationFailureStage::DecodeResponse,
                        Some(imaging_process_id),
                        format!("Evento inválido do Processador de Imagens: {error}"),
                    ));
                }
            }
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

async fn wait_for_termination(events: &mut tauri::async_runtime::Receiver<CommandEvent>) {
    while let Some(event) = events.recv().await {
        if matches!(event, CommandEvent::Terminated(_)) {
            break;
        }
    }
}

fn report_complete_event_lines(
    pending_line: &mut Vec<u8>,
    response_seen: &mut bool,
    expected_request_id: &str,
    control: InvocationControl<'_>,
) -> Result<(), String> {
    while let Some(newline) = pending_line.iter().position(|byte| *byte == b'\n') {
        let mut line = pending_line.drain(..=newline).collect::<Vec<_>>();
        line.pop();
        if line.is_empty() {
            continue;
        }
        match decode_event(&line)? {
            ImagingEvent::Progress(_) if *response_seen => {
                return Err("o Processador devolveu progresso após a resposta final".into());
            }
            ImagingEvent::Progress(progress) if progress.request_id != expected_request_id => {
                return Err("o progresso não corresponde à operação solicitada".into());
            }
            ImagingEvent::Progress(progress) => control.report(progress),
            ImagingEvent::Response(_) if *response_seen => {
                return Err("o Processador devolveu mais de uma resposta final".into());
            }
            ImagingEvent::Response(response) if response.request_id() != expected_request_id => {
                return Err("a resposta final não corresponde à operação solicitada".into());
            }
            ImagingEvent::Response(_) => *response_seen = true,
        }
    }
    Ok(())
}

pub(crate) fn complete_invocation(
    process_id: u32,
    exit_code: Option<i32>,
    stdout: &[u8],
) -> Result<ImagingResponse, InvocationFailure> {
    if exit_code != Some(0) {
        return Err(InvocationFailure::terminated(process_id, exit_code));
    }
    decode_event_stream(stdout)
        .map(|(_, response)| response)
        .map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::DecodeResponse,
                Some(process_id),
                format!("Resposta inválida do Processador de Imagens: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, atomic::AtomicBool};

    use myalbuns_imaging_protocol::{
        ImagingEvent, ImagingFailureStage, ImagingProgress, ImagingProgressStage, ImagingResponse,
        RenderCompletion, encode_event,
    };

    use super::{
        InvocationControl, InvocationFailure, InvocationFailureStage, complete_invocation,
        report_complete_event_lines,
    };

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

    #[test]
    fn collected_process_status_uses_the_same_recovery_classification_for_every_adapter() {
        let unexpected =
            complete_invocation(4242, Some(1), b"").expect_err("an abrupt exit is a failure");
        assert!(unexpected.is_unexpected_termination());

        let deterministic = complete_invocation(
            4343,
            Some(ImagingFailureStage::SourceDecode.exit_code().into()),
            b"",
        )
        .expect_err("a typed processor exit is a failure");
        assert!(!deterministic.is_unexpected_termination());
        assert_eq!(
            deterministic.stage,
            InvocationFailureStage::Processor(ImagingFailureStage::SourceDecode)
        );
    }

    #[test]
    fn host_reports_chunked_progress_before_decoding_the_final_response() {
        let progress = ImagingProgress::new("render-stream", ImagingProgressStage::Composing, 1, 2)
            .expect("the progress fixture is valid");
        let progress_event = encode_event(&ImagingEvent::Progress(progress.clone()))
            .expect("the progress fixture serializes");
        let split = progress_event.len() / 2;
        let collected = Mutex::new(Vec::new());
        let callback = |event| {
            collected
                .lock()
                .expect("the progress collector is available")
                .push(event);
        };
        let cancellation = AtomicBool::new(false);
        let control = InvocationControl::controlled(&cancellation, &callback);
        let mut pending = Vec::new();
        let mut response_seen = false;

        pending.extend_from_slice(&progress_event[..split]);
        report_complete_event_lines(&mut pending, &mut response_seen, "render-stream", control)
            .expect("an incomplete event remains buffered");
        assert!(
            collected
                .lock()
                .expect("the progress collector is available")
                .is_empty()
        );
        pending.extend_from_slice(&progress_event[split..]);
        report_complete_event_lines(&mut pending, &mut response_seen, "render-stream", control)
            .expect("the completed event is reported");
        assert_eq!(
            collected
                .lock()
                .expect("the progress collector is available")
                .as_slice(),
            [progress]
        );

        let response = ImagingResponse::completed(
            "render-stream",
            RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 1,
                source_bytes: 100,
                output_bytes: 200,
                output_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
            },
        );
        let response_event = encode_event(&ImagingEvent::Response(response.clone()))
            .expect("the response fixture serializes");
        assert_eq!(
            complete_invocation(4242, Some(0), &response_event)
                .expect("the final event stream decodes"),
            response
        );
    }
}
