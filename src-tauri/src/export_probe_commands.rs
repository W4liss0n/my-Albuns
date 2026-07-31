use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow, ipc::Channel};

use crate::{
    cache_engine::CacheEngine,
    export_attempts::{CancelDisposition, ExportAttempts},
    export_pipeline,
    imaging_processor::{ImagingProcessor, InvocationContext, TauriImagingTransport},
    logging::{LoggingState, log_imaging_failure},
    operation_gate::{OperationGate, OperationGateError, OperationMode},
    operation_lease::OperationLease,
    path_io,
    project_host::ProjectHost,
};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportResult {
    output_path: String,
    width_px: u32,
    height_px: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub(crate) enum ExportEvent {
    Started {
        operation_id: String,
        cancellable: bool,
    },
    Progress {
        operation_id: String,
        stage: &'static str,
        units: ExportProgressUnitsPayload,
        cancellable: bool,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub(crate) enum ExportProgressUnitsPayload {
    Unmeasured,
    Measured {
        completed_units: u32,
        total_units: u32,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportCommandError {
    code: &'static str,
    message: String,
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
            stage: progress.stage.as_str(),
            units,
            cancellable: progress.cancellable,
        }
    }
}

impl ExportCommandError {
    fn cancelled() -> Self {
        Self {
            code: "cancelled",
            message: "A Exportação foi cancelada.".into(),
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            code: "failed",
            message: message.into(),
        }
    }

    fn from_gate(error: OperationGateError) -> Self {
        match error {
            OperationGateError::Conflict { .. } => Self {
                code: "conflict",
                message: "Outra operação exclusiva já está em andamento. Aguarde sua conclusão e tente novamente.".into(),
            },
            OperationGateError::Unavailable { reason, .. } => Self {
                code: "failed",
                message: format!("Não foi possível reservar a Exportação: {reason}"),
            },
        }
    }

    fn from_pipeline(failure: export_pipeline::ExportFailure) -> Self {
        if failure.stage == export_pipeline::ExportFailureStage::Cancelled {
            Self::cancelled()
        } else {
            Self::failed(failure.message)
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
pub(crate) async fn export_spike(
    app: AppHandle,
    window: WebviewWindow,
    on_event: Channel<ExportEvent>,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
    operation_gate: State<'_, OperationGate>,
    cache: State<'_, CacheEngine>,
    processor: State<'_, ImagingProcessor>,
    attempts: State<'_, ExportAttempts>,
) -> Result<ExportResult, ExportCommandError> {
    let export_sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("export-{}-{export_sequence}", std::process::id());
    let snapshot = state
        .render_snapshot(window.label())
        .inspect_err(|_| {
            log_imaging_failure("export_failed", &request_id, None, "session_lock", None);
        })
        .map_err(ExportCommandError::failed)?;

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    std::fs::create_dir_all(&output_dir).map_err(|error| {
        ExportCommandError::failed(format!(
            "Não foi possível preparar o Destino da Exportação: {error}"
        ))
    })?;
    let output_path = output_dir.join(format!(
        "Album-Horizonte_{}_{export_sequence:03}.png",
        std::process::id()
    ));
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .ok_or_else(|| ExportCommandError::failed("O snapshot não contém Lâminas."))?
        .sheet_id
        .clone();
    let sources = state
        .export_sources(window.label(), &snapshot, &sheet_id)
        .map_err(ExportCommandError::failed)?;
    let dpi = if sources.is_some() { 300 } else { 25 };
    let project_id = safe_log_identifier(&snapshot.project_id).map(str::to_owned);
    let plan = export_pipeline::plan(
        snapshot,
        export_pipeline::ExportOptions::new(
            request_id.clone(),
            output_path,
            sheet_id,
            dpi,
            sources,
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
        .map(|path| path.to_path_buf())
        .collect();
    let acquisition =
        OperationLease::begin(&operation_gate, OperationMode::NormalExport).map_err(|error| {
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
            ExportCommandError::failed(error)
        })?,
        () = attempt.cancelled() => {
            log_export_cancelled(
                &request_id,
                project_id.as_deref(),
                window.label(),
                "capture_root_bindings",
            );
            return Err(ExportCommandError::cancelled());
        },
    };

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
        operation_mode = lease.mode().as_str(),
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
    let output_path = published
        .output_path
        .to_str()
        .ok_or_else(|| {
            log_imaging_failure(
                "export_failed",
                &request_id,
                project_id.as_deref(),
                "serialize_output_path",
                None,
            );
            ExportCommandError::failed(
                "o caminho da Exportação não pode ser representado pela interface",
            )
        })?
        .to_owned();
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
        output_path,
        width_px: completed.width_px,
        height_px: completed.height_px,
    })
}

#[tauri::command]
pub(crate) fn cancel_export_spike(
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;
    use tauri::ipc::{Channel, InvokeResponseBody};

    use crate::{
        export_pipeline::{ExportProgress, ExportProgressStage, ExportProgressUnits},
        operation_gate::{OperationGateError, OperationMode},
    };

    use super::{ExportCommandError, ExportEvent};

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
    fn gate_conflict_is_typed_without_exposing_the_internal_mode() {
        assert_eq!(
            serde_json::to_value(ExportCommandError::from_gate(
                OperationGateError::Conflict {
                    requested: OperationMode::NormalExport,
                },
            ))
            .expect("the command error serializes"),
            json!({
                "code": "conflict",
                "message": "Outra operação exclusiva já está em andamento. Aguarde sua conclusão e tente novamente.",
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
