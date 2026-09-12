use std::{
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{desktop_webview_policy, ipc_contract::SettingsSection};

pub(crate) const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_SECTION_EVENT: &str = "myalbuns://settings-section";

pub(crate) struct SettingsWindowState {
    serial: tokio::sync::Mutex<()>,
    paths: AppPaths,
}

impl SettingsWindowState {
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self {
            serial: tokio::sync::Mutex::new(()),
            paths,
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
    let (signal, readiness) = desktop_webview_policy::page_load_handshake();
    let window = WebviewWindowBuilder::new(
        app,
        SETTINGS_WINDOW_LABEL,
        WebviewUrl::App(format!("global.html?surface=settings&section={section}").into()),
    )
    .title("Configurações — MyAlbuns")
    .inner_size(640.0, 520.0)
    .min_inner_size(480.0, 440.0)
    .resizable(true)
    .maximizable(false)
    .visible(false)
    .decorations(false)
    .data_directory(
        state
            .paths
            .webview_data_directory("global")
            .map_err(|error| error.to_string())?,
    )
    .center()
    .prevent_overflow()
    .on_page_load(move |window, payload| signal.observe(&window, payload.event()))
    .build()
    .map_err(|error| error.to_string())?;
    if let Err(error) = readiness.wait().await {
        let _ = window.destroy();
        return Err(error.to_string());
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn close_application_settings(window: WebviewWindow) -> Result<(), String> {
    if window.label() != SETTINGS_WINDOW_LABEL {
        return Err("Janela de Configurações inválida.".into());
    }
    window.close().map_err(|error| error.to_string())?;
    Ok(())
}
