use std::sync::{Arc, Mutex};

#[cfg(debug_assertions)]
use std::{ffi::OsString, io, path::PathBuf};

use tauri::{WebviewWindow, webview::PageLoadEvent};

#[derive(Default)]
pub(crate) struct WindowWebviewVisibility {
    #[cfg(windows)]
    minimized: Mutex<std::collections::HashMap<String, MinimizedWebviews>>,
}

#[cfg(windows)]
#[derive(Default)]
struct MinimizedWebviews {
    restore: std::collections::HashMap<String, usize>,
}

#[cfg(windows)]
impl MinimizedWebviews {
    fn synchronize<E>(
        &mut self,
        label: &str,
        controller_id: usize,
        minimized: bool,
        is_visible: impl FnOnce() -> Result<bool, E>,
        set_visible: impl FnOnce(bool) -> Result<(), E>,
    ) -> Result<(), E> {
        // Save As can reuse a label after replacing its controller. A page-load
        // notification from the same controller must retain its pending restore.
        if self
            .restore
            .get(label)
            .is_some_and(|id| *id != controller_id)
        {
            self.restore.remove(label);
        }
        if minimized {
            if !self.restore.contains_key(label) && is_visible()? {
                set_visible(false)?;
                self.restore.insert(label.to_owned(), controller_id);
            }
        } else if self.restore.contains_key(label) {
            set_visible(true)?;
            self.restore.remove(label);
        }
        Ok(())
    }
}

pub(crate) fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    #[cfg(windows)]
    match event {
        tauri::WindowEvent::Resized(_) => {
            // Tao reports Windows minimize/restore through Resized. WebView2
            // does not receive those messages from its top-level parent.
            for webview in window.webviews() {
                synchronize_webview_visibility(&webview);
            }
        }
        tauri::WindowEvent::Destroyed => {
            let state = window.state::<WindowWebviewVisibility>();
            if let Ok(mut minimized) = state.minimized.lock() {
                minimized.remove(window.label());
            }
        }
        _ => {}
    }
    #[cfg(not(windows))]
    let _ = (window, event);
}

#[cfg(windows)]
fn synchronize_webview_visibility(webview: &tauri::Webview) {
    let window = webview.window();
    let app = webview.app_handle().clone();
    let label = webview.label().to_owned();
    let result = webview.with_webview(move |native| {
        let synchronize = || -> Result<(), Box<dyn std::error::Error>> {
            let is_minimized = window.is_minimized()?;
            let state = app.state::<WindowWebviewVisibility>();
            let mut windows = state.minimized.lock().map_err(|_| {
                std::io::Error::other("the WebView visibility state became unavailable")
            })?;
            let minimized = windows.entry(window.label().to_owned()).or_default();
            let controller = native.controller();
            minimized.synchronize(
                &label,
                controller.as_raw() as usize,
                is_minimized,
                || unsafe {
                    let mut visible = windows::core::BOOL::default();
                    controller.IsVisible(&mut visible)?;
                    Ok::<_, windows::core::Error>(visible.as_bool())
                },
                |visible| unsafe { controller.SetIsVisible(visible) },
            )?;
            Ok(())
        };
        if let Err(error) = synchronize() {
            tracing::warn!(
                target: "myalbuns.desktop",
                window_label = window.label(),
                webview_label = label,
                error = %error,
                event = "webview_visibility_sync_failed",
            );
        }
    });
    if let Err(error) = result {
        tracing::warn!(
            target: "myalbuns.desktop",
            webview_label = webview.label(),
            error = %error,
            event = "webview_visibility_dispatch_failed",
        );
    }
}

const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";
#[cfg(debug_assertions)]
pub(crate) const SAVE_AS_WEBVIEW_DEBUG_PORT_ENV: &str = "MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT";
#[cfg(debug_assertions)]
pub(crate) const PROJECT_DIALOG_WEBVIEW_DEBUG_PORT_ENV: &str =
    "MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DEBUG_PORT";
#[cfg(debug_assertions)]
pub(crate) const PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY_ENV: &str =
    "MYALBUNS_DEV_PROJECT_DIALOG_WEBVIEW_DATA_DIRECTORY";

#[cfg(debug_assertions)]
const WRY_DEFAULT_DISABLED_FEATURES: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

#[cfg(windows)]
use {
    tauri::Manager, webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3,
    windows::core::Interface,
};

