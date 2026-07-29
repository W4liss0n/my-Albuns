use std::{
    fs::File,
    future::Future,
    io::{BufReader, Read},
};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingRequest, ImagingResponse, RenderCompletion,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{ExportPathPlan, PreparedExportStorage};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{
    imaging_processor::{
        ImagingOperation, InvocationContext, InvocationFailure, InvocationFailureStage, invoke_once,
    },
    logging::LoggingState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportFailureStage {
    Prepare,
    Serialize,
    Processor(InvocationFailureStage),
    DecodeResponse,
    ValidateResponse,
    VerifyPreparation,
    Publish,
}

impl ExportFailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => "prepare_output",
            Self::Serialize => "serialize_snapshot",
            Self::Processor(stage) => stage.as_str(),
            Self::DecodeResponse => "decode_response",
            Self::ValidateResponse => "validate_response",
            Self::VerifyPreparation => "verify_preparation",
            Self::Publish => "publish_output",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExportFailure {
    pub(crate) stage: ExportFailureStage,
    pub(crate) exit_code: Option<i32>,
    pub(crate) message: String,
}

impl ExportFailure {
    fn new(stage: ExportFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            exit_code: None,
            message: message.into(),
        }
    }

    fn processor(failure: InvocationFailure) -> Self {
        Self {
            stage: ExportFailureStage::Processor(failure.stage),
            exit_code: failure.exit_code,
            message: failure.message,
        }
    }
}

pub(crate) async fn execute(
    app: &AppHandle,
    logging: &LoggingState,
    path_plan: &ExportPathPlan,
    request: &ImagingRequest,
    context: InvocationContext<'_>,
) -> Result<RenderCompletion, ExportFailure> {
    execute_with(
        path_plan,
        request,
        |payload| async move {
            invoke_once(app, logging, &payload, context, ImagingOperation::Export, 1).await
        },
        context,
    )
    .await
}

