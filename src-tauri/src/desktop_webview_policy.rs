use std::sync::{Arc, Mutex};

#[cfg(debug_assertions)]
use std::{ffi::OsString, io};

use tauri::{WebviewWindow, webview::PageLoadEvent};

const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";
pub(crate) const SAVE_AS_WEBVIEW_DEBUG_PORT_ENV: &str = "MYALBUNS_DEV_SAVE_AS_WEBVIEW_DEBUG_PORT";

#[cfg(debug_assertions)]
const WRY_DEFAULT_DISABLED_FEATURES: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

#[cfg(windows)]
use {
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3, windows::core::Interface,
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
pub(crate) fn retire_inherited_debug_arguments_before_replacement() -> io::Result<()> {
    if std::env::var_os(SAVE_AS_WEBVIEW_DEBUG_PORT_ENV).is_none() {
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
    use std::ffi::OsString;

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
