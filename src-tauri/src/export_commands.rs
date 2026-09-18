use std::{path::PathBuf, time::Instant};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingFailureCode, ImagingPathCode, root_binding_plan_sha256,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::ExportWriteAuthorization;
use tauri::{AppHandle, Manager, State, WebviewWindow, ipc::Channel};

use crate::{
    cache_engine::CacheEngine,
    export_attempts::{ExportAttempt, ExportAttempts},
    export_pipeline,
    imaging_processor::{ImagingProcessor, InvocationContext, TauriImagingTransport},
    ipc_contract::{
        CancelDisposition, ExportCommandError, ExportCommandErrorCode, ExportEvent, ExportPathCode,
        ExportProgressStagePayload, ExportProgressUnitsPayload, ExportResult,
    },
    logging::{LoggingState, log_imaging_failure},
    operation_gate::{OperationGate, OperationGateError},
    operation_lease::{OperationLease, OperationLeaseAcquisition},
    path_io,
    project_host::ProjectHost,
};

pub(crate) mod normal;

#[derive(Debug)]
struct PreparedExportCommand {
    acquisition: OperationLeaseAcquisition,
    attempt: ExportAttempt,
    operation_paths: Vec<PathBuf>,
    plan: ExportCommandPlan,
    project_id: Option<String>,
    request_id: String,
}

#[derive(Debug)]
enum ExportCommandPlan {
    Album(Box<export_pipeline::AlbumExportPlan>),
    Resume(Box<export_pipeline::AlbumExportRecovery>),
}

impl ExportEvent {
    fn started(operation_id: impl Into<String>) -> Self {
        Self::Started {
            operation_id: operation_id.into(),
            cancellable: true,
        }
    }

    fn from_progress(
        operation_id: impl Into<String>,
        progress: export_pipeline::ExportProgress,
    ) -> Self {
        let units = match progress.units {
            export_pipeline::ExportProgressUnits::Unmeasured => {
                ExportProgressUnitsPayload::Unmeasured
            }
            export_pipeline::ExportProgressUnits::Measured {
                completed_units,
                total_units,
            } => ExportProgressUnitsPayload::Measured {
                completed_units,
                total_units,
            },
        };
        Self::Progress {
            operation_id: operation_id.into(),
            stage: progress.stage.into(),
            units,
            overall_percent: progress.overall_percent(),
            cancellable: progress.cancellable,
        }
    }
}

impl From<export_pipeline::ExportProgressStage> for ExportProgressStagePayload {
    fn from(stage: export_pipeline::ExportProgressStage) -> Self {
        match stage {
            export_pipeline::ExportProgressStage::Preparing => Self::Preparing,
            export_pipeline::ExportProgressStage::LoadingSources => Self::LoadingSources,
            export_pipeline::ExportProgressStage::Composing => Self::Composing,
            export_pipeline::ExportProgressStage::EncodingOutput => Self::EncodingOutput,
            export_pipeline::ExportProgressStage::Verifying => Self::Verifying,
            export_pipeline::ExportProgressStage::Publishing => Self::Publishing,
            export_pipeline::ExportProgressStage::Completed => Self::Completed,
        }
    }
}

impl ExportCommandError {
    fn cancelled() -> Self {
        Self {
            code: ExportCommandErrorCode::Cancelled,
            message: "A exportação foi cancelada.".into(),
            media_id: None,
            path_code: None,
            media_problems: None,
            layout_problems: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            code: ExportCommandErrorCode::Failed,
            message: message.into(),
            media_id: None,
            path_code: None,
            media_problems: None,
            layout_problems: None,
        }
    }

    fn from_gate(error: OperationGateError) -> Self {
        match error {
            OperationGateError::Conflict => Self {
                code: ExportCommandErrorCode::Conflict,
                message: "Outra operação exclusiva já está em andamento. Aguarde sua conclusão e tente novamente.".into(),
                media_id: None,
                path_code: None,
                media_problems: None,
                layout_problems: None,
            },
            OperationGateError::Unavailable { reason } => {
                tracing::warn!(target: "myalbuns.desktop", %reason, event = "export_reservation_failed");
                Self {
                code: ExportCommandErrorCode::Failed,
                message: "Não foi possível iniciar a exportação.".into(),
                media_id: None,
                path_code: None,
                media_problems: None,
                layout_problems: None,
                }
            },
        }
    }

