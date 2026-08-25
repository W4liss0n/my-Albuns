use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

use crate::{
    ipc_contract::{
        ProjectDialogAction, ProjectDialogDetail, ProjectDialogProgress, ProjectDialogState,
    },
    native_dialog_window,
    product_runtime::PROJECT_WINDOW_LABEL,
};

pub(crate) const PROJECT_DIALOG_ACTION_EVENT: &str = "myalbuns://project-dialog-action";
pub(crate) const PROJECT_DIALOG_STATE_EVENT: &str = "myalbuns://project-dialog-state";
const PROJECT_DIALOG_LABEL: &str = "project-dialog";
const MAX_DIALOG_TEXT_CHARS: usize = 800;
const MAX_DIALOG_DETAILS: usize = 10;

impl ProjectDialogState {
    fn sanitized(self) -> Self {
        match self {
            Self::AlbumInformationConfirmation { busy, details } => {
                Self::AlbumInformationConfirmation {
                    busy,
                    details: details
                        .into_iter()
                        .take(MAX_DIALOG_DETAILS)
                        .map(|detail| ProjectDialogDetail {
                            label: bound_text(detail.label),
                            value: bound_text(detail.value),
                        })
                        .collect(),
                }
            }
            Self::ProjectCloseConfirmation { busy } => Self::ProjectCloseConfirmation { busy },
            Self::ProjectCloseFailure { message } => Self::ProjectCloseFailure {
                message: bound_text(message),
            },
            Self::ProjectOperationFailure { message } => Self::ProjectOperationFailure {
                message: bound_text(message),
            },
            Self::ExportProgress {
                cancel_requested,
                cancellable,
                progress,
            } => Self::ExportProgress {
                cancel_requested,
                cancellable,
                progress: match progress {
                    ProjectDialogProgress::Indeterminate { status } => {
                        ProjectDialogProgress::Indeterminate {
                            status: bound_text(status),
                        }
                    }
                    ProjectDialogProgress::Determinate {
                        completed,
                        status,
                        total,
                    } => ProjectDialogProgress::Determinate {
                        completed,
                        status: bound_text(status),
                        total,
                    },
                },
            },
            Self::ExportFailure {
                cancelled,
                message,
                retry_disabled,
            } => Self::ExportFailure {
                cancelled,
                message: bound_text(message),
                retry_disabled,
            },
            Self::ExportSuccess { message } => Self::ExportSuccess {
                message: bound_text(message),
            },
        }
    }

    fn initial_dimensions(&self) -> (f64, f64) {
        match self {
            Self::AlbumInformationConfirmation { .. } => (
                520.0,
                280.0 + native_dialog_window::OWNED_WINDOW_TITLEBAR_HEIGHT,
            ),
            Self::ProjectCloseConfirmation { .. } => (
                520.0,
                214.0 + native_dialog_window::OWNED_WINDOW_TITLEBAR_HEIGHT,
            ),
            Self::ProjectCloseFailure { .. }
            | Self::ProjectOperationFailure { .. }
            | Self::ExportFailure { .. }
            | Self::ExportSuccess { .. } => (
                440.0,
                202.0 + native_dialog_window::OWNED_WINDOW_TITLEBAR_HEIGHT,
            ),
            Self::ExportProgress { .. } => (
                440.0,
                176.0 + native_dialog_window::OWNED_WINDOW_TITLEBAR_HEIGHT,
            ),
        }
    }
}

#[derive(Default)]
pub(crate) struct ProjectDialogStateStore(Mutex<Option<ProjectDialogState>>);

impl ProjectDialogStateStore {
    fn replace(&self, state: ProjectDialogState) -> Result<(), String> {
        *self
            .0
            .lock()
            .map_err(|_| "the Project dialog state is unavailable")? = Some(state);
        Ok(())
    }

    fn current(&self) -> Result<Option<ProjectDialogState>, String> {
        self.0
            .lock()
            .map(|state| state.clone())
            .map_err(|_| "the Project dialog state is unavailable".into())
    }
}

