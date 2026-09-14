//! Project-owned generation. Native state, rather than the invoking WebView, owns each attempt.
use crate::{
    generation_operation::{GenerationPresentation, GenerationRequest, execute_generation},
    generation_runner::GenerationRunner,
    ipc_contract::{GenerationDecision, GenerationOptions, GenerationProgress, GenerationView},
    native_dialog_window::{self, HiddenOwnedWindowConfig},
    project_host::ProjectHost,
    project_ui_operations::{ProjectUiBatchPause, ProjectUiOperations},
};
use myalbuns_core::{ProjectCore, ProjectTemplate};
use myalbuns_paths::AppPaths;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};
use tauri_plugin_dialog::{DialogExt, FilePath};

const LABEL: &str = "generation";
const PROGRESS: &str = "generation-progress";
const VIEW_EVENT: &str = "myalbuns://generation-view";
const PROGRESS_EVENT: &str = "myalbuns://generation-progress";

struct TemplateOwner {
    name: String,
    template: ProjectTemplate,
    _pause: ProjectUiBatchPause,
}
pub(crate) struct GenerationWindowState {
    paths: AppPaths,
    serial: tokio::sync::Mutex<()>,
    template: Mutex<Option<TemplateOwner>>,
    runner: Arc<tokio::sync::Mutex<Option<GenerationRunner>>>,
    view: Mutex<Option<GenerationView>>,
    progress: Mutex<Option<GenerationProgress>>,
    active: AtomicBool,
    close_requested: AtomicBool,
    cancel: Arc<AtomicBool>,
    result_ready: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl GenerationWindowState {
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            serial: tokio::sync::Mutex::new(()),
            template: Mutex::new(None),
            runner: Arc::new(tokio::sync::Mutex::new(None)),
            view: Mutex::new(None),
            progress: Mutex::new(None),
            active: AtomicBool::new(false),
            close_requested: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            result_ready: Mutex::new(None),
        }
    }
    fn publish(&self, view: GenerationView) {
        *self
            .view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(view);
    }
}
fn configuration(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == LABEL {
        Ok(())
    } else {
        Err("Janela de geração inválida.".into())
    }
}
struct Attempt(AppHandle);
impl Attempt {
    fn begin(app: &AppHandle) -> Result<Self, String> {
        let state = app.state::<GenerationWindowState>();
        if state.close_requested.load(Ordering::Acquire) {
            return Err("A janela de geração está fechando.".into());
        }
        state
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Aguarde a operação atual.")?;
        state.cancel.store(false, Ordering::Release);
        let attempt = Self(app.clone());
        if state.close_requested.load(Ordering::Acquire) {
            state.cancel.store(true, Ordering::Release);
            return Err("A janela de geração está fechando.".into());
        }
        Ok(attempt)
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        self.0
            .state::<GenerationWindowState>()
            .active
            .store(false, Ordering::Release);
        if self
            .0
            .state::<GenerationWindowState>()
            .close_requested
            .load(Ordering::Acquire)
            && let Some(window) = self.0.get_webview_window(LABEL)
        {
            let _ = window.destroy();
        }
    }
}

