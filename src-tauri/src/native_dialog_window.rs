use std::{fmt::Write as _, io, time::Duration};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::{desktop_webview_policy, global_runtime::GLOBAL_WINDOW_LABEL};

const DIALOG_LOAD_TIMEOUT: Duration = Duration::from_secs(5);
const DIALOG_WIDTH: f64 = 380.0;
pub(crate) const OWNED_WINDOW_TITLEBAR_HEIGHT: f64 = 38.0;
const OPENING_PROGRESS_LABEL: &str = "dialog-opening-progress";
const PROJECT_FAILURE_LABEL: &str = "dialog-project-failure";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LaunchProgressKind {
    Creating,
    Opening,
}

impl LaunchProgressKind {
    fn url(self) -> &'static str {
        match self {
            Self::Creating => "dialog.html?kind=creating-project",
            Self::Opening => "dialog.html?kind=opening-project",
        }
    }

    fn owner_presentation(self) -> OwnerPresentation {
        match self {
            Self::Opening => OwnerPresentation::Replace,
            Self::Creating => OwnerPresentation::BlockedBehindDialog,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerPresentation {
    Replace,
    BlockedBehindDialog,
}

pub(crate) struct LaunchProgressDialog {
    closed: bool,
    owner: WebviewWindow,
    owner_presentation: OwnerPresentation,
    window: WebviewWindow,
}

impl LaunchProgressDialog {
    pub(crate) fn finish(mut self, restore_owner_window: bool) {
        let _ = self.window.destroy();
        match self.owner_presentation {
            OwnerPresentation::Replace if restore_owner_window => {
                restore_owner(&self.owner);
            }
            OwnerPresentation::BlockedBehindDialog => {
                release_blocked_owner(&self.owner, restore_owner_window);
            }
            OwnerPresentation::Replace => {}
        }
        self.closed = true;
    }
}

impl Drop for LaunchProgressDialog {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.window.destroy();
            restore_owner(&self.owner);
        }
    }
}

pub(crate) async fn show_launch_progress(
    app: &AppHandle,
    owner_label: &str,
    kind: LaunchProgressKind,
) -> io::Result<LaunchProgressDialog> {
    let owner = owned_window(app, owner_label)?;
    let window = build_hidden_owned_window(
        app,
        &owner,
        OPENING_PROGRESS_LABEL,
        kind.url(),
        DIALOG_WIDTH,
        126.0 + OWNED_WINDOW_TITLEBAR_HEIGHT,
        false,
        false,
    )
    .await?;

    let owner_presentation = kind.owner_presentation();
    match owner_presentation {
        OwnerPresentation::Replace => display_transition_dialog(&owner, &window)?,
        OwnerPresentation::BlockedBehindDialog => display_owned_dialog(&owner, &window)?,
    }
    Ok(LaunchProgressDialog {
        closed: false,
        owner,
        owner_presentation,
        window,
    })
}

pub(crate) async fn show_project_failure(
    app: &AppHandle,
    message: &str,
    action: Option<&str>,
) -> io::Result<()> {
    let owner = owned_window(app, GLOBAL_WINDOW_LABEL)?;
    let url = format!(
        "dialog.html?kind=project-failure&message={}&action={}",
        encode_component(message),
        encode_component(action.unwrap_or("Feche esta janela e tente novamente.")),
    );
    let window = match build_hidden_owned_window(
        app,
        &owner,
        PROJECT_FAILURE_LABEL,
        &url,
        DIALOG_WIDTH,
        210.0 + OWNED_WINDOW_TITLEBAR_HEIGHT,
        true,
        false,
    )
    .await
    {
        Ok(window) => window,
        Err(error) => {
            restore_owner(&owner);
            return Err(error);
        }
    };
    let owner_after_close = owner.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            restore_owner(&owner_after_close);
        }
    });

    display_owned_dialog(&owner, &window)
}

fn owned_window(app: &AppHandle, label: &str) -> io::Result<WebviewWindow> {
    app.get_webview_window(label)
        .ok_or_else(|| io::Error::other(format!("the {label} owner window is unavailable")))
}

pub(crate) async fn build_hidden_owned_window(
    app: &AppHandle,
    owner: &WebviewWindow,
    label: &str,
    url: &str,
    width: f64,
    height: f64,
    closable: bool,
    resizable: bool,
) -> io::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
    }

    let (policy_signal, policy_readiness) = desktop_webview_policy::page_load_handshake();
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("MyAlbuns")
        .inner_size(width, height)
        .resizable(resizable)
        .maximizable(false)
        .minimizable(false)
        .closable(closable)
        .decorations(false)
        .skip_taskbar(true)
        .shadow(true)
        .focused(true)
        .visible(false)
        .center()
        .prevent_overflow();
    let builder = builder.parent(owner).map_err(io::Error::other)?;
    let window = builder
        .on_page_load(move |window, payload| {
            policy_signal.observe(&window, payload.event());
        })
        .build()
        .map_err(io::Error::other)?;

    match tokio::time::timeout(DIALOG_LOAD_TIMEOUT, policy_readiness.wait()).await {
        Ok(Ok(())) => Ok(window),
        Ok(Err(error)) => {
            let _ = window.destroy();
            Err(error)
        }
        Err(_) => {
            let _ = window.destroy();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the native dialog page did not become ready",
            ))
        }
    }
}

