use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

use crate::{
    cache_previews::CachePreviewRegistry,
    ipc_contract::{ViewerAction, ViewerPresentation},
    native_dialog_window,
    product_runtime::PROJECT_WINDOW_LABEL,
};

#[cfg(debug_assertions)]
use crate::desktop_webview_policy;

pub(crate) const LABEL: &str = "image-viewer";
pub(crate) const NAVIGATE_EVENT: &str = "myalbuns://image-viewer-navigate";
pub(crate) const CLOSED_EVENT: &str = "myalbuns://image-viewer-closed";
pub(crate) const PRESENTATION_EVENT: &str = "myalbuns://image-viewer-presentation";

#[derive(Default)]
pub(crate) struct ViewerStore(Mutex<Option<ViewerPresentation>>);

impl ViewerStore {
    fn current(&self) -> Result<Option<ViewerPresentation>, String> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "viewer state unavailable".into())
    }

    fn replace(&self, presentation: ViewerPresentation) -> Result<(), String> {
        let mut value = self.0.lock().map_err(|_| "viewer state unavailable")?;
        if value
            .as_ref()
            .is_some_and(|old| old.session_id != presentation.session_id)
        {
            return Err("another viewer session is active".into());
        }
        if value
            .as_ref()
            .is_some_and(|old| old.revision > presentation.revision)
        {
            return Ok(());
        }
        *value = Some(presentation);
        Ok(())
    }

    fn clear(&self, session_id: &str) -> Result<bool, String> {
        let mut value = self.0.lock().map_err(|_| "viewer state unavailable")?;
        if value
            .as_ref()
            .is_some_and(|old| old.session_id == session_id)
        {
            *value = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn validate(
    presentation: &mut ViewerPresentation,
    previews: &CachePreviewRegistry,
) -> Result<(), String> {
    if presentation.session_id.len() > 128
        || presentation.session_id.is_empty()
        || presentation.media_id.len() > 256
        || presentation.media_id.is_empty()
    {
        return Err("invalid viewer presentation identity".into());
    }
    presentation.name = presentation.name.chars().take(512).collect();
    if presentation
        .url
        .as_deref()
        .is_some_and(|url| !previews.is_published_url(url))
    {
        return Err("the viewer URL is not a published cache preview".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn open_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    mut presentation: ViewerPresentation,
    store: State<'_, ViewerStore>,
    previews: State<'_, CachePreviewRegistry>,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("only the Project window can open the viewer".into());
    }
    validate(&mut presentation, &previews)?;
    store.replace(presentation.clone())?;
    previews.set_viewer_access(true);
    let result = async {
        if let Some(existing) = app.get_webview_window(LABEL) {
            existing
                .emit(PRESENTATION_EVENT, &presentation)
                .map_err(|error| error.to_string())?;
            return native_dialog_window::display_owned_dialog(&window, &existing)
                .map_err(|error| error.to_string());
        }
        #[cfg(debug_assertions)]
        let browser_arguments = desktop_webview_policy::replacement_webview_debug_arguments(
            std::env::var_os("MYALBUNS_DEV_IMAGE_VIEWER_WEBVIEW_DEBUG_PORT"),
        )
        .map_err(|error| error.to_string())?;
        #[cfg(debug_assertions)]
        let browser_data_directory = desktop_webview_policy::project_dialog_debug_data_directory(
            std::env::var_os("MYALBUNS_DEV_IMAGE_VIEWER_WEBVIEW_DATA_DIRECTORY"),
        )
        .map_err(|error| error.to_string())?;
        #[cfg(not(debug_assertions))]
        let browser_arguments: Option<String> = None;
        #[cfg(not(debug_assertions))]
        let browser_data_directory: Option<std::path::PathBuf> = None;
        let viewer = native_dialog_window::build_hidden_owned_viewer_window(
            &app,
            &window,
            native_dialog_window::HiddenOwnedWindowConfig {
                label: LABEL,
                url: "image-viewer.html",
                width: 1050.0,
                height: 720.0,
                browser_arguments: browser_arguments.as_deref(),
                browser_data_directory: browser_data_directory.as_deref(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        let owner = window.clone();
        let app_for_close = app.clone();
        let session_for_close = presentation.session_id.clone();
        viewer.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                retire(&app_for_close, &owner, &session_for_close);
            }
        });
        if store
            .current()?
            .as_ref()
            .is_none_or(|current| current.session_id != presentation.session_id)
        {
            let _ = native_dialog_window::dismiss_blocked_window(&window, &viewer, true);
            return Ok(());
        }
        native_dialog_window::display_owned_dialog(&window, &viewer)
            .map_err(|error| error.to_string())
    }
    .await;
    if result.is_err() {
        if let Some(viewer) = app.get_webview_window(LABEL) {
            let _ = native_dialog_window::dismiss_blocked_window(&window, &viewer, true);
        }
        retire(&app, &window, &presentation.session_id);
    }
    result
}

#[tauri::command]
pub(crate) fn update_image_viewer(
    window: WebviewWindow,
    mut presentation: ViewerPresentation,
    store: State<'_, ViewerStore>,
    previews: State<'_, CachePreviewRegistry>,
    app: AppHandle,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("only the Project window can update the viewer".into());
    }
    validate(&mut presentation, &previews)?;
    if store
        .current()?
        .as_ref()
        .is_none_or(|old| old.session_id != presentation.session_id)
    {
        return Ok(());
    }
    store.replace(presentation.clone())?;
    if let Some(viewer) = app.get_webview_window(LABEL) {
        viewer
            .emit(PRESENTATION_EVENT, presentation)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn current_image_viewer(
    window: WebviewWindow,
    store: State<'_, ViewerStore>,
) -> Result<Option<ViewerPresentation>, String> {
    if window.label() != LABEL {
        return Err("viewer presentation belongs to the viewer window".into());
    }
    store.current()
}

#[tauri::command]
pub(crate) fn navigate_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    offset: i8,
    store: State<'_, ViewerStore>,
) -> Result<(), String> {
    if window.label() != LABEL || !matches!(offset, -1 | 1) {
        return Err("invalid viewer navigation".into());
    }
    let Some(current) = store.current()? else {
        return Ok(());
    };
    if current.session_id != session_id
        || (offset == -1 && !current.can_previous)
        || (offset == 1 && !current.can_next)
    {
        return Ok(());
    }
    app.emit_to(
        PROJECT_WINDOW_LABEL,
        NAVIGATE_EVENT,
        ViewerAction { session_id, offset },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn close_image_viewer(
    app: AppHandle,
    window: WebviewWindow,
    session_id: String,
    store: State<'_, ViewerStore>,
) -> Result<(), String> {
    if window.label() != LABEL && window.label() != PROJECT_WINDOW_LABEL {
        return Err("invalid viewer closer".into());
    }
    if store
        .current()?
        .as_ref()
        .is_none_or(|current| current.session_id != session_id)
    {
        return Ok(());
    }
    let owner = app
        .get_webview_window(PROJECT_WINDOW_LABEL)
        .ok_or("the Project window is unavailable")?;
    if let Some(viewer) = app.get_webview_window(LABEL) {
        native_dialog_window::dismiss_blocked_window(&owner, &viewer, true)
            .map_err(|error| error.to_string())?;
    }
    retire(&app, &owner, &session_id);
    Ok(())
}

fn retire(app: &AppHandle, owner: &WebviewWindow, session_id: &str) {
    let store = app.state::<ViewerStore>();
    if store
        .current()
        .ok()
        .flatten()
        .is_some_and(|current| current.session_id != session_id)
    {
        return;
    }
    if let Ok(Some(current)) = store.current()
        && current.session_id == session_id
    {
        let _ = store.clear(session_id);
        app.state::<CachePreviewRegistry>().set_viewer_access(false);
        let _ = owner.emit(CLOSED_EVENT, current.session_id);
    }
    native_dialog_window::release_blocked_owner_if_disabled(owner, true);
}

pub(crate) fn retire_for_editor_recovery(app: &AppHandle) -> Result<(), String> {
    let owner = app
        .get_webview_window(PROJECT_WINDOW_LABEL)
        .ok_or("the Project window is unavailable")?;
    if let Some(viewer) = app.get_webview_window(LABEL) {
        native_dialog_window::dismiss_blocked_window(&owner, &viewer, true)
            .map_err(|error| error.to_string())?;
    }
    if let Some(current) = app.state::<ViewerStore>().current()? {
        retire(app, &owner, &current.session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presentation(session_id: &str, revision: u64, media_id: &str) -> ViewerPresentation {
        ViewerPresentation {
            session_id: session_id.into(),
            revision,
            media_id: media_id.into(),
            name: media_id.into(),
            url: None,
            state: crate::ipc_contract::ViewerPreviewState::Loading,
            can_previous: false,
            can_next: true,
        }
    }

    #[test]
    fn viewer_store_rejects_other_sessions_and_stale_updates() {
        let store = ViewerStore::default();
        store.replace(presentation("one", 1, "a")).unwrap();
        store.replace(presentation("one", 3, "c")).unwrap();
        store.replace(presentation("one", 2, "b")).unwrap();
        assert_eq!(store.current().unwrap().unwrap().media_id, "c");
        assert!(store.replace(presentation("two", 1, "d")).is_err());
        assert!(store.clear("one").unwrap());
        store.replace(presentation("two", 1, "d")).unwrap();
    }
}
