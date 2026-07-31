use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(test)]
use myalbuns_imaging_protocol::decode_event_stream;
use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingEventStreamDecoder, ImagingFailureStage,
    ImagingProgress, ImagingResponse, encode_command,
    root_binding_plan_sha256 as digest_root_binding_plan,
};
use myalbuns_logging::{LOG_DIRECTORY_ENV, ProcessRole};
use tauri::AppHandle;
use tauri_plugin_shell::{
    ShellExt,
    process::{CommandChild, CommandEvent},
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::logging::LoggingState;

const PROCESS_TERMINATION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub(crate) struct ImagingProcessor {
    reservation: Arc<AsyncMutex<()>>,
    quarantined: Arc<AtomicBool>,
}

#[derive(Debug)]
pub(crate) struct ProcessorReservation {
    _guard: OwnedMutexGuard<()>,
    quarantined: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessorUnavailable;

impl ImagingProcessor {
    pub(crate) async fn reserve(&self) -> Result<ProcessorReservation, ProcessorUnavailable> {
        if self.quarantined.load(Ordering::Acquire) {
            return Err(ProcessorUnavailable);
        }
        let guard = self.reservation.clone().lock_owned().await;
        if self.quarantined.load(Ordering::Acquire) {
            return Err(ProcessorUnavailable);
        }
        Ok(ProcessorReservation {
            _guard: guard,
            quarantined: Arc::clone(&self.quarantined),
        })
    }
}

impl ProcessorReservation {
    pub(crate) fn quarantine(&self) {
        self.quarantined.store(true, Ordering::Release);
    }
}

impl fmt::Display for ProcessorUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "o Processador de Imagens está em quarentena porque o encerramento anterior não foi confirmado; reinicie o aplicativo antes de tentar novamente",
        )
    }
}

impl std::error::Error for ProcessorUnavailable {}

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
    TerminationUnconfirmed,
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
            Self::TerminationUnconfirmed => "termination_unconfirmed",
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

    pub(crate) fn termination_unconfirmed(process_id: u32, message: impl Into<String>) -> Self {
        Self {
            stage: InvocationFailureStage::TerminationUnconfirmed,
            exit_code: None,
            process_id: Some(process_id),
            message: message.into(),
            termination_observed: false,
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.stage == InvocationFailureStage::Cancelled
    }

    pub(crate) fn is_termination_unconfirmed(&self) -> bool {
        self.stage == InvocationFailureStage::TerminationUnconfirmed
    }

    pub(crate) fn is_unexpected_termination(&self) -> bool {
        self.termination_observed
            && self
                .exit_code
                .and_then(ImagingFailureStage::from_exit_code)
                .is_none()
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
    _reservation: &'a ProcessorReservation,
}

impl<'a> TauriImagingTransport<'a> {
    pub(crate) fn new(
        app: &'a AppHandle,
        logging: &'a LoggingState,
        reservation: &'a ProcessorReservation,
    ) -> Self {
        Self {
            app,
            logging,
            _reservation: reservation,
        }
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
        Box::pin(async move {
            protect_processor_after_invocation(
                self._reservation,
                invoke_once(
                    self.app,
                    self.logging,
                    command,
                    context,
                    operation,
                    attempt,
                    control,
                )
                .await,
            )
        })
    }
}

fn protect_processor_after_invocation(
    reservation: &ProcessorReservation,
    result: Result<ImagingResponse, InvocationFailure>,
) -> Result<ImagingResponse, InvocationFailure> {
    if result
        .as_ref()
        .is_err_and(InvocationFailure::is_termination_unconfirmed)
    {
        reservation.quarantine();
    }
    result
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
    let root_binding_plan_sha256 = command
        .root_bindings()
        .map(digest_root_binding_plan)
        .transpose()
        .map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::EncodeRequest,
                None,
                format!("Não foi possível correlacionar o plano de caminhos: {error}"),
            )
        })?;
    let mut decoder =
        ImagingEventStreamDecoder::for_request(&context.operation_id).map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::EncodeRequest,
                None,
                format!("A correlação da solicitação é inválida: {error}"),
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
        process_id = std::process::id(),
        imaging_process_id,
        root_binding_plan_sha256 = root_binding_plan_sha256.as_deref(),
        event = "imaging_process_spawned",
    );
    if let Err(error) = child.write(&payload) {
        terminate_process(child, &mut events, imaging_process_id).await?;
        return Err(InvocationFailure::at_stage(
            InvocationFailureStage::WriteRequest,
            Some(imaging_process_id),
            format!("Não foi possível enviar a solicitação: {error}"),
        ));
    }

    let mut exit_code = None;
    let mut termination_observed = false;
    let mut stream_error = None;
    loop {
        if control.is_cancelled() {
            let kill_error = terminate_process(child, &mut events, imaging_process_id).await?;
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
                if let Err(error) = decode_and_report_event_chunk(&mut decoder, &bytes, control) {
                    terminate_process(child, &mut events, imaging_process_id).await?;
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
                termination_observed = true;
                exit_code = payload.code;
                break;
            }
            _ => {}
        }
    }

    if !termination_observed {
        let message = stream_error.map_or_else(
            || {
                "O canal de eventos foi fechado sem confirmar o encerramento do Processador de Imagens."
                    .to_string()
            },
            |error| {
                format!(
                    "O canal de eventos falhou sem confirmar o encerramento do Processador de Imagens: {error}"
                )
            },
        );
        return Err(InvocationFailure::termination_unconfirmed(
            imaging_process_id,
            message,
        ));
    }
    if let Some(error) = stream_error {
        return Err(InvocationFailure {
            stage: InvocationFailureStage::ReadResponse,
            exit_code,
            process_id: Some(imaging_process_id),
            message: format!("Não foi possível receber a resposta do Processador: {error}"),
            termination_observed: true,
        });
    }
    if exit_code != Some(0) {
        return Err(InvocationFailure::terminated(imaging_process_id, exit_code));
    }
    decoder.finish().map_err(|error| {
        InvocationFailure::at_stage(
            InvocationFailureStage::DecodeResponse,
            Some(imaging_process_id),
            format!("Resposta inválida do Processador de Imagens: {error}"),
        )
    })
}

