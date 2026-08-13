use std::error::Error;

use tauri::WebviewWindow;

const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";

#[cfg(windows)]
use {
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3, windows::core::Interface,
};

pub(crate) async fn enforce(window: &WebviewWindow) -> Result<(), Box<dyn Error + Send + Sync>> {
    #[cfg(windows)]
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        window.with_webview(move |webview| {
            let result = enforce_windows_policy(&webview).map_err(|error| error.to_string());
            let _ = sender.send(result);
        })?;

        receiver
            .await
            .map_err(|_| std::io::Error::other("a política nativa da WebView ficou indisponível"))?
            .map_err(|error| {
                std::io::Error::other(format!(
                    "não foi possível aplicar a política nativa da WebView: {error}"
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
