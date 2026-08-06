use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use myalbuns_core::{ComposedOutputUnit, RenderSnapshot};
use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingFailure, ImagingFailureCode, ImagingProgress,
    ImagingProgressStage, ImagingRequest, RenderCompletion, RenderSource, has_jpeg_extension,
    validate_render_content,
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
pub(crate) struct ExportPlan {
    unit: ComposedOutputUnit,
    dpi: u32,
    project_id: String,
    revision: u64,
    request_id: String,
    path_plan: ExportPathPlan,
    sources: Vec<RenderSource>,
}

#[derive(Clone, Debug)]
pub(crate) struct ExportOptions {
    request_id: String,
    output_path: PathBuf,
    authorization: ExportWriteAuthorization,
    sheet_id: String,
    sources: Vec<RenderSource>,
}

impl ExportOptions {
    pub(crate) fn new(
        request_id: impl Into<String>,
        output_path: PathBuf,
        authorization: ExportWriteAuthorization,
        sheet_id: impl Into<String>,
        sources: Vec<RenderSource>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            output_path,
            authorization,
            sheet_id: sheet_id.into(),
            sources,
        }
    }
}

impl ExportPlan {
    pub(crate) fn required_paths(&self) -> Vec<&std::path::Path> {
        let mut paths = Vec::with_capacity(self.sources.len() + 1);
        paths.push(self.path_plan.output_path());
        paths.extend(self.sources.iter().map(RenderSource::source_path));
        paths
    }

    #[cfg(test)]
    fn path_plan(&self) -> &ExportPathPlan {
        &self.path_plan
    }
}

#[derive(Debug)]
pub(crate) struct PublishedExport {
    pub(crate) completion: RenderCompletion,
}

struct ExportPreparationGuard {
    storage: Option<PreparedExportStorage>,
    context: InvocationContext,
}

impl ExportPreparationGuard {
    fn new(storage: PreparedExportStorage, context: &InvocationContext) -> Self {
        Self {
            storage: Some(storage),
            context: context.clone(),
        }
    }

    fn publish(mut self) -> Result<(), myalbuns_paths::AppPathsError> {
        self.storage
            .take()
            .expect("an active Export preparation is published at most once")
            .publish()
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

struct PreparedExport {
    preparation: ExportPreparationGuard,
    completion: RenderCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportFailureStage {
    Plan,
    Cancelled,
    ExportConflict,
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
            Self::ExportConflict => "export_conflict",
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
}

impl ExportFailure {
    pub(crate) fn new(stage: ExportFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            exit_code: None,
            message: message.into(),
            processor_failure: None,
        }
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

pub(crate) fn plan(
    snapshot: RenderSnapshot,
    options: ExportOptions,
) -> Result<ExportPlan, ExportFailure> {
    let ExportOptions {
        request_id,
        output_path,
        authorization,
        sheet_id,
        sources,
    } = options;
    let path_plan = ExportPathPlan::new_authorized(output_path.clone(), &request_id, authorization)
        .map_err(|error| {
            ExportFailure::new(
                ExportFailureStage::Plan,
                format!("Não foi possível planejar o Destino da Exportação: {error}"),
            )
        })?;
    if !has_jpeg_extension(&output_path) {
        return Err(ExportFailure::new(
            ExportFailureStage::Plan,
            "O Destino da Exportação precisa usar a extensão .jpg.",
        ));
    }
    snapshot.validate().map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível congelar a Exportação: {error}"),
        )
    })?;
    let unit = snapshot.output_unit(&sheet_id).map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível selecionar a Lâmina da Exportação: {error}"),
        )
    })?;
    validate_render_content(&unit, snapshot.dpi, &sources).map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar a Exportação: {error}"),
        )
    })?;
    Ok(ExportPlan {
        unit,
        dpi: snapshot.dpi,
        project_id: snapshot.project_id,
        revision: snapshot.revision,
        request_id,
        path_plan,
        sources,
    })
}