    fn from_pipeline(failure: export_pipeline::ExportFailure) -> Self {
        if failure.is_storage_full() {
            return Self {
                code: ExportCommandErrorCode::OutputStorageFull,
                message: failure.message,
                media_id: None,
                path_code: None,
                media_problems: None,
                layout_problems: None,
            };
        }
        if let Some(processor) = failure.processor_failure {
            return Self {
                code: processor.code.into(),
                message: failure.message,
                media_id: processor.media_id,
                path_code: processor.path_code.map(Into::into),
                media_problems: None,
                layout_problems: None,
            };
        }
        let mut result = match failure.stage {
            export_pipeline::ExportFailureStage::Cancelled => Self::cancelled(),
            export_pipeline::ExportFailureStage::Publish { .. } => Self {
                code: ExportCommandErrorCode::PublicationFailed,
                message: failure.message,
                media_id: None,
                path_code: None,
                media_problems: None,
                layout_problems: None,
            },
            _ => Self::failed(failure.message),
        };
        if failure.path_failure == Some(myalbuns_paths::AppPathsError::OperationPathAccessDenied) {
            result.path_code = Some(ExportPathCode::AccessDenied);
            let guidance = "Sem permissão para gravar no destino. Escolha outra pasta ou ajuste as permissões e tente novamente.";
            result.message = if result.code == ExportCommandErrorCode::PublicationFailed {
                format!("{} {guidance}", result.message)
            } else {
                guidance.into()
            };
        }
        result
    }
}

impl From<ImagingFailureCode> for ExportCommandErrorCode {
    fn from(code: ImagingFailureCode) -> Self {
        match code {
            ImagingFailureCode::InvalidRenderRequest => Self::InvalidRenderRequest,
            ImagingFailureCode::SourceUnavailable => Self::SourceUnavailable,
            ImagingFailureCode::UnsupportedSourceFormat => Self::UnsupportedSourceFormat,
            ImagingFailureCode::UnsupportedSourceVariant => Self::UnsupportedSourceVariant,
            ImagingFailureCode::UnsupportedColorModel => Self::UnsupportedColorModel,
            ImagingFailureCode::UnsupportedColorProfile => Self::UnsupportedColorProfile,
            ImagingFailureCode::DecodeFailed => Self::DecodeFailed,
            ImagingFailureCode::CompositionFailed => Self::CompositionFailed,
            ImagingFailureCode::ResourceLimitExceeded => Self::ResourceLimitExceeded,
            ImagingFailureCode::EncodeFailed => Self::EncodeFailed,
            ImagingFailureCode::OutputStorageFull => Self::OutputStorageFull,
            ImagingFailureCode::VerificationFailed => Self::VerificationFailed,
        }
    }
}

impl From<ImagingPathCode> for ExportPathCode {
    fn from(code: ImagingPathCode) -> Self {
        match code {
            ImagingPathCode::NotFound => Self::NotFound,
            ImagingPathCode::Unavailable => Self::Unavailable,
            ImagingPathCode::AccessDenied => Self::AccessDenied,
            ImagingPathCode::InvalidPath => Self::InvalidPath,
            ImagingPathCode::UnexpectedObjectType => Self::UnexpectedObjectType,
            ImagingPathCode::Conflict => Self::Conflict,
            ImagingPathCode::IoFailure => Self::IoFailure,
        }
    }
}

fn log_export_cancelled(
    operation_id: &str,
    project_id: Option<&str>,
    window_label: &str,
    stage: &str,
) {
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id,
        project_id,
        window_label,
        stage,
        event = "export_cancelled",
    );
}

