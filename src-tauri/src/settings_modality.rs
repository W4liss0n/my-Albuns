//! Application-wide Settings modality across the Global and Project Hosts.
//! Each process disables its own windows. A kernel-owned reservation ensures
//! that even a crashed Global cannot leave other processes permanently blocked.
use std::{collections::HashMap, sync::Mutex, time::Duration};

use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Manager, WebviewWindow, Window, WindowEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

use crate::{
    named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant},
    settings_window::SETTINGS_WINDOW_LABEL,
};

pub(crate) struct SettingsModality {
    gate: NamedMutex,
    blocked: Mutex<HashMap<isize, (WebviewWindow, bool)>>,
}

impl SettingsModality {
    pub(crate) fn new(paths: &AppPaths) -> Self {
        Self {
            gate: NamedMutex::scoped(
                paths,
                "settings-modal",
                "application",
                "settings-modal-owner",
            ),
            blocked: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn reserve(&self) -> Result<NamedMutexGrant, String> {
        // A non-owning observer can briefly acquire the free kernel mutex.
        // Settings requests themselves are serialized by SettingsWindowState.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match self.gate.try_acquire() {
                Ok(grant) => return Ok(grant),
                Err(NamedMutexError::Conflict) if std::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => {
                    return Err(format!("Não foi possível abrir Configurações: {error:?}"));
                }
            }
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        // A failed observation must not permit a project to close under Settings.
        self.gate.is_owned().unwrap_or(true)
    }

    /// Called only on the window thread, including restoration after close.
    fn synchronize(&self, app: &AppHandle) {
        let active = self.is_active();
        let mut blocked = self
            .blocked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !active {
            for (_, (window, was_enabled)) in blocked.drain() {
                if was_enabled {
                    let _ = window.set_enabled(true);
                }
            }
            return;
        }
        let windows = app.webview_windows();
        blocked.retain(|_, (window, _)| windows.contains_key(window.label()));
        for window in windows
            .values()
            .filter(|window| window.label() != SETTINGS_WINDOW_LABEL)
        {
            let Ok(handle) = window.hwnd() else { continue };
            // SAFETY: the live WebviewWindow supplied this HWND on its UI thread.
            let enabled = unsafe { IsWindowEnabled(handle) }.as_bool();
            blocked
                .entry(handle.0 as isize)
                .or_insert_with(|| (window.clone(), enabled));
            if enabled {
                let _ = window.set_enabled(false);
            }
        }
    }
}

pub(crate) fn install(app: &AppHandle, paths: &AppPaths) {
    app.manage(SettingsModality::new(paths));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let (done, completed) = tokio::sync::oneshot::channel();
            let current = app.clone();
            if app
                .run_on_main_thread(move || {
                    current.state::<SettingsModality>().synchronize(&current);
                    let _ = done.send(());
                })
                .is_err()
            {
                break;
            }
            if completed.await.is_err() {
                break;
            }
            // Also covers projects launched externally while Settings is open,
            // and restores input after an unexpected Global process exit.
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    });
}

pub(crate) fn blocks(window: &Window) -> bool {
    window.label() != SETTINGS_WINDOW_LABEL
        && window
            .try_state::<SettingsModality>()
            .is_some_and(|state| state.is_active())
}

pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) -> bool {
    if let WindowEvent::CloseRequested { api, .. } = event
        && blocks(window)
    {
        api.prevent_close();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        process::{Command, Stdio},
        time::Instant,
    };

    #[test]
    fn settings_reservation_blocks_other_hosts_and_releases_on_close() {
        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().unwrap();
            let paths = AppPaths::from_roots(root.path(), root.path());
            let settings =
                NamedMutex::scoped(&paths, "settings-modal", "application", "test-settings");
            let host = NamedMutex::scoped(&paths, "settings-modal", "application", "test-settings");
            assert!(!host.is_owned().unwrap());
            let reservation = settings.try_acquire().unwrap();
            assert!(host.is_owned().unwrap());
            assert!(settings.is_owned().unwrap());
            let independent = tempfile::tempdir().unwrap();
            let independent = NamedMutex::scoped(
                &AppPaths::from_roots(independent.path(), independent.path()),
                "settings-modal",
                "application",
                "test-settings",
            );
            assert!(!independent.is_owned().unwrap());
            drop(reservation);
            assert!(!host.is_owned().unwrap());
            assert!(!settings.is_owned().unwrap());
        });
    }

    #[test]
    fn settings_process_failure_releases_other_hosts() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let host = NamedMutex::scoped(&paths, "settings-modal", "application", "test-settings");
        let ready = root.path().join("ready");
        let mut child = Command::new(env::current_exe().unwrap())
            .args([
                "settings_modality::tests::settings_modal_owner_process",
                "--exact",
                "--ignored",
            ])
            .env("MYALBUNS_TEST_SETTINGS_MODAL_ROOT", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline && child.try_wait().unwrap().is_none() {
            std::thread::sleep(Duration::from_millis(10));
        }
        let was_ready = ready.exists();
        let was_blocked = host.is_owned().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(was_ready);
        assert!(was_blocked);
        assert!(
            !host.is_owned().unwrap(),
            "an abandoned Settings process cannot strand a Project"
        );
    }

    #[test]
    #[ignore = "independent process fixture"]
    fn settings_modal_owner_process() {
        tauri::async_runtime::block_on(async {
            let root =
                std::path::PathBuf::from(env::var_os("MYALBUNS_TEST_SETTINGS_MODAL_ROOT").unwrap());
            let state = NamedMutex::scoped(
                &AppPaths::from_roots(&root, &root),
                "settings-modal",
                "application",
                "test-settings",
            );
            let _reservation = state.try_acquire().unwrap();
            std::fs::write(root.join("ready"), b"ready").unwrap();
            std::thread::sleep(Duration::from_secs(30));
        });
    }
}
