use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::{
    ipc_contract::{
        PhotoshopCommandError, PhotoshopErrorCode, PhotoshopPhotoTarget, PhotoshopStatus,
    },
    project_host::ProjectHost,
};

use super::PhotoshopStateStore;

#[tauri::command]
pub(crate) async fn photoshop_status(
    app: AppHandle,
) -> Result<PhotoshopStatus, PhotoshopCommandError> {
    tauri::async_runtime::spawn_blocking(move || app.state::<PhotoshopStateStore>().status())
        .await
        .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?
}

#[tauri::command]
pub(crate) async fn select_photoshop(
    app: AppHandle,
    installation_id: String,
) -> Result<PhotoshopStatus, PhotoshopCommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<PhotoshopStateStore>().select(&installation_id)
    })
    .await
    .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?
}

#[tauri::command]
pub(crate) async fn choose_photoshop(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<PhotoshopStatus>, PhotoshopCommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("Localizar Photoshop")
            .add_filter("Adobe Photoshop", &["exe"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file
            .into_path()
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::InvalidInstallation))?;
        app.state::<PhotoshopStateStore>().choose(path).map(Some)
    })
    .await
    .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::DialogUnavailable))?
}

#[tauri::command]
pub(crate) async fn open_in_photoshop(
    app: AppHandle,
    target: PhotoshopPhotoTarget,
) -> Result<(), PhotoshopCommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let binding = app.state::<ProjectHost>().photoshop_photo(&target)?;
        app.state::<PhotoshopStateStore>().open_original(&binding)
    })
    .await
    .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::LaunchFailed))?
}