// Tauri injects these independently owned services at the command boundary.
// Grouping them only to shorten this signature would create a false coordinator.
async fn run_export(
    app: AppHandle,
    window: WebviewWindow,
    on_event: Channel<ExportEvent>,
    logging: State<'_, LoggingState>,
    cache: State<'_, CacheEngine>,
    processor: State<'_, ImagingProcessor>,
    prepared: PreparedExportCommand,
) -> Result<ExportResult, ExportCommandError> {
    let PreparedExportCommand {
        acquisition,
        attempt,
        operation_paths,
        plan,
        project_id,
        request_id,
    } = prepared;
    let storage_recoveries = app.state::<crate::storage_recovery::StorageRecoveries>();
    storage_recoveries.finish("export");
    let output_path = operation_paths.first().cloned();
    if on_event
        .send(ExportEvent::started(request_id.clone()))
        .is_err()
    {
        attempt.request_cancel();
        log_export_cancelled(
            &request_id,
            project_id.as_deref(),
            window.label(),
            "progress_channel",
        );
        return Err(ExportCommandError::cancelled());
    }
    let started = Instant::now();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = project_id.as_deref(),
        window_label = window.label(),
        event = "export_started",
    );
    let retained_roots = match &plan {
        ExportCommandPlan::Resume(recovery) => Some(recovery.roots().clone()),
        _ => None,
    };
    let root_bindings_completion = async move {
        if let Some(roots) = retained_roots {
            Ok(roots)
        } else {
            path_io::capture_root_bindings(operation_paths).await
        }
    };
    tokio::pin!(root_bindings_completion);
    let root_bindings = tokio::select! {
        bindings = &mut root_bindings_completion => bindings.map_err(|error| {
            log_imaging_failure(
                "export_failed",
                &request_id,
                project_id.as_deref(),
                "capture_root_bindings",
                None,
            );
            ExportCommandError::failed(format!(
                "Não foi possível acessar um caminho necessário à exportação (destino ou arquivo original). Verifique o destino; se a mídia estiver ausente, use Localizar imagem… no painel de imagens e tente novamente. Detalhes: {error}"
            ))
        })?,
        () = attempt.cancelled() => {
            let _ = root_bindings_completion.as_mut().await;
            log_export_cancelled(
                &request_id,
                project_id.as_deref(),
                window.label(),
                "capture_root_bindings",
            );
            return Err(ExportCommandError::cancelled());
        },
    };
    let root_binding_plan_sha256 =
        root_binding_plan_sha256(&root_bindings).map_err(ExportCommandError::failed)?;
    let storage_volume = output_path
        .as_ref()
        .and_then(|path| root_bindings.resolve(path).ok())
        .and_then(|path| myalbuns_paths::StorageVolume::containing(&path));
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        operation_id = request_id.as_str(),
        project_id = project_id.as_deref(),
        window_label = window.label(),
        root_binding_plan_sha256,
        event = "root_binding_plan_captured",
    );
    let context = InvocationContext::new(request_id.clone(), project_id.clone());
    let lease_completion = acquisition.complete(&cache, &processor);
    tokio::pin!(lease_completion);
    let lease = tokio::select! {
        lease = &mut lease_completion => lease.map_err(|error| {
            log_imaging_failure(
                "export_failed",
                &request_id,
                project_id.as_deref(),
                "operation_lease",
                None,
            );
            ExportCommandError::failed(error.to_string())
        })?,
        () = attempt.cancelled() => {
            log_export_cancelled(
                &request_id,
                project_id.as_deref(),
                window.label(),
                "operation_lease",
            );
            return Err(ExportCommandError::cancelled());
        },
    };
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = request_id.as_str(),
        project_id = project_id.as_deref(),
        event = "operation_lease_acquired",
    );
    let mut transport = TauriImagingTransport::new(&app, &logging, lease.processor_reservation());
    let progress = |progress: export_pipeline::ExportProgress| {
        let (completed_units, total_units) = match progress.units {
            export_pipeline::ExportProgressUnits::Unmeasured => (None, None),
            export_pipeline::ExportProgressUnits::Measured {
                completed_units,
                total_units,
            } => (Some(completed_units), Some(total_units)),
        };
        tracing::debug!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = request_id.as_str(),
            project_id = project_id.as_deref(),
            stage = ?progress.stage,
            completed_units,
            total_units,
            cancellable = progress.cancellable,
            event = "export_progress",
        );
        if on_event
            .send(ExportEvent::from_progress(request_id.clone(), progress))
            .is_err()
        {
            tracing::debug!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                protocol_version = IMAGING_PROTOCOL_VERSION,
                operation_id = request_id.as_str(),
                project_id = project_id.as_deref(),
                event = "export_progress_observer_unavailable",
            );
        }
    };
    let published = match plan {
        ExportCommandPlan::Resume(recovery) => {
            export_pipeline::resume_album(
                &mut transport,
                recovery,
                attempt.execution_control(),
                &progress,
                &context,
            )
            .await
        }
        ExportCommandPlan::Album(plan) => {
            export_pipeline::execute_album(
                &mut transport,
                *plan,
                &root_bindings,
                attempt.execution_control(),
                &progress,
                &context,
            )
            .await
        }
    }
    .map_err(|mut failure| {
        if failure.is_storage_full() {
            storage_recoveries.retain_export(storage_volume, failure.recovery.take());
        }
        if failure.stage == export_pipeline::ExportFailureStage::Cancelled {
            log_export_cancelled(
                &request_id,
                project_id.as_deref(),
                window.label(),
                failure.stage.as_str(),
            );
        } else {
            log_imaging_failure(
                "export_failed",
                &request_id,
                project_id.as_deref(),
                failure.stage.as_str(),
                failure.exit_code,
            );
        }
        ExportCommandError::from_pipeline(failure)
    })?;
    let completed = published.completion;
    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        process_id = std::process::id(),
        project_id = project_id.as_deref(),
        window_label = window.label(),
        width_px = completed.width_px,
        height_px = completed.height_px,
        dpi = completed.dpi,
        source_count = completed.source_count,
        source_bytes = completed.source_bytes,
        output_bytes = completed.output_bytes,
        output_sha256 = completed.output_sha256.as_str(),
        elapsed_ms,
        event = "export_completed",
    );

    Ok(ExportResult {
        width_px: completed.width_px,
        height_px: completed.height_px,
    })
}

