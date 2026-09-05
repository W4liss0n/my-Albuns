use myalbuns_core::{SaveProjectError, SaveProjectOutcome};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use tauri::{State, Window};

use crate::{
    ipc_contract::{
        ProjectCloseChoice, ProjectCloseRequestOutcome, ProjectCloseResolution,
        SaveProjectCommandError,
    },
    project_commands::map_save_project_error,
    project_host::{
        ProjectCloseRequestOutcome as HostCloseRequestOutcome, ProjectHost, ProjectHostSaveError,
    },
    project_window_lifecycle::complete_project_close,
};

#[tauri::command]
pub(crate) fn request_project_close(
    window: Window,
    state: State<'_, ProjectHost>,
) -> Result<ProjectCloseRequestOutcome, SaveProjectCommandError> {
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        window_label = window.label(),
        event = "project_close_command_received",
    );
    match state
        .begin_close()
        .map_err(|_| SaveProjectCommandError::SessionUnavailable)?
    {
        HostCloseRequestOutcome::CloseImmediately => {
            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                process_id = std::process::id(),
                event = "clean_project_close_requested",
            );
            complete_project_close(&window);
            Ok(ProjectCloseRequestOutcome::Closed)
        }
        HostCloseRequestOutcome::ConfirmationRequired => {
            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                event = "dirty_project_close_confirmation_required",
            );
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        }
    }
}

#[tauri::command]
pub(crate) async fn resolve_project_close(
    choice: ProjectCloseChoice,
    window: Window,
    state: State<'_, ProjectHost>,
) -> Result<ProjectCloseResolution, SaveProjectCommandError> {
    match choice {
        ProjectCloseChoice::Cancel => cancel_close(&window, &state),
        ProjectCloseChoice::DiscardAndClose => discard_and_close(&window, &state),
        ProjectCloseChoice::SaveAndClose => save_and_close(&window, state.inner().clone()).await,
    }
}

fn cancel_close(
    window: &Window,
    host: &ProjectHost,
) -> Result<ProjectCloseResolution, SaveProjectCommandError> {
    let projection = host
        .cancel_close()
        .map_err(|_| SaveProjectCommandError::SessionUnavailable)?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_close_cancelled",
    );
    Ok(ProjectCloseResolution::Cancelled {
        projection: Box::new(projection),
    })
}

fn discard_and_close(
    window: &Window,
    host: &ProjectHost,
) -> Result<ProjectCloseResolution, SaveProjectCommandError> {
    host.discard_close()
        .map_err(|_| SaveProjectCommandError::SessionUnavailable)?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        event = "project_close_discarded",
    );
    complete_project_close(window);
    Ok(ProjectCloseResolution::Closed)
}

async fn save_and_close(
    window: &Window,
    host: ProjectHost,
) -> Result<ProjectCloseResolution, SaveProjectCommandError> {
    match tauri::async_runtime::spawn_blocking(move || host.save_and_close()).await {
        Ok(Ok(outcome)) => {
            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                revision = saved_revision(outcome),
                event = "project_save_and_close_completed",
            );
            complete_project_close(window);
            Ok(ProjectCloseResolution::Closed)
        }
        Ok(Err(error)) => {
            let terminal = must_terminate_after_close_save_failure(&error);
            let command_error = map_save_project_error(error);
            if terminal {
                tracing::error!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    window_label = window.label(),
                    event = "project_save_and_close_indeterminate",
                );
                complete_project_close(window);
            } else {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    window_label = window.label(),
                    event = "project_save_and_close_rejected",
                );
            }
            Err(command_error)
        }
        Err(error) => {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                error = %error,
                event = "project_save_and_close_worker_failed",
            );
            complete_project_close(window);
            Err(SaveProjectCommandError::SessionUnavailable)
        }
    }
}

fn saved_revision(outcome: SaveProjectOutcome) -> u64 {
    match outcome {
        SaveProjectOutcome::Saved { revision }
        | SaveProjectOutcome::AlreadyCurrent { revision } => revision,
    }
}

fn must_terminate_after_close_save_failure(error: &ProjectHostSaveError) -> bool {
    matches!(
        error,
        ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate)
    )
}

#[cfg(test)]
mod tests {
    use myalbuns_core::SaveProjectError;

    use crate::project_host::ProjectHostSaveError;

    use super::must_terminate_after_close_save_failure;

    #[test]
    fn only_an_indeterminate_close_save_failure_makes_the_session_terminal() {
        assert!(must_terminate_after_close_save_failure(
            &ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate)
        ));
        assert!(!must_terminate_after_close_save_failure(
            &ProjectHostSaveError::Project(SaveProjectError::PersistedBaselineConflict)
        ));
        assert!(!must_terminate_after_close_save_failure(
            &ProjectHostSaveError::RecoveryCleanupFailed
        ));
    }
}