#[derive(Clone)]
pub(crate) struct WebviewPolicyLoadSignal {
    sender: Arc<Mutex<Option<tokio::sync::oneshot::Sender<std::io::Result<()>>>>>,
}

pub(crate) struct WebviewPolicyReadiness {
    receiver: tokio::sync::oneshot::Receiver<std::io::Result<()>>,
}

pub(crate) fn page_load_handshake() -> (WebviewPolicyLoadSignal, WebviewPolicyReadiness) {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    (
        WebviewPolicyLoadSignal {
            sender: Arc::new(Mutex::new(Some(sender))),
        },
        WebviewPolicyReadiness { receiver },
    )
}

impl WebviewPolicyLoadSignal {
    pub(crate) fn observe(&self, window: &WebviewWindow, event: PageLoadEvent) {
        self.observe_webview(window.as_ref(), event);
    }

    pub(crate) fn observe_webview(&self, webview: &tauri::Webview, event: PageLoadEvent) {
        if event != PageLoadEvent::Finished {
            return;
        }
        let sender = self.sender.lock().ok().and_then(|mut sender| sender.take());
        if let Some(sender) = sender {
            let _ = sender.send(enforce_webview(webview));
        }
    }
}

impl WebviewPolicyReadiness {
    pub(crate) async fn wait(self) -> std::io::Result<()> {
        self.receiver
            .await
            .map_err(|_| std::io::Error::other("the native WebView policy became unavailable"))?
    }
}

pub(crate) fn enforce_webview(webview: &tauri::Webview) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        webview
            .with_webview(move |webview| {
                let result = enforce_windows_policy(&webview).map_err(|error| error.to_string());
                let _ = sender.send(result);
            })
            .map_err(std::io::Error::other)?;

        receiver
            .try_recv()
            .map_err(|_| {
                std::io::Error::other("the WebView did not become available on the main thread")
            })?
            .map_err(|error| {
                std::io::Error::other(format!(
                    "could not apply the native WebView policy: {error}"
                ))
            })?;
        synchronize_webview_visibility(webview);
    }

    #[cfg(not(windows))]
    let _ = webview;

    Ok(())
}

pub(crate) fn automation_enabled() -> bool {
    cfg!(debug_assertions) && std::env::var_os(TAURI_WEBVIEW_AUTOMATION_ENV).is_some()
}

#[cfg(debug_assertions)]
pub(crate) fn remote_debugging_argument(port: Option<OsString>) -> io::Result<Option<OsString>> {
    let Some(port) = port else {
        return Ok(None);
    };
    let port = port
        .to_str()
        .and_then(|port| port.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid WebView debug port"))?;
    Ok(Some(OsString::from(format!(
        "--remote-debugging-port={port}"
    ))))
}

#[cfg(debug_assertions)]
pub(crate) fn replacement_webview_debug_arguments(
    port: Option<OsString>,
) -> io::Result<Option<String>> {
    remote_debugging_argument(port).map(|argument| {
        argument.map(|argument| {
            format!(
                "{WRY_DEFAULT_DISABLED_FEATURES} {}",
                argument.to_string_lossy()
            )
        })
    })
}

#[cfg(debug_assertions)]
pub(crate) fn project_dialog_debug_data_directory(
    directory: Option<OsString>,
) -> io::Result<Option<PathBuf>> {
    let Some(directory) = directory else {
        return Ok(None);
    };
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the Project dialog WebView data directory must be absolute",
        ));
    }
    Ok(Some(directory))
}

