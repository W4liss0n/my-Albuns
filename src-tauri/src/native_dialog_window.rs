use std::{
    collections::HashMap,
    fmt::Write as _,
    io,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::Deserialize;
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

use crate::{
    desktop_webview_policy,
    global_runtime::GLOBAL_WINDOW_LABEL,
    ipc_contract::{OpeningExternalCopyDecision, ProjectRecoveryDecision},
};

const DIALOG_LOAD_TIMEOUT: Duration = Duration::from_secs(5);
const DIALOG_WIDTH: f64 = 380.0;
const EXTERNAL_COPY_DIALOG_WIDTH: f64 = 440.0;
const PROJECT_RECOVERY_DIALOG_WIDTH: f64 = 492.0;
const OWNED_WINDOW_READY_PARAMETER: &str = "ownedReadyToken";
pub(crate) const OWNED_WINDOW_TITLEBAR_HEIGHT: f64 = 38.0;
const OPENING_PROGRESS_LABEL: &str = "dialog-opening-progress";
pub(crate) const PROGRESS_WEBVIEW_NAMESPACE: &str = "global-progress";
const OPENING_IMAGE_PROGRESS_EVENT: &str = "myalbuns://opening-image-progress";
const PROJECT_FAILURE_LABEL: &str = "dialog-project-failure";
static NEXT_OWNED_WINDOW_READY_TOKEN: AtomicU64 = AtomicU64::new(1);
static OWNED_WINDOW_READINESS: OnceLock<Mutex<OwnedWindowReadinessRegistry>> = OnceLock::new();
static PROJECT_RECOVERY_DECISIONS: OnceLock<
    Mutex<OpeningDecisionRegistry<ProjectRecoveryDecision>>,
> = OnceLock::new();
static EXTERNAL_COPY_DECISIONS: OnceLock<
    Mutex<OpeningDecisionRegistry<OpeningExternalCopyDecision>>,
> = OnceLock::new();

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

struct OpeningDecisionRegistry<T> {
    waiters: HashMap<String, tokio::sync::oneshot::Sender<T>>,
}

impl<T> Default for OpeningDecisionRegistry<T> {
    fn default() -> Self {
        Self {
            waiters: HashMap::new(),
        }
    }
}

impl<T> OpeningDecisionRegistry<T> {
    fn register(&mut self, attempt_id: &str) -> io::Result<tokio::sync::oneshot::Receiver<T>> {
        if attempt_id.is_empty() || self.waiters.contains_key(attempt_id) {
            return Err(io::Error::other(
                "the opening decision attempt is not available",
            ));
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.waiters.insert(attempt_id.to_owned(), sender);
        Ok(receiver)
    }

    fn resolve(&mut self, attempt_id: &str, decision: T) -> Result<(), String> {
        let sender = self
            .waiters
            .remove(attempt_id)
            .ok_or_else(|| "the opening decision attempt is not current".to_owned())?;
        sender
            .send(decision)
            .map_err(|_| "the opening decision owner is unavailable".to_owned())
    }

    fn cancel(&mut self, attempt_id: &str) {
        self.waiters.remove(attempt_id);
    }
}

fn project_recovery_decisions() -> &'static Mutex<OpeningDecisionRegistry<ProjectRecoveryDecision>>
{
    PROJECT_RECOVERY_DECISIONS.get_or_init(|| Mutex::new(OpeningDecisionRegistry::default()))
}

fn external_copy_decisions() -> &'static Mutex<OpeningDecisionRegistry<OpeningExternalCopyDecision>>
{
    EXTERNAL_COPY_DECISIONS.get_or_init(|| Mutex::new(OpeningDecisionRegistry::default()))
}

fn cancel_project_recovery_decision(attempt_id: &str) {
    if let Ok(mut registry) = project_recovery_decisions().lock() {
        registry.cancel(attempt_id);
    }
}

fn cancel_external_copy_decision(attempt_id: &str) {
    if let Ok(mut registry) = external_copy_decisions().lock() {
        registry.cancel(attempt_id);
    }
}

#[derive(Clone)]
enum OpeningDecisionAttempt {
    ExternalCopy(String),
    Recovery(String),
}

impl OpeningDecisionAttempt {
    fn cancel(&self) {
        match self {
            Self::ExternalCopy(attempt_id) => cancel_external_copy_decision(attempt_id),
            Self::Recovery(attempt_id) => cancel_project_recovery_decision(attempt_id),
        }
    }
}

#[tauri::command]
pub(crate) fn owned_window_content_ready(window: WebviewWindow, token: u64) -> Result<(), String> {
    owned_window_readiness()
        .lock()
        .map_err(|_| "the owned window readiness registry is unavailable".to_owned())?
        .signal(window.label(), token)
}

/// A replacement must not acknowledge an already consumed content-ready token.
pub(crate) fn recovery_url(label: &str, mut url: tauri::Url) -> tauri::Url {
    let parameters = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .filter(|(key, value)| {
            key != OWNED_WINDOW_READY_PARAMETER
                || owned_window_readiness()
                    .lock()
                    .ok()
                    .is_some_and(|registry| {
                        registry
                            .waiters
                            .get(label)
                            .is_some_and(|(token, _)| value.parse::<u64>().ok() == Some(*token))
                    })
        })
        .collect::<Vec<_>>();
    if url.query().is_some() {
        url.set_query(None);
        if !parameters.is_empty() {
            url.query_pairs_mut().extend_pairs(parameters);
        }
    }
    url
}

#[tauri::command]
pub(crate) async fn fit_owned_window(
    window: WebviewWindow,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if !matches!(
        window.label(),
        "project-dialog"
            | "batch-export"
            | "batch-progress"
            | "generation"
            | "generation-progress"
            | OPENING_PROGRESS_LABEL
            | PROJECT_FAILURE_LABEL
    ) {
        return Err("content fitting belongs only to owned dialog windows".into());
    }
    if ![width, height]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && *value <= 65535.0)
    {
        return Err("the owned window dimensions are invalid".into());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = sender.send(fit_owned_window_frame(&target, width, height));
        })
        .map_err(|error| error.to_string())?;
    receiver
        .await
        .map_err(|_| "the owned window fitting became unavailable".to_owned())?
        .map_err(|error| error.to_string())
}

