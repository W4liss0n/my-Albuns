//! Shows the original file of a Project medium selected in its folder.

use std::path::Path;

use myalbuns_paths::{ExpectedObject, OperationPathContext, ResolveError};
use tauri::{AppHandle, Manager, WebviewWindow};

use crate::{product_runtime::PROJECT_WINDOW_LABEL, project_host::ProjectHost};

const MEDIA_REMOVED: &str = "Esta imagem não está mais no projeto.";
const ORIGINAL_ABSENT: &str =
    "A imagem não foi encontrada. Use Localizar imagem para indicar a pasta onde ela está.";
const ORIGINAL_UNAVAILABLE: &str = "Não foi possível acessar o arquivo da imagem. Verifique o acesso à pasta onde ele está e tente novamente.";
const SHELL_FAILED: &str = "Não foi possível abrir a pasta da imagem. Tente novamente.";
// Reached only outside a supported interaction, so it offers no next step.
const UNSUPPORTED_CONTEXT: &str = "Não foi possível abrir a pasta da imagem.";

#[tauri::command]
pub(crate) async fn show_media_in_folder(
    media_id: String,
    window: WebviewWindow,
    app: AppHandle,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(UNSUPPORTED_CONTEXT.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let catalog = app
            .state::<ProjectHost>()
            .authorized_media_catalog()
            .map_err(|error| {
                tracing::warn!(target: "myalbuns.desktop", error = %error, "show_media_in_folder_without_project");
                UNSUPPORTED_CONTEXT.to_string()
            })?;
        let binding = catalog
            .bindings
            .into_iter()
            .find(|binding| binding.media_id == media_id)
            .ok_or_else(|| MEDIA_REMOVED.to_string())?;
        show_file_in_folder(&media_id, &binding.logical_path)
    })
    .await
    .map_err(|_| UNSUPPORTED_CONTEXT.to_string())?
}

fn show_file_in_folder(media_id: &str, logical_path: &Path) -> Result<(), String> {
    let mut context = OperationPathContext::new();
    let original = context
        .resolve_existing(logical_path, ExpectedObject::RegularFile)
        .map_err(|error| {
            tracing::warn!(target: "myalbuns.desktop", media_id, error = %error, "show_media_in_folder_unresolved");
            match error {
                ResolveError::NotFound => ORIGINAL_ABSENT,
                _ => ORIGINAL_UNAVAILABLE,
            }
            .to_string()
        })?;
    // The Shell does not accept the verbatim operational form, so it receives the logical path.
    select_in_explorer(original.logical_path()).map_err(|error| {
        tracing::warn!(target: "myalbuns.desktop", media_id, error = %error, "show_media_in_folder_shell_failed");
        SHELL_FAILED.to_string()
    })
}

#[cfg(windows)]
fn select_in_explorer(path: &Path) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{
            System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems},
        },
        core::PCWSTR,
    };

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: COM is balanced on this blocking thread, `wide` is NUL-terminated and
    // outlives the call, and the item list is freed after the Shell has used it.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let item = ILCreateFromPathW(PCWSTR(wide.as_ptr()));
        let result = if item.is_null() {
            Err(windows::core::Error::from_win32())
        } else {
            let shown = SHOpenFolderAndSelectItems(item, None, 0);
            ILFree(Some(item));
            shown
        };
        if initialized {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
fn select_in_explorer(_path: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_original_points_to_locate_image() {
        let directory = tempfile::tempdir().unwrap();
        let error =
            show_file_in_folder("media-1", &directory.path().join("ausente.jpg")).unwrap_err();
        assert_eq!(error, ORIGINAL_ABSENT);
    }

    #[test]
    fn folder_in_place_of_original_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let error = show_file_in_folder("media-1", directory.path()).unwrap_err();
        assert_eq!(error, ORIGINAL_UNAVAILABLE);
    }
}
