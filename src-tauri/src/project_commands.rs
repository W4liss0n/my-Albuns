use myalbuns_core::{
    AlbumInformation, AlbumInformationValidation, EditorProjection, PathFailure, ProjectIntent,
    SaveProjectError, SaveProjectOutcome as CoreSaveProjectOutcome,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use tauri::{State, WebviewWindow};

use crate::{
    ipc_contract::{SaveProjectCommandError, SaveProjectOutcome, SaveProjectResult},
    logging::validate_optional_identifier,
    project_host::{ProjectHost, ProjectHostSaveError},
};

#[tauri::command]
pub(crate) fn project_state(
    operation_id: String,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    validate_optional_identifier("operationId", Some(&operation_id))?;
    let projection = state.projection()?;
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = operation_id.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_state_read",
    );
    Ok(projection)
}

#[tauri::command]
pub(crate) fn apply_project_intent(
    intent: ProjectIntent,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let intent_kind = match &intent {
        ProjectIntent::SetAlbumInformation { .. } => "set_album_information",
        ProjectIntent::SetDpi { .. } => "set_dpi",
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::FillLeftmostPlaceholder { .. } => "fill_leftmost_placeholder",
    };
    let projection = state.apply(intent).inspect_err(|_| {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            window_label = window.label(),
            intent = intent_kind,
            event = "project_intent_rejected",
        );
    })?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        intent = intent_kind,
        event = "project_intent_applied",
    );
    Ok(projection)
}

#[tauri::command]
pub(crate) fn validate_album_information(
    information: AlbumInformation,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<AlbumInformationValidation, String> {
    let validation = state.validate_album_information(&information)?;
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        error_count = validation.errors.len(),
        event = "album_information_validated",
    );
    Ok(validation)
}

#[tauri::command]
pub(crate) fn undo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.undo()?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_undo_completed",
    );
    Ok(projection)
}

#[tauri::command]
pub(crate) fn redo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.redo()?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_redo_completed",
    );
    Ok(projection)
}

#[tauri::command]
pub(crate) async fn save_project(
    expected_revision: u64,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<SaveProjectResult, SaveProjectCommandError> {
    let host = state.inner().clone();
    let window_label = window.label().to_owned();
    let save = tauri::async_runtime::spawn_blocking(move || host.save(expected_revision))
        .await
        .map_err(|error| {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window_label.as_str(),
                expected_revision,
                error = %error,
                event = "project_save_worker_failed",
            );
            SaveProjectCommandError::SessionUnavailable
        })?;
    let saved = save.map_err(|error| {
        let indeterminate = matches!(
            &error,
            ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate)
        );
        if indeterminate {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                expected_revision,
                error = ?error,
                event = "project_save_state_indeterminate",
            );
        } else {
            tracing::warn!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                expected_revision,
                error = ?error,
                event = "project_save_rejected",
            );
        }
        map_save_project_error(error)
    })?;
    let outcome = map_save_project_outcome(saved.outcome);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&saved.projection.state.project_id),
        revision = saved.projection.state.revision,
        save_outcome = match outcome {
            SaveProjectOutcome::Saved { .. } => "saved",
            SaveProjectOutcome::AlreadyCurrent { .. } => "already_current",
        },
        event = "project_save_completed",
    );
    Ok(SaveProjectResult {
        outcome,
        projection: saved.projection,
    })
}

fn map_save_project_outcome(outcome: CoreSaveProjectOutcome) -> SaveProjectOutcome {
    match outcome {
        CoreSaveProjectOutcome::Saved { revision } => SaveProjectOutcome::Saved { revision },
        CoreSaveProjectOutcome::AlreadyCurrent { revision } => {
            SaveProjectOutcome::AlreadyCurrent { revision }
        }
    }
}

pub(crate) fn map_save_project_error(error: ProjectHostSaveError) -> SaveProjectCommandError {
    match error {
        ProjectHostSaveError::Project(SaveProjectError::StaleRevision { expected, current }) => {
            SaveProjectCommandError::StaleRevision {
                expected_revision: expected,
                current_revision: current,
            }
        }
        ProjectHostSaveError::Project(SaveProjectError::PersistedBaselineConflict) => {
            SaveProjectCommandError::PersistedBaselineConflict
        }
        ProjectHostSaveError::Project(SaveProjectError::Path(path)) => match path {
            PathFailure::NotFound => SaveProjectCommandError::NotFound,
            PathFailure::Unavailable => SaveProjectCommandError::Unavailable,
            PathFailure::AccessDenied => SaveProjectCommandError::AccessDenied,
            PathFailure::InvalidPath => SaveProjectCommandError::InvalidPath,
            PathFailure::UnexpectedObjectType => SaveProjectCommandError::UnexpectedObjectType,
            PathFailure::Conflict => SaveProjectCommandError::Conflict,
            PathFailure::IoFailure => SaveProjectCommandError::IoFailure,
        },
        ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate) => {
            SaveProjectCommandError::SaveStateIndeterminate
        }
        ProjectHostSaveError::SessionUnavailable => SaveProjectCommandError::SessionUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{PathFailure, SaveProjectError};
    use serde_json::json;

    use crate::project_host::ProjectHostSaveError;

    use super::map_save_project_error;

    #[test]
    fn maps_every_save_failure_to_stable_wire_data_without_messages() {
        let stale = serde_json::to_value(map_save_project_error(ProjectHostSaveError::Project(
            SaveProjectError::StaleRevision {
                expected: 3,
                current: 4,
            },
        )))
        .expect("the stale-revision command error serializes");
        assert_eq!(
            stale,
            json!({
                "code": "stale_revision",
                "expectedRevision": 3,
                "currentRevision": 4
            })
        );

        let cases = [
            (
                ProjectHostSaveError::Project(SaveProjectError::PersistedBaselineConflict),
                "persisted_baseline_conflict",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::NotFound)),
                "not_found",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::Unavailable)),
                "unavailable",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::AccessDenied)),
                "access_denied",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::InvalidPath)),
                "invalid_path",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(
                    PathFailure::UnexpectedObjectType,
                )),
                "unexpected_object_type",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::Conflict)),
                "conflict",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::IoFailure)),
                "io_failure",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate),
                "save_state_indeterminate",
            ),
            (
                ProjectHostSaveError::SessionUnavailable,
                "session_unavailable",
            ),
        ];

        for (error, expected_code) in cases {
            let value = serde_json::to_value(map_save_project_error(error))
                .expect("the command error serializes");
            assert_eq!(value, json!({ "code": expected_code }));
            assert!(value.get("message").is_none());
        }
    }
}
