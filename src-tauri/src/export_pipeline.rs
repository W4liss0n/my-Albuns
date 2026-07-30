use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use myalbuns_core::RenderSnapshot;
use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingRequest, MediaSource, RenderCompletion,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{ExportPathPlan, PreparedExportStorage};
use sha2::{Digest, Sha256};

use crate::imaging_processor::{
    ImagingOperation, ImagingTransport, InvocationContext, InvocationFailureStage, OperationFailure,
};

#[derive(Debug)]
pub(crate) struct ExportPlan {
    output_path: PathBuf,
    path_plan: ExportPathPlan,
    request: ImagingRequest,
}

#[derive(Debug)]
pub(crate) struct PublishedExport {
    pub(crate) output_path: PathBuf,
    pub(crate) completion: RenderCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportFailureStage {
    Plan,
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
            Self::Prepare => "prepare_output",
            Self::Processor(stage) => stage.as_str(),
            Self::ValidateResponse => "validate_response",
            Self::VerifyPreparation => "verify_preparation",
            Self::Publish => "publish_output",
        }
    }
}

pub(crate) type ExportFailure = OperationFailure<ExportFailureStage>;

pub(crate) fn plan(
    request_id: impl Into<String>,
    output_path: PathBuf,
    snapshot: RenderSnapshot,
    sheet_id: impl Into<String>,
    dpi: u32,
    sources: Option<Vec<MediaSource>>,
) -> Result<ExportPlan, ExportFailure> {
    let request_id = request_id.into();
    let sheet_id = sheet_id.into();
    let destination = output_path.parent().ok_or_else(|| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            "O Destino da Exportação é inválido.",
        )
    })?;
    std::fs::create_dir_all(destination).map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Prepare,
            format!("Não foi possível preparar a Exportação: {error}"),
        )
    })?;
    let path_plan = ExportPathPlan::new(output_path.clone(), &request_id).map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar o Destino da Exportação: {error}"),
        )
    })?;
    let request = match sources {
        Some(sources) => ImagingRequest::new(
            request_id,
            path_plan.prepared_output_path().to_path_buf(),
            snapshot,
            sheet_id,
            dpi,
            sources,
        ),
        None => ImagingRequest::procedural_fixture(
            request_id,
            path_plan.prepared_output_path().to_path_buf(),
            snapshot,
            sheet_id,
            dpi,
        ),
    }
    .map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Plan,
            format!("Não foi possível planejar a Exportação: {error}"),
        )
    })?;
    Ok(ExportPlan {
        output_path,
        path_plan,
        request,
    })
}

pub(crate) async fn execute<T: ImagingTransport>(
    transport: &mut T,
    plan: ExportPlan,
    context: &InvocationContext,
) -> Result<PublishedExport, ExportFailure> {
    if plan.request.prepared_output_path != plan.path_plan.prepared_output_path() {
        return Err(ExportFailure::new(
            ExportFailureStage::Prepare,
            "A preparação da Exportação não corresponde ao plano de caminhos.",
        ));
    }
    let preparation = plan.path_plan.prepare().map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Prepare,
            format!("Não foi possível preparar a Exportação: {error}"),
        )
    })?;
    let command = ImagingCommand::render(plan.request.clone());
    let response = match transport
        .invoke(&command, context, ImagingOperation::Export, 1)
        .await
    {
        Ok(response) => response,
        Err(failure) => {
            discard_failed_preparation(preparation, context);
            return Err(ExportFailure::from_invocation(
                failure,
                ExportFailureStage::Processor,
            ));
        }
    };
    let Some(completion) = response.completed_for(&plan.request.request_id).cloned() else {
        discard_failed_preparation(preparation, context);
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O Processador de Imagens devolveu uma resposta inesperada.",
        ));
    };
    if let Err(message) = verify_preparation(&plan.path_plan, &completion) {
        discard_failed_preparation(preparation, context);
        return Err(ExportFailure::new(
            ExportFailureStage::VerifyPreparation,
            message,
        ));
    }
    preparation.publish().map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Publish,
            format!("Não foi possível publicar a Exportação: {error}"),
        )
    })?;
    Ok(PublishedExport {
        output_path: plan.output_path,
        completion,
    })
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
    use std::path::PathBuf;

    use myalbuns_core::ProjectCore;
    use myalbuns_imaging_protocol::{ImagingCommand, ImagingResponse, RenderCompletion};
    use sha2::{Digest, Sha256};

    use super::{ExportFailureStage, ExportPlan, execute, plan};
    use crate::{
        imaging_processor::{
            ImagingOperation, ImagingTransport, InvocationContext, InvocationFailure,
            InvocationFuture,
        },
        sample_project::SampleProject,
    };

    struct ScriptedTransport {
        prepared_path: PathBuf,
        prepared_bytes: Vec<u8>,
        result: Option<Result<ImagingResponse, InvocationFailure>>,
        invocations: usize,
    }

    impl ImagingTransport for ScriptedTransport {
        fn invoke<'a>(
            &'a mut self,
            command: &'a ImagingCommand,
            _context: &'a InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
        ) -> InvocationFuture<'a> {
            assert!(matches!(command, ImagingCommand::Render(_)));
            assert_eq!(operation, ImagingOperation::Export);
            assert_eq!(attempt, 1);
            self.invocations += 1;
            std::fs::write(&self.prepared_path, &self.prepared_bytes)
                .expect("the processor writes its preparation");
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
        plan(request_id, output, snapshot, sheet_id, 25, None).expect("the Export request is valid")
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
                prepared_path: plan.path_plan.prepared_output_path().to_path_buf(),
                prepared_bytes: b"incomplete".to_vec(),
                result: Some(Err(InvocationFailure::unexpected_termination(4242))),
                invocations: 0,
            };

            let failure = execute(&mut transport, plan, &context("export-failure"))
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
                prepared_path: plan.path_plan.prepared_output_path().to_path_buf(),
                prepared_bytes: bytes,
                result: Some(Ok(response)),
                invocations: 0,
            };

            let published = execute(&mut transport, plan, &context("export-success"))
                .await
                .expect("the verified Export is published");

            assert_eq!(published.completion, completion);
            assert_eq!(published.output_path, output);
            assert_eq!(
                std::fs::read(&published.output_path).expect("the published Export is readable"),
                b"verified export"
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
                prepared_path: plan.path_plan.prepared_output_path().to_path_buf(),
                prepared_bytes: bytes,
                result: Some(Ok(response)),
                invocations: 0,
            };

            let failure = execute(&mut transport, plan, &context("export-invalid"))
                .await
                .expect_err("the mismatched preparation is rejected");

            assert_eq!(failure.stage, ExportFailureStage::VerifyPreparation);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
        });
    }
}