#[tauri::command]
pub(crate) async fn open_project_generation(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<(), String> {
    if window.label() != "project" {
        return Err("Abra a geração a partir do Projeto modelo.".into());
    }
    let state = app.state::<GenerationWindowState>();
    let _serial = state.serial.lock().await;
    if let Some(existing) = app.get_webview_window(LABEL) {
        return existing.set_focus().map_err(|error| error.to_string());
    }
    let operations = app.state::<ProjectUiOperations>();
    state.close_requested.store(false, Ordering::Release);
    let pause = operations.pause_for_batch()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !operations.is_idle() {
        if tokio::time::Instant::now() >= deadline {
            return Err("Aguarde a operação do Projeto terminar e tente novamente.".into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let (name, template) = app.state::<ProjectHost>().generation_template()?;
    *state.template.lock().map_err(|_| "Modelo indisponível.")? = Some(TemplateOwner {
        name,
        template,
        _pause: pause,
    });
    let result = build(&app, &window, LABEL, "generation.html", 800.0, 478.0).await;
    match result {
        Ok(dialog) => {
            if let Err(error) = native_dialog_window::display_owned_dialog(&window, &dialog) {
                state
                    .template
                    .lock()
                    .map_err(|_| "Modelo indisponível.")?
                    .take();
                return Err(error.to_string());
            }
            Ok(())
        }
        Err(error) => {
            state
                .template
                .lock()
                .map_err(|_| "Modelo indisponível.")?
                .take();
            Err(error)
        }
    }
}

async fn build(
    app: &AppHandle,
    owner: &WebviewWindow,
    label: &str,
    url: &str,
    width: f64,
    height: f64,
) -> Result<WebviewWindow, String> {
    let state = app.state::<GenerationWindowState>();
    let namespace = app
        .state::<crate::project_webview_authority::ProjectWebviewAuthority>()
        .current_namespace();
    let profile = state
        .paths
        .webview_data_directory(&namespace)
        .map_err(|error| error.to_string())?;
    let window = native_dialog_window::build_hidden_owned_window(
        app,
        owner,
        HiddenOwnedWindowConfig {
            label,
            url,
            width,
            height,
            browser_arguments: None,
            browser_data_directory: Some(&profile),
        },
    )
    .await
    .map_err(|error| error.to_string())?;
    window
        .set_title("Gerar Projetos em lote")
        .map_err(|error| error.to_string())?;
    Ok(window)
}

#[tauri::command]
pub(crate) fn generation_model(app: AppHandle) -> Result<String, String> {
    app.state::<GenerationWindowState>()
        .template
        .lock()
        .map_err(|_| "Modelo indisponível.")?
        .as_ref()
        .map(|model| model.name.clone())
        .ok_or("Modelo indisponível.".into())
}
#[tauri::command]
pub(crate) fn generation_current(app: AppHandle) -> Option<GenerationView> {
    app.state::<GenerationWindowState>()
        .view
        .lock()
        .ok()
        .and_then(|view| view.clone())
}
#[tauri::command]
pub(crate) fn generation_progress(app: AppHandle) -> Option<GenerationProgress> {
    app.state::<GenerationWindowState>()
        .progress
        .lock()
        .ok()
        .and_then(|value| value.clone())
}
#[tauri::command]
pub(crate) async fn generation_choose_folder(
    window: WebviewWindow,
) -> Result<Option<String>, String> {
    configuration(&window)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .pick_folder(move |selection| {
            let _ = sender.send(selection);
        });
    match receiver
        .await
        .map_err(|_| "Não foi possível escolher a pasta.")?
    {
        Some(FilePath::Path(path)) => Ok(Some(path.to_string_lossy().into())),
        Some(_) => Err("Escolha uma pasta do Windows.".into()),
        None => Ok(None),
    }
}
#[tauri::command]
pub(crate) async fn generation_count(
    window: WebviewWindow,
    source: String,
) -> Result<usize, String> {
    configuration(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        GenerationRunner::count_folders(Path::new(&source))
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) async fn generation_prepare(
    app: AppHandle,
    window: WebviewWindow,
    options: GenerationOptions,
) -> Result<GenerationView, String> {
    configuration(&window)?;
    let state = app.state::<GenerationWindowState>();
    let template = state
        .template
        .lock()
        .map_err(|_| "Modelo indisponível.")?
        .as_ref()
        .ok_or("Modelo indisponível.")?
        .template
        .clone();
    let core = ProjectCore::new().with_identity_storage_roots(
        state.paths.project_identity_leases_dir(),
        state.paths.project_identities_dir(),
    );
    run_request(
        app,
        window,
        GenerationRequest::Prepare {
            options,
            template: Box::new(template),
            core,
        },
    )
    .await
}
#[tauri::command]
pub(crate) async fn generation_decide(
    app: AppHandle,
    window: WebviewWindow,
    id: Option<String>,
    decision: GenerationDecision,
) -> Result<GenerationView, String> {
    configuration(&window)?;
    update(app, move |runner| runner.decide(id.as_deref(), decision)).await
}
#[tauri::command]
pub(crate) async fn generation_recheck(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<GenerationView, String> {
    configuration(&window)?;
    run_request(app, window, GenerationRequest::Recheck).await
}
async fn update(
    app: AppHandle,
    change: impl FnOnce(&mut GenerationRunner) -> Result<(), String> + Send + 'static,
) -> Result<GenerationView, String> {
    let attempt = Attempt::begin(&app)?;
    tauri::async_runtime::spawn(async move {
        let _attempt = attempt;
        let state = app.state::<GenerationWindowState>();
        let mut runner = state.runner.clone().lock_owned().await;
        let view = tauri::async_runtime::spawn_blocking(move || {
            let runner = runner.as_mut().ok_or("Verifique as pastas primeiro.")?;
            change(runner)?;
            Ok::<_, String>(runner.view())
        })
        .await
        .map_err(|error| error.to_string())??;
        state.publish(view.clone());
        Ok(view)
    })
    .await
    .map_err(|error| error.to_string())?
}

struct ProgressSurface {
    window: WebviewWindow,
    owner: WebviewWindow,
}
impl Drop for ProgressSurface {
    fn drop(&mut self) {
        let _ = self.window.destroy();
        if !self
            .owner
            .app_handle()
            .state::<GenerationWindowState>()
            .close_requested
            .load(Ordering::Acquire)
        {
            native_dialog_window::restore_owner(&self.owner);
        }
    }
}
#[derive(Clone)]
struct NativeGenerationPresentation {
    app: AppHandle,
    owner: WebviewWindow,
}
impl GenerationPresentation for NativeGenerationPresentation {
    type Surface = ProgressSurface;
    async fn open(&self, progress: GenerationProgress) -> Result<ProgressSurface, String> {
        self.progress(progress);
        let dialog = build(
            &self.app,
            &self.owner,
            PROGRESS,
            "generation.html?surface=progress",
            400.0,
            185.0,
        )
        .await?;
        native_dialog_window::display_transition_dialog(&self.owner, &dialog)
            .map_err(|error| error.to_string())?;
        Ok(ProgressSurface {
            window: dialog,
            owner: self.owner.clone(),
        })
    }
    fn progress(&self, progress: GenerationProgress) {
        *self
            .app
            .state::<GenerationWindowState>()
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(progress.clone());
        let _ = self.app.emit_to(PROGRESS, PROGRESS_EVENT, progress);
    }
    async fn finish(&self, view: &GenerationView) -> Result<(), String> {
        let state = self.app.state::<GenerationWindowState>();
        if state.close_requested.load(Ordering::Acquire) {
            return Ok(());
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *state
            .result_ready
            .lock()
            .map_err(|_| "Geração indisponível.")? = Some(sender);
        state.publish(view.clone());
        let _ = self.app.emit_to(LABEL, VIEW_EVENT, view);
        let _ = tokio::time::timeout(Duration::from_secs(5), receiver).await;
        state
            .result_ready
            .lock()
            .map_err(|_| "Geração indisponível.")?
            .take();
        Ok(())
    }
}

async fn run_request(
    app: AppHandle,
    window: WebviewWindow,
    request: GenerationRequest,
) -> Result<GenerationView, String> {
    let attempt = Attempt::begin(&app)?;
    tauri::async_runtime::spawn(async move {
        let _attempt = attempt;
        let state = app.state::<GenerationWindowState>();
        let presentation = NativeGenerationPresentation {
            app: app.clone(),
            owner: window,
        };
        let view = execute_generation(
            request,
            state.runner.clone(),
            state.cancel.clone(),
            presentation,
        )
        .await?;
        state.publish(view.clone());
        Ok(view)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn generation_run(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<GenerationView, String> {
    configuration(&window)?;
    run_request(app, window, GenerationRequest::Run).await
}
#[tauri::command]
pub(crate) fn generation_result_ready(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    configuration(&window)?;
    if let Some(sender) = app
        .state::<GenerationWindowState>()
        .result_ready
        .lock()
        .map_err(|_| "Geração indisponível.")?
        .take()
    {
        let _ = sender.send(());
    }
    Ok(())
}
#[tauri::command]
pub(crate) fn generation_cancel(app: AppHandle) {
    app.state::<GenerationWindowState>()
        .cancel
        .store(true, Ordering::Release);
}
#[tauri::command]
pub(crate) fn close_project_generation(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<(), String> {
    configuration(&window)?;
    let state = app.state::<GenerationWindowState>();
    state.close_requested.store(true, Ordering::Release);
    if state.active.load(Ordering::Acquire) {
        state.cancel.store(true, Ordering::Release);
        return Ok(());
    }
    window.destroy().map_err(|error| error.to_string())
}
pub(crate) fn on_window_event(window: &tauri::Window, event: &WindowEvent) -> bool {
    let app = window.app_handle();
    let state = app.state::<GenerationWindowState>();
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == LABEL {
            state.close_requested.store(true, Ordering::Release);
        }
        if window.label() == "project" && state.template.lock().is_ok_and(|model| model.is_some()) {
            api.prevent_close();
            return true;
        }
        if window.label() == PROGRESS
            || window.label() == LABEL && state.active.load(Ordering::Acquire)
        {
            api.prevent_close();
            state.cancel.store(true, Ordering::Release);
            return true;
        }
    }
    if window.label() == LABEL && matches!(event, WindowEvent::Destroyed) {
        state
            .template
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        state
            .view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Ok(mut runner) = state.runner.try_lock() {
            runner.take();
        }
        if let Some(owner) = app.get_webview_window("project") {
            native_dialog_window::release_blocked_owner_if_disabled(&owner, true);
        }
    }
    false
}