#[cfg(debug_assertions)]
pub(crate) fn retire_inherited_debug_arguments_before_replacement() -> io::Result<()> {
    if std::env::var_os(SAVE_AS_WEBVIEW_DEBUG_PORT_ENV).is_none()
        && std::env::var_os(PROJECT_DIALOG_WEBVIEW_DEBUG_PORT_ENV).is_none()
    {
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        let name = std::ffi::OsStr::new("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS")
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let succeeded = unsafe {
            windows_sys::Win32::System::Environment::SetEnvironmentVariableW(
                name.as_ptr(),
                std::ptr::null(),
            )
        };
        if succeeded == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    #[cfg(windows)]
    #[test]
    fn minimize_restore_resumes_only_the_webviews_hidden_by_minimization() {
        use std::cell::Cell;

        let mut state = super::MinimizedWebviews::default();
        let visible = Cell::new(true);
        let changes = Cell::new(0);
        let mut synchronize = |minimized| {
            state
                .synchronize(
                    "project",
                    1,
                    minimized,
                    || Ok::<_, ()>(visible.get()),
                    |next| {
                        visible.set(next);
                        changes.set(changes.get() + 1);
                        Ok(())
                    },
                )
                .unwrap();
        };
        synchronize(false); // Ordinary resize/maximize must not toggle the WebView.
        assert_eq!(changes.get(), 0);
        for cycle in 1..=3 {
            synchronize(true);
            assert!(
                !visible.get(),
                "minimizing must notify WebView2 that it is hidden"
            );
            synchronize(true); // Duplicate size notifications must retain the restore decision.
            synchronize(false);
            assert!(
                visible.get(),
                "restoring must resume the same WebView2 controller"
            );
            synchronize(false);
            assert_eq!(changes.get(), cycle * 2);
        }

        let mut hidden = super::MinimizedWebviews::default();
        for minimized in [false, true, true, false] {
            hidden
                .synchronize(
                    "preflight",
                    2,
                    minimized,
                    || Ok::<_, ()>(false),
                    |_| {
                        panic!("a WebView already hidden by its owner must remain hidden");
                    },
                )
                .unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_failed_visibility_change_can_be_retried() {
        let mut state = super::MinimizedWebviews::default();
        assert!(
            state
                .synchronize("project", 1, true, || Ok(true), |_| Err("hide"))
                .is_err()
        );
        assert!(!state.restore.contains_key("project"));
        state
            .synchronize("project", 1, true, || Ok::<_, &str>(true), |_| Ok(()))
            .unwrap();
        assert!(
            state
                .synchronize("project", 1, false, || Ok(false), |_| Err("show"))
                .is_err()
        );
        assert!(state.restore.contains_key("project"));
        state
            .synchronize("project", 1, false, || Ok::<_, &str>(false), |_| Ok(()))
            .unwrap();
        assert!(state.restore.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn page_load_preserves_pending_restore_but_a_replacement_has_its_own_visibility() {
        let mut state = super::MinimizedWebviews::default();
        state
            .synchronize("project", 1, true, || Ok::<_, ()>(true), |_| Ok(()))
            .unwrap();
        // The same controller finishes loading while minimized.
        state
            .synchronize(
                "project",
                1,
                true,
                || Ok::<_, ()>(false),
                |_| panic!("already hidden"),
            )
            .unwrap();
        assert_eq!(state.restore.get("project"), Some(&1));
        // A hidden replacement must not inherit the old controller's restore.
        state
            .synchronize(
                "project",
                2,
                false,
                || Ok::<_, ()>(false),
                |_| panic!("replacement is hidden"),
            )
            .unwrap();
        assert!(state.restore.is_empty());
        state
            .synchronize("project", 2, true, || Ok::<_, ()>(true), |_| Ok(()))
            .unwrap();
        assert_eq!(state.restore.get("project"), Some(&2));
    }

    #[test]
    fn replacement_debug_arguments_override_the_process_port_last() {
        let arguments = super::replacement_webview_debug_arguments(Some(OsString::from("48123")))
            .expect("valid replacement debug port")
            .expect("replacement debug arguments");

        assert_eq!(
            arguments,
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=48123"
        );
    }

    #[test]
    fn project_dialog_debug_data_directory_requires_an_absolute_path() {
        let directory = super::project_dialog_debug_data_directory(Some(OsString::from(
            r"C:\gate\project-dialog",
        )))
        .expect("absolute debug data directory");

        assert_eq!(directory, Some(PathBuf::from(r"C:\gate\project-dialog")));
        assert!(
            super::project_dialog_debug_data_directory(Some(OsString::from("relative"))).is_err()
        );
    }
}

#[cfg(windows)]
fn enforce_windows_policy(webview: &tauri::webview::PlatformWebview) -> windows::core::Result<()> {
    unsafe {
        let core_webview = webview.controller().CoreWebView2()?;
        let settings = core_webview.Settings()?;
        settings.SetAreDefaultContextMenusEnabled(false)?;
        settings
            .cast::<ICoreWebView2Settings3>()?
            .SetAreBrowserAcceleratorKeysEnabled(false)?;
    }
    Ok(())
}