pub(crate) async fn execute<T: ImagingTransport>(
    transport: &mut T,
    plan: ExportPlan,
    root_bindings: &RootBindingPlan,
    control: &ExportExecutionControl,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
    context: &InvocationContext,
) -> Result<PublishedExport, ExportFailure> {
    let mut published = execute_group(
        transport,
        vec![(plan, context.clone())],
        root_bindings,
        control,
        progress,
    )
    .await?;
    Ok(published
        .pop()
        .expect("a single Export execution publishes exactly one output"))
}

pub(crate) async fn execute_group<T: ImagingTransport>(
    transport: &mut T,
    exports: Vec<(ExportPlan, InvocationContext)>,
    root_bindings: &RootBindingPlan,
    control: &ExportExecutionControl,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
) -> Result<Vec<PublishedExport>, ExportFailure> {
    if exports.is_empty() {
        return Err(ExportFailure::new(
            ExportFailureStage::Plan,
            "A Exportação agrupada precisa conter ao menos uma saída.",
        ));
    }
    ensure_not_cancelled(control)?;
    let total_units = u32::try_from(exports.len()).map_err(|_| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            "A Exportação agrupada excede a quantidade de saídas suportada.",
        )
    })?;
    let mut preparations = Vec::with_capacity(exports.len());
    for (plan, context) in exports {
        preparations.push(
            prepare_export(transport, plan, root_bindings, control, progress, &context).await?,
        );
    }
    if !control.begin_publishing() {
        return Err(cancelled_failure());
    }
    progress(ExportProgress::unmeasured(
        ExportProgressStage::Publishing,
        false,
    ));
    let total = preparations.len();
    let mut published = Vec::with_capacity(total);
    for prepared in preparations {
        let PreparedExport {
            preparation,
            completion,
        } = prepared;
        if let Err(error) = preparation.publish() {
            let message = if total == 1 {
                format!("Não foi possível publicar a Exportação: {error}")
            } else {
                format!(
                    "Não foi possível publicar a Exportação após promover {} de {total} saídas: {error}",
                    published.len()
                )
            };
            let stage = if error == AppPathsError::ExportTargetConflict {
                ExportFailureStage::ExportConflict
            } else {
                ExportFailureStage::Publish {
                    promoted_outputs: u32::try_from(published.len())
                        .expect("the promoted count fits the validated total"),
                    total_outputs: total_units,
                }
            };
            return Err(ExportFailure::new(stage, message));
        }
        published.push(PublishedExport { completion });
    }
    progress(ExportProgress::measured(
        ExportProgressStage::Completed,
        total_units,
        total_units,
        false,
    ));
    Ok(published)
}

