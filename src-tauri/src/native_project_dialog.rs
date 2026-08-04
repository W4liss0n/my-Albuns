use std::path::PathBuf;

#[cfg(windows)]
use tauri::Manager;

use crate::project_bootstrap::CreateWriteAuthorization;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectSaveDialogOutcome {
    Cancelled,
    Selected {
        path: PathBuf,
        authorization: CreateWriteAuthorization,
    },
}

#[derive(Debug)]
pub(crate) enum NativeProjectDialogError {
    #[cfg(windows)]
    GlobalWindowUnavailable,
    #[cfg(windows)]
    NativeWindowUnavailable(tauri::Error),
    #[cfg(windows)]
    DialogThreadUnavailable(String),
    #[cfg(windows)]
    Windows(windows::core::Error),
    #[cfg(not(windows))]
    UnsupportedPlatform,
}

impl std::fmt::Display for NativeProjectDialogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(windows)]
            Self::GlobalWindowUnavailable => {
                formatter.write_str("a janela global não está disponível")
            }
            #[cfg(windows)]
            Self::NativeWindowUnavailable(error) => {
                write!(
                    formatter,
                    "o HWND da janela global não está disponível: {error}"
                )
            }
            #[cfg(windows)]
            Self::DialogThreadUnavailable(error) => {
                write!(formatter, "a thread do diálogo nativo falhou: {error}")
            }
            #[cfg(windows)]
            Self::Windows(error) => write!(formatter, "o diálogo nativo falhou: {error}"),
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => {
                formatter.write_str("o diálogo de criação só está disponível no Windows")
            }
        }
    }
}

impl std::error::Error for NativeProjectDialogError {}

#[cfg(windows)]
impl From<windows::core::Error> for NativeProjectDialogError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error)
    }
}

