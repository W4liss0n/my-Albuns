use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

use crate::{
    ipc_contract::{
        ProjectDialogAction, ProjectDialogActionEvent, ProjectDialogDetail, ProjectDialogProgress,
        ProjectDialogState,
    },
    native_dialog_window,
    product_runtime::PROJECT_WINDOW_LABEL,
};

pub(crate) const PROJECT_DIALOG_ACTION_EVENT: &str = "myalbuns://project-dialog-action";
pub(crate) const PROJECT_DIALOG_STATE_EVENT: &str = "myalbuns://project-dialog-state";
const PROJECT_DIALOG_LABEL: &str = "project-dialog";
const MAX_DIALOG_TEXT_CHARS: usize = 800;
const MAX_DIALOG_DETAILS: usize = 10;
const MAX_DIALOG_SESSION_ID_CHARS: usize = 128;

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

#[derive(Clone)]
struct OwnedProjectDialogState {
    session_id: String,
    state: ProjectDialogState,
}

#[derive(Default)]
pub(crate) struct ProjectDialogStateStore(Mutex<Option<OwnedProjectDialogState>>);

impl ProjectDialogStateStore {
    fn present(&self, session_id: &str, state: ProjectDialogState) -> Result<(), String> {
        let mut current = self
            .0
            .lock()
            .map_err(|_| "the Project dialog state is unavailable")?;
        if let Some(current) = current.as_mut() {
            if current.session_id != session_id {
                return Err("another Project dialog session owns the window".into());
            }
            current.state = state;
            return Ok(());
        }
        *current = Some(OwnedProjectDialogState {
            session_id: session_id.into(),
            state,
        });
        Ok(())
    }

    fn current(&self) -> Result<Option<OwnedProjectDialogState>, String> {
        self.0
            .lock()
            .map(|state| state.clone())
            .map_err(|_| "the Project dialog state is unavailable".into())
    }

    fn clear(&self, session_id: &str) -> Result<bool, String> {
        let mut current = self
            .0
            .lock()
            .map_err(|_| "the Project dialog state is unavailable")?;
        if current
            .as_ref()
            .is_some_and(|current| current.session_id == session_id)
        {
            *current = None;
            return Ok(true);
        }
        Ok(false)
    }

    fn is_owned_by(&self, session_id: &str) -> Result<bool, String> {
        self.0
            .lock()
            .map(|current| {
                current
                    .as_ref()
                    .is_some_and(|current| current.session_id == session_id)
            })
            .map_err(|_| "the Project dialog state is unavailable".into())
    }
}

#[tauri::command]
pub(crate) async fn present_project_dialog(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    state: ProjectDialogState,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<(), String> {
    require_project_owner(&window)?;
    require_dialog_session_id(&session_id)?;
    let state = state.sanitized();
    state_store.present(&session_id, state.clone())?;
    let owner = window;
    let presentation = async {
        if let Some(dialog) = app.get_webview_window(PROJECT_DIALOG_LABEL) {
            dialog
                .emit(PROJECT_DIALOG_STATE_EVENT, &state)
                .map_err(|error| error.to_string())?;
            return native_dialog_window::display_owned_dialog(&owner, &dialog)
                .map_err(|error| error.to_string());
        }

        let serialized = serde_json::to_string(&state).map_err(|error| error.to_string())?;
        let url = format!(
            "project-dialog.html?sessionId={}&state={}",
            native_dialog_window::encode_unbounded_component(&session_id),
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
        native_dialog_window::display_owned_dialog(&owner, &dialog)
            .map_err(|error| error.to_string())
    }
    .await;
    if presentation.is_err() {
        let window_was_released = match app.get_webview_window(PROJECT_DIALOG_LABEL) {
            Some(dialog) => {
                native_dialog_window::dismiss_blocked_window(&owner, &dialog, true).is_ok()
            }
            None => true,
        };
        if window_was_released {
            state_store.clear(&session_id)?;
        }
    }
    presentation
}

#[tauri::command]
pub(crate) fn current_project_dialog_state(
    window: WebviewWindow,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<Option<ProjectDialogState>, String> {
    if window.label() != PROJECT_DIALOG_LABEL {
        return Err("dialog state belongs only to the Project dialog window".into());
    }
    state_store
        .current()
        .map(|current| current.map(|current| current.state))
}

#[tauri::command]
pub(crate) fn dismiss_project_dialog(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<(), String> {
    require_project_owner(&window)?;
    require_dialog_session_id(&session_id)?;
    if !state_store.is_owned_by(&session_id)? {
        return Ok(());
    }
    if let Some(dialog) = app.get_webview_window(PROJECT_DIALOG_LABEL) {
        native_dialog_window::dismiss_blocked_window(&window, &dialog, true)
            .map_err(|error| error.to_string())?;
    }
    state_store.clear(&session_id)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn submit_project_dialog_action(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    action: ProjectDialogAction,
    state_store: State<'_, ProjectDialogStateStore>,
) -> Result<(), String> {
    if window.label() != PROJECT_DIALOG_LABEL {
        return Err("dialog actions belong only to the Project dialog window".into());
    }
    require_dialog_session_id(&session_id)?;
    if !state_store.is_owned_by(&session_id)? {
        return Ok(());
    }
    app.emit_to(
        PROJECT_WINDOW_LABEL,
        PROJECT_DIALOG_ACTION_EVENT,
        ProjectDialogActionEvent { action, session_id },
    )
    .map_err(|error| error.to_string())
}

fn require_project_owner(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == PROJECT_WINDOW_LABEL {
        Ok(())
    } else {
        Err("Project dialogs belong only to the Project window".into())
    }
}

fn require_dialog_session_id(session_id: &str) -> Result<(), String> {
    let length = session_id.chars().count();
    if length == 0 || length > MAX_DIALOG_SESSION_ID_CHARS {
        Err("the Project dialog session id is invalid".into())
    } else {
        Ok(())
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
            .present(
                "close",
                ProjectDialogState::ProjectCloseConfirmation { busy: false },
            )
            .expect("the first dialog state is stored");
        store
            .present(
                "close",
                ProjectDialogState::ExportFailure {
                    cancelled: false,
                    message: "Falha mais recente".into(),
                    retry_disabled: false,
                },
            )
            .expect("the newer dialog state replaces the first");

        assert!(matches!(
            store.current().expect("the current state is readable"),
            Some(OwnedProjectDialogState {
                state: ProjectDialogState::ExportFailure { message, .. },
                ..
            })
                if message == "Falha mais recente"
        ));
    }

    #[test]
    fn stale_dialog_session_cannot_replace_or_clear_the_current_owner() {
        let store = ProjectDialogStateStore::default();
        store
            .present(
                "album-information",
                ProjectDialogState::AlbumInformationConfirmation {
                    busy: false,
                    details: Vec::new(),
                },
            )
            .expect("the first session owns the dialog");

        assert!(
            store
                .present(
                    "operation-failure",
                    ProjectDialogState::ProjectOperationFailure {
                        message: "Falha concorrente".into(),
                    },
                )
                .is_err()
        );
        assert!(
            !store
                .clear("operation-failure")
                .expect("a stale dismissal is a harmless no-op")
        );

        let current = store
            .current()
            .expect("the current projection is readable")
            .expect("the original owner remains current");
        assert_eq!(current.session_id, "album-information");
        assert!(matches!(
            current.state,
            ProjectDialogState::AlbumInformationConfirmation { .. }
        ));
    }
}