#[tauri::command]
pub(crate) async fn present_project_dialog(
    app: AppHandle,
    window: WebviewWindow,
    state: ProjectDialogState,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<(), String> {
    require_project_owner(&window)?;
    let state = state.sanitized();
    state_store.replace(state.clone())?;
    let owner = window;

    if let Some(dialog) = app.get_webview_window(PROJECT_DIALOG_LABEL) {
        dialog
            .emit(PROJECT_DIALOG_STATE_EVENT, &state)
            .map_err(|error| error.to_string())?;
        return native_dialog_window::display_owned_dialog(&owner, &dialog)
            .map_err(|error| error.to_string());
    }

    let serialized = serde_json::to_string(&state).map_err(|error| error.to_string())?;
    let url = format!(
        "project-dialog.html?state={}",
        native_dialog_window::encode_unbounded_component(&serialized)
    );
    let (width, height) = state.initial_dimensions();
    let dialog = native_dialog_window::build_hidden_owned_window(
        &app,
        &owner,
        PROJECT_DIALOG_LABEL,
        &url,
        width,
        height,
    )
    .await
    .map_err(|error| error.to_string())?;
    let owner_after_close = owner.clone();
    dialog.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            native_dialog_window::release_blocked_owner_if_disabled(&owner_after_close, true);
        }
    });
    native_dialog_window::display_owned_dialog(&owner, &dialog).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn current_project_dialog_state(
    window: WebviewWindow,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<Option<ProjectDialogState>, String> {
    if window.label() != PROJECT_DIALOG_LABEL {
        return Err("dialog state belongs only to the Project dialog window".into());
    }
    state_store.current()
}

#[tauri::command]
pub(crate) fn dismiss_project_dialog(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    require_project_owner(&window)?;
    if let Some(dialog) = app.get_webview_window(PROJECT_DIALOG_LABEL) {
        native_dialog_window::dismiss_blocked_window(&window, &dialog, true)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn submit_project_dialog_action(
    app: AppHandle,
    window: WebviewWindow,
    action: ProjectDialogAction,
) -> Result<(), String> {
    if window.label() != PROJECT_DIALOG_LABEL {
        return Err("dialog actions belong only to the Project dialog window".into());
    }
    app.emit_to(PROJECT_WINDOW_LABEL, PROJECT_DIALOG_ACTION_EVENT, action)
        .map_err(|error| error.to_string())
}

fn require_project_owner(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == PROJECT_WINDOW_LABEL {
        Ok(())
    } else {
        Err("Project dialogs belong only to the Project window".into())
    }
}

fn bound_text(value: String) -> String {
    value.chars().take(MAX_DIALOG_TEXT_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dialog_text_is_bounded_before_entering_the_window_url() {
        let state = ProjectDialogState::ProjectCloseFailure {
            message: "a".repeat(MAX_DIALOG_TEXT_CHARS + 20),
        }
        .sanitized();
        let ProjectDialogState::ProjectCloseFailure { message } = state else {
            panic!("the variant is preserved")
        };
        assert_eq!(message.chars().count(), MAX_DIALOG_TEXT_CHARS);
    }

    #[test]
    fn album_information_dialog_keeps_the_complete_change_summary() {
        let state = ProjectDialogState::AlbumInformationConfirmation {
            busy: false,
            details: (0..12)
                .map(|index| ProjectDialogDetail {
                    label: format!("Alteração {index}"),
                    value: format!("Valor {index}"),
                })
                .collect(),
        }
        .sanitized();
        let ProjectDialogState::AlbumInformationConfirmation { details, .. } = state else {
            panic!("the variant is preserved")
        };
        assert_eq!(details.len(), 10);
    }

    #[test]
    fn project_dialog_state_store_returns_only_the_latest_projection() {
        let store = ProjectDialogStateStore::default();
        store
            .replace(ProjectDialogState::ProjectCloseConfirmation { busy: false })
            .expect("the first dialog state is stored");
        store
            .replace(ProjectDialogState::ExportFailure {
                cancelled: false,
                message: "Falha mais recente".into(),
                retry_disabled: false,
            })
            .expect("the newer dialog state replaces the first");

        assert!(matches!(
            store.current().expect("the current state is readable"),
            Some(ProjectDialogState::ExportFailure { message, .. })
                if message == "Falha mais recente"
        ));
    }
}