async fn terminate_process(
    child: CommandChild,
    events: &mut tauri::async_runtime::Receiver<CommandEvent>,
    process_id: u32,
) -> Result<Option<String>, InvocationFailure> {
    let kill_error = child.kill().err().map(|error| error.to_string());
    if wait_for_termination(events).await {
        return Ok(kill_error);
    }
    let message = kill_error.map_or_else(
        || {
            "O encerramento do Processador de Imagens não foi confirmado dentro do limite."
                .to_string()
        },
        |error| {
            format!("O encerramento do Processador de Imagens falhou e não foi confirmado: {error}")
        },
    );
    Err(InvocationFailure::termination_unconfirmed(
        process_id, message,
    ))
}

async fn wait_for_termination(events: &mut tauri::async_runtime::Receiver<CommandEvent>) -> bool {
    wait_for_termination_for(events, PROCESS_TERMINATION_TIMEOUT).await
}

async fn wait_for_termination_for(
    events: &mut tauri::async_runtime::Receiver<CommandEvent>,
    timeout: Duration,
) -> bool {
    tokio::time::timeout(timeout, async {
        loop {
            match events.recv().await {
                Some(CommandEvent::Terminated(_)) => return true,
                Some(_) => {}
                None => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

fn decode_and_report_event_chunk(
    decoder: &mut ImagingEventStreamDecoder,
    chunk: &[u8],
    control: InvocationControl<'_>,
) -> Result<(), String> {
    for progress in decoder.push(chunk)? {
        control.report(progress);
    }
    Ok(())
}

#[cfg(test)]
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
    use std::{
        sync::{Mutex, atomic::AtomicBool},
        time::Duration,
    };

    use myalbuns_imaging_protocol::{
        ImagingEvent, ImagingEventStreamDecoder, ImagingFailureStage, ImagingProgress,
        ImagingProgressStage, ImagingResponse, RenderCompletion, encode_event,
    };
    use tauri_plugin_shell::process::{CommandEvent, TerminatedPayload};

    use super::{
        ImagingProcessor, InvocationControl, InvocationFailure, InvocationFailureStage,
        complete_invocation, decode_and_report_event_chunk, protect_processor_after_invocation,
        wait_for_termination_for,
    };

    #[test]
    fn processor_reservation_serializes_callers_and_is_released_with_its_guard() {
        tauri::async_runtime::block_on(async {
            let processor = ImagingProcessor::default();
            let first = processor
                .reserve()
                .await
                .expect("the healthy Processor can be reserved");
            let second = processor.reserve();
            tokio::pin!(second);

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut second)
                    .await
                    .is_err(),
                "a second caller must wait while the Processor is reserved"
            );

            drop(first);
            tokio::time::timeout(Duration::from_secs(1), &mut second)
                .await
                .expect("the Processor is available when its reservation is released")
                .expect("the Processor remains healthy");
        });
    }

    #[test]
    fn unconfirmed_termination_quarantines_the_processor_before_releasing_the_guard() {
        tauri::async_runtime::block_on(async {
            let processor = ImagingProcessor::default();
            let reservation = processor
                .reserve()
                .await
                .expect("a healthy Processor can be reserved");

            reservation.quarantine();
            drop(reservation);

            assert!(
                processor.reserve().await.is_err(),
                "a new sidecar cannot start after termination became unconfirmed"
            );
        });
    }

    #[test]
    fn transport_failure_marks_the_shared_processor_quarantine() {
        tauri::async_runtime::block_on(async {
            let processor = ImagingProcessor::default();
            let reservation = processor
                .reserve()
                .await
                .expect("a healthy Processor can be reserved");
            let result: Result<ImagingResponse, InvocationFailure> =
                Err(InvocationFailure::termination_unconfirmed(
                    4242,
                    "injected unconfirmed termination",
                ));

            let failure = protect_processor_after_invocation(&reservation, result)
                .expect_err("the transport failure remains visible");
            assert!(failure.is_termination_unconfirmed());
            drop(reservation);
            assert!(processor.reserve().await.is_err());
        });
    }

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
        let progress = [
            ImagingProgress::new("render-stream", ImagingProgressStage::LoadingSources, 1, 1)
                .expect("the source progress fixture is valid"),
            ImagingProgress::new("render-stream", ImagingProgressStage::Composing, 2, 2)
                .expect("the composition progress fixture is valid"),
            ImagingProgress::new("render-stream", ImagingProgressStage::EncodingOutput, 1, 1)
                .expect("the encoding progress fixture is valid"),
        ];
        let mut event_stream = Vec::new();
        for event in &progress {
            event_stream.extend(
                encode_event(&ImagingEvent::Progress(event.clone()))
                    .expect("the progress fixture serializes"),
            );
        }
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
        event_stream.extend(
            encode_event(&ImagingEvent::Response(response.clone()))
                .expect("the response fixture serializes"),
        );
        let split = event_stream.len() / 2;
        let collected = Mutex::new(Vec::new());
        let callback = |event| {
            collected
                .lock()
                .expect("the progress collector is available")
                .push(event);
        };
        let cancellation = AtomicBool::new(false);
        let control = InvocationControl::controlled(&cancellation, &callback);
        let mut decoder =
            ImagingEventStreamDecoder::for_request("render-stream").expect("the request is valid");

        decode_and_report_event_chunk(&mut decoder, &event_stream[..split], control)
            .expect("an incomplete event remains buffered");
        decode_and_report_event_chunk(&mut decoder, &event_stream[split..], control)
            .expect("the completed event is reported");
        assert_eq!(
            collected
                .lock()
                .expect("the progress collector is available")
                .as_slice(),
            progress
        );
        assert_eq!(
            decoder.finish().expect("the response is decoded once"),
            response
        );
    }

    #[test]
    fn process_reaping_has_a_finite_wait() {
        tauri::async_runtime::block_on(async {
            let (_sender, mut events) = tauri::async_runtime::channel(1);
            assert!(
                !wait_for_termination_for(&mut events, Duration::from_millis(1)).await,
                "an open event channel cannot make process reaping wait indefinitely"
            );
        });
    }

    #[test]
    fn a_closed_event_channel_does_not_claim_process_termination() {
        tauri::async_runtime::block_on(async {
            let (sender, mut events) = tauri::async_runtime::channel(1);
            drop(sender);
            assert!(
                !wait_for_termination_for(&mut events, Duration::from_secs(1)).await,
                "only an explicit Terminated event confirms process termination"
            );
        });
    }

    #[test]
    fn an_explicit_terminated_event_confirms_process_termination() {
        tauri::async_runtime::block_on(async {
            let (sender, mut events) = tauri::async_runtime::channel(1);
            sender
                .send(CommandEvent::Terminated(TerminatedPayload {
                    code: None,
                    signal: Some(9),
                }))
                .await
                .expect("the termination event is delivered");
            assert!(
                wait_for_termination_for(&mut events, Duration::from_secs(1)).await,
                "a signal termination is confirmed even without an exit code"
            );
        });
    }
}
