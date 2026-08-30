use std::{
    collections::HashMap,
    fmt::Write as _,
    io,
    path::Path,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

use crate::{
    desktop_webview_policy, global_runtime::GLOBAL_WINDOW_LABEL,
    ipc_contract::ProjectRecoveryDecision,
};

const DIALOG_LOAD_TIMEOUT: Duration = Duration::from_secs(5);
const DIALOG_WIDTH: f64 = 380.0;
const PROJECT_RECOVERY_DIALOG_WIDTH: f64 = 492.0;
const OWNED_WINDOW_READY_PARAMETER: &str = "ownedReadyToken";
pub(crate) const OWNED_WINDOW_TITLEBAR_HEIGHT: f64 = 38.0;
const OPENING_PROGRESS_LABEL: &str = "dialog-opening-progress";
const PROJECT_FAILURE_LABEL: &str = "dialog-project-failure";
static NEXT_OWNED_WINDOW_READY_TOKEN: AtomicU64 = AtomicU64::new(1);
static OWNED_WINDOW_READINESS: OnceLock<Mutex<OwnedWindowReadinessRegistry>> = OnceLock::new();
static PROJECT_RECOVERY_DECISIONS: OnceLock<Mutex<ProjectRecoveryDecisionRegistry>> =
    OnceLock::new();

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

#[derive(Default)]
struct ProjectRecoveryDecisionRegistry {
    waiters: HashMap<String, tokio::sync::oneshot::Sender<ProjectRecoveryDecision>>,
}

impl ProjectRecoveryDecisionRegistry {
    fn register(
        &mut self,
        attempt_id: &str,
    ) -> io::Result<tokio::sync::oneshot::Receiver<ProjectRecoveryDecision>> {
        if attempt_id.is_empty() || self.waiters.contains_key(attempt_id) {
            return Err(io::Error::other(
                "the Recovery decision attempt is not available",
            ));
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.waiters.insert(attempt_id.to_owned(), sender);
        Ok(receiver)
    }

    fn resolve(
        &mut self,
        attempt_id: &str,
        decision: ProjectRecoveryDecision,
    ) -> Result<(), String> {
        let sender = self
            .waiters
            .remove(attempt_id)
            .ok_or_else(|| "the Recovery decision attempt is not current".to_owned())?;
        sender
            .send(decision)
            .map_err(|_| "the Recovery decision owner is unavailable".to_owned())
    }

    fn cancel(&mut self, attempt_id: &str) {
        self.waiters.remove(attempt_id);
    }
}

fn project_recovery_decisions() -> &'static Mutex<ProjectRecoveryDecisionRegistry> {
    PROJECT_RECOVERY_DECISIONS
        .get_or_init(|| Mutex::new(ProjectRecoveryDecisionRegistry::default()))
}

fn cancel_project_recovery_decision(attempt_id: &str) {
    if let Ok(mut registry) = project_recovery_decisions().lock() {
        registry.cancel(attempt_id);
    }
}

#[tauri::command]
pub(crate) fn owned_window_content_ready(window: WebviewWindow, token: u64) -> Result<(), String> {
    owned_window_readiness()
        .lock()
        .map_err(|_| "the owned window readiness registry is unavailable".to_owned())?
        .signal(window.label(), token)
}

#[tauri::command]
pub(crate) fn resolve_opening_recovery(
    window: WebviewWindow,
    attempt_id: String,
    decision: ProjectRecoveryDecision,
) -> Result<(), String> {
    if window.label() != OPENING_PROGRESS_LABEL {
        return Err("Recovery belongs only to the owned opening dialog".into());
    }
    project_recovery_decisions()
        .lock()
        .map_err(|_| "the Recovery decision registry is unavailable".to_owned())?
        .resolve(&attempt_id, decision)
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectFailureDialogContext {
    ProjectOpening,
    ConfigurationValidation,
    DecorativeSelection,
    ProjectCreation,
}

impl ProjectFailureDialogContext {
    fn title(self) -> &'static str {
        match self {
            Self::ProjectOpening => "Não foi possível abrir o Projeto",
            Self::ConfigurationValidation => "Não foi possível validar as Configurações",
            Self::DecorativeSelection => "Não foi possível escolher a Imagem decorativa",
            Self::ProjectCreation => "Não foi possível criar o Projeto",
        }
    }

    fn owner_presentation(self) -> OwnerPresentation {
        OwnerPresentation::BlockedBehindDialog
    }
}

pub(crate) struct LaunchProgressDialog {
    closed: bool,
    owner: WebviewWindow,
    owner_presentation: OwnerPresentation,
    recovery_attempt_id: Option<String>,
    window: WebviewWindow,
}

impl LaunchProgressDialog {
    pub(crate) async fn request_recovery_decision(
        &mut self,
        attempt_id: &str,
    ) -> io::Result<ProjectRecoveryDecision> {
        if self.owner_presentation != OwnerPresentation::Replace
            || self.recovery_attempt_id.is_some()
        {
            return Err(io::Error::other(
                "Recovery is unavailable on this launch dialog",
            ));
        }
        resize_owned_window_width(&self.window, PROJECT_RECOVERY_DIALOG_WIDTH)?;
        let decision_receiver = project_recovery_decisions()
            .lock()
            .map_err(|_| io::Error::other("the Recovery decision registry is unavailable"))?
            .register(attempt_id)?;
        self.recovery_attempt_id = Some(attempt_id.to_owned());
        let destroyed_attempt_id = attempt_id.to_owned();
        self.window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                cancel_project_recovery_decision(&destroyed_attempt_id);
            }
        });

        let (ready_token, ready_receiver) = owned_window_readiness()
            .lock()
            .map_err(|_| io::Error::other("the owned window readiness registry is unavailable"))?
            .register(self.window.label());
        let mut url = self.window.url().map_err(io::Error::other)?;
        url.set_path("/dialog.html");
        url.set_query(Some(&format!(
            "kind=project-recovery&attemptId={}&{OWNED_WINDOW_READY_PARAMETER}={ready_token}",
            encode_unbounded_component(attempt_id),
        )));
        if let Err(error) = self.window.navigate(url) {
            cancel_owned_window_readiness(self.window.label(), ready_token);
            cancel_project_recovery_decision(attempt_id);
            self.recovery_attempt_id = None;
            return Err(io::Error::other(error));
        }

        let ready = tokio::time::timeout(DIALOG_LOAD_TIMEOUT, ready_receiver).await;
        cancel_owned_window_readiness(self.window.label(), ready_token);
        match ready {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                cancel_project_recovery_decision(attempt_id);
                self.recovery_attempt_id = None;
                return Err(io::Error::other(
                    "the Recovery dialog readiness became unavailable",
                ));
            }
            Err(_) => {
                cancel_project_recovery_decision(attempt_id);
                self.recovery_attempt_id = None;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the Recovery dialog did not become ready",
                ));
            }
        }

        let decision = decision_receiver.await.map_err(|_| {
            io::Error::other("the Recovery dialog closed without a terminal decision")
        })?;
        self.recovery_attempt_id = None;
        Ok(decision)
    }

    pub(crate) fn finish(mut self, restore_owner_window: bool) {
        if let Some(attempt_id) = self.recovery_attempt_id.take() {
            cancel_project_recovery_decision(&attempt_id);
        }
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

fn resize_owned_window_width(window: &WebviewWindow, width: f64) -> io::Result<()> {
    let scale_factor = window.scale_factor().map_err(io::Error::other)?;
    let current_size = window
        .inner_size()
        .map_err(io::Error::other)?
        .to_logical::<f64>(scale_factor);
    window
        .set_size(tauri::LogicalSize::new(width, current_size.height))
        .map_err(io::Error::other)?;
    window.center().map_err(io::Error::other)
}

impl Drop for LaunchProgressDialog {
    fn drop(&mut self) {
        if let Some(attempt_id) = self.recovery_attempt_id.take() {
            cancel_project_recovery_decision(&attempt_id);
        }
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
    owner_webview_data_directory: &Path,
) -> io::Result<LaunchProgressDialog> {
    let owner = owned_window(app, owner_label)?;
    let window = build_hidden_owned_window(
        app,
        &owner,
        HiddenOwnedWindowConfig {
            label: OPENING_PROGRESS_LABEL,
            url: kind.url(),
            width: DIALOG_WIDTH,
            height: 126.0 + OWNED_WINDOW_TITLEBAR_HEIGHT,
            browser_arguments: None,
            browser_data_directory: Some(owner_webview_data_directory),
        },
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
        recovery_attempt_id: None,
        window,
    })
}

pub(crate) async fn show_project_failure(
    app: &AppHandle,
    context: ProjectFailureDialogContext,
    message: &str,
    action: Option<&str>,
) -> io::Result<()> {
    let owner = owned_window(app, GLOBAL_WINDOW_LABEL)?;
    let url = format!(
        "dialog.html?kind=project-failure&title={}&message={}&action={}",
        encode_component(context.title()),
        encode_component(message),
        encode_component(action.unwrap_or("Feche esta janela e tente novamente.")),
    );
    let window = match build_hidden_owned_window(
        app,
        &owner,
        HiddenOwnedWindowConfig {
            label: PROJECT_FAILURE_LABEL,
            url: &url,
            width: DIALOG_WIDTH,
            height: 210.0 + OWNED_WINDOW_TITLEBAR_HEIGHT,
            browser_arguments: None,
            browser_data_directory: None,
        },
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

    display_dialog(&owner, &window, context.owner_presentation())
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

pub(crate) struct HiddenOwnedWindowConfig<'a> {
    pub(crate) label: &'a str,
    pub(crate) url: &'a str,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) browser_arguments: Option<&'a str>,
    pub(crate) browser_data_directory: Option<&'a Path>,
}

pub(crate) async fn build_hidden_owned_window(
    app: &AppHandle,
    owner: &WebviewWindow,
    config: HiddenOwnedWindowConfig<'_>,
) -> io::Result<WebviewWindow> {
    let HiddenOwnedWindowConfig {
        label,
        url,
        width,
        height,
        browser_arguments,
        browser_data_directory,
    } = config;
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
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(ready_url.into()))
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
    if let Some(arguments) = browser_arguments {
        builder = builder.additional_browser_args(arguments);
    }
    if let Some(directory) = browser_data_directory {
        builder = builder.data_directory(directory.to_path_buf());
    }
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
        OwnerPresentation::Replace => {
            owner.set_dialog_owner_enabled(false)?;
            if let Err(error) = owner.hide_dialog_owner() {
                let _ = owner.set_dialog_owner_enabled(true);
                return Err(error);
            }
            Ok(())
        }
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
    fn every_project_failure_context_keeps_the_owner_blocked_and_uses_a_specific_title() {
        let cases = [
            (
                ProjectFailureDialogContext::ProjectOpening,
                "Não foi possível abrir o Projeto",
            ),
            (
                ProjectFailureDialogContext::ConfigurationValidation,
                "Não foi possível validar as Configurações",
            ),
            (
                ProjectFailureDialogContext::DecorativeSelection,
                "Não foi possível escolher a Imagem decorativa",
            ),
            (
                ProjectFailureDialogContext::ProjectCreation,
                "Não foi possível criar o Projeto",
            ),
        ];

        for (context, expected_title) in cases {
            assert_eq!(
                context.owner_presentation(),
                OwnerPresentation::BlockedBehindDialog
            );
            assert_eq!(context.title(), expected_title);
        }
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
    fn recovery_decision_registry_accepts_one_terminal_for_the_exact_attempt() {
        let mut registry = ProjectRecoveryDecisionRegistry::default();
        let mut decision = registry
            .register("attempt-17")
            .expect("the first exact attempt is registered");

        assert!(registry.register("attempt-17").is_err());
        assert!(
            registry
                .resolve("attempt-18", ProjectRecoveryDecision::NowNot)
                .is_err()
        );
        assert_eq!(
            decision.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        );

        registry
            .resolve("attempt-17", ProjectRecoveryDecision::ReopenAndRecover)
            .expect("the correlated decision resolves once");
        assert_eq!(
            decision.try_recv(),
            Ok(ProjectRecoveryDecision::ReopenAndRecover)
        );
        assert!(
            registry
                .resolve("attempt-17", ProjectRecoveryDecision::NowNot)
                .is_err()
        );
    }

    #[test]
    fn cancelling_recovery_closes_the_pending_decision_without_a_fallback_choice() {
        let mut registry = ProjectRecoveryDecisionRegistry::default();
        let mut decision = registry
            .register("attempt-19")
            .expect("the attempt is registered");

        registry.cancel("attempt-19");

        assert_eq!(
            decision.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        );
    }

    #[test]
    fn opening_transition_hides_then_restores_the_owner() {
        let owner = RecordingOwner::default();

        prepare_owner(&owner, OwnerPresentation::Replace).unwrap();
        assert_eq!(owner.actions.take(), ["enabled:false", "hide"]);
        assert!(!owner.enabled.get());
        assert!(!owner.visible.get());

        release_owner(&owner, OwnerPresentation::Replace, false);
        assert_eq!(owner.actions.take(), ["enabled:true", "show"]);
    }
}
