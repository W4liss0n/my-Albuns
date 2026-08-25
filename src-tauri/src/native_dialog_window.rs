use std::{
    collections::HashMap,
    fmt::Write as _,
    io,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

use crate::{desktop_webview_policy, global_runtime::GLOBAL_WINDOW_LABEL};

const DIALOG_LOAD_TIMEOUT: Duration = Duration::from_secs(5);
const DIALOG_WIDTH: f64 = 380.0;
const OWNED_WINDOW_READY_PARAMETER: &str = "ownedReadyToken";
pub(crate) const OWNED_WINDOW_TITLEBAR_HEIGHT: f64 = 38.0;
const OPENING_PROGRESS_LABEL: &str = "dialog-opening-progress";
const PROJECT_FAILURE_LABEL: &str = "dialog-project-failure";
static NEXT_OWNED_WINDOW_READY_TOKEN: AtomicU64 = AtomicU64::new(1);
static OWNED_WINDOW_READINESS: OnceLock<Mutex<OwnedWindowReadinessRegistry>> = OnceLock::new();

#[derive(Default)]
struct OwnedWindowReadinessRegistry {
    waiters: HashMap<String, (u64, tokio::sync::oneshot::Sender<()>)>,
}

impl OwnedWindowReadinessRegistry {
    fn register(&mut self, label: &str) -> (u64, tokio::sync::oneshot::Receiver<()>) {
        let token = NEXT_OWNED_WINDOW_READY_TOKEN.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.waiters.insert(label.to_owned(), (token, sender));
        (token, receiver)
    }

    fn signal(&mut self, label: &str, token: u64) -> Result<(), String> {
        let matches_waiter = self
            .waiters
            .get(label)
            .is_some_and(|(expected_token, _)| *expected_token == token);
        if !matches_waiter {
            return Err("the owned window readiness token is not current".into());
        }
        let (_, sender) = self
            .waiters
            .remove(label)
            .expect("a matching readiness waiter must still be registered");
        sender
            .send(())
            .map_err(|_| "the owned window readiness receiver is unavailable".into())
    }

    fn cancel(&mut self, label: &str, token: u64) {
        if self
            .waiters
            .get(label)
            .is_some_and(|(expected_token, _)| *expected_token == token)
        {
            self.waiters.remove(label);
        }
    }
}

fn owned_window_readiness() -> &'static Mutex<OwnedWindowReadinessRegistry> {
    OWNED_WINDOW_READINESS.get_or_init(|| Mutex::new(OwnedWindowReadinessRegistry::default()))
}

fn cancel_owned_window_readiness(label: &str, token: u64) {
    if let Ok(mut registry) = owned_window_readiness().lock() {
        registry.cancel(label, token);
    }
}

#[tauri::command]
pub(crate) fn owned_window_content_ready(window: WebviewWindow, token: u64) -> Result<(), String> {
    owned_window_readiness()
        .lock()
        .map_err(|_| "the owned window readiness registry is unavailable".to_owned())?
        .signal(window.label(), token)
}

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
        match self.owner_presentation {
            OwnerPresentation::Replace if restore_owner_window => {
                let _ = self.window.destroy();
                restore_owner(&self.owner);
            }
            OwnerPresentation::BlockedBehindDialog => {
                let _ = dismiss_blocked_dialog(&self.owner, &self.window, restore_owner_window);
            }
            OwnerPresentation::Replace => {
                let _ = self.window.destroy();
            }
        }
        self.closed = true;
    }
}

impl Drop for LaunchProgressDialog {
    fn drop(&mut self) {
        if !self.closed {
            match self.owner_presentation {
                OwnerPresentation::BlockedBehindDialog => {
                    let _ = dismiss_blocked_dialog(&self.owner, &self.window, true);
                }
                OwnerPresentation::Replace => {
                    let _ = self.window.destroy();
                    release_owner(&self.owner, self.owner_presentation, true);
                }
            }
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
            release_blocked_owner_if_disabled(&owner_after_close, true);
        }
    });

    display_owned_dialog(&owner, &window)
}