pub(crate) async fn choose_project_destination(
    app: &tauri::AppHandle,
) -> Result<ProjectSaveDialogOutcome, NativeProjectDialogError> {
    #[cfg(windows)]
    {
        let window = app
            .get_webview_window(crate::global_runtime::GLOBAL_WINDOW_LABEL)
            .ok_or(NativeProjectDialogError::GlobalWindowUnavailable)?;
        let owner = window
            .hwnd()
            .map_err(NativeProjectDialogError::NativeWindowUnavailable)?
            .0 as isize;

        tauri::async_runtime::spawn_blocking(move || show_project_save_dialog(owner))
            .await
            .map_err(|error| NativeProjectDialogError::DialogThreadUnavailable(error.to_string()))?
    }

    #[cfg(not(windows))]
    {
        let _ = app;
        Err(NativeProjectDialogError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod windows_dialog {
    use std::{cell::RefCell, ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, rc::Rc};

    use windows::{
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            UI::{
                Shell::{
                    Common::COMDLG_FILTERSPEC, FDE_OVERWRITE_RESPONSE, FDE_SHAREVIOLATION_RESPONSE,
                    FDEOR_ACCEPT, FDEOR_REFUSE, FDESVR_DEFAULT, FOS_FORCEFILESYSTEM,
                    FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FOS_STRICTFILETYPES, FileSaveDialog,
                    IFileDialog, IFileDialogEvents, IFileDialogEvents_Impl, IFileSaveDialog,
                    IShellItem, SICHINT_CANONICAL, SIGDN_FILESYSPATH,
                },
                WindowsAndMessaging::{
                    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MessageBoxW,
                },
            },
        },
        core::{HRESULT, Interface, Ref, w},
    };

    use super::{CreateWriteAuthorization, NativeProjectDialogError, ProjectSaveDialogOutcome};

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self, windows::core::Error> {
            // SAFETY: this worker thread owns the STA for the complete lifetime of the dialog.
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            result.ok()?;
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: `initialize` succeeded on this same worker thread.
            unsafe { CoUninitialize() };
        }
    }

    #[windows::core::implement(IFileDialogEvents)]
    struct SaveDialogEvents {
        owner: isize,
        confirmed_replacement: Rc<RefCell<Option<IShellItem>>>,
    }

    impl SaveDialogEvents {
        fn new(owner: isize, confirmed_replacement: Rc<RefCell<Option<IShellItem>>>) -> Self {
            Self {
                owner,
                confirmed_replacement,
            }
        }

        fn remember_confirmed_replacement(&self, item: Option<IShellItem>) {
            *self.confirmed_replacement.borrow_mut() = item;
        }
    }

    impl IFileDialogEvents_Impl for SaveDialogEvents_Impl {
        fn OnFileOk(&self, _dialog: Ref<'_, IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnFolderChanging(
            &self,
            _dialog: Ref<'_, IFileDialog>,
            _folder: Ref<'_, IShellItem>,
        ) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnFolderChange(&self, _dialog: Ref<'_, IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnSelectionChange(&self, _dialog: Ref<'_, IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnShareViolation(
            &self,
            _dialog: Ref<'_, IFileDialog>,
            _item: Ref<'_, IShellItem>,
        ) -> windows::core::Result<FDE_SHAREVIOLATION_RESPONSE> {
            Ok(FDESVR_DEFAULT)
        }

        fn OnTypeChange(&self, _dialog: Ref<'_, IFileDialog>) -> windows::core::Result<()> {
            Ok(())
        }

        fn OnOverwrite(
            &self,
            _dialog: Ref<'_, IFileDialog>,
            item: Ref<'_, IShellItem>,
        ) -> windows::core::Result<FDE_OVERWRITE_RESPONSE> {
            // SAFETY: the text is statically NUL-terminated and the HWND belongs to the global
            // window that owns the save dialog. No pointer escapes this callback.
            let response = unsafe {
                MessageBoxW(
                    Some(HWND(self.owner as *mut _)),
                    w!("Já existe um arquivo com este nome. Deseja substituí-lo?"),
                    w!("Substituir Projeto MyAlbuns"),
                    MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                )
            };
            let confirmed = response == IDYES;
            let confirmed_item = if confirmed {
                Some(item.ok()?.clone())
            } else {
                None
            };
            self.remember_confirmed_replacement(confirmed_item);

            Ok(overwrite_response(confirmed))
        }
    }

    fn overwrite_response(confirmed: bool) -> FDE_OVERWRITE_RESPONSE {
        if confirmed {
            FDEOR_ACCEPT
        } else {
            FDEOR_REFUSE
        }
    }

    pub(super) fn show_project_save_dialog(
        owner: isize,
    ) -> Result<ProjectSaveDialogOutcome, NativeProjectDialogError> {
        let _apartment = ComApartment::initialize()?;
        // SAFETY: COM is initialized as an STA on this thread; the resulting interfaces never
        // leave it and are released before `ComApartment` is dropped.
        let save_dialog: IFileSaveDialog =
            unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)? };
        let dialog: IFileDialog = save_dialog.cast()?;

        let filters = [COMDLG_FILTERSPEC {
            pszName: w!("Projeto MyAlbuns (*.myalbuns)"),
            pszSpec: w!("*.myalbuns"),
        }];
        // SAFETY: all strings and filter storage remain valid through these synchronous COM calls.
        unsafe {
            dialog.SetFileTypes(&filters)?;
            dialog.SetFileTypeIndex(1)?;
            dialog.SetDefaultExtension(w!("myalbuns"))?;
            dialog.SetFileName(w!("Novo Projeto.myalbuns"))?;
            dialog.SetTitle(w!("Criar Projeto MyAlbuns"))?;
            dialog.SetOkButtonLabel(w!("Criar"))?;
            let options = dialog.GetOptions()?;
            dialog.SetOptions(
                options
                    | FOS_FORCEFILESYSTEM
                    | FOS_PATHMUSTEXIST
                    | FOS_OVERWRITEPROMPT
                    | FOS_STRICTFILETYPES,
            )?;
        }

        let confirmed_replacement = Rc::new(RefCell::new(None));
        let events = SaveDialogEvents::new(owner, Rc::clone(&confirmed_replacement));
        let events_interface: IFileDialogEvents = events.into();
        // SAFETY: `events_interface` remains alive until after `Unadvise`.
        let cookie = unsafe { dialog.Advise(&events_interface)? };
        // SAFETY: `owner` is the HWND captured from the live global Tauri window.
        let shown = unsafe { dialog.Show(Some(HWND(owner as *mut _))) };
        // SAFETY: `cookie` was returned by `Advise` on this dialog.
        let unadvised = unsafe { dialog.Unadvise(cookie) };

        match shown {
            Err(error) if is_cancelled(&error) => return Ok(ProjectSaveDialogOutcome::Cancelled),
            Err(error) => return Err(error.into()),
            Ok(()) => unadvised?,
        }

        // SAFETY: the successful modal result remains owned by this STA until it is converted.
        let result = unsafe { dialog.GetResult()? };
        let confirmed_item = confirmed_replacement.borrow().clone();
        let authorization = match confirmed_item {
            Some(confirmed_item) => {
                // SAFETY: both shell items are live COM interfaces on this STA.
                let comparison =
                    unsafe { confirmed_item.Compare(&result, SICHINT_CANONICAL.0 as u32)? };
                if comparison == 0 {
                    CreateWriteAuthorization::ReplaceConfirmed
                } else {
                    CreateWriteAuthorization::CreateOnly
                }
            }
            None => CreateWriteAuthorization::CreateOnly,
        };
        let path = shell_item_path(&result)?;

        Ok(ProjectSaveDialogOutcome::Selected {
            path,
            authorization,
        })
    }

    fn shell_item_path(item: &IShellItem) -> Result<PathBuf, windows::core::Error> {
        // SAFETY: `GetDisplayName` returns a COM-allocated, NUL-terminated UTF-16 string.
        let native_path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
        // SAFETY: the returned pointer remains valid until `CoTaskMemFree` below.
        let path = PathBuf::from(OsString::from_wide(unsafe { native_path.as_wide() }));
        // SAFETY: the pointer was allocated by the shell for the caller.
        unsafe { CoTaskMemFree(Some(native_path.as_ptr().cast())) };
        Ok(path)
    }

    fn is_cancelled(error: &windows::core::Error) -> bool {
        error.code() == HRESULT::from_win32(ERROR_CANCELLED.0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn only_an_explicit_native_yes_accepts_overwrite() {
            assert_eq!(overwrite_response(true), FDEOR_ACCEPT);
            assert_eq!(overwrite_response(false), FDEOR_REFUSE);
        }

        #[test]
        fn windows_cancel_hresult_is_distinct_from_dialog_failure() {
            let cancelled =
                windows::core::Error::from_hresult(HRESULT::from_win32(ERROR_CANCELLED.0));
            assert!(is_cancelled(&cancelled));
            assert!(!is_cancelled(&windows::core::Error::from_hresult(HRESULT(
                0x80004005_u32 as i32
            ))));
        }
    }
}

#[cfg(windows)]
use windows_dialog::show_project_save_dialog;
