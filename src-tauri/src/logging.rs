use std::path::{Path, PathBuf};

use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::{LoggingGuard, ProcessRole, init_local_logging, safe_log_identifier};
use myalbuns_paths::AppPaths;
use serde::Deserialize;
use tauri::{App, Manager, Runtime};

pub(crate) struct LoggingState {
    log_directory: PathBuf,
    _guard: Option<LoggingGuard>,
}

impl LoggingState {
    pub(crate) fn directory(&self) -> &Path {
        &self.log_directory
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FrontendLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FrontendLogEvent {
    level: FrontendLogLevel,
    component: String,
    event: String,
    project_id: Option<String>,
    operation_id: Option<String>,
    instance_id: Option<String>,
    reason: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sheet_count: Option<usize>,
}

impl FrontendLogEvent {
    fn validate(&self) -> Result<(), String> {
        validate_log_token("component", &self.component, 48)?;
        validate_log_token("event", &self.event, 80)?;
        validate_optional_log_token("reason", self.reason.as_deref(), 80)?;
        validate_optional_identifier("projectId", self.project_id.as_deref())?;
        validate_optional_identifier("operationId", self.operation_id.as_deref())?;
        validate_optional_identifier("instanceId", self.instance_id.as_deref())
    }
}

fn validate_log_token(label: &str, value: &str, maximum_length: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > maximum_length
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
    {
        return Err(format!("Campo de log inválido: {label}."));
    }
    Ok(())
}

fn validate_optional_log_token(
    label: &str,
    value: Option<&str>,
    maximum_length: usize,
) -> Result<(), String> {
    match value {
        Some(value) => validate_log_token(label, value, maximum_length),
        None => Ok(()),
    }
}

pub(crate) fn validate_optional_identifier(label: &str, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value) if safe_log_identifier(value).is_none() => {
            Err(format!("Identificador de log inválido: {label}."))
        }
        _ => Ok(()),
    }
}

macro_rules! write_frontend_log {
    ($level:ident, $event:ident) => {
        tracing::$level!(
            target: "myalbuns.frontend",
            process_role = "frontend",
            component = %$event.component,
            event = %$event.event,
            project_id = $event.project_id.as_deref(),
            operation_id = $event.operation_id.as_deref(),
            instance_id = $event.instance_id.as_deref(),
            reason = $event.reason.as_deref(),
            width = $event.width,
            height = $event.height,
            sheet_count = $event.sheet_count,
        )
    };
}

#[tauri::command]
pub(crate) fn frontend_log(event: FrontendLogEvent) -> Result<(), String> {
    event.validate()?;
    match event.level {
        FrontendLogLevel::Debug => write_frontend_log!(debug, event),
        FrontendLogLevel::Info => write_frontend_log!(info, event),
        FrontendLogLevel::Warn => write_frontend_log!(warn, event),
        FrontendLogLevel::Error => write_frontend_log!(error, event),
    }
    Ok(())
}

pub(crate) fn initialize<R: Runtime>(app: &mut App<R>, app_paths: &AppPaths) {
    let log_directory = app_paths.logs_dir();
    let guard = match init_local_logging(&log_directory, ProcessRole::DesktopHost) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("logging indisponível: {error}");
            None
        }
    };
    app.manage(LoggingState {
        log_directory,
        _guard: guard,
    });
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        event = "application_started",
    );
}

#[cfg(test)]
mod tests {
    use super::FrontendLogEvent;

    fn valid_event() -> FrontendLogEvent {
        serde_json::from_str(include_str!("../../tests/fixtures/frontend-log-event.json"))
            .expect("the shared fixture must deserialize into the Rust contract")
    }

    #[test]
    fn accepts_bounded_structured_frontend_events() {
        assert!(valid_event().validate().is_ok());
    }

    #[test]
    fn rejects_path_shaped_fields_and_control_characters() {
        let mut path_event = valid_event();
        path_event.event = r"c:\users\someone\photo.jpg".into();
        assert!(path_event.validate().is_err());

        let mut path_identifier = valid_event();
        path_identifier.project_id = Some(r"c:\users\someone\project.myalbum".into());
        assert!(path_identifier.validate().is_err());

        let mut control_character = valid_event();
        control_character.project_id = Some("project\nforged-event".into());
        assert!(control_character.validate().is_err());
    }

    #[test]
    fn rejects_unknown_frontend_log_fields() {
        let mut event: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/frontend-log-event.json"))
                .expect("the shared fixture must be valid JSON");
        event["message"] = serde_json::json!("arbitrary content");

        assert!(serde_json::from_value::<FrontendLogEvent>(event).is_err());
    }
}
