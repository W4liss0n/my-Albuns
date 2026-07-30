use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use myalbuns_core::RenderSnapshot;
use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingProgress, ImagingProgressStage,
    ImagingRequest, MediaSource, RenderCompletion, RenderSourcePolicy, validate_render_content,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{ExportPathPlan, PreparedExportStorage, RootBindingPlan};
use sha2::{Digest, Sha256};

use crate::imaging_processor::{
    ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
    InvocationFailureStage, OperationFailure,
};

#[derive(Debug)]
pub(crate) struct ExportPlan {
    snapshot: RenderSnapshot,
    options: ExportOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct ExportOptions {
    request_id: String,
    output_path: PathBuf,
    sheet_id: String,
    dpi: u32,
    sources: Option<Vec<MediaSource>>,
}

impl ExportOptions {
    pub(crate) fn new(
        request_id: impl Into<String>,
        output_path: PathBuf,
        sheet_id: impl Into<String>,
        dpi: u32,
        sources: Option<Vec<MediaSource>>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            output_path,
            sheet_id: sheet_id.into(),
            dpi,
            sources,
        }
    }
}

impl ExportPlan {
    pub(crate) fn required_paths(&self) -> Vec<&Path> {
        let mut paths = Vec::with_capacity(
            self.options
                .sources
                .as_ref()
                .map_or(1, |sources| sources.len() + 1),
        );
        paths.push(self.options.output_path.as_path());
        if let Some(sources) = &self.options.sources {
            paths.extend(sources.iter().map(MediaSource::source_path));
        }
        paths
    }

    fn path_plan(&self) -> ExportPathPlan {
        ExportPathPlan::new(self.options.output_path.clone(), &self.options.request_id)
            .expect("ExportPlan only exists after its path plan was validated")
    }
}

#[derive(Debug)]
pub(crate) struct PublishedExport {
    pub(crate) output_path: PathBuf,
    pub(crate) completion: RenderCompletion,
}

struct ExportPreparationGuard<'a> {
    storage: Option<PreparedExportStorage>,
    context: &'a InvocationContext,
}

impl<'a> ExportPreparationGuard<'a> {
    fn new(storage: PreparedExportStorage, context: &'a InvocationContext) -> Self {
        Self {
            storage: Some(storage),
            context,
        }
    }

    fn publish(mut self) -> Result<(), myalbuns_paths::AppPathsError> {
        self.storage
            .take()
            .expect("an active Export preparation is published at most once")
            .publish()
    }
}

impl Drop for ExportPreparationGuard<'_> {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            discard_failed_preparation(storage, self.context);
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
    Publish,
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
            Self::Publish => "publish_output",
        }
    }
}

pub(crate) type ExportFailure = OperationFailure<ExportFailureStage>;

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
pub(crate) struct ExportProgress {
    pub(crate) stage: ExportProgressStage,
    pub(crate) completed_units: u32,
    pub(crate) total_units: u32,
}

impl ExportProgress {
    const fn at(stage: ExportProgressStage, completed_units: u32) -> Self {
        Self {
            stage,
            completed_units,
            total_units: 1,
        }
    }
}

pub(crate) fn plan(
    snapshot: RenderSnapshot,
    options: ExportOptions,
) -> Result<ExportPlan, ExportFailure> {
    let _path_plan = ExportPathPlan::new(options.output_path.clone(), &options.request_id)
        .map_err(|error| {
            ExportFailure::new(
                ExportFailureStage::Plan,
                format!("Não foi possível planejar o Destino da Exportação: {error}"),
            )
        })?;
    validate_plan_inputs(&snapshot, &options)?;
    Ok(ExportPlan { snapshot, options })
}