#[tauri::command]
pub(crate) fn dismiss_owned_dialog(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    if window.label() != PROJECT_FAILURE_LABEL {
        return Err("this window is not a standard owned dialog".into());
    }
    let owner = owned_window(&app, GLOBAL_WINDOW_LABEL).map_err(|error| error.to_string())?;
    dismiss_blocked_dialog(&owner, &window, true).map_err(|error| error.to_string())
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
) -> io::Result<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.destroy();
    }

    let (ready_token, ready_receiver) = owned_window_readiness()
        .lock()
        .map_err(|_| io::Error::other("the owned window readiness registry is unavailable"))?
        .register(label);
    let ready_url =
        append_query_parameter(url, OWNED_WINDOW_READY_PARAMETER, &ready_token.to_string());
    let (policy_signal, policy_readiness) = desktop_webview_policy::page_load_handshake();
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(ready_url.into()))
        .title("MyAlbuns")
        .inner_size(width, height)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .decorations(false)
        .skip_taskbar(true)
        .shadow(true)
        .focused(true)
        .visible(false)
        .center()
        .prevent_overflow();
    let builder = builder.parent(owner).map_err(io::Error::other)?;
    let window = match builder
        .on_page_load(move |window, payload| {
            policy_signal.observe(&window, payload.event());
        })
        .build()
    {
        Ok(window) => window,
        Err(error) => {
            cancel_owned_window_readiness(label, ready_token);
            return Err(io::Error::other(error));
        }
    };

    let readiness = async {
        policy_readiness.wait().await?;
        ready_receiver.await.map_err(|_| {
            io::Error::other("the native dialog content readiness became unavailable")
        })?;
        Ok::<(), io::Error>(())
    };
    let readiness_result = tokio::time::timeout(DIALOG_LOAD_TIMEOUT, readiness).await;
    cancel_owned_window_readiness(label, ready_token);
    match readiness_result {
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

fn append_query_parameter(url: &str, name: &str, value: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!(
        "{url}{separator}{name}={}",
        encode_unbounded_component(value)
    )
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
    owner: &impl DialogOwner,
    window: &impl DialogSurface,
    owner_presentation: OwnerPresentation,
) -> io::Result<()> {
    if window.is_dialog_visible()? {
        return Ok(());
    }
    prepare_owner(owner, owner_presentation)?;
    if let Err(error) = window.show_dialog() {
        let _ = window.destroy_dialog();
        release_owner(owner, owner_presentation, true);
        return Err(error);
    }
    if let Err(error) = window.focus_dialog() {
        let _ = window.destroy_dialog();
        release_owner(owner, owner_presentation, true);
        return Err(error);
    }
    Ok(())
}

pub(crate) fn restore_owner(owner: &WebviewWindow) {
    release_owner(owner, OwnerPresentation::Replace, true);
}

pub(crate) fn release_blocked_owner_if_disabled(owner: &WebviewWindow, focus: bool) {
    release_owner_if_disabled(owner, focus);
}

fn release_owner_if_disabled(owner: &impl DialogOwner, focus: bool) {
    if !owner.is_dialog_owner_enabled().unwrap_or(false) {
        release_owner(owner, OwnerPresentation::BlockedBehindDialog, focus);
    }
}

trait DialogOwner {
    fn is_dialog_owner_visible(&self) -> io::Result<bool>;
    fn is_dialog_owner_enabled(&self) -> io::Result<bool>;
    fn hide_dialog_owner(&self) -> io::Result<()>;
    fn show_dialog_owner(&self) -> io::Result<()>;
    fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()>;
    fn focus_dialog_owner(&self) -> io::Result<()>;
}

trait DialogSurface {
    fn is_dialog_visible(&self) -> io::Result<bool>;
    fn show_dialog(&self) -> io::Result<()>;
    fn focus_dialog(&self) -> io::Result<()>;
    fn destroy_dialog(&self) -> io::Result<()>;
}

impl DialogOwner for WebviewWindow {
    fn is_dialog_owner_visible(&self) -> io::Result<bool> {
        self.is_visible().map_err(io::Error::other)
    }

    fn is_dialog_owner_enabled(&self) -> io::Result<bool> {
        #[cfg(windows)]
        {
            let handle = self.hwnd().map_err(io::Error::other)?;
            Ok(unsafe { IsWindowEnabled(handle) }.as_bool())
        }
        #[cfg(not(windows))]
        {
            Ok(false)
        }
    }

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

impl DialogSurface for WebviewWindow {
    fn is_dialog_visible(&self) -> io::Result<bool> {
        self.is_visible().map_err(io::Error::other)
    }

    fn show_dialog(&self) -> io::Result<()> {
        self.show().map_err(io::Error::other)
    }

    fn focus_dialog(&self) -> io::Result<()> {
        self.set_focus().map_err(io::Error::other)
    }

    fn destroy_dialog(&self) -> io::Result<()> {
        self.destroy().map_err(io::Error::other)
    }
}

fn prepare_owner(owner: &impl DialogOwner, presentation: OwnerPresentation) -> io::Result<()> {
    match presentation {
        OwnerPresentation::Replace => owner.hide_dialog_owner(),
        OwnerPresentation::BlockedBehindDialog => {
            if !owner.is_dialog_owner_visible()? {
                owner.show_dialog_owner()?;
            }
            owner.set_dialog_owner_enabled(false)
        }
    }
}

pub(crate) fn dismiss_blocked_window(
    owner: &WebviewWindow,
    dialog: &WebviewWindow,
    focus: bool,
) -> io::Result<()> {
    dismiss_blocked_dialog(owner, dialog, focus)
}

fn dismiss_blocked_dialog(
    owner: &impl DialogOwner,
    dialog: &impl DialogSurface,
    focus: bool,
) -> io::Result<()> {
    owner.set_dialog_owner_enabled(true)?;
    if let Err(error) = dialog.destroy_dialog() {
        let _ = owner.set_dialog_owner_enabled(false);
        return Err(error);
    }
    if focus {
        owner.focus_dialog_owner()?;
    }
    Ok(())
}

fn release_owner(owner: &impl DialogOwner, presentation: OwnerPresentation, focus: bool) {
    let _ = owner.set_dialog_owner_enabled(true);
    if presentation == OwnerPresentation::Replace {
        let _ = owner.show_dialog_owner();
    }
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
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use super::*;

    struct RecordingOwner {
        actions: RefCell<Vec<String>>,
        enabled: Cell<bool>,
        visible: Cell<bool>,
    }

    impl Default for RecordingOwner {
        fn default() -> Self {
            Self {
                actions: RefCell::new(Vec::new()),
                enabled: Cell::new(true),
                visible: Cell::new(true),
            }
        }
    }

    impl RecordingOwner {
        fn hidden() -> Self {
            Self {
                visible: Cell::new(false),
                ..Self::default()
            }
        }
    }

    struct RecordingDialog {
        actions: RefCell<Vec<String>>,
        visible: Cell<bool>,
    }

    impl RecordingDialog {
        fn visible() -> Self {
            Self {
                actions: RefCell::new(Vec::new()),
                visible: Cell::new(true),
            }
        }
    }

    impl DialogOwner for RecordingOwner {
        fn is_dialog_owner_visible(&self) -> io::Result<bool> {
            Ok(self.visible.get())
        }

        fn is_dialog_owner_enabled(&self) -> io::Result<bool> {
            Ok(self.enabled.get())
        }

        fn hide_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("hide".into());
            self.visible.set(false);
            Ok(())
        }

        fn show_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("show".into());
            self.visible.set(true);
            Ok(())
        }

        fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()> {
            self.actions.borrow_mut().push(format!("enabled:{enabled}"));
            self.enabled.set(enabled);
            Ok(())
        }

        fn focus_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("focus".into());
            Ok(())
        }
    }

    impl DialogSurface for RecordingDialog {
        fn is_dialog_visible(&self) -> io::Result<bool> {
            Ok(self.visible.get())
        }

        fn show_dialog(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("show".into());
            self.visible.set(true);
            Ok(())
        }

        fn focus_dialog(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("focus".into());
            Ok(())
        }

        fn destroy_dialog(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("destroy".into());
            self.visible.set(false);
            Ok(())
        }
    }

    struct TimelineOwner(Rc<RefCell<Vec<&'static str>>>);

    impl DialogOwner for TimelineOwner {
        fn is_dialog_owner_visible(&self) -> io::Result<bool> {
            Ok(true)
        }

        fn is_dialog_owner_enabled(&self) -> io::Result<bool> {
            Ok(true)
        }

        fn hide_dialog_owner(&self) -> io::Result<()> {
            self.0.borrow_mut().push("owner.hide");
            Ok(())
        }

        fn show_dialog_owner(&self) -> io::Result<()> {
            self.0.borrow_mut().push("owner.show");
            Ok(())
        }

        fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()> {
            self.0.borrow_mut().push(if enabled {
                "owner.enabled:true"
            } else {
                "owner.enabled:false"
            });
            Ok(())
        }

        fn focus_dialog_owner(&self) -> io::Result<()> {
            self.0.borrow_mut().push("owner.focus");
            Ok(())
        }
    }

    struct TimelineDialog(Rc<RefCell<Vec<&'static str>>>);

    impl DialogSurface for TimelineDialog {
        fn is_dialog_visible(&self) -> io::Result<bool> {
            Ok(true)
        }

        fn show_dialog(&self) -> io::Result<()> {
            self.0.borrow_mut().push("dialog.show");
            Ok(())
        }

        fn focus_dialog(&self) -> io::Result<()> {
            self.0.borrow_mut().push("dialog.focus");
            Ok(())
        }

        fn destroy_dialog(&self) -> io::Result<()> {
            self.0.borrow_mut().push("dialog.destroy");
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
        assert_eq!(owner.actions.take(), ["enabled:false"]);

        release_owner(&owner, OwnerPresentation::BlockedBehindDialog, true);
        assert_eq!(owner.actions.take(), ["enabled:true", "focus"]);
    }

    #[test]
    fn dismissing_an_owned_dialog_enables_its_owner_before_destroying_the_dialog() {
        let timeline = Rc::new(RefCell::new(Vec::new()));
        let owner = TimelineOwner(timeline.clone());
        let dialog = TimelineDialog(timeline.clone());

        dismiss_blocked_dialog(&owner, &dialog, true).unwrap();

        assert_eq!(
            timeline.borrow().as_slice(),
            ["owner.enabled:true", "dialog.destroy", "owner.focus"]
        );
    }

    #[test]
    fn destroyed_fallback_does_not_reactivate_an_owner_released_by_normal_dismissal() {
        let owner = RecordingOwner::default();
        let dialog = RecordingDialog::visible();
        prepare_owner(&owner, OwnerPresentation::BlockedBehindDialog).unwrap();
        owner.actions.take();

        dismiss_blocked_dialog(&owner, &dialog, true).unwrap();
        release_owner_if_disabled(&owner, true);

        assert_eq!(owner.actions.take(), ["enabled:true", "focus"]);
    }

    #[test]
    fn destroyed_fallback_releases_an_owner_after_an_unexpected_dialog_close() {
        let owner = RecordingOwner::default();
        prepare_owner(&owner, OwnerPresentation::BlockedBehindDialog).unwrap();
        owner.actions.take();

        release_owner_if_disabled(&owner, true);

        assert_eq!(owner.actions.take(), ["enabled:true", "focus"]);
    }

    #[test]
    fn a_hidden_owner_is_revealed_before_an_owned_dialog_blocks_it() {
        let owner = RecordingOwner::hidden();

        prepare_owner(&owner, OwnerPresentation::BlockedBehindDialog).unwrap();

        assert_eq!(owner.actions.take(), ["show", "enabled:false"]);
        assert!(owner.visible.get());
    }

    #[test]
    fn presenting_an_already_visible_owned_dialog_is_a_noop() {
        let owner = RecordingOwner::default();
        let dialog = RecordingDialog::visible();

        display_dialog(&owner, &dialog, OwnerPresentation::BlockedBehindDialog).unwrap();

        assert!(owner.actions.borrow().is_empty());
        assert!(dialog.actions.borrow().is_empty());
    }

    #[test]
    fn content_readiness_rejects_the_wrong_token_without_consuming_the_waiter() {
        let mut registry = OwnedWindowReadinessRegistry::default();
        let (token, mut readiness) = registry.register("project-dialog");

        assert!(
            registry
                .signal("project-dialog", token.saturating_add(1))
                .is_err()
        );
        assert_eq!(
            readiness.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );

        registry.signal("project-dialog", token).unwrap();
        assert_eq!(readiness.try_recv(), Ok(()));
        assert!(registry.signal("project-dialog", token).is_err());
    }

    #[test]
    fn replacing_a_window_readiness_waiter_cannot_cancel_the_new_window() {
        let mut registry = OwnedWindowReadinessRegistry::default();
        let (old_token, mut old_readiness) = registry.register("project-dialog");
        let (new_token, mut new_readiness) = registry.register("project-dialog");

        assert_eq!(
            old_readiness.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        );
        registry.cancel("project-dialog", old_token);
        registry.signal("project-dialog", new_token).unwrap();
        assert_eq!(new_readiness.try_recv(), Ok(()));
    }

    #[test]
    fn opening_transition_hides_then_restores_the_owner() {
        let owner = RecordingOwner::default();

        prepare_owner(&owner, OwnerPresentation::Replace).unwrap();
        assert_eq!(owner.actions.take(), ["hide"]);

        release_owner(&owner, OwnerPresentation::Replace, false);
        assert_eq!(owner.actions.take(), ["enabled:true", "show"]);
    }
}
