use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use myalbuns_imaging_protocol::{
    IMAGING_PROTOCOL_VERSION, ImagingFailureCode, ImagingPathCode, root_binding_plan_sha256,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use tauri::{AppHandle, State, WebviewWindow, ipc::Channel};

use crate::{
    cache_activity_gate::CacheActivityGate,
    export_attempts::ExportAttempts,
    export_pipeline,
    imaging_processor::{ImagingProcessor, InvocationContext, TauriImagingTransport},
    ipc_contract::{
        CancelDisposition, ExportCommandError, ExportCommandErrorCode, ExportEvent, ExportPathCode,
        ExportProgressStagePayload, ExportProgressUnitsPayload, ExportResult,
    },
    logging::{LoggingState, log_imaging_failure},
    native_project_dialog,
    operation_gate::{OperationGate, OperationGateError},
    operation_lease::OperationLease,
    path_io,
    project_host::ProjectHost,
};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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
            message: "A Exportação foi cancelada.".into(),
            media_id: None,
            path_code: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            code: ExportCommandErrorCode::Failed,
            message: message.into(),
            media_id: None,
            path_code: None,
        }
    }

    fn from_gate(error: OperationGateError) -> Self {
        match error {
            OperationGateError::Conflict => Self {
                code: ExportCommandErrorCode::Conflict,
                message: "Outra operação exclusiva já está em andamento. Aguarde sua conclusão e tente novamente.".into(),
                media_id: None,
                path_code: None,
            },
            OperationGateError::Unavailable { reason } => Self {
                code: ExportCommandErrorCode::Failed,
                message: format!("Não foi possível reservar a Exportação: {reason}"),
                media_id: None,
                path_code: None,
            },
        }
    }

    fn from_pipeline(failure: export_pipeline::ExportFailure) -> Self {
        if let Some(processor) = failure.processor_failure {
            return Self {
                code: processor.code.into(),
                message: failure.message,
                media_id: processor.media_id,
                path_code: processor.path_code.map(Into::into),
            };
        }
        match failure.stage {
            export_pipeline::ExportFailureStage::Cancelled => Self::cancelled(),
            export_pipeline::ExportFailureStage::ExportConflict => Self {
                code: ExportCommandErrorCode::ExportConflict,
                message: failure.message,
                media_id: None,
                path_code: None,
            },
            export_pipeline::ExportFailureStage::Publish { .. } => Self {
                code: ExportCommandErrorCode::PublicationFailed,
                message: failure.message,
                media_id: None,
                path_code: None,
            },
            _ => Self::failed(failure.message),
        }
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
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn export_sheet(
    app: AppHandle,
    window: WebviewWindow,
    sheet_id: String,
    on_event: Channel<ExportEvent>,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
    operation_gate: State<'_, OperationGate>,
    cache: State<'_, CacheActivityGate>,
    processor: State<'_, ImagingProcessor>,
    attempts: State<'_, ExportAttempts>,
) -> Result<ExportResult, ExportCommandError> {
    let frozen = state
        .freeze_sheet_export(&sheet_id)
        .map_err(ExportCommandError::failed)?;
    let sheet = frozen
        .snapshot
        .composition
        .sheets
        .iter()
        .find(|sheet| sheet.sheet_id == sheet_id)
        .ok_or_else(|| ExportCommandError::failed("A Lâmina selecionada não existe."))?;
    let suggested_filename = suggested_export_filename(&frozen.snapshot.project_name, sheet.number);
    let destination = native_project_dialog::choose_export_destination(&window, suggested_filename)
        .await
        .map_err(|error| {
            ExportCommandError::failed(format!(
                "Não foi possível escolher o Destino da Exportação: {error}"
            ))
        })?;
    let native_project_dialog::ExportSaveDialogOutcome::Selected {
        path: output_path,
        authorization,
    } = destination
    else {
        return Err(ExportCommandError::cancelled());
    };

    let export_sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("export-{}-{export_sequence}", std::process::id());
    let project_id = safe_log_identifier(&frozen.snapshot.project_id).map(str::to_owned);
    let plan = export_pipeline::plan(
        frozen.snapshot,
        export_pipeline::ExportOptions::new(
            request_id.clone(),
            output_path,
            authorization,
            sheet_id,
            frozen.sources,
        ),
    )
    .map_err(|failure| {
        log_imaging_failure(
            "export_failed",
            &request_id,
            project_id.as_deref(),
            failure.stage.as_str(),
            failure.exit_code,
        );
        ExportCommandError::from_pipeline(failure)
    })?;
    let operation_paths = plan
        .required_paths()
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    let acquisition = OperationLease::begin(&operation_gate).map_err(|error| {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            operation_id = request_id.as_str(),
            project_id = project_id.as_deref(),
            window_label = window.label(),
            reason = %error,
            event = "export_start_rejected",
        );
        ExportCommandError::from_gate(error)
    })?;
    let attempt = attempts
        .begin(request_id.clone(), window.label())
        .map_err(|error| ExportCommandError::failed(error.to_string()))?;
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
    let root_bindings_completion = path_io::capture_root_bindings(operation_paths);
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
            ExportCommandError::failed(error.to_string())
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
    let published = export_pipeline::execute(
        &mut transport,
        plan,
        &root_bindings,
        attempt.execution_control(),
        &progress,
        &context,
    )
    .await
    .map_err(|failure| {
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

fn suggested_export_filename(project_name: &str, sheet_number: usize) -> String {
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
    format!("{project_name}_{sheet_number:03}.jpg")
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

    use crate::ipc_contract::{ExportCommandError, ExportEvent};

    use super::suggested_export_filename;

    #[test]
    fn suggested_jpeg_name_uses_the_project_and_sheet_position_safely() {
        assert_eq!(
            suggested_export_filename("Casamento da Júlia", 2),
            "Casamento da Júlia_002.jpg"
        );
        assert_eq!(
            suggested_export_filename("Álbum: Horizonte", 2),
            "Álbum_ Horizonte_002.jpg"
        );
        assert_eq!(suggested_export_filename("...", 1), "Projeto_001.jpg");
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
                "message": "A Exportação foi cancelada.",
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
    fn publication_outcomes_keep_distinct_typed_ipc_results() {
        let conflict = ExportCommandError::from_pipeline(ExportFailure::new(
            ExportFailureStage::ExportConflict,
            "O Destino surgiu depois da confirmação.",
        ));
        assert_eq!(
            serde_json::to_value(conflict).expect("the conflict serializes"),
            json!({
                "code": "export_conflict",
                "message": "O Destino surgiu depois da confirmação.",
            })
        );

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
                    "units": {
                        "kind": "unmeasured",
                    },
                    "cancellable": false,
                },
            })
        );
    }
}