fn validate_plan_inputs(
    snapshot: &RenderSnapshot,
    options: &ExportOptions,
) -> Result<(), ExportFailure> {
    let (sources, source_policy) = options.sources.as_deref().map_or(
        (&[] as &[MediaSource], RenderSourcePolicy::ProceduralFixture),
        |sources| (sources, RenderSourcePolicy::LinkedOriginals),
    );
    validate_render_content(
        snapshot,
        &options.sheet_id,
        options.dpi,
        sources,
        source_policy,
    )
    .map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar a Exportação: {error}"),
        )
    })
}

pub(crate) async fn execute<T: ImagingTransport>(
    transport: &mut T,
    plan: ExportPlan,
    root_bindings: &RootBindingPlan,
    cancellation: &AtomicBool,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
    context: &InvocationContext,
) -> Result<PublishedExport, ExportFailure> {
    ensure_not_cancelled(cancellation)?;
    let path_plan = plan.path_plan();
    let ExportPlan { snapshot, options } = plan;
    let ExportOptions {
        request_id,
        output_path,
        sheet_id,
        dpi,
        sources,
    } = options;
    if context.operation_id != request_id {
        return Err(ExportFailure::new(
            ExportFailureStage::Plan,
            "A correlação da Exportação não corresponde ao plano.",
        ));
    }
    let execution_path_plan = bind_execution_paths(&path_plan, root_bindings, &request_id)?;
    let request = match sources {
        Some(sources) => ImagingRequest::new(
            request_id,
            path_plan.prepared_output_path().to_path_buf(),
            snapshot,
            sheet_id,
            dpi,
            sources,
            root_bindings.clone(),
        ),
        None => ImagingRequest::procedural_fixture(
            request_id,
            path_plan.prepared_output_path().to_path_buf(),
            snapshot,
            sheet_id,
            dpi,
            root_bindings.clone(),
        ),
    }
    .map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar a Exportação: {error}"),
        )
    })?;
    if request.prepared_output_path != path_plan.prepared_output_path() {
        return Err(ExportFailure::new(
            ExportFailureStage::Prepare,
            "A preparação da Exportação não corresponde ao plano de caminhos.",
        ));
    }
    progress(ExportProgress::at(ExportProgressStage::Preparing, 0));
    let preparation = ExportPreparationGuard::new(
        execution_path_plan.prepare().map_err(|error| {
            ExportFailure::new(
                ExportFailureStage::Prepare,
                format!("Não foi possível preparar a Exportação: {error}"),
            )
        })?,
        context,
    );
    if cancellation.load(Ordering::Acquire) {
        return Err(cancelled_failure());
    }
    let processor_progress = |event: ImagingProgress| {
        let stage = match event.stage {
            ImagingProgressStage::LoadingSources => ExportProgressStage::LoadingSources,
            ImagingProgressStage::Composing => ExportProgressStage::Composing,
            ImagingProgressStage::EncodingOutput => ExportProgressStage::EncodingOutput,
        };
        progress(ExportProgress {
            stage,
            completed_units: event.completed_units,
            total_units: event.total_units,
        });
    };
    let command = ImagingCommand::render(request.clone());
    let response = match transport
        .invoke(
            &command,
            context,
            ImagingOperation::Export,
            1,
            InvocationControl::controlled(cancellation, &processor_progress),
        )
        .await
    {
        Ok(response) => response,
        Err(failure) if failure.is_cancelled() => return Err(cancelled_failure()),
        Err(failure) => {
            return Err(ExportFailure::from_invocation(
                failure,
                ExportFailureStage::Processor,
            ));
        }
    };
    if cancellation.load(Ordering::Acquire) {
        return Err(cancelled_failure());
    }
    let Some(completion) = response.completed_for(&request.request_id).cloned() else {
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O Processador de Imagens devolveu uma resposta inesperada.",
        ));
    };
    progress(ExportProgress::at(ExportProgressStage::Verifying, 0));
    if let Err(message) = verify_preparation(&execution_path_plan, &completion) {
        return Err(ExportFailure::new(
            ExportFailureStage::VerifyPreparation,
            message,
        ));
    }
    if cancellation.load(Ordering::Acquire) {
        return Err(cancelled_failure());
    }
    progress(ExportProgress::at(ExportProgressStage::Publishing, 0));
    preparation.publish().map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Publish,
            format!("Não foi possível publicar a Exportação: {error}"),
        )
    })?;
    progress(ExportProgress::at(ExportProgressStage::Completed, 1));
    Ok(PublishedExport {
        output_path,
        completion,
    })
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
    let operational_plan =
        ExportPathPlan::new(operational_output, request_id).map_err(|error| {
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

fn ensure_not_cancelled(cancellation: &AtomicBool) -> Result<(), ExportFailure> {
    if cancellation.load(Ordering::Acquire) {
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
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    use myalbuns_core::ProjectCore;
    use myalbuns_imaging_protocol::{
        ImagingCommand, ImagingProgress, ImagingProgressStage, ImagingResponse, RenderCompletion,
    };
    use myalbuns_paths::{OperationPathContext, RootBindingPlan};
    use sha2::{Digest, Sha256};

    use super::{
        ExportFailureStage, ExportOptions, ExportPlan, ExportProgressStage, bind_execution_paths,
        execute, plan,
    };
    use crate::{
        imaging_processor::{
            ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
            InvocationFailure, InvocationFuture,
        },
        sample_project::SampleProject,
    };

    struct ScriptedTransport {
        prepared_path: PathBuf,
        prepared_bytes: Vec<u8>,
        result: Option<Result<ImagingResponse, InvocationFailure>>,
        invocations: usize,
    }

    struct CancellationAwareTransport {
        prepared_path: PathBuf,
        invocations: usize,
    }

    impl ImagingTransport for CancellationAwareTransport {
        fn invoke<'a>(
            &'a mut self,
            _command: &'a ImagingCommand,
            _context: &'a InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
            control: InvocationControl<'a>,
        ) -> InvocationFuture<'a> {
            assert_eq!(operation, ImagingOperation::Export);
            assert_eq!(attempt, 1);
            self.invocations += 1;
            std::fs::write(&self.prepared_path, b"incomplete export")
                .expect("the processor creates an incomplete preparation");
            Box::pin(async move {
                loop {
                    if control.is_cancelled() {
                        return Err(InvocationFailure::cancelled(4545));
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
        }
    }

    impl ImagingTransport for ScriptedTransport {
        fn invoke<'a>(
            &'a mut self,
            command: &'a ImagingCommand,
            _context: &'a InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
            control: InvocationControl<'a>,
        ) -> InvocationFuture<'a> {
            let ImagingCommand::Render(request) = command else {
                panic!("the scripted Export receives a Render command");
            };
            assert_eq!(operation, ImagingOperation::Export);
            assert_eq!(attempt, 1);
            self.invocations += 1;
            std::fs::write(&self.prepared_path, &self.prepared_bytes)
                .expect("the processor writes its preparation");
            for (stage, completed_units) in [
                (ImagingProgressStage::LoadingSources, 1),
                (ImagingProgressStage::Composing, 1),
                (ImagingProgressStage::EncodingOutput, 1),
            ] {
                control.report(
                    ImagingProgress::new(&request.request_id, stage, completed_units, 1)
                        .expect("scripted progress is valid"),
                );
            }
            let result = self.result.take().expect("one invocation");
            Box::pin(async move { result })
        }
    }

    fn export_plan(output: PathBuf, request_id: &str) -> ExportPlan {
        let source = SampleProject::Horizon
            .persisted_source(2)
            .expect("the sample project serializes");
        let snapshot = ProjectCore::open_editable_session(&source)
            .expect("the sample project opens")
            .render_snapshot();
        let sheet_id = snapshot.composition.sheets[0].sheet_id.clone();
        plan(
            snapshot,
            ExportOptions::new(request_id, output, sheet_id, 25, None),
        )
        .expect("the Export request is valid")
    }

    fn root_bindings(plan: &ExportPlan) -> RootBindingPlan {
        let mut context = OperationPathContext::new();
        for path in plan.required_paths() {
            context
                .capture(path)
                .expect("the Export path root is captured");
        }
        context.freeze()
    }

    #[test]
    fn host_execution_uses_the_same_frozen_mapped_drive_binding_as_the_processor() {
        let logical_plan = myalbuns_paths::ExportPathPlan::new(
            PathBuf::from(r"Z:\Exports\Album.png"),
            "export-mapped",
        )
        .expect("the logical Export path is valid");
        let mut context = OperationPathContext::new();
        context
            .capture_with_binding(
                logical_plan.output_path(),
                std::path::Path::new(r"\\servidor\destino\"),
            )
            .expect("the platform binding is captured");

        let operational = bind_execution_paths(&logical_plan, &context.freeze(), "export-mapped")
            .expect("the host derives its paths from the frozen binding");

        assert_eq!(
            operational.output_path(),
            std::path::Path::new(r"\\servidor\destino\Exports\Album.png")
        );
        assert_eq!(
            operational.prepared_output_path(),
            std::path::Path::new(
                r"\\servidor\destino\Exports\.myalbuns-export-export-mapped.tmp\Album.png"
            )
        );
    }

    #[test]
    fn planning_an_export_does_not_create_its_destination() {
        let root = tempfile::tempdir().expect("temporary Export root");
        let destination = root.path().join("not-created").join("nested");
        let output = destination.join("Album.png");

        let _plan = export_plan(output, "export-pure-plan");

        assert!(
            !destination.exists(),
            "planning must remain a pure operation"
        );
    }

    fn context(request_id: &str) -> InvocationContext {
        InvocationContext::new(request_id, Some("project-test"))
    }

    #[test]
    fn a_processor_crash_is_not_retried_and_preserves_the_previous_output() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan = export_plan(output.clone(), "export-failure");
            let mut transport = ScriptedTransport {
                prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
                prepared_bytes: b"incomplete".to_vec(),
                result: Some(Err(InvocationFailure::unexpected_termination(4242))),
                invocations: 0,
            };
            let bindings = root_bindings(&plan);
            let cancellation = AtomicBool::new(false);
            let progress = |_| {};

            let failure = execute(
                &mut transport,
                plan,
                &bindings,
                &cancellation,
                &progress,
                &context("export-failure"),
            )
            .await
            .expect_err("the Export failure remains visible");

            assert_eq!(
                failure.stage,
                ExportFailureStage::Processor(
                    crate::imaging_processor::InvocationFailureStage::ImagingProcess
                )
            );
            assert_eq!(transport.invocations, 1);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
            assert!(
                !destination
                    .path()
                    .join(".myalbuns-export-export-failure.tmp")
                    .exists()
            );
        });
    }

    #[test]
    fn a_verified_preparation_is_published_only_after_the_response_is_validated() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan = export_plan(output.clone(), "export-success");
            let bytes = b"verified export".to_vec();
            let completion = RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&bytes)),
            };
            let response = ImagingResponse::completed("export-success", completion.clone());
            let mut transport = ScriptedTransport {
                prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
                prepared_bytes: bytes,
                result: Some(Ok(response)),
                invocations: 0,
            };
            let bindings = root_bindings(&plan);
            let cancellation = AtomicBool::new(false);
            let stages = Mutex::new(Vec::new());
            let progress = |progress: super::ExportProgress| {
                stages
                    .lock()
                    .expect("the progress collector is available")
                    .push(progress.stage);
            };

            let published = execute(
                &mut transport,
                plan,
                &bindings,
                &cancellation,
                &progress,
                &context("export-success"),
            )
            .await
            .expect("the verified Export is published");

            assert_eq!(published.completion, completion);
            assert_eq!(published.output_path, output);
            assert_eq!(
                std::fs::read(&published.output_path).expect("the published Export is readable"),
                b"verified export"
            );
            assert_eq!(
                *stages.lock().expect("the progress collector is available"),
                [
                    ExportProgressStage::Preparing,
                    ExportProgressStage::LoadingSources,
                    ExportProgressStage::Composing,
                    ExportProgressStage::EncodingOutput,
                    ExportProgressStage::Verifying,
                    ExportProgressStage::Publishing,
                    ExportProgressStage::Completed,
                ]
            );
        });
    }

    #[test]
    fn an_unverified_preparation_never_replaces_the_previous_output() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan = export_plan(output.clone(), "export-invalid");
            let bytes = b"unverified export".to_vec();
            let response = ImagingResponse::completed(
                "export-invalid",
                RenderCompletion {
                    width_px: 10,
                    height_px: 5,
                    dpi: 25,
                    source_count: 0,
                    source_bytes: 0,
                    output_bytes: bytes.len() as u64,
                    output_sha256:
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                },
            );
            let mut transport = ScriptedTransport {
                prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
                prepared_bytes: bytes,
                result: Some(Ok(response)),
                invocations: 0,
            };
            let bindings = root_bindings(&plan);
            let cancellation = AtomicBool::new(false);
            let progress = |_| {};

            let failure = execute(
                &mut transport,
                plan,
                &bindings,
                &cancellation,
                &progress,
                &context("export-invalid"),
            )
            .await
            .expect_err("the mismatched preparation is rejected");

            assert_eq!(failure.stage, ExportFailureStage::VerifyPreparation);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
        });
    }

    #[test]
    fn cancellation_before_execution_creates_no_preparation_or_processor_work() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            let plan = export_plan(output, "export-cancelled");
            let preparation_path = plan.path_plan().preparation_directory().to_path_buf();
            let bindings = root_bindings(&plan);
            let cancellation = AtomicBool::new(true);
            let mut transport = ScriptedTransport {
                prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
                prepared_bytes: Vec::new(),
                result: None,
                invocations: 0,
            };
            let progress_count = Mutex::new(0_u32);
            let progress = |_| {
                *progress_count
                    .lock()
                    .expect("the progress counter is available") += 1;
            };

            let failure = execute(
                &mut transport,
                plan,
                &bindings,
                &cancellation,
                &progress,
                &context("export-cancelled"),
            )
            .await
            .expect_err("the cancelled Export does not start");

            assert_eq!(failure.stage, ExportFailureStage::Cancelled);
            assert_eq!(transport.invocations, 0);
            assert_eq!(
                *progress_count
                    .lock()
                    .expect("the progress counter is available"),
                0
            );
            assert!(!preparation_path.exists());
            assert!(cancellation.load(Ordering::Acquire));
        });
    }

    #[test]
    fn cancellation_during_processing_discards_preparation_and_preserves_previous_output() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan = export_plan(output.clone(), "export-cancelled-during-processing");
            let preparation_directory = plan.path_plan().preparation_directory().to_path_buf();
            let mut transport = CancellationAwareTransport {
                prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
                invocations: 0,
            };
            let bindings = root_bindings(&plan);
            let cancellation = Arc::new(AtomicBool::new(false));
            let cancellation_request = Arc::clone(&cancellation);
            let canceller = thread::spawn(move || {
                thread::sleep(Duration::from_millis(25));
                cancellation_request.store(true, Ordering::Release);
            });
            let progress = |_| {};

            let failure = execute(
                &mut transport,
                plan,
                &bindings,
                cancellation.as_ref(),
                &progress,
                &context("export-cancelled-during-processing"),
            )
            .await
            .expect_err("the in-flight Export is cancelled");
            canceller
                .join()
                .expect("the cancellation request completes");

            assert_eq!(failure.stage, ExportFailureStage::Cancelled);
            assert_eq!(transport.invocations, 1);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
            assert!(!preparation_directory.exists());
        });
    }
}