async fn execute_with<Invoke, Invocation>(
    path_plan: &ExportPathPlan,
    request: &ImagingRequest,
    invoke: Invoke,
    context: InvocationContext<'_>,
) -> Result<RenderCompletion, ExportFailure>
where
    Invoke: FnOnce(Vec<u8>) -> Invocation,
    Invocation: Future<Output = Result<Vec<u8>, InvocationFailure>>,
{
    if request.prepared_output_path != path_plan.prepared_output_path() {
        return Err(ExportFailure::new(
            ExportFailureStage::Prepare,
            "A preparação da Exportação não corresponde ao plano de caminhos.",
        ));
    }
    let preparation = path_plan.prepare().map_err(|error| {
        ExportFailure::new(
            ExportFailureStage::Prepare,
            format!("Não foi possível preparar a Exportação: {error}"),
        )
    })?;
    let mut payload = match serde_json::to_vec(request) {
        Ok(payload) => payload,
        Err(error) => {
            discard_failed_preparation(preparation, context);
            return Err(ExportFailure::new(
                ExportFailureStage::Serialize,
                format!("Não foi possível preparar o snapshot: {error}"),
            ));
        }
    };
    payload.push(b'\n');

    let stdout = match invoke(payload).await {
        Ok(stdout) => stdout,
        Err(failure) => {
            discard_failed_preparation(preparation, context);
            return Err(ExportFailure::processor(failure));
        }
    };
    let response: ImagingResponse = match serde_json::from_slice(&stdout) {
        Ok(response) => response,
        Err(error) => {
            discard_failed_preparation(preparation, context);
            return Err(ExportFailure::new(
                ExportFailureStage::DecodeResponse,
                format!("Resposta inválida do Processador de Imagens: {error}"),
            ));
        }
    };
    let Some(completion) = response.completed_for(&request.request_id).cloned() else {
        discard_failed_preparation(preparation, context);
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O Processador de Imagens devolveu uma resposta inesperada.",
        ));
    };
    if let Err(message) = verify_preparation(path_plan, &completion) {
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
    Ok(completion)
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

fn discard_failed_preparation(preparation: PreparedExportStorage, context: InvocationContext<'_>) {
    match preparation.discard() {
        Ok(removed) => tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = context.operation_id,
            project_id = context.project_id,
            removed,
            event = "incomplete_export_discarded",
        ),
        Err(_) => tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = context.operation_id,
            project_id = context.project_id,
            event = "incomplete_export_cleanup_failed",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, future::ready};

    use myalbuns_core::ProjectCore;
    use myalbuns_imaging_protocol::{ImagingRequest, ImagingResponse, RenderCompletion};
    use myalbuns_paths::ExportPathPlan;
    use sha2::{Digest, Sha256};

    use super::{ExportFailureStage, execute_with};
    use crate::{
        imaging_processor::{InvocationContext, InvocationFailure},
        sample_project::SampleProject,
    };

    const CONTEXT: InvocationContext<'static> = InvocationContext {
        operation_id: "export-test",
        project_id: Some("project-test"),
    };

    fn request(path_plan: &ExportPathPlan, request_id: &str) -> ImagingRequest {
        let source = SampleProject::Horizon
            .persisted_source(2)
            .expect("the sample project serializes");
        let snapshot = ProjectCore::open_editable_session(&source)
            .expect("the sample project opens")
            .render_snapshot();
        let sheet_id = snapshot.composition.sheets[0].sheet_id.clone();
        ImagingRequest::procedural_fixture(
            request_id,
            path_plan.prepared_output_path().to_path_buf(),
            snapshot,
            sheet_id,
            25,
        )
        .expect("the Export request is valid")
    }

    #[test]
    fn a_processor_crash_is_not_retried_and_preserves_the_previous_output() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan =
                ExportPathPlan::new(output.clone(), "export-failure").expect("valid path plan");
            let request = request(&plan, "export-failure");
            let invocations = Cell::new(0);
            let prepared = plan.prepared_output_path().to_path_buf();

            let failure = execute_with(
                &plan,
                &request,
                |_| {
                    invocations.set(invocations.get() + 1);
                    std::fs::write(&prepared, b"incomplete")
                        .expect("the failed processor writes a partial preparation");
                    ready(Err(InvocationFailure::unexpected_termination(4242)))
                },
                CONTEXT,
            )
            .await
            .expect_err("the Export failure remains visible");

            assert_eq!(
                failure.stage,
                ExportFailureStage::Processor(
                    crate::imaging_processor::InvocationFailureStage::ImagingProcess
                )
            );
            assert_eq!(invocations.get(), 1);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
            assert!(!plan.preparation_directory().exists());
        });
    }

    #[test]
    fn a_verified_preparation_is_published_only_after_the_response_is_validated() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan =
                ExportPathPlan::new(output.clone(), "export-success").expect("valid path plan");
            let request = request(&plan, "export-success");
            let prepared = plan.prepared_output_path().to_path_buf();
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

            let completed = execute_with(
                &plan,
                &request,
                |_| {
                    std::fs::write(&prepared, &bytes)
                        .expect("the processor writes its verified preparation");
                    ready(Ok(
                        serde_json::to_vec(&response).expect("the response serializes")
                    ))
                },
                CONTEXT,
            )
            .await
            .expect("the verified Export is published");

            assert_eq!(completed, completion);
            assert_eq!(
                std::fs::read(output).expect("the published Export is readable"),
                b"verified export"
            );
            assert!(!plan.preparation_directory().exists());
        });
    }

    #[test]
    fn an_unverified_preparation_never_replaces_the_previous_output() {
        tauri::async_runtime::block_on(async {
            let destination = tempfile::tempdir().expect("temporary Export destination");
            let output = destination.path().join("Album.png");
            std::fs::write(&output, b"previous export").expect("the previous Export is writable");
            let plan =
                ExportPathPlan::new(output.clone(), "export-invalid").expect("valid path plan");
            let request = request(&plan, "export-invalid");
            let prepared = plan.prepared_output_path().to_path_buf();
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

            let failure = execute_with(
                &plan,
                &request,
                |_| {
                    std::fs::write(&prepared, &bytes)
                        .expect("the processor writes its invalid preparation");
                    ready(Ok(
                        serde_json::to_vec(&response).expect("the response serializes")
                    ))
                },
                CONTEXT,
            )
            .await
            .expect_err("the mismatched preparation is rejected");

            assert_eq!(failure.stage, ExportFailureStage::VerifyPreparation);
            assert_eq!(
                std::fs::read(output).expect("the previous Export remains readable"),
                b"previous export"
            );
            assert!(!plan.preparation_directory().exists());
        });
    }
}
