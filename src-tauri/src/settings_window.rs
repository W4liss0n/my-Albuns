use std::{
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::IsWindowEnabled,
        WindowsAndMessaging::{
            GetForegroundWindow, IsWindow, IsWindowVisible, SetForegroundWindow,
        },
    },
};

use crate::{desktop_webview_policy, ipc_contract::SettingsSection, named_mutex::NamedMutexGrant};

pub(crate) const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_SECTION_EVENT: &str = "myalbuns://settings-section";
/// The opener's process lifts its block on its next modality pass (80 ms).
const OPENER_RELEASE_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) struct SettingsWindowState {
    serial: tokio::sync::Mutex<()>,
    paths: AppPaths,
    open: Mutex<Option<OpenSettings>>,
}

/// The open Settings window's block on every other window, and the window
/// that was active when Settings opened.
struct OpenSettings {
    reservation: Arc<Mutex<Option<NamedMutexGrant>>>,
    opener: Option<isize>,
}

impl SettingsWindowState {
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self {
            serial: tokio::sync::Mutex::new(()),
            paths,
            open: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub(crate) async fn open_application_settings(
    app: AppHandle,
    section: SettingsSection,
) -> Result<(), String> {
    if app.try_state::<SettingsWindowState>().is_some() {
        return show(&app, section).await;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let argument = match section {
            SettingsSection::Performance => "--myalbuns-settings=performance",
            SettingsSection::Photoshop => "--myalbuns-settings=photoshop",
        };
        let child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .arg(argument)
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Não foi possível abrir Configurações.".to_owned())?;
        drop(child);
        Ok(())
    })
    .await
    .map_err(|_| "Não foi possível abrir Configurações.".to_owned())?
}

pub(crate) async fn show(app: &AppHandle, section: SettingsSection) -> Result<(), String> {
    let state = app.state::<SettingsWindowState>();
    let _serial = state.serial.lock().await;
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        window
            .emit(SETTINGS_SECTION_EVENT, section)
            .map_err(|error| error.to_string())?;
        let _ = window.unminimize();
        window.show().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }
    let section = match section {
        SettingsSection::Photoshop => "photoshop",
        SettingsSection::Performance => "performance",
    };
    // SAFETY: reads the system foreground window; a null result means none.
    let foreground = unsafe { GetForegroundWindow() };
    let opener = (!foreground.0.is_null()).then_some(foreground.0 as isize);
    let reservation = app
        .state::<crate::application_modality::ApplicationModality>()
        .reserve()
        .await?;
    #[cfg(debug_assertions)]
    let arguments = desktop_webview_policy::global_webview_debug_arguments()
        .map_err(|error| error.to_string())?;
    #[cfg(not(debug_assertions))]
    let arguments: Option<String> = None;
    let (signal, readiness) = desktop_webview_policy::page_load_handshake(arguments.as_deref());
    let builder = WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW_LABEL,
        WebviewUrl::App(format!("global.html?surface=settings&section={section}").into()),
    )
    .title("Configurações")
    .inner_size(720.0, 440.0)
    .min_inner_size(480.0, 440.0)
    .resizable(true)
    .maximizable(false)
    .minimizable(false)
    .visible(false)
    .focused(false)
    .decorations(false)
    .data_directory(
        state
            .paths
            .webview_data_directory("global")
            .map_err(|error| error.to_string())?,
    )
    .center()
    .prevent_overflow();
    #[cfg(debug_assertions)]
    let builder = match arguments {
        Some(arguments) => builder.additional_browser_args(&arguments),
        None => builder,
    };
    let window = builder
        .on_page_load(move |window, payload| signal.observe(&window, payload.event()))
        .build()
        .map_err(|error| error.to_string())?;
    let reservation = Arc::new(Mutex::new(Some(reservation)));
    let held = Arc::clone(&reservation);
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            held.lock().unwrap_or_else(PoisonError::into_inner).take();
        }
    });
    if let Err(error) = readiness.wait().await {
        let _ = window.destroy();
        return Err(error.to_string());
    }
    if let Err(error) = window.show().and_then(|()| window.set_focus()) {
        let _ = window.destroy();
        return Err(error.to_string());
    }
    *state.open.lock().unwrap_or_else(PoisonError::into_inner) = Some(OpenSettings {
        reservation,
        opener,
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn close_application_settings(window: WebviewWindow) -> Result<(), String> {
    if window.label() != SETTINGS_WINDOW_LABEL {
        return Err("Janela de Configurações inválida.".into());
    }
    window.close().map_err(|error| error.to_string())?;
    Ok(())
}

/// Closes Settings (its X, Alt+F4 or the taskbar) and gives activation back to
/// the window it was opened from. Windows never activates a disabled window:
/// destroying Settings while the Album windows are still blocked handed
/// activation to another application, which hid the editor behind it. As with
/// owned dialogs, the block is lifted before Settings is destroyed.
pub(crate) async fn close(app: AppHandle) {
    let state = app.state::<SettingsWindowState>();
    let _serial = state.serial.lock().await;
    let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) else {
        return;
    };
    let open = state
        .open
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    // Only an active Settings hands activation on; otherwise the person has
    // already moved elsewhere.
    let active = window
        .hwnd()
        // SAFETY: reads the system foreground window.
        .is_ok_and(|handle| unsafe { GetForegroundWindow() } == handle);
    let opener = open.as_ref().and_then(|open| open.opener);
    if let Some(open) = open {
        drop(
            open.reservation
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
    }
    let _ = crate::application_modality::synchronize(&app).await;
    if active && let Some(opener) = opener {
        activate_when_released(&app, opener).await;
    }
    let _ = window.destroy();
}

/// Activates the opener once its own process has lifted the block.
async fn activate_when_released(app: &AppHandle, opener: isize) {
    let deadline = Instant::now() + OPENER_RELEASE_TIMEOUT;
    loop {
        let handle = HWND(opener as *mut _);
        // SAFETY: plain queries on a handle that may already be gone.
        let present =
            unsafe { IsWindow(Some(handle)).as_bool() && IsWindowVisible(handle).as_bool() };
        if !present {
            return;
        }
        // SAFETY: as above.
        if unsafe { IsWindowEnabled(handle) }.as_bool() {
            break;
        }
        if Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    if app
        .run_on_main_thread(move || {
            // SAFETY: Settings is still the foreground window of this process,
            // so the system lets it pass activation on.
            let _ = unsafe { SetForegroundWindow(HWND(opener as *mut _)) };
            let _ = sender.send(());
        })
        .is_ok()
    {
        let _ = receiver.await;
    }
}
