//! The editor delegates new sessions to the existing Global bootstrap.
//! No command here owns or mutates the current ProjectHost.
use std::{
    ffi::OsString,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

#[derive(Default)]
pub(crate) struct EditorProjectLauncher(tokio::sync::Mutex<()>);

fn require_editor(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "project" {
        return Err("Abra outro Projeto a partir do editor.".into());
    }
    Ok(())
}

async fn launch_global(argument: OsString) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .arg(argument)
            // An automation host path belongs only to the originating editor.
            .env_remove("MYALBUNS_TAURI_WEBDRIVER_PROJECT")
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Não foi possível abrir outro Projeto. Tente novamente.".to_owned())?;
        drop(child);
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn new_project_from_editor(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<(), String> {
    require_editor(&window)?;
    let state = app.state::<EditorProjectLauncher>();
    let Ok(_pending) = state.0.try_lock() else {
        return Ok(());
    };
    let _operation = app
        .state::<crate::project_ui_operations::ProjectUiOperations>()
        .begin()?;
    launch_global(crate::runtime_role::NEW_PROJECT_ARGUMENT.into()).await
}

#[tauri::command]
pub(crate) async fn open_project_from_editor(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<(), String> {
    require_editor(&window)?;
    let state = app.state::<EditorProjectLauncher>();
    let Ok(_pending) = state.0.try_lock() else {
        return Ok(());
    };
    let _operation = app
        .state::<crate::project_ui_operations::ProjectUiOperations>()
        .begin()?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .add_filter("Projeto MyAlbuns", &["myalbuns"])
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível concluir o diálogo de abertura.")?;
    match opening_argument(selection)? {
        None => Ok(()),
        Some(argument) => launch_global(argument).await,
    }
}

fn opening_argument(selection: Option<FilePath>) -> Result<Option<OsString>, String> {
    match selection {
        None => Ok(None),
        Some(FilePath::Path(path))
            if path.is_absolute()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("myalbuns")) =>
        {
            Ok(Some(path.into_os_string()))
        }
        Some(_) => Err("Escolha um arquivo .myalbuns local ou de uma unidade disponível.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_does_not_launch_a_global_and_native_paths_remain_intact() {
        assert_eq!(opening_argument(None), Ok(None));
        for path in [
            r"C:\Álbuns\João e Ana.MYALBUNS",
            r"\\servidor\Álbuns\Turma.myalbuns",
        ] {
            assert_eq!(
                opening_argument(Some(FilePath::Path(path.into()))),
                Ok(Some(path.into()))
            );
        }
        for invalid in [
            r"C:\Álbuns\foto.jpg",
            "--myalbuns-new-project",
            "relative.myalbuns",
        ] {
            assert!(opening_argument(Some(FilePath::Path(invalid.into()))).is_err());
        }
    }
}
