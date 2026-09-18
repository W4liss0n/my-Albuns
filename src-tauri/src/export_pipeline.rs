use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
mod album;
pub(crate) use album::{
    AlbumExportOptions, AlbumExportPlan, AlbumExportRecovery, execute_album, plan_album,
    plan_album_in_paths, resume_album,
};

use myalbuns_core::RenderSnapshot;
use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingFailure, ImagingFailureCode, ImagingProgress,
    ImagingProgressStage, RenderCompletion, RenderSource,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{
    AppPathsError, ExportPathPlan, ExportWriteAuthorization, NativePathDto, PreparedExportStorage,
    RootBindingPlan,
};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use crate::imaging_processor::{
    ImagingOperation, ImagingTransport, InvocationContext, InvocationControl, InvocationFailure,
    InvocationFailureStage,
};
#[derive(Debug)]
pub(crate) struct PublishedExport {
    pub(crate) completion: RenderCompletion,
}

#[derive(Debug)]
struct ExportPreparationGuard {
    storage: Option<PreparedExportStorage>,
    context: InvocationContext,
}

impl ExportPreparationGuard {
    fn publish_retaining(&mut self) -> Result<(), myalbuns_paths::AppPathsError> {
        self.storage
            .as_ref()
            .expect("owned output")
            .publish_retaining()?;
        // Drop the directory handles before attempting to remove the empty folder.
        if let Some(storage) = self.storage.take() {
            let _ = storage.discard();
        }
        Ok(())
    }
    fn new(storage: PreparedExportStorage, context: &InvocationContext) -> Self {
        Self {
            storage: Some(storage),
            context: context.clone(),
        }
    }

    fn preserve(mut self) {
        if self.storage.take().is_some() {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                protocol_version = IMAGING_PROTOCOL_VERSION,
                operation_id = self.context.operation_id.as_str(),
                project_id = self.context.project_id.as_deref(),
                event = "incomplete_export_preserved",
            );
        }
    }
}

impl Drop for ExportPreparationGuard {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            discard_failed_preparation(storage, &self.context);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportFailureStage {
    Plan,
    Cancelled,
    Prepare,
    Processor(InvocationFailureStage),
    ValidateResponse,
    VerifyPreparation,
    Publish {
        promoted_outputs: u32,
        total_outputs: u32,
    },
}

impl ExportFailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan_request",
            Self::Cancelled => "cancelled",
            Self::Prepare => "prepare_output",
            Self::Processor(stage) => stage.as_str(),
            Self::ValidateResponse => "validate_response",
            Self::VerifyPreparation => "verify_preparation",
            Self::Publish { .. } => "publish_output",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExportFailure {
    pub(crate) stage: ExportFailureStage,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message: String,
    pub(crate) processor_failure: Option<ImagingFailure>,
    pub(crate) path_failure: Option<AppPathsError>,
    pub(crate) recovery: Option<Box<AlbumExportRecovery>>,
}

impl ExportFailure {
    pub(crate) fn new(stage: ExportFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            exit_code: None,
            message: message.into(),
            processor_failure: None,
            path_failure: None,
            recovery: None,
        }
    }