fn display_transition_dialog(owner: &WebviewWindow, window: &WebviewWindow) -> io::Result<()> {
    display_dialog(owner, window, OwnerPresentation::Replace)
}

pub(crate) fn display_owned_dialog(
    owner: &WebviewWindow,
    window: &WebviewWindow,
) -> io::Result<()> {
    display_dialog(owner, window, OwnerPresentation::BlockedBehindDialog)
}

fn display_dialog(
    owner: &WebviewWindow,
    window: &WebviewWindow,
    owner_presentation: OwnerPresentation,
) -> io::Result<()> {
    prepare_owner(owner, owner_presentation)?;
    if let Err(error) = window.show() {
        let _ = window.destroy();
        restore_owner(owner);
        return Err(io::Error::other(error));
    }
    if let Err(error) = window.set_focus() {
        let _ = window.destroy();
        restore_owner(owner);
        return Err(io::Error::other(error));
    }
    Ok(())
}

pub(crate) fn restore_owner(owner: &WebviewWindow) {
    release_owner(owner, true);
}

fn release_blocked_owner(owner: &WebviewWindow, focus: bool) {
    release_owner(owner, focus);
}

trait DialogOwner {
    fn hide_dialog_owner(&self) -> io::Result<()>;
    fn show_dialog_owner(&self) -> io::Result<()>;
    fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()>;
    fn focus_dialog_owner(&self) -> io::Result<()>;
}

impl DialogOwner for WebviewWindow {
    fn hide_dialog_owner(&self) -> io::Result<()> {
        self.hide().map_err(io::Error::other)
    }

    fn show_dialog_owner(&self) -> io::Result<()> {
        self.show().map_err(io::Error::other)
    }

    fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()> {
        self.set_enabled(enabled).map_err(io::Error::other)
    }

    fn focus_dialog_owner(&self) -> io::Result<()> {
        self.set_focus().map_err(io::Error::other)
    }
}

fn prepare_owner(owner: &impl DialogOwner, presentation: OwnerPresentation) -> io::Result<()> {
    match presentation {
        OwnerPresentation::Replace => owner.hide_dialog_owner(),
        OwnerPresentation::BlockedBehindDialog => {
            owner.show_dialog_owner()?;
            owner.set_dialog_owner_enabled(false)
        }
    }
}

fn release_owner(owner: &impl DialogOwner, focus: bool) {
    let _ = owner.set_dialog_owner_enabled(true);
    let _ = owner.show_dialog_owner();
    if focus {
        let _ = owner.focus_dialog_owner();
    }
}

fn encode_component(value: &str) -> String {
    encode_component_chars(value.chars().take(800))
}

pub(crate) fn encode_unbounded_component(value: &str) -> String {
    encode_component_chars(value.chars())
}

fn encode_component_chars(chars: impl Iterator<Item = char>) -> String {
    let mut encoded = String::new();
    for byte in chars.collect::<String>().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct RecordingOwner {
        actions: RefCell<Vec<String>>,
    }

    impl DialogOwner for RecordingOwner {
        fn hide_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("hide".into());
            Ok(())
        }

        fn show_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("show".into());
            Ok(())
        }

        fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()> {
            self.actions.borrow_mut().push(format!("enabled:{enabled}"));
            Ok(())
        }

        fn focus_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("focus".into());
            Ok(())
        }
    }

    #[test]
    fn dialog_text_is_bounded_and_encoded_as_a_query_component() {
        assert_eq!(
            encode_component("Projeto inválido & tente novamente."),
            "Projeto%20inv%C3%A1lido%20%26%20tente%20novamente."
        );
        assert!(encode_component(&"a".repeat(900)).len() <= 800);
    }

    #[test]
    fn only_opening_a_project_replaces_the_owner_window() {
        assert_eq!(
            LaunchProgressKind::Opening.owner_presentation(),
            OwnerPresentation::Replace
        );
        assert_eq!(
            LaunchProgressKind::Creating.owner_presentation(),
            OwnerPresentation::BlockedBehindDialog
        );
    }

    #[test]
    fn owned_dialogs_keep_the_owner_visible_and_blocked_until_release() {
        let owner = RecordingOwner::default();

        prepare_owner(&owner, OwnerPresentation::BlockedBehindDialog).unwrap();
        assert_eq!(owner.actions.take(), ["show", "enabled:false"]);

        release_owner(&owner, true);
        assert_eq!(owner.actions.take(), ["enabled:true", "show", "focus"]);
    }

    #[test]
    fn opening_transition_hides_then_restores_the_owner() {
        let owner = RecordingOwner::default();

        prepare_owner(&owner, OwnerPresentation::Replace).unwrap();
        assert_eq!(owner.actions.take(), ["hide"]);

        release_owner(&owner, false);
        assert_eq!(owner.actions.take(), ["enabled:true", "show"]);
    }
}