#[tauri::command]
pub(crate) fn cancel_export(
    window: WebviewWindow,
    operation_id: String,
    attempts: State<'_, ExportAttempts>,
) -> CancelDisposition {
    let disposition = attempts.request_cancel(&operation_id, window.label());
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = operation_id.as_str(),
        window_label = window.label(),
        disposition = ?disposition,
        event = "export_cancel_requested",
    );
    disposition
}

pub(crate) fn export_name(project_name: &str) -> String {
    let sanitized = project_name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches(|character| matches!(character, ' ' | '.'));
    let project_name = if sanitized.is_empty() {
        "Projeto"
    } else {
        sanitized
    };
    project_name.to_owned()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use myalbuns_imaging_protocol::{
        ImagingFailure, ImagingFailureCode, ImagingFailureStage, ImagingPathCode,
    };
    use serde_json::json;
    use tauri::ipc::{Channel, InvokeResponseBody};

    use crate::{
        export_pipeline::{
            ExportFailure, ExportFailureStage, ExportProgress, ExportProgressStage,
            ExportProgressUnits,
        },
        operation_gate::OperationGateError,
    };

    use crate::ipc_contract::{ExportCommandError, ExportCommandErrorCode, ExportEvent};

    use super::export_name;

    #[test]
    fn export_name_sanitizes_the_project_name() {
        assert_eq!(export_name("Casamento da Júlia"), "Casamento da Júlia");
        assert_eq!(export_name("Álbum: Horizonte"), "Álbum_ Horizonte");
        assert_eq!(export_name("..."), "Projeto");
    }

    #[test]
    fn export_channel_has_one_closed_camel_case_contract() {
        let messages = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let received = Arc::clone(&messages);
        let channel = Channel::new(move |body| {
            let InvokeResponseBody::Json(value) = body else {
                panic!("Export events use JSON");
            };
            received
                .lock()
                .expect("the event collector remains available")
                .push(serde_json::from_str(&value)?);
            Ok(())
        });

        channel
            .send(ExportEvent::started("export-42"))
            .expect("the Started event is sent");
        channel
            .send(ExportEvent::from_progress(
                "export-42",
                ExportProgress {
                    stage: ExportProgressStage::Composing,
                    units: ExportProgressUnits::Measured {
                        completed_units: 2,
                        total_units: 5,
                    },
                    cancellable: true,
                },
            ))
            .expect("the Progress event is sent");

        assert_eq!(
            *messages
                .lock()
                .expect("the event collector remains available"),
            [
                json!({
                    "event": "started",
                    "data": {
                        "operationId": "export-42",
                        "cancellable": true,
                    },
                }),
                json!({
                    "event": "progress",
                    "data": {
                        "operationId": "export-42",
                        "stage": "composing",
                        "overallPercent": 36.0,
                        "units": {
                            "kind": "measured",
                            "completedUnits": 2,
                            "totalUnits": 5,
                        },
                        "cancellable": true,
                    },
                }),
            ]
        );
    }

    #[test]
    fn cancelled_export_is_a_typed_terminal_result() {
        assert_eq!(
            serde_json::to_value(ExportCommandError::cancelled())
                .expect("the command error serializes"),
            json!({
                "code": "cancelled",
                "message": "A exportação foi cancelada.",
            })
        );
    }

    #[test]
    fn gate_conflict_keeps_its_typed_ipc_result() {
        assert_eq!(
            serde_json::to_value(ExportCommandError::from_gate(OperationGateError::Conflict))
                .expect("the command error serializes"),
            json!({
                "code": "conflict",
                "message": "Outra operação exclusiva já está em andamento. Aguarde sua conclusão e tente novamente.",
            })
        );
    }

    #[test]
    fn publication_failure_keeps_its_typed_ipc_result() {
        let publication = ExportCommandError::from_pipeline(ExportFailure::new(
            ExportFailureStage::Publish {
                promoted_outputs: 0,
                total_outputs: 1,
            },
            "A Publicação não pôde ser confirmada.",
        ));
        assert_eq!(
            serde_json::to_value(publication).expect("the publication failure serializes"),
            json!({
                "code": "publication_failed",
                "message": "A Publicação não pôde ser confirmada.",
            })
        );
    }

    #[test]
    fn processor_failure_keeps_actionable_media_and_path_context_over_ipc() {
        let failure = ExportCommandError::from_pipeline(ExportFailure {
            stage: ExportFailureStage::Processor(
                crate::imaging_processor::InvocationFailureStage::Processor(
                    ImagingFailureStage::SourceVerification,
                ),
            ),
            exit_code: None,
            message: "O original não está mais disponível.".into(),
            path_failure: None,
            recovery: None,
            processor_failure: Some(ImagingFailure {
                code: ImagingFailureCode::SourceUnavailable,
                media_id: Some("media-cover".into()),
                path_code: Some(ImagingPathCode::NotFound),
            }),
        });

        assert_eq!(
            serde_json::to_value(failure).expect("the Processador failure serializes"),
            json!({
                "code": "source_unavailable",
                "message": "O original não está mais disponível.",
                "mediaId": "media-cover",
                "pathCode": "not_found",
            })
        );
    }

    #[test]
    fn native_publication_disk_full_keeps_the_shared_export_error_code() {
        let failure = ExportCommandError::from_pipeline(ExportFailure::from_path_error(
            ExportFailureStage::Publish {
                promoted_outputs: 1,
                total_outputs: 2,
            },
            myalbuns_paths::AppPathsError::ExportStorageFull,
            myalbuns_paths::AppPathsError::EXPORT_STORAGE_FULL_MESSAGE,
        ));
        assert_eq!(failure.code, ExportCommandErrorCode::OutputStorageFull);
    }

    #[test]
    fn native_destination_permission_failure_keeps_actionable_context_over_ipc() {
        let guidance = "Sem permissão para gravar no destino. Escolha outra pasta ou ajuste as permissões e tente novamente.";
        for (stage, expected_code, context, expected_message) in [
            (
                ExportFailureStage::Prepare,
                "failed",
                "A preparação está indisponível.",
                guidance.into(),
            ),
            (
                ExportFailureStage::Publish {
                    promoted_outputs: 0,
                    total_outputs: 1,
                },
                "publication_failed",
                "Os arquivos já existentes foram mantidos.",
                format!("Os arquivos já existentes foram mantidos. {guidance}"),
            ),
            (
                ExportFailureStage::Publish {
                    promoted_outputs: 1,
                    total_outputs: 2,
                },
                "publication_failed",
                "O álbum foi publicado parcialmente. Tente exportar novamente para concluir.",
                format!(
                    "O álbum foi publicado parcialmente. Tente exportar novamente para concluir. {guidance}"
                ),
            ),
        ] {
            let error = ExportCommandError::from_pipeline(ExportFailure::from_path_error(
                stage,
                myalbuns_paths::AppPathsError::export_io(
                    &std::io::ErrorKind::PermissionDenied.into(),
                ),
                context,
            ));
            assert_eq!(
                serde_json::to_value(error).unwrap(),
                json!({
                    "code": expected_code,
                    "pathCode": "access_denied",
                    "message": expected_message,
                })
            );
        }
    }

    #[test]
    fn backend_cancelability_is_forwarded_without_adapter_decisions() {
        assert_eq!(
            serde_json::to_value(ExportEvent::from_progress(
                "export-publishing",
                ExportProgress {
                    stage: ExportProgressStage::Publishing,
                    units: ExportProgressUnits::Unmeasured,
                    cancellable: false,
                },
            ))
            .expect("the event serializes"),
            json!({
                "event": "progress",
                "data": {
                    "operationId": "export-publishing",
                    "stage": "publishing",
                    "overallPercent": 85.0,
                    "units": {
                        "kind": "unmeasured",
                    },
                    "cancellable": false,
                },
            })
        );
    }
}