    pub(crate) fn from_path_error(
        stage: ExportFailureStage,
        error: AppPathsError,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path_failure: Some(error),
            ..Self::new(stage, message)
        }
    }

    pub(crate) fn is_storage_full(&self) -> bool {
        self.path_failure == Some(AppPathsError::ExportStorageFull)
            || matches!(
                self.stage,
                ExportFailureStage::Processor(InvocationFailureStage::Processor(
                    myalbuns_imaging_protocol::ImagingFailureStage::OutputStorageFull
                ))
            )
            || self
                .processor_failure
                .as_ref()
                .is_some_and(|failure| failure.code == ImagingFailureCode::OutputStorageFull)
    }

    fn from_invocation(
        failure: InvocationFailure,
        map_stage: impl FnOnce(InvocationFailureStage) -> ExportFailureStage,
    ) -> Self {
        Self {
            stage: map_stage(failure.stage),
            exit_code: failure.exit_code,
            message: failure.message,
            processor_failure: None,
            path_failure: None,
            recovery: None,
        }
    }

    fn from_processor(
        stage: ExportFailureStage,
        failure: ImagingFailure,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            exit_code: None,
            message: message.into(),
            processor_failure: Some(failure),
            path_failure: None,
            recovery: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportCancellationResult {
    Requested,
    AlreadyRequested,
    TooLate,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ExportExecutionPhase {
    #[default]
    Running,
    Cancelled,
    Publishing,
}

#[derive(Debug, Default)]
pub(crate) struct ExportExecutionControl {
    cancelled: AtomicBool,
    phase: Mutex<ExportExecutionPhase>,
    notification: Notify,
}

impl ExportExecutionControl {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        loop {
            let notified = self.notification.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }

    pub(crate) fn request_cancel(&self) -> ExportCancellationResult {
        let mut phase = self
            .phase
            .lock()
            .expect("the Export execution state remains available");
        match *phase {
            ExportExecutionPhase::Running => {
                self.cancelled.store(true, Ordering::Release);
                *phase = ExportExecutionPhase::Cancelled;
                drop(phase);
                self.notification.notify_one();
                ExportCancellationResult::Requested
            }
            ExportExecutionPhase::Cancelled => ExportCancellationResult::AlreadyRequested,
            ExportExecutionPhase::Publishing => ExportCancellationResult::TooLate,
        }
    }

    fn begin_publishing(&self) -> bool {
        let mut phase = self
            .phase
            .lock()
            .expect("the Export execution state remains available");
        match *phase {
            ExportExecutionPhase::Running => {
                *phase = ExportExecutionPhase::Publishing;
                true
            }
            ExportExecutionPhase::Cancelled => false,
            ExportExecutionPhase::Publishing => true,
        }
    }

    fn cancellation_flag(&self) -> &AtomicBool {
        &self.cancelled
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportProgressStage {
    Preparing,
    LoadingSources,
    Composing,
    EncodingOutput,
    Verifying,
    Publishing,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportProgressUnits {
    Unmeasured,
    Measured {
        completed_units: u32,
        total_units: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExportProgress {
    pub(crate) stage: ExportProgressStage,
    pub(crate) units: ExportProgressUnits,
    pub(crate) cancellable: bool,
}

impl ExportProgress {
    /// Overall progress of one export. Consumers own monotonic presentation and
    /// terminal completion (the batch counts an item after recording its result).
    pub(crate) fn overall_percent(self) -> f64 {
        let fraction = match self.units {
            ExportProgressUnits::Measured {
                completed_units,
                total_units,
            } if total_units > 0 => {
                f64::from(completed_units.min(total_units)) / f64::from(total_units)
            }
            _ => 0.0,
        };
        let (start, span) = match self.stage {
            ExportProgressStage::Preparing => (0.0, 0.0),
            ExportProgressStage::LoadingSources => (0.0, 10.0),
            ExportProgressStage::Composing | ExportProgressStage::EncodingOutput => (10.0, 65.0),
            ExportProgressStage::Verifying => (75.0, 10.0),
            ExportProgressStage::Publishing => (85.0, 14.0),
            ExportProgressStage::Completed => (100.0, 0.0),
        };
        start + span * fraction
    }

    const fn unmeasured(stage: ExportProgressStage, cancellable: bool) -> Self {
        Self {
            stage,
            units: ExportProgressUnits::Unmeasured,
            cancellable,
        }
    }

    const fn measured(
        stage: ExportProgressStage,
        completed_units: u32,
        total_units: u32,
        cancellable: bool,
    ) -> Self {
        Self {
            stage,
            units: ExportProgressUnits::Measured {
                completed_units,
                total_units,
            },
            cancellable,
        }
    }
}

fn processor_failure_message(code: ImagingFailureCode) -> &'static str {
    match code {
        ImagingFailureCode::InvalidRenderRequest => "Não foi possível preparar a exportação.",
        ImagingFailureCode::SourceUnavailable => {
            "Uma imagem não foi encontrada ou não pôde ser aberta. Localize-a no painel de imagens e tente exportar novamente."
        }
        ImagingFailureCode::UnsupportedSourceFormat => {
            "Uma imagem não está em um formato aceito. Use JPEG ou PNG sem animação."
        }
        ImagingFailureCode::UnsupportedSourceVariant => {
            "Uma imagem usa uma versão do formato que não é aceita para exportação."
        }
        ImagingFailureCode::UnsupportedColorModel => {
            "Uma imagem usa um modelo de cor que não é aceito para exportação."
        }
        ImagingFailureCode::UnsupportedColorProfile => {
            "Uma imagem tem um perfil de cor inválido ou incompatível com a exportação."
        }
        ImagingFailureCode::DecodeFailed => "Não foi possível ler uma imagem para exportar.",
        ImagingFailureCode::CompositionFailed => {
            "Não foi possível preparar a lâmina para exportar."
        }
        ImagingFailureCode::ResourceLimitExceeded => {
            "A exportação precisa de mais recursos do que esta versão permite."
        }
        ImagingFailureCode::EncodeFailed => "Não foi possível gravar o arquivo exportado.",
        ImagingFailureCode::OutputStorageFull => {
            myalbuns_paths::AppPathsError::EXPORT_STORAGE_FULL_MESSAGE
        }
        ImagingFailureCode::VerificationFailed => {
            "Não foi possível confirmar se o arquivo exportado está completo."
        }
    }
}

fn bind_execution_paths(
    logical_plan: &ExportPathPlan,
    root_bindings: &RootBindingPlan,
    request_id: &str,
) -> Result<ExportPathPlan, ExportFailure> {
    let operational_output =
        root_bindings
            .resolve(logical_plan.output_path())
            .map_err(|error| {
                ExportFailure::new(
                    ExportFailureStage::Prepare,
                    format!("Não foi possível aplicar o plano de caminhos: {error}"),
                )
            })?;
    let operational_plan = ExportPathPlan::new_authorized(
        operational_output,
        request_id,
        logical_plan.authorization(),
    )
    .map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Prepare,
            format!("O Destino operacional da Exportação é inválido: {error}"),
        )
    })?;
    let expected_preparation = root_bindings
        .resolve(logical_plan.prepared_output_path())
        .map_err(|error| {
            ExportFailure::new(
                ExportFailureStage::Prepare,
                format!("Não foi possível aplicar o plano de caminhos: {error}"),
            )
        })?;
    if operational_plan.prepared_output_path() != expected_preparation {
        return Err(ExportFailure::new(
            ExportFailureStage::Prepare,
            "A preparação operacional não corresponde ao plano de raízes.",
        ));
    }
    Ok(operational_plan)
}

fn ensure_not_cancelled(control: &ExportExecutionControl) -> Result<(), ExportFailure> {
    if control.is_cancelled() {
        Err(cancelled_failure())
    } else {
        Ok(())
    }
}

fn cancelled_failure() -> ExportFailure {
    ExportFailure::new(ExportFailureStage::Cancelled, "A Exportação foi cancelada.")
}

fn verify_preparation(
    path_plan: &ExportPathPlan,
    completion: &RenderCompletion,
) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path_plan.prepared_output_path())
        .map_err(|error| format!("A preparação da Exportação está indisponível: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != completion.output_bytes
    {
        return Err("A preparação da Exportação não corresponde à resposta recebida.".into());
    }
    let file = File::open(path_plan.prepared_output_path())
        .map_err(|error| format!("Não foi possível verificar a Exportação: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Não foi possível verificar a Exportação: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let sha256 = format!("{:x}", hasher.finalize());
    if !sha256.eq_ignore_ascii_case(&completion.output_sha256) {
        return Err("O conteúdo preparado não corresponde à resposta recebida.".into());
    }
    Ok(())
}

fn discard_failed_preparation(preparation: PreparedExportStorage, context: &InvocationContext) {
    match preparation.discard() {
        Ok(removed) => tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = context.operation_id.as_str(),
            project_id = context.project_id.as_deref(),
            removed,
            event = "incomplete_export_discarded",
        ),
        Err(_) => tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = context.operation_id.as_str(),
            project_id = context.project_id.as_deref(),
            event = "incomplete_export_cleanup_failed",
        ),
    }
}

#[cfg(test)]
mod tests;