async fn prepare_export<T: ImagingTransport>(
    transport: &mut T,
    plan: ExportPlan,
    root_bindings: &RootBindingPlan,
    control: &ExportExecutionControl,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
    context: &InvocationContext,
) -> Result<PreparedExport, ExportFailure> {
    ensure_not_cancelled(control)?;
    let ExportPlan {
        unit,
        dpi,
        project_id,
        revision,
        request_id,
        path_plan,
        sources,
    } = plan;
    if context.operation_id != request_id {
        return Err(ExportFailure::new(
            ExportFailureStage::Plan,
            "A correlação da Exportação não corresponde ao plano.",
        ));
    }
    let execution_path_plan = bind_execution_paths(&path_plan, root_bindings, &request_id)?;
    let request = ImagingRequest::new(
        request_id,
        project_id,
        revision,
        NativePathDto::from(path_plan.prepared_output_path()),
        unit,
        dpi,
        sources,
        root_bindings.clone(),
    )
    .map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar a Exportação: {error}"),
        )
    })?;
    if request.prepared_output_path() != path_plan.prepared_output_path() {
        return Err(ExportFailure::new(
            ExportFailureStage::Prepare,
            "A preparação da Exportação não corresponde ao plano de caminhos.",
        ));
    }
    progress(ExportProgress::unmeasured(
        ExportProgressStage::Preparing,
        true,
    ));
    let preparation = ExportPreparationGuard::new(
        execution_path_plan.prepare().map_err(|error| {
            ExportFailure::new(
                ExportFailureStage::Prepare,
                format!("Não foi possível preparar a Exportação: {error}"),
            )
        })?,
        context,
    );
    ensure_not_cancelled(control)?;
    let processor_progress = |event: ImagingProgress| {
        let stage = match event.stage {
            ImagingProgressStage::LoadingSources => ExportProgressStage::LoadingSources,
            ImagingProgressStage::Composing => ExportProgressStage::Composing,
            ImagingProgressStage::EncodingOutput => ExportProgressStage::EncodingOutput,
        };
        progress(ExportProgress::measured(
            stage,
            event.completed_units,
            event.total_units,
            true,
        ));
    };
    let command = ImagingCommand::render(request.clone());
    let response = match transport
        .invoke(
            &command,
            context,
            ImagingOperation::Export,
            1,
            InvocationControl::controlled(control.cancellation_flag(), &processor_progress),
        )
        .await
    {
        Ok(response) => response,
        Err(failure) if failure.is_cancelled() => return Err(cancelled_failure()),
        Err(failure) if failure.is_termination_unconfirmed() => {
            preparation.preserve();
            return Err(ExportFailure::from_invocation(
                failure,
                ExportFailureStage::Processor,
            ));
        }
        Err(failure) => {
            return Err(ExportFailure::from_invocation(
                failure,
                ExportFailureStage::Processor,
            ));
        }
    };
    ensure_not_cancelled(control)?;
    if let Some(failure) = response.failure_for(&request.request_id) {
        let stage = failure.code.stage();
        let message = processor_failure_message(failure.code);
        return Err(ExportFailure::from_processor(
            ExportFailureStage::Processor(InvocationFailureStage::Processor(stage)),
            failure,
            message,
        ));
    }
    let Some(completion) = response.completed_for(&request.request_id).cloned() else {
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O Processador de Imagens devolveu uma resposta inesperada.",
        ));
    };
    progress(ExportProgress::unmeasured(
        ExportProgressStage::Verifying,
        true,
    ));
    if let Err(message) = verify_preparation(&execution_path_plan, &completion) {
        return Err(ExportFailure::new(
            ExportFailureStage::VerifyPreparation,
            message,
        ));
    }
    Ok(PreparedExport {
        preparation,
        completion,
    })
}

fn processor_failure_message(code: ImagingFailureCode) -> &'static str {
    match code {
        ImagingFailureCode::InvalidRenderRequest => {
            "A solicitação de Exportação não corresponde ao contrato do Processador."
        }
        ImagingFailureCode::SourceUnavailable => {
            "Uma fonte original necessária não está disponível para a Exportação."
        }
        ImagingFailureCode::UnsupportedSourceFormat => {
            "Uma fonte original não usa JPEG ou PNG estático aceito neste fluxo."
        }
        ImagingFailureCode::UnsupportedSourceVariant => {
            "Uma fonte original usa uma variante de imagem não aceita neste fluxo."
        }
        ImagingFailureCode::UnsupportedColorModel => {
            "Uma fonte original usa um modelo de cor não aceito neste fluxo."
        }
        ImagingFailureCode::UnsupportedColorProfile => {
            "Uma fonte original contém um perfil de cor não permitido ou malformado."
        }
        ImagingFailureCode::DecodeFailed => {
            "Uma fonte original permitida não pôde ser decodificada para a Exportação."
        }
        ImagingFailureCode::CompositionFailed => "A composição da Lâmina não pôde ser concluída.",
        ImagingFailureCode::ResourceLimitExceeded => {
            "A Exportação excede o limite seguro de recursos desta versão."
        }
        ImagingFailureCode::EncodeFailed => "O JPEG não pôde ser codificado e sincronizado.",
        ImagingFailureCode::VerificationFailed => {
            "O JPEG preparado não passou pela verificação de integridade."
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
