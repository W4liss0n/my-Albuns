use std::sync::{Arc, Mutex};

use tauri::{WebviewWindow, webview::PageLoadEvent};

const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";

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
        if event != PageLoadEvent::Finished {
            return;
        }
        let sender = self.sender.lock().ok().and_then(|mut sender| sender.take());
        if let Some(sender) = sender {
            let _ = sender.send(enforce_on_main_thread(window));
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

fn enforce_on_main_thread(window: &WebviewWindow) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        window
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
    let _ = window;

    Ok(())
}

pub(crate) fn automation_enabled() -> bool {
    cfg!(debug_assertions) && std::env::var_os(TAURI_WEBVIEW_AUTOMATION_ENV).is_some()
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
