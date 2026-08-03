use std::error::Error;

use tauri::WebviewWindow;

#[cfg(windows)]
use {
    std::sync::{Arc, Mutex},
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3,
    windows::core::Interface,
};

pub(crate) fn enforce(window: &WebviewWindow) -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    {
        let callback_result = Arc::new(Mutex::new(None));
        let callback_result_writer = Arc::clone(&callback_result);

        window.with_webview(move |webview| {
            let result = enforce_windows_policy(&webview);
            *callback_result_writer
                .lock()
                .expect("the native WebView policy result is not poisoned") = Some(result);
        })?;

        let result = callback_result
            .lock()
            .map_err(|_| std::io::Error::other("a política nativa da WebView ficou indisponível"))?
            .take()
            .ok_or_else(|| {
                std::io::Error::other("a política nativa da WebView não foi aplicada")
            })?;
        result?;
    }

    #[cfg(not(windows))]
    let _ = window;

    Ok(())
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