fn fit_owned_window_frame(window: &WebviewWindow, width: f64, height: f64) -> io::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
        };

        let scale = window.scale_factor().map_err(io::Error::other)?;
        let requested = tauri::LogicalSize::new(width, height).to_physical::<i32>(scale);
        let inner = window.inner_size().map_err(io::Error::other)?;
        let outer = window.outer_size().map_err(io::Error::other)?;
        // Include the native frame/shadow offsets when converting client dimensions.
        let outer_width = requested.width + outer.width as i32 - inner.width as i32;
        let outer_height = requested.height + outer.height as i32 - inner.height as i32;
        let monitor = window
            .current_monitor()
            .map_err(io::Error::other)?
            .or(window.primary_monitor().map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("the dialog monitor is unavailable"))?;
        let area = monitor.work_area();
        let left = area.position.x + (area.size.width as i32 - outer_width) / 2;
        let top = area.position.y + (area.size.height as i32 - outer_height) / 2;

        // Resize and reposition in one native operation; separate calls expose an
        // off-center frame between the content fit and the subsequent centering.
        unsafe {
            SetWindowPos(
                window.hwnd().map_err(io::Error::other)?,
                None,
                left,
                top,
                outer_width,
                outer_height,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .map_err(io::Error::other)
    }
    #[cfg(not(windows))]
    {
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(io::Error::other)?;
        window.center().map_err(io::Error::other)
    }
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

#[tauri::command]
pub(crate) fn resolve_opening_external_copy(
    window: WebviewWindow,
    attempt_id: String,
    decision: OpeningExternalCopyDecision,
) -> Result<(), String> {
    if window.label() != OPENING_PROGRESS_LABEL {
        return Err("the external-copy decision belongs only to the owned opening dialog".into());
    }
    external_copy_decisions()
        .lock()
        .map_err(|_| "the external-copy decision registry is unavailable".to_owned())?
        .resolve(&attempt_id, decision)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeProgressKind {
    Creating,
    Opening,
    ProcessingImages,
}

impl NativeProgressKind {
    fn url(self) -> &'static str {
        match self {
            Self::Creating => "dialog.html?kind=creating-project",
            Self::Opening => "dialog.html?kind=opening-project",
            Self::ProcessingImages => "dialog.html?kind=processing-images",
        }
    }

    fn owner_presentation(self) -> OwnerPresentation {
        match self {
            Self::Opening => OwnerPresentation::Replace,
            Self::Creating | Self::ProcessingImages => OwnerPresentation::BlockedBehindDialog,
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

#[derive(Clone, Default)]
pub(crate) struct OpeningImageProgressState(
    Arc<Mutex<Option<crate::ipc_contract::StartupImageProgress>>>,
);

#[tauri::command]
pub(crate) fn opening_image_progress(
    window: WebviewWindow,
    state: tauri::State<'_, OpeningImageProgressState>,
) -> Result<Option<crate::ipc_contract::StartupImageProgress>, String> {
    if window.label() != OPENING_PROGRESS_LABEL {
        return Err("Opening progress belongs only to its owned dialog".into());
    }
    state
        .0
        .lock()
        .map(|progress| *progress)
        .map_err(|_| "the opening progress state is unavailable".into())
}

pub(crate) struct NativeProgressDialog {
    closed: bool,
    decision_attempt: Option<OpeningDecisionAttempt>,
    owner: WebviewWindow,
    owner_presentation: OwnerPresentation,
    window: WebviewWindow,
}

impl NativeProgressDialog {
    pub(crate) fn image_progress_reporter(
        &self,
    ) -> crate::project_bootstrap::StartupProgressReporter {
        let window = self.window.clone();
        let state = window.state::<OpeningImageProgressState>().inner().clone();
        crate::project_bootstrap::StartupProgressReporter::new(move |progress| {
            if let Ok(mut current) = state.0.lock() {
                *current = Some(progress);
                let _ = window.emit_to(window.label(), OPENING_IMAGE_PROGRESS_EVENT, progress);
            }
        })
    }

    pub(crate) async fn request_external_copy_decision(
        &mut self,
        attempt_id: &str,
    ) -> io::Result<OpeningExternalCopyDecision> {
        self.ensure_opening_decision_available("the external-copy decision")?;
        resize_owned_window_width(&self.window, EXTERNAL_COPY_DIALOG_WIDTH)?;
        let decision_receiver = external_copy_decisions()
            .lock()
            .map_err(|_| io::Error::other("the external-copy decision registry is unavailable"))?
            .register(attempt_id)?;
        self.request_opening_decision(
            OpeningDecisionAttempt::ExternalCopy(attempt_id.to_owned()),
            "external-copy",
            decision_receiver,
            "the external-copy decision",
        )
        .await
    }

    pub(crate) async fn request_recovery_decision(
        &mut self,
        attempt_id: &str,
    ) -> io::Result<ProjectRecoveryDecision> {
        self.ensure_opening_decision_available("Recovery")?;
        resize_owned_window_width(&self.window, PROJECT_RECOVERY_DIALOG_WIDTH)?;
        let decision_receiver = project_recovery_decisions()
            .lock()
            .map_err(|_| io::Error::other("the Recovery decision registry is unavailable"))?
            .register(attempt_id)?;
        self.request_opening_decision(
            OpeningDecisionAttempt::Recovery(attempt_id.to_owned()),
            "project-recovery",
            decision_receiver,
            "the Recovery decision",
        )
        .await
    }

    fn ensure_opening_decision_available(&self, name: &str) -> io::Result<()> {
        if self.owner_presentation != OwnerPresentation::Replace || self.decision_attempt.is_some()
        {
            return Err(io::Error::other(format!(
                "{name} is unavailable on this launch dialog"
            )));
        }
        Ok(())
    }

    async fn request_opening_decision<T>(
        &mut self,
        attempt: OpeningDecisionAttempt,
        kind: &str,
        decision_receiver: tokio::sync::oneshot::Receiver<T>,
        name: &str,
    ) -> io::Result<T> {
        self.decision_attempt = Some(attempt.clone());
        let destroyed_attempt = attempt.clone();
        self.window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                destroyed_attempt.cancel();
            }
        });

        let (ready_token, ready_receiver) = owned_window_readiness()
            .lock()
            .map_err(|_| io::Error::other("the owned window readiness registry is unavailable"))?
            .register(self.window.label());
        let current_webview = self
            .window
            .app_handle()
            .get_webview(self.window.label())
            .ok_or_else(|| io::Error::other("the opening presentation is unavailable"))?;
        let mut url = current_webview.url().map_err(io::Error::other)?;
        url.set_path("/dialog.html");
        url.set_query(Some(&format!(
            "kind={kind}&attemptId={}&{OWNED_WINDOW_READY_PARAMETER}={ready_token}",
            encode_unbounded_component(match &attempt {
                OpeningDecisionAttempt::ExternalCopy(attempt_id)
                | OpeningDecisionAttempt::Recovery(attempt_id) => attempt_id,
            }),
        )));
        if let Err(error) = current_webview.navigate(url) {
            cancel_owned_window_readiness(self.window.label(), ready_token);
            self.cancel_opening_decision();
            return Err(io::Error::other(error));
        }

        let ready = tokio::time::timeout(DIALOG_LOAD_TIMEOUT, ready_receiver).await;
        cancel_owned_window_readiness(self.window.label(), ready_token);
        match ready {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                self.cancel_opening_decision();
                return Err(io::Error::other(format!(
                    "{name} dialog readiness became unavailable"
                )));
            }
            Err(_) => {
                self.cancel_opening_decision();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{name} dialog did not become ready"),
                ));
            }
        }

        let decision = decision_receiver.await.map_err(|_| {
            io::Error::other(format!("{name} dialog closed without a terminal decision"))
        });
        self.decision_attempt = None;
        decision
    }

    fn cancel_opening_decision(&mut self) {
        if let Some(attempt) = self.decision_attempt.take() {
            attempt.cancel();
        }
    }

    pub(crate) fn decision_window(&self) -> WebviewWindow {
        self.window.clone()
    }

    pub(crate) fn finish(mut self, restore_owner_window: bool) {
        self.cancel_opening_decision();
        match self.owner_presentation {
            OwnerPresentation::Replace if restore_owner_window => {
                let _ = self.window.destroy_dialog();
                restore_owner(&self.owner);
            }
            OwnerPresentation::BlockedBehindDialog => {
                let _ = dismiss_blocked_dialog(&self.owner, &self.window, restore_owner_window);
            }
            OwnerPresentation::Replace => {
                let _ = self.window.destroy_dialog();
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
    fit_owned_window_frame(window, width, current_size.height)
}

impl Drop for NativeProgressDialog {
    fn drop(&mut self) {
        self.cancel_opening_decision();
        if !self.closed {
            match self.owner_presentation {
                OwnerPresentation::BlockedBehindDialog => {
                    let _ = dismiss_blocked_dialog(&self.owner, &self.window, true);
                }
                OwnerPresentation::Replace => {
                    let _ = self.window.destroy_dialog();
                    release_owner(&self.owner, self.owner_presentation, true);
                }
            }
        }
    }
}

pub(crate) async fn show_native_progress(
    app: &AppHandle,
    owner_label: &str,
    kind: NativeProgressKind,
    progress_webview_data_directory: &Path,
) -> io::Result<NativeProgressDialog> {
    if let Some(state) = app.try_state::<OpeningImageProgressState>() {
        *state
            .0
            .lock()
            .map_err(|_| io::Error::other("the opening progress state is unavailable"))? = None;
    }
    let owner = owned_window(app, owner_label)?;
    #[cfg(debug_assertions)]
    let browser_arguments = desktop_webview_policy::replacement_webview_debug_arguments(
        std::env::var_os(desktop_webview_policy::OPENING_DIALOG_WEBVIEW_DEBUG_PORT_ENV),
    )?;
    #[cfg(not(debug_assertions))]
    let browser_arguments: Option<String> = None;
    // Temporary rendering mitigation for the small progress surface. It does
    // not establish a GPU root cause and leaves the Canvas GPU intact.
    let browser_arguments = format!(
        "{} --disable-gpu",
        browser_arguments
            .unwrap_or_else(|| desktop_webview_policy::WRY_DEFAULT_DISABLED_FEATURES.to_owned())
    );
    let window = build_hidden_owned_window(
        app,
        &owner,
        HiddenOwnedWindowConfig {
            label: OPENING_PROGRESS_LABEL,
            url: kind.url(),
            width: DIALOG_WIDTH,
            height: 126.0 + OWNED_WINDOW_TITLEBAR_HEIGHT,
            browser_arguments: Some(&browser_arguments),
            // A failure in the hidden Global browser must not blank the progress
            // dialog while the independent Project Host is still preparing images.
            browser_data_directory: Some(progress_webview_data_directory),
        },
    )
    .await?;

    let owner_presentation = kind.owner_presentation();
    display_progress_dialog(&owner, &window, kind)?;
    Ok(NativeProgressDialog {
        closed: false,
        decision_attempt: None,
        owner,
        owner_presentation,
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

    display_dialog(&owner, &window, context.owner_presentation(), false)
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
    let (policy_signal, policy_readiness) =
        desktop_webview_policy::page_load_handshake(browser_arguments);
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
        // Wry 0.55 can discard the WebView if MoveFocus fails during creation.
        // The presentation transaction focuses this dialog after it is shown.
        .focused(false)
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

pub(crate) fn display_transition_dialog(
    owner: &WebviewWindow,
    window: &WebviewWindow,
) -> io::Result<()> {
    display_dialog(owner, window, OwnerPresentation::Replace, false)
}

pub(crate) fn display_owned_dialog(
    owner: &WebviewWindow,
    window: &WebviewWindow,
) -> io::Result<()> {
    display_dialog(owner, window, OwnerPresentation::BlockedBehindDialog, false)
}

fn display_progress_dialog(
    owner: &impl DialogOwner,
    window: &impl DialogSurface,
    kind: NativeProgressKind,
) -> io::Result<()> {
    display_dialog(
        owner,
        window,
        kind.owner_presentation(),
        kind == NativeProgressKind::Opening,
    )
}

fn display_dialog(
    owner: &impl DialogOwner,
    window: &impl DialogSurface,
    owner_presentation: OwnerPresentation,
    show_in_taskbar: bool,
) -> io::Result<()> {
    if window.is_dialog_visible()? {
        return Ok(());
    }
    prepare_owner(owner, owner_presentation)?;
    if let Err(error) = window.show_dialog().and_then(|()| {
        // Opening hides Welcome, so this owned window must be explicitly registered
        // with the taskbar after it is shown. Other dialogs keep their visible owner.
        if show_in_taskbar {
            window.set_dialog_taskbar_visible(true)
        } else {
            Ok(())
        }
    }) {
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
    fn set_dialog_taskbar_visible(&self, visible: bool) -> io::Result<()>;
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

    fn set_dialog_taskbar_visible(&self, visible: bool) -> io::Result<()> {
        #[cfg(windows)]
        if visible {
            return crate::native_dialog_taskbar::show_owned_window(self);
        }
        self.set_skip_taskbar(!visible).map_err(io::Error::other)
    }

    fn destroy_dialog(&self) -> io::Result<()> {
        // Tao registers owned windows through ITaskbarList::AddTab. Pair that
        // registration with DeleteTab before destroying the native window.
        let _ = self.set_dialog_taskbar_visible(false);
        self.destroy().map_err(io::Error::other)
    }
}

fn prepare_owner(owner: &impl DialogOwner, presentation: OwnerPresentation) -> io::Result<()> {
    match presentation {
        OwnerPresentation::Replace => {
            let was_visible = owner.is_dialog_owner_visible()?;
            // Tao rewrites native styles when hiding a visible window, clearing
            // WS_DISABLED. Apply the block after that visibility transition.
            owner.hide_dialog_owner()?;
            if let Err(error) = owner.set_dialog_owner_enabled(false) {
                if was_visible {
                    let _ = owner.show_dialog_owner();
                }
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
        fail_disable: bool,
    }

    impl Default for RecordingOwner {
        fn default() -> Self {
            Self {
                actions: RefCell::new(Vec::new()),
                enabled: Cell::new(true),
                visible: Cell::new(true),
                fail_disable: false,
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
        taskbar_visible: Cell<bool>,
        fail_taskbar: bool,
    }

    impl RecordingDialog {
        fn visible() -> Self {
            Self {
                actions: RefCell::new(Vec::new()),
                visible: Cell::new(true),
                taskbar_visible: Cell::new(false),
                fail_taskbar: false,
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
            // Tao rebuilds native window styles when visibility changes. Its
            // cached flags do not include the WS_DISABLED bit set by EnableWindow.
            if self.visible.replace(false) {
                self.enabled.set(true);
            }
            Ok(())
        }

        fn show_dialog_owner(&self) -> io::Result<()> {
            self.actions.borrow_mut().push("show".into());
            self.visible.set(true);
            Ok(())
        }

        fn set_dialog_owner_enabled(&self, enabled: bool) -> io::Result<()> {
            self.actions.borrow_mut().push(format!("enabled:{enabled}"));
            if !enabled && self.fail_disable {
                return Err(io::Error::other("injected owner disable failure"));
            }
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

        fn set_dialog_taskbar_visible(&self, visible: bool) -> io::Result<()> {
            self.actions.borrow_mut().push(format!("taskbar:{visible}"));
            if visible && self.fail_taskbar {
                return Err(io::Error::other("injected taskbar failure"));
            }
            self.taskbar_visible.set(visible);
            Ok(())
        }

        fn destroy_dialog(&self) -> io::Result<()> {
            self.taskbar_visible.set(false);
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

        fn set_dialog_taskbar_visible(&self, visible: bool) -> io::Result<()> {
            self.0.borrow_mut().push(if visible {
                "dialog.taskbar:true"
            } else {
                "dialog.taskbar:false"
            });
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
            NativeProgressKind::Opening.owner_presentation(),
            OwnerPresentation::Replace
        );
        assert_eq!(
            NativeProgressKind::Creating.owner_presentation(),
            OwnerPresentation::BlockedBehindDialog
        );
        assert_eq!(
            NativeProgressKind::ProcessingImages.owner_presentation(),
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
    fn opening_progress_remains_in_the_taskbar_when_welcome_is_hidden() {
        for welcome_visible in [true, false] {
            let owner = RecordingOwner {
                visible: Cell::new(welcome_visible),
                ..RecordingOwner::default()
            };
            // Hidden owned dialogs start with skip_taskbar(true) in the builder.
            let dialog = RecordingDialog::visible();
            dialog.visible.set(false);
            display_progress_dialog(&owner, &dialog, NativeProgressKind::Opening).unwrap();
            assert!(!owner.visible.get());
            assert!(dialog.visible.get());
            assert!(
                dialog.taskbar_visible.get(),
                "Opening hides Welcome, so its progress must provide the taskbar entry"
            );
            assert_eq!(dialog.actions.take(), ["show", "taskbar:true", "focus"]);
            // Changing opening content into an image progress or Recovery decision
            // reuses this visible native surface and must not re-register its button.
            display_progress_dialog(&owner, &dialog, NativeProgressKind::Opening).unwrap();
            assert!(dialog.actions.borrow().is_empty());
            assert!(dialog.taskbar_visible.get());
        }
    }

    #[test]
    fn progress_with_a_visible_owner_does_not_add_another_taskbar_entry() {
        for kind in [
            NativeProgressKind::Creating,
            NativeProgressKind::ProcessingImages,
        ] {
            let owner = RecordingOwner::default();
            let dialog = RecordingDialog::visible();
            dialog.visible.set(false);
            display_progress_dialog(&owner, &dialog, kind).unwrap();
            assert!(owner.visible.get());
            assert!(!owner.enabled.get());
            assert!(!dialog.taskbar_visible.get());
            assert_eq!(dialog.actions.take(), ["show", "focus"]);
        }
    }

    #[test]
    fn a_taskbar_registration_failure_does_not_leave_opening_without_a_visible_owner() {
        let owner = RecordingOwner::default();
        let dialog = RecordingDialog {
            fail_taskbar: true,
            ..RecordingDialog::visible()
        };
        dialog.visible.set(false);
        assert!(display_progress_dialog(&owner, &dialog, NativeProgressKind::Opening).is_err());
        assert!(owner.visible.get());
        assert!(owner.enabled.get());
        assert!(!dialog.visible.get());
        assert!(!dialog.taskbar_visible.get());
        assert_eq!(dialog.actions.take(), ["show", "taskbar:true", "destroy"]);
    }

    #[test]
    fn presenting_an_already_visible_owned_dialog_is_a_noop() {
        let owner = RecordingOwner::default();
        let dialog = RecordingDialog::visible();

        display_dialog(
            &owner,
            &dialog,
            OwnerPresentation::BlockedBehindDialog,
            false,
        )
        .unwrap();

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
    fn recovered_dialog_preserves_pending_readiness_and_discards_a_consumed_token() {
        let label = "recovery-url-test";
        let (token, mut receiver) = owned_window_readiness().lock().unwrap().register(label);
        let original = tauri::Url::parse(&format!(
            "http://tauri.localhost/dialog.html?kind=project-recovery&attemptId=attempt-42&ownedReadyToken={token}#decision"
        )).unwrap();
        assert_eq!(recovery_url(label, original.clone()), original);
        owned_window_readiness()
            .lock()
            .unwrap()
            .signal(label, token)
            .unwrap();
        assert_eq!(receiver.try_recv(), Ok(()));
        let recovered = recovery_url(label, original);
        assert_eq!(
            recovered.as_str(),
            "http://tauri.localhost/dialog.html?kind=project-recovery&attemptId=attempt-42#decision"
        );
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
        let mut registry = OpeningDecisionRegistry::<ProjectRecoveryDecision>::default();
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
        let mut registry = OpeningDecisionRegistry::<ProjectRecoveryDecision>::default();
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
    fn external_copy_decision_registry_is_distinct_and_exactly_correlated() {
        let mut registry = OpeningDecisionRegistry::<OpeningExternalCopyDecision>::default();
        let mut decision = registry
            .register("external-attempt-7")
            .expect("the external-copy attempt is registered");

        assert!(
            registry
                .resolve("external-attempt-8", OpeningExternalCopyDecision::Cancel)
                .is_err()
        );
        registry
            .resolve(
                "external-attempt-7",
                OpeningExternalCopyDecision::SaveCopyAs,
            )
            .expect("the exact opening dialog resolves the attempt");
        assert_eq!(
            decision.try_recv(),
            Ok(OpeningExternalCopyDecision::SaveCopyAs)
        );
        assert!(
            registry
                .resolve("external-attempt-7", OpeningExternalCopyDecision::Cancel)
                .is_err()
        );
    }

    #[test]
    fn failed_opening_block_restores_only_a_previously_visible_owner() {
        for was_visible in [true, false] {
            let owner = RecordingOwner {
                visible: Cell::new(was_visible),
                fail_disable: true,
                ..RecordingOwner::default()
            };

            assert!(prepare_owner(&owner, OwnerPresentation::Replace).is_err());
            assert_eq!(owner.visible.get(), was_visible);
            assert!(owner.enabled.get());
            assert_eq!(owner.actions.borrow().contains(&"show".into()), was_visible);
        }
    }
    #[test]
    fn opening_transition_hides_then_restores_the_owner() {
        let owner = RecordingOwner::default();

        prepare_owner(&owner, OwnerPresentation::Replace).unwrap();
        assert_eq!(owner.actions.take(), ["hide", "enabled:false"]);
        assert!(!owner.enabled.get());
        assert!(!owner.visible.get());

        release_owner(&owner, OwnerPresentation::Replace, false);
        assert_eq!(owner.actions.take(), ["enabled:true", "show"]);
    }
}
