use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use myalbuns_logging::ProcessRole;
#[cfg(windows)]
use myalbuns_paths::ProcessInstanceHandle;
use myalbuns_paths::{AppPaths, AppPathsError, NativePathDto, ProcessInstanceId, RootBindingPlan};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    cache_service::{CacheScheduledCleanupOutcome, CacheService},
    desktop_webview_policy,
    global_activation::{GlobalActivationEntry, PrimaryGlobalActivation, enter_global_activation},
    graphics_launch_gate::{
        GRAPHICS_GATE_TIMEOUT, GraphicsGateCompletion, GraphicsGateReport, GraphicsLaunchGate,
    },
    logging,
    native_dialog_window::{self, LaunchProgressKind, ProjectFailureDialogContext},
    native_project_dialog, path_io,
    project_bootstrap::{
        BootstrapFailure, BootstrapFailureKind, BootstrapOutcome, CreateWriteAuthorization,
        FailureCode, FailureStage, InitialProjectConfiguration,
        InitialProjectCreationConfiguration, PendingExternalCopyProcess,
        ProjectConfigurationValidation, ProjectHostBootstrap, TargetAuthority,
        validate_configuration,
    },
    provisional_decoratives::{
        ProvisionalDecorativeError, ProvisionalDecorativeRegistry,
        ProvisionalProjectCreationConfiguration,
    },
    recent_projects::{RecentProjectSummary, RecentProjectsStore},
};

pub(crate) const GLOBAL_WINDOW_LABEL: &str = "global";
const GLOBAL_ACTIVATION_TERMINAL_EVENT: &str = "myalbuns://global-activation-terminal";
const GLOBAL_WEBVIEW_NAMESPACE: &str = "global";
const HOST_TERMINAL_TIMEOUT: Duration = Duration::from_secs(30);

type ScheduledCleanupResult = Result<CacheScheduledCleanupOutcome, String>;

#[derive(Clone, Default)]
struct ScheduledCleanupGate {
    result: Arc<Mutex<Option<ScheduledCleanupResult>>>,
    notification: Arc<tokio::sync::Notify>,
}

impl ScheduledCleanupGate {
    fn complete(&self, result: ScheduledCleanupResult) {
        let mut slot = self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_some() {
            return;
        }
        *slot = Some(result);
        drop(slot);
        self.notification.notify_waiters();
    }

    async fn wait(&self) -> ScheduledCleanupResult {
        loop {
            let notified = self.notification.notified();
            if let Some(result) = self
                .result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
            {
                return result;
            }
            notified.await;
        }
    }
}

#[cfg(debug_assertions)]
const WEBDRIVER_PROJECT_ENV: &str = "MYALBUNS_TAURI_WEBDRIVER_PROJECT";
#[cfg(debug_assertions)]
const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";

struct PendingExternalCopyState<T> {
    entries: VecDeque<T>,
    handoff_completed: bool,
    deferred_failure: Option<ProjectLaunchFailure>,
}

impl<T> Default for PendingExternalCopyState<T> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            handoff_completed: false,
            deferred_failure: None,
        }
    }
}

struct PendingExternalCopyCoordinator<T> {
    state: Arc<Mutex<PendingExternalCopyState<T>>>,
}

impl<T> Clone for PendingExternalCopyCoordinator<T> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> Default for PendingExternalCopyCoordinator<T> {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(PendingExternalCopyState::default())),
        }
    }
}

impl<T> PendingExternalCopyCoordinator<T> {
    fn remember(&self, pending: T) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .push_back(pending);
    }

    fn take(&self) -> Option<T> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .pop_front()
    }

    fn settle_activation_batch(&self, summary: &ActivationBatchSummary) -> ProjectLaunchOutcome {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.entries.is_empty() {
            return summary.terminal();
        }
        if summary.has_handoff() {
            state.handoff_completed = true;
        }
        if state.deferred_failure.is_none() {
            state.deferred_failure = summary.first_failure();
        }
        ProjectLaunchOutcome::ExternalCopyNotWritable
    }

    fn settle_external_copy_attempt(
        &self,
        outcome: ProjectLaunchOutcome,
    ) -> (ProjectLaunchOutcome, bool) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(
            outcome,
            ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
        ) {
            state.handoff_completed = true;
        }
        if state.deferred_failure.is_none()
            && let ProjectLaunchOutcome::Failed { error } = &outcome
        {
            state.deferred_failure = Some(error.clone());
        }
        if !state.entries.is_empty() {
            return (ProjectLaunchOutcome::ExternalCopyNotWritable, false);
        }
        if let Some(error) = state.deferred_failure.take() {
            state.handoff_completed = false;
            return (ProjectLaunchOutcome::Failed { error }, false);
        }
        let should_exit = state.handoff_completed;
        state.handoff_completed = false;
        (outcome, should_exit)
    }
}

#[derive(Clone)]
struct GlobalRuntimeState {
    bootstrap: ProjectHostBootstrap,
    graphics_gate: GraphicsLaunchGate,
    recent_projects: RecentProjectsStore,
    startup_failure: Arc<Mutex<Option<ProjectLaunchFailure>>>,
    activation_terminals: GlobalActivationTerminalStore,
    pending_external_copies: PendingExternalCopyCoordinator<PendingExternalCopyProcess>,
    scheduled_cleanup: ScheduledCleanupGate,
    global_activation: Option<Arc<PrimaryGlobalActivation>>,
    activation_serial: Arc<tokio::sync::Mutex<()>>,
    exit_requested: Arc<AtomicBool>,
    runtime_exiting: Arc<AtomicBool>,
}

impl GlobalRuntimeState {
    fn new(
        app_paths: &AppPaths,
        activation_projects: Vec<PathBuf>,
        global_activation: Option<Arc<PrimaryGlobalActivation>>,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            bootstrap: ProjectHostBootstrap::new(std::env::current_exe()?, HOST_TERMINAL_TIMEOUT),
            graphics_gate: GraphicsLaunchGate::new(activation_projects),
            recent_projects: RecentProjectsStore::new(app_paths),
            startup_failure: Arc::new(Mutex::new(None)),
            activation_terminals: GlobalActivationTerminalStore::default(),
            pending_external_copies: PendingExternalCopyCoordinator::default(),
            scheduled_cleanup: ScheduledCleanupGate::default(),
            global_activation,
            activation_serial: Arc::new(tokio::sync::Mutex::new(())),
            exit_requested: Arc::new(AtomicBool::new(false)),
            runtime_exiting: Arc::new(AtomicBool::new(false)),
        })
    }

    fn startup_failure(&self) -> Option<ProjectLaunchFailure> {
        self.startup_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn record_startup_failure(&self, failure: ProjectLaunchFailure) {
        *self
            .startup_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(failure);
    }

    fn project_host_gate_rejection(&self) -> Option<ProjectLaunchOutcome> {
        (!self.graphics_gate.allows_project_host()).then(|| ProjectLaunchOutcome::Failed {
            error: graphics_gate_failure(),
        })
    }

    fn take_external_copy(&self) -> Option<PendingExternalCopyProcess> {
        self.pending_external_copies.take()
    }

    fn remember_external_copy(&self, pending: PendingExternalCopyProcess) {
        self.pending_external_copies.remember(pending);
    }

    fn settle_external_copy_attempt(
        &self,
        outcome: ProjectLaunchOutcome,
    ) -> (ProjectLaunchOutcome, bool) {
        self.pending_external_copies
            .settle_external_copy_attempt(outcome)
    }

    async fn scheduled_cleanup_failure(&self) -> Option<ProjectLaunchFailure> {
        self.scheduled_cleanup
            .wait()
            .await
            .err()
            .map(|reason| startup_cleanup_failure(&reason))
    }

    fn cancel_requested_exit(&self) {
        self.exit_requested.store(false, Ordering::Release);
        if let Some(activation) = &self.global_activation {
            activation.resume_accepting();
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectLaunchFailure {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<FailureStage>,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub(crate) enum ProjectLaunchOutcome {
    Opened,
    Focused,
    ExternalCopyNotWritable,
    Cancelled,
    Failed { error: ProjectLaunchFailure },
}

struct ActivationBatchSummary {
    success: ProjectLaunchOutcome,
    first_non_success: Option<ProjectLaunchOutcome>,
    opened_count: u32,
    focused_count: u32,
    external_copy_count: u32,
    failed_count: u32,
}

impl Default for ActivationBatchSummary {
    fn default() -> Self {
        Self {
            success: ProjectLaunchOutcome::Focused,
            first_non_success: None,
            opened_count: 0,
            focused_count: 0,
            external_copy_count: 0,
            failed_count: 0,
        }
    }
}

impl ActivationBatchSummary {
    fn observe(&mut self, outcome: ProjectLaunchOutcome) {
        match outcome {
            ProjectLaunchOutcome::Opened => {
                self.opened_count += 1;
                self.success = ProjectLaunchOutcome::Opened;
            }
            ProjectLaunchOutcome::Focused => self.focused_count += 1,
            ProjectLaunchOutcome::ExternalCopyNotWritable => {
                self.external_copy_count += 1;
            }
            other => {
                self.failed_count += 1;
                if self.first_non_success.is_none() {
                    self.first_non_success = Some(other);
                }
            }
        }
    }

    fn has_handoff(&self) -> bool {
        self.opened_count > 0 || self.focused_count > 0
    }

    fn has_external_copy(&self) -> bool {
        self.external_copy_count > 0
    }

    fn first_failure(&self) -> Option<ProjectLaunchFailure> {
        match &self.first_non_success {
            Some(ProjectLaunchOutcome::Failed { error }) => Some(error.clone()),
            _ => None,
        }
    }

    fn terminal(&self) -> ProjectLaunchOutcome {
        if self.has_external_copy() {
            ProjectLaunchOutcome::ExternalCopyNotWritable
        } else {
            self.first_non_success
                .clone()
                .unwrap_or_else(|| self.success.clone())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GlobalActivationTerminal {
    sequence: u64,
    outcome: ProjectLaunchOutcome,
}

#[derive(Default)]
struct GlobalActivationTerminalState {
    sequence: u64,
    latest: Option<GlobalActivationTerminal>,
}

#[derive(Clone, Default)]
struct GlobalActivationTerminalStore {
    state: Arc<Mutex<GlobalActivationTerminalState>>,
}

impl GlobalActivationTerminalStore {
    fn record(&self, outcome: ProjectLaunchOutcome) -> Option<GlobalActivationTerminal> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sequence = state.sequence.checked_add(1)?;
        let terminal = GlobalActivationTerminal { sequence, outcome };
        state.sequence = sequence;
        state.latest = Some(terminal.clone());
        Some(terminal)
    }

    fn latest(&self) -> Option<GlobalActivationTerminal> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latest
            .clone()
    }
}

#[derive(Clone, Debug)]
enum ConfirmedLaunch {
    OpenExisting,
    CreateNew {
        configuration: Box<InitialProjectCreationConfiguration>,
        authorization: CreateWriteAuthorization,
    },
}

#[tauri::command]
async fn complete_graphics_gate(
    app: AppHandle,
    report: GraphicsGateReport,
) -> Option<ProjectLaunchOutcome> {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return Some(ProjectLaunchOutcome::Failed { error });
    }
    match state.graphics_gate.complete(report) {
        GraphicsGateCompletion::Ready(projects) if !projects.is_empty() => {
            let _serial = state.activation_serial.lock().await;
            let outcome = launch_activation_batch(state.clone(), projects).await;
            match &outcome {
                ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused => {
                    exit_global_after_handoff(&app)
                }
                ProjectLaunchOutcome::Failed { error } => {
                    state.cancel_requested_exit();
                    state.record_startup_failure(error.clone());
                    show_existing_global_window(&app);
                }
                ProjectLaunchOutcome::ExternalCopyNotWritable | ProjectLaunchOutcome::Cancelled => {
                    state.cancel_requested_exit();
                    show_existing_global_window(&app)
                }
            }
            Some(outcome)
        }
        GraphicsGateCompletion::Rejected => {
            show_existing_global_window(&app);
            None
        }
        GraphicsGateCompletion::Ready(_) | GraphicsGateCompletion::AlreadyFinal => None,
    }
}

#[tauri::command]
async fn open_project(app: AppHandle) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return ProjectLaunchOutcome::Failed { error };
    }
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Projeto MyAlbuns", &["myalbuns"])
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = match receiver.await {
        Ok(selection) => selection,
        Err(error) => {
            tracing::warn!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::Global.as_str(),
                error = %error,
                event = "project_open_dialog_failed",
            );
            return ProjectLaunchOutcome::Failed {
                error: simple_failure(
                    "dialog_unavailable",
                    "Não foi possível concluir o diálogo de abertura.",
                    "Tente novamente.",
                ),
            };
        }
    };
    let Some(selection) = selection else {
        return ProjectLaunchOutcome::Cancelled;
    };
    let FilePath::Path(path) = selection else {
        return ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "invalid_path",
                "O local escolhido não é um arquivo do Windows válido.",
                "Escolha um arquivo .myalbuns local ou de uma unidade disponível.",
            ),
        };
    };

    let outcome = launch_confirmed_project_with_progress(
        &app,
        state,
        path,
        ConfirmedLaunch::OpenExisting,
        LaunchProgressKind::Opening,
        GLOBAL_WINDOW_LABEL,
        false,
    )
    .await;
    if matches!(
        outcome,
        ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
    ) {
        exit_global_after_handoff(&app);
    }
    outcome
}

#[tauri::command]
async fn save_external_copy_as(app: AppHandle, window: WebviewWindow) -> ProjectLaunchOutcome {
    if window.label() != GLOBAL_WINDOW_LABEL {
        return ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "invalid_external_copy_surface",
                "A cópia editável deve ser criada pela Janela de Boas-vindas.",
                "Volte à Tela de Boas-vindas e tente novamente.",
            ),
        };
    }
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return ProjectLaunchOutcome::Failed { error };
    }
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let activation_serial = Arc::clone(&state.activation_serial);
    let _serial = activation_serial.lock().await;
    let Some(pending) = state.take_external_copy() else {
        return ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "external_copy_source_expired",
                "A Cópia externa precisa ser validada novamente.",
                "Abra novamente a cópia somente leitura.",
            ),
        };
    };
    let (destination_path, authorization) =
        match native_project_dialog::choose_project_destination(&window).await {
            Ok(native_project_dialog::ProjectSaveDialogOutcome::Cancelled) => {
                return finish_external_copy_attempt(&app, &state, ProjectLaunchOutcome::Cancelled);
            }
            Ok(native_project_dialog::ProjectSaveDialogOutcome::Selected {
                path,
                authorization,
            }) => (path, authorization),
            Err(error) => {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    error = %error,
                    event = "external_copy_destination_dialog_failed",
                );
                return finish_external_copy_attempt(
                    &app,
                    &state,
                    ProjectLaunchOutcome::Failed {
                        error: simple_failure(
                            "dialog_unavailable",
                            "Não foi possível concluir o diálogo para salvar a cópia.",
                            "Tente novamente.",
                        ),
                    },
                );
            }
        };
    let root_bindings = match path_io::capture_root_bindings(vec![destination_path.clone()]).await {
        Ok(root_bindings) => root_bindings,
        Err(error) => {
            return finish_external_copy_attempt(
                &app,
                &state,
                ProjectLaunchOutcome::Failed {
                    error: binding_failure(error),
                },
            );
        }
    };
    let destination_path = NativePathDto::from(destination_path);
    let recent_path = destination_path.clone();
    let destination = TargetAuthority {
        logical_target: destination_path,
        root_bindings,
    };
    let bootstrap = state.bootstrap.clone();
    let recent_projects = state.recent_projects.clone();
    let outcome = match tauri::async_runtime::spawn_blocking(move || {
        let outcome = bootstrap.save_external_copy_as(pending, destination, authorization)?;
        let recent_result = match &outcome {
            BootstrapOutcome::Ready(ready) => {
                Some(recent_projects.promote(&ready.project_id, recent_path))
            }
            BootstrapOutcome::FocusExisting { .. } => None,
            BootstrapOutcome::ExternalCopyNotWritable(_) => None,
        };
        Ok::<_, BootstrapFailure>((outcome, recent_result))
    })
    .await
    {
        Ok(Ok((BootstrapOutcome::Ready(ready), recent_result))) => {
            if recent_result.is_some_and(|result| result.is_err()) {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id = ready.project_id,
                    event = "recent_project_promotion_failed",
                );
            }
            ProjectLaunchOutcome::Opened
        }
        Ok(Ok((
            BootstrapOutcome::FocusExisting {
                project_id,
                owner_process,
            },
            _,
        ))) => resolve_focus_existing(project_id, owner_process),
        Ok(Ok((BootstrapOutcome::ExternalCopyNotWritable(pending), _))) => {
            drop(pending);
            ProjectLaunchOutcome::Failed {
                error: simple_failure(
                    "invalid_host_terminal",
                    "O Host não concluiu a criação da cópia.",
                    "Abra novamente a cópia somente leitura.",
                ),
            }
        }
        Ok(Err(failure)) => ProjectLaunchOutcome::Failed {
            error: bootstrap_failure(failure),
        },
        Err(_) => ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "host_unavailable",
                "Não foi possível iniciar a Janela da nova cópia.",
                "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
            ),
        },
    };
    finish_external_copy_attempt(&app, &state, outcome)
}

fn finish_external_copy_attempt(
    app: &AppHandle,
    state: &GlobalRuntimeState,
    outcome: ProjectLaunchOutcome,
) -> ProjectLaunchOutcome {
    let (outcome, should_exit) = state.settle_external_copy_attempt(outcome);
    if should_exit {
        exit_global_after_handoff(app);
    }
    outcome
}

#[tauri::command]
fn validate_project_configuration(
    configuration: InitialProjectConfiguration,
) -> ProjectConfigurationValidation {
    validate_configuration(configuration)
}

#[tauri::command]
async fn create_project(
    app: AppHandle,
    window: WebviewWindow,
    configuration: ProvisionalProjectCreationConfiguration,
) -> ProjectLaunchOutcome {
    if window.label() != GLOBAL_WINDOW_LABEL {
        return ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "invalid_creation_surface",
                "A criação deve ser iniciada pelo fluxo de Novo Projeto.",
                "Volte à Tela de Boas-vindas e tente novamente.",
            ),
        };
    }
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return ProjectLaunchOutcome::Failed { error };
    }
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let provisional_decoratives = app.state::<ProvisionalDecorativeRegistry>().inner().clone();
    let resolution_registry = provisional_decoratives.clone();
    let configuration = match tauri::async_runtime::spawn_blocking(move || {
        resolution_registry.resolve_creation_configuration(configuration)
    })
    .await
    {
        Ok(Ok(configuration)) => configuration,
        Ok(Err(error)) => {
            return ProjectLaunchOutcome::Failed {
                error: decorative_resolution_failure(error),
            };
        }
        Err(_) => {
            return ProjectLaunchOutcome::Failed {
                error: simple_failure(
                    "decorative_resolution_unavailable",
                    "N\u{e3}o foi poss\u{ed}vel preparar as Imagens decorativas.",
                    "Tente novamente.",
                ),
            };
        }
    };
    let destination = match native_project_dialog::choose_project_destination(&window).await {
        Ok(native_project_dialog::ProjectSaveDialogOutcome::Cancelled) => {
            return ProjectLaunchOutcome::Cancelled;
        }
        Ok(native_project_dialog::ProjectSaveDialogOutcome::Selected {
            path,
            authorization,
        }) => (path, authorization),
        Err(error) => {
            tracing::warn!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::Global.as_str(),
                error = %error,
                event = "project_creation_dialog_failed",
            );
            return ProjectLaunchOutcome::Failed {
                error: simple_failure(
                    "dialog_unavailable",
                    "Não foi possível concluir o diálogo de criação.",
                    "Tente novamente.",
                ),
            };
        }
    };

    let outcome = launch_confirmed_project_with_progress(
        &app,
        state,
        destination.0,
        ConfirmedLaunch::CreateNew {
            configuration: Box::new(configuration),
            authorization: destination.1,
        },
        LaunchProgressKind::Creating,
        GLOBAL_WINDOW_LABEL,
        true,
    )
    .await;
    if matches!(
        outcome,
        ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
    ) {
        provisional_decoratives.clear();
        exit_global_after_handoff(&app);
    }
    outcome
}

#[tauri::command]
async fn recent_projects(
    state: tauri::State<'_, GlobalRuntimeState>,
) -> Result<Vec<RecentProjectSummary>, ProjectLaunchFailure> {
    let state = state.inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return Err(error);
    }
    let store = state.recent_projects.clone();
    tauri::async_runtime::spawn_blocking(move || store.list())
        .await
        .map_err(|_| state_failure())?
        .map_err(|_| state_failure())
}

#[tauri::command]
async fn open_recent_project(app: AppHandle, project_id: String) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return ProjectLaunchOutcome::Failed { error };
    }
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let store = state.recent_projects.clone();
    let lookup_id = project_id.clone();
    let path = match tauri::async_runtime::spawn_blocking(move || store.path_for(&lookup_id)).await
    {
        Ok(Ok(Some(path))) => path.into_path_buf(),
        Ok(Ok(None)) => {
            return ProjectLaunchOutcome::Failed {
                error: simple_failure(
                    "recent_project_missing",
                    "Este Projeto não está mais na lista de recentes.",
                    "Use Abrir Projeto para escolhê-lo novamente.",
                ),
            };
        }
        Ok(Err(_)) | Err(_) => {
            return ProjectLaunchOutcome::Failed {
                error: state_failure(),
            };
        }
    };
    let outcome = launch_confirmed_project_with_progress(
        &app,
        state,
        path,
        ConfirmedLaunch::OpenExisting,
        LaunchProgressKind::Opening,
        GLOBAL_WINDOW_LABEL,
        false,
    )
    .await;
    if matches!(
        outcome,
        ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
    ) {
        exit_global_after_handoff(&app);
    }
    outcome
}

#[tauri::command]
async fn startup_open_failure(app: AppHandle) -> Option<ProjectLaunchFailure> {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(error) = state.scheduled_cleanup_failure().await {
        return Some(error);
    }
    state.startup_failure()
}

#[tauri::command]
async fn show_project_failure_dialog(
    app: AppHandle,
    context: ProjectFailureDialogContext,
    error: ProjectLaunchFailure,
) {
    if let Err(dialog_error) = native_dialog_window::show_project_failure(
        &app,
        context,
        &error.message,
        error.action.as_deref(),
    )
    .await
    {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            error = %dialog_error,
            event = "project_failure_dialog_unavailable",
        );
    }
}

async fn launch_confirmed_project_with_progress(
    app: &AppHandle,
    state: GlobalRuntimeState,
    project_path: PathBuf,
    launch: ConfirmedLaunch,
    progress_kind: LaunchProgressKind,
    progress_owner_label: &str,
    restore_owner_on_failure: bool,
) -> ProjectLaunchOutcome {
    let progress =
        match native_dialog_window::show_launch_progress(app, progress_owner_label, progress_kind)
            .await
        {
            Ok(progress) => Some(progress),
            Err(error) => {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    error = %error,
                    event = "project_launch_progress_dialog_unavailable",
                );
                None
            }
        };
    let outcome = launch_confirmed_project(state, project_path, launch).await;
    if let Some(progress) = progress {
        progress.finish(
            restore_owner_on_failure
                && !matches!(
                    outcome,
                    ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
                ),
        );
    }
    outcome
}

#[tauri::command]
fn latest_global_activation_terminal(app: AppHandle) -> Option<GlobalActivationTerminal> {
    app.state::<GlobalRuntimeState>()
        .activation_terminals
        .latest()
}

async fn launch_confirmed_project(
    state: GlobalRuntimeState,
    project_path: PathBuf,
    launch: ConfirmedLaunch,
) -> ProjectLaunchOutcome {
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let root_bindings = match path_io::capture_root_bindings(vec![project_path.clone()]).await {
        Ok(root_bindings) => root_bindings,
        Err(error) => {
            return ProjectLaunchOutcome::Failed {
                error: binding_failure(error),
            };
        }
    };
    launch_confirmed_project_with_bindings(state, project_path, launch, root_bindings).await
}

async fn launch_confirmed_project_with_bindings(
    state: GlobalRuntimeState,
    project_path: PathBuf,
    launch: ConfirmedLaunch,
    root_bindings: RootBindingPlan,
) -> ProjectLaunchOutcome {
    let native_path = NativePathDto::from(project_path);
    let recent_path = native_path.clone();
    let authority = TargetAuthority {
        logical_target: native_path,
        root_bindings,
    };
    let bootstrap = state.bootstrap.clone();
    let recent_projects = state.recent_projects.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        let outcome = match launch {
            ConfirmedLaunch::OpenExisting => bootstrap.open(authority),
            ConfirmedLaunch::CreateNew {
                configuration,
                authorization,
            } => bootstrap.create(authority, configuration, authorization),
        }?;
        let recent_result = match &outcome {
            BootstrapOutcome::Ready(ready) => {
                Some(recent_projects.promote(&ready.project_id, recent_path))
            }
            BootstrapOutcome::FocusExisting { .. } => None,
            BootstrapOutcome::ExternalCopyNotWritable(_) => None,
        };
        Ok::<_, BootstrapFailure>((outcome, recent_result))
    })
    .await
    {
        Ok(Ok((BootstrapOutcome::Ready(ready), recent_result))) => {
            if recent_result.is_some_and(|result| result.is_err()) {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id = ready.project_id,
                    event = "recent_project_promotion_failed",
                );
            }
            ProjectLaunchOutcome::Opened
        }
        Ok(Ok((
            BootstrapOutcome::FocusExisting {
                project_id,
                owner_process,
            },
            _,
        ))) => resolve_focus_existing(project_id, owner_process),
        Ok(Ok((BootstrapOutcome::ExternalCopyNotWritable(pending), _))) => {
            state.remember_external_copy(pending);
            ProjectLaunchOutcome::ExternalCopyNotWritable
        }
        Ok(Err(failure)) => ProjectLaunchOutcome::Failed {
            error: bootstrap_failure(failure),
        },
        Err(_) => ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "host_unavailable",
                "Não foi possível iniciar a Janela do Projeto.",
                "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
            ),
        },
    }
}

/// Executes every project in one native activation under one frozen binding
/// plan. Binding failure stops the batch before any Host side effect; after
/// that boundary, one Host terminal does not suppress the remaining files.
async fn launch_activation_batch(
    state: GlobalRuntimeState,
    projects: Vec<PathBuf>,
) -> ProjectLaunchOutcome {
    let project_count = projects.len();
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let root_bindings = match path_io::capture_root_bindings(projects.clone()).await {
        Ok(root_bindings) => root_bindings,
        Err(error) => {
            return ProjectLaunchOutcome::Failed {
                error: binding_failure(error),
            };
        }
    };
    let mut summary = ActivationBatchSummary::default();
    for project in projects {
        let outcome = launch_confirmed_project_with_bindings(
            state.clone(),
            project,
            ConfirmedLaunch::OpenExisting,
            root_bindings.clone(),
        )
        .await;
        summary.observe(outcome);
    }
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::Global.as_str(),
        project_count,
        opened_count = summary.opened_count,
        focused_count = summary.focused_count,
        external_copy_count = summary.external_copy_count,
        failed_count = summary.failed_count,
        event = "global_activation_batch_completed",
    );
    state
        .pending_external_copies
        .settle_activation_batch(&summary)
}

fn resolve_focus_existing(
    project_id: String,
    owner_process: ProcessInstanceId,
) -> ProjectLaunchOutcome {
    if focus_existing_project_window(owner_process) {
        tracing::info!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            project_id,
            owner_process_id = owner_process.process_id(),
            event = "existing_project_window_focused",
        );
        ProjectLaunchOutcome::Focused
    } else {
        ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "project_in_use",
                "Este Projeto já está aberto em outra janela.",
                "Use a janela já aberta ou feche-a antes de tentar novamente.",
            ),
        }
    }
}

fn bootstrap_failure(failure: BootstrapFailure) -> ProjectLaunchFailure {
    let stage = failure.stage;
    if failure.code == Some(FailureCode::ProjectInUse) {
        return staged_failure(
            "project_in_use",
            stage,
            "Este Projeto já está aberto em outra janela.",
            "Use a janela já aberta ou feche-a antes de tentar novamente.",
        );
    }
    if let Some(path_failure) = failure
        .code
        .and_then(|code| public_path_failure(code, stage))
    {
        return path_failure;
    }
    let (code, message, action) = match (failure.kind, failure.code) {
        (_, Some(FailureCode::DestinationConflict)) => (
            "destination_conflict",
            "O destino mudou antes da criação do Projeto.",
            "Escolha novamente o destino para confirmar o estado atual do arquivo.",
        ),
        (_, Some(FailureCode::CreateStateIndeterminate)) => (
            "create_state_indeterminate",
            "Não foi possível confirmar se a criação do Projeto terminou.",
            "Não repita a criação agora. Verifique o arquivo escolhido e tente abri-lo antes de decidir o próximo passo.",
        ),
        (_, Some(FailureCode::SaveCopyStateIndeterminate)) => (
            "save_copy_state_indeterminate",
            "Não foi possível confirmar se a nova cópia terminou de ser salva.",
            "Não repita agora. Verifique o destino escolhido antes de tentar novamente.",
        ),
        (_, Some(FailureCode::InvalidInitialProject)) => (
            "invalid_initial_project",
            "O estado inicial do Projeto não é válido.",
            "Feche e abra o MyAlbuns antes de tentar novamente.",
        ),
        (_, Some(FailureCode::InvalidDocumentType)) => (
            "invalid_document_type",
            "O arquivo escolhido não é um Documento de Projeto MyAlbuns.",
            "Escolha um arquivo .myalbuns válido.",
        ),
        (_, Some(FailureCode::UnsupportedFutureSchema)) => (
            "unsupported_future_schema",
            "Este Projeto foi criado por uma versão mais nova do MyAlbuns.",
            "Atualize o MyAlbuns antes de abrir o Projeto.",
        ),
        (_, Some(FailureCode::UnsupportedLegacySchema)) => (
            "unsupported_legacy_schema",
            "Esta versão antiga do Projeto ainda não pode ser aberta.",
            "Restaure uma cópia compatível ou use uma versão que suporte a migração.",
        ),
        (_, Some(FailureCode::InvalidProjectDocument)) => (
            "invalid_project_document",
            "O arquivo não contém um Projeto MyAlbuns v1 válido.",
            "Escolha outro arquivo ou restaure uma cópia válida.",
        ),
        (_, Some(FailureCode::InvalidProjectState)) => (
            "invalid_project_state",
            "O Projeto contém um estado que não pode ser editado com segurança.",
            "Restaure uma cópia válida do Projeto.",
        ),
        (_, Some(FailureCode::ExternalCopyRequiresInteractiveResolution)) => (
            "external_copy_requires_interactive_resolution",
            "O arquivo parece ser uma Cópia externa de outro Projeto.",
            "A resolução interativa de Cópias externas será disponibilizada em um fluxo próprio.",
        ),
        (_, Some(FailureCode::ExternalCopyNotWritable)) => (
            "external_copy_not_writable",
            "A Cópia externa não pode receber uma nova Identidade neste local.",
            "Use Salvar cópia como... para criar uma versão editável em outro local.",
        ),
        (_, Some(FailureCode::IdentityIndeterminate)) => (
            "identity_indeterminate",
            "Não foi possível confirmar com segurança a Identidade deste Projeto.",
            "Não altere o arquivo e tente novamente quando o local estiver estável.",
        ),
        (BootstrapFailureKind::Timeout, _) => (
            "host_timeout",
            "A Janela do Projeto não respondeu no prazo.",
            "Confirme a disponibilidade do local e tente novamente.",
        ),
        (BootstrapFailureKind::CorrelationMismatch | BootstrapFailureKind::InvalidTerminal, _) => (
            "host_protocol_error",
            "A Janela do Projeto respondeu de forma inválida e foi encerrada.",
            "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
        ),
        _ => (
            "open_project_failed",
            "Não foi possível abrir este Projeto.",
            "Confirme o arquivo e tente novamente.",
        ),
    };
    staged_failure(code, stage, message, action)
}

fn decorative_resolution_failure(error: ProvisionalDecorativeError) -> ProjectLaunchFailure {
    match error {
        ProvisionalDecorativeError::UnknownSelection => simple_failure(
            "image_selection_expired",
            "Uma Imagem decorativa selecionada n\u{e3}o est\u{e1} mais dispon\u{ed}vel.",
            "Escolha novamente a imagem antes de criar o Projeto.",
        ),
        ProvisionalDecorativeError::InvalidPath => simple_failure(
            "invalid_image_path",
            "O caminho de uma Imagem decorativa n\u{e3}o \u{e9} v\u{e1}lido.",
            "Escolha novamente a imagem.",
        ),
        ProvisionalDecorativeError::Unavailable => simple_failure(
            "image_unavailable",
            "Uma Imagem decorativa n\u{e3}o est\u{e1} dispon\u{ed}vel.",
            "Reconecte o local ou escolha novamente a imagem.",
        ),
        ProvisionalDecorativeError::UnsupportedImage => simple_failure(
            "unsupported_image",
            "Uma Imagem decorativa deixou de ser JPEG ou PNG.",
            "Escolha novamente a imagem.",
        ),
        ProvisionalDecorativeError::ReadFailed => simple_failure(
            "image_read_failed",
            "N\u{e3}o foi poss\u{ed}vel ler uma Imagem decorativa.",
            "Confirme o acesso ao arquivo ou escolha novamente a imagem.",
        ),
    }
}

fn binding_failure(error: AppPathsError) -> ProjectLaunchFailure {
    let public_code = match error {
        AppPathsError::InvalidOperationPath | AppPathsError::UnsupportedOperationNamespace => {
            Some(FailureCode::InvalidPath)
        }
        AppPathsError::OperationPathAccessDenied => Some(FailureCode::AccessDenied),
        AppPathsError::OperationPathUnavailable => Some(FailureCode::Unavailable),
        _ => None,
    };
    public_code
        .and_then(|code| public_path_failure(code, Some(FailureStage::Resolve)))
        .unwrap_or_else(|| {
            staged_failure(
                "path_preparation_failed",
                Some(FailureStage::Resolve),
                "Não foi possível preparar o caminho escolhido.",
                "Confirme se a unidade ou o compartilhamento está disponível e tente novamente.",
            )
        })
}

fn public_path_failure(
    code: FailureCode,
    stage: Option<FailureStage>,
) -> Option<ProjectLaunchFailure> {
    let (code, message, action) = match code {
        FailureCode::NotFound => (
            "not_found",
            "O arquivo do Projeto não foi encontrado.",
            "Confirme se ele foi movido ou removido e escolha o local correto.",
        ),
        FailureCode::AccessDenied => (
            "access_denied",
            "O Windows negou acesso ao arquivo do Projeto.",
            "Verifique as permissões do arquivo e tente novamente.",
        ),
        FailureCode::Unavailable => (
            "unavailable",
            "O local do Projeto está indisponível.",
            "Reconecte a unidade ou o compartilhamento e tente novamente.",
        ),
        FailureCode::InvalidPath => (
            "invalid_path",
            "O caminho escolhido não é válido para um Projeto.",
            "Escolha um arquivo .myalbuns e tente novamente.",
        ),
        FailureCode::UnexpectedObjectType => (
            "unexpected_object_type",
            "O local escolhido não é um arquivo de Projeto válido.",
            "Escolha um arquivo .myalbuns e tente novamente.",
        ),
        FailureCode::Conflict => (
            "conflict",
            "O arquivo do Projeto mudou durante a tentativa de abertura.",
            "Confirme o arquivo e tente novamente.",
        ),
        FailureCode::IoFailure => (
            "io_failure",
            "O Windows não conseguiu concluir a leitura do Projeto.",
            "Confirme a disponibilidade do local e tente novamente.",
        ),
        _ => return None,
    };
    Some(staged_failure(code, stage, message, action))
}

fn state_failure() -> ProjectLaunchFailure {
    simple_failure(
        "recent_projects_unavailable",
        "A lista de Projetos recentes está indisponível.",
        "Você ainda pode usar Abrir Projeto.",
    )
}

fn graphics_gate_failure() -> ProjectLaunchFailure {
    simple_failure(
        "graphics_requirement_not_met",
        "O editor exige WebGL2 com aceleração por hardware confirmada.",
        "Consulte o Diagnóstico gráfico antes de abrir ou criar um Projeto.",
    )
}

fn graphics_gate_timeout_failure() -> ProjectLaunchFailure {
    simple_failure(
        "graphics_gate_timeout",
        "Não foi possível confirmar o requisito gráfico do editor no prazo.",
        "Reinicie o MyAlbuns e consulte o Diagnóstico gráfico.",
    )
}

fn simple_failure(code: &str, message: &str, action: &str) -> ProjectLaunchFailure {
    staged_failure(code, None, message, action)
}

fn startup_cleanup_failure(reason: &str) -> ProjectLaunchFailure {
    simple_failure(
        "startup_cache_cleanup_unavailable",
        &format!("A limpeza segura do Cache na inicialização não foi concluída: {reason}"),
        "Reinicie o MyAlbuns. Se o problema continuar, verifique o armazenamento local.",
    )
}

fn staged_failure(
    code: &str,
    stage: Option<FailureStage>,
    message: &str,
    action: &str,
) -> ProjectLaunchFailure {
    ProjectLaunchFailure {
        code: code.into(),
        stage,
        message: message.into(),
        action: Some(action.into()),
    }
}

fn build_global_window(
    app: &AppHandle,
    webview_data_directory: PathBuf,
) -> Result<
    (
        WebviewWindow,
        desktop_webview_policy::WebviewPolicyReadiness,
    ),
    std::io::Error,
> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == GLOBAL_WINDOW_LABEL)
        .ok_or_else(|| std::io::Error::other("the Global window configuration does not exist"))?;
    let (policy_signal, policy_readiness) = desktop_webview_policy::page_load_handshake();
    let window = WebviewWindowBuilder::from_config(app, config)
        .map_err(std::io::Error::other)?
        .data_directory(webview_data_directory)
        .on_page_load(move |window, payload| {
            policy_signal.observe(&window, payload.event());
        })
        .build()
        .map_err(std::io::Error::other)?;
    Ok((window, policy_readiness))
}

fn show_existing_global_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(GLOBAL_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn publish_global_activation_terminal(
    app: &AppHandle,
    state: &GlobalRuntimeState,
    outcome: ProjectLaunchOutcome,
) {
    let Some(terminal) = state.activation_terminals.record(outcome) else {
        tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            event = "global_activation_terminal_sequence_exhausted",
        );
        return;
    };
    let Some(window) = app.get_webview_window(GLOBAL_WINDOW_LABEL) else {
        return;
    };
    if let Err(error) = window.emit(GLOBAL_ACTIVATION_TERMINAL_EVENT, &terminal) {
        tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            error = %error,
            sequence = terminal.sequence,
            event = "global_activation_terminal_emit_failed",
        );
    }
}

async fn wait_for_graphics_terminal(state: &GlobalRuntimeState) {
    while state.graphics_gate.is_pending() && !state.runtime_exiting.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn listen_for_forwarded_activations(app: AppHandle, state: GlobalRuntimeState) {
    let Some(primary) = state.global_activation.clone() else {
        return;
    };
    while !state.runtime_exiting.load(Ordering::Acquire) {
        let receiver = primary.clone();
        let received = tauri::async_runtime::spawn_blocking(move || {
            receiver.receive_timeout(Duration::from_millis(100))
        })
        .await;
        let batch = match received {
            Ok(Ok(batch)) => batch,
            Ok(Err(std::sync::mpsc::RecvTimeoutError::Timeout)) => continue,
            Ok(Err(std::sync::mpsc::RecvTimeoutError::Disconnected)) | Err(_) => break,
        };

        tracing::info!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            client_process_id = batch.client.process_id(),
            project_count = batch.projects.len(),
            event = "global_activation_forwarded",
        );
        wait_for_graphics_terminal(&state).await;
        if state.runtime_exiting.load(Ordering::Acquire) {
            break;
        }

        let outcome = if batch.projects.is_empty() {
            None
        } else if let Some(error) = state.scheduled_cleanup_failure().await {
            Some(ProjectLaunchOutcome::Failed { error })
        } else {
            let _serial = state.activation_serial.lock().await;
            Some(launch_activation_batch(state.clone(), batch.projects).await)
        };
        let exit_was_waiting = primary.complete_activation();

        match outcome {
            Some(ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused) => {
                exit_global_after_handoff(&app);
            }
            Some(ProjectLaunchOutcome::Failed { error }) => {
                state.cancel_requested_exit();
                state.record_startup_failure(error.clone());
                show_existing_global_window(&app);
                publish_global_activation_terminal(
                    &app,
                    &state,
                    ProjectLaunchOutcome::Failed { error },
                );
            }
            Some(ProjectLaunchOutcome::ExternalCopyNotWritable) => {
                state.cancel_requested_exit();
                show_existing_global_window(&app);
                publish_global_activation_terminal(
                    &app,
                    &state,
                    ProjectLaunchOutcome::ExternalCopyNotWritable,
                );
            }
            Some(ProjectLaunchOutcome::Cancelled) | None => {
                state.cancel_requested_exit();
                show_existing_global_window(&app);
            }
        }

        if exit_was_waiting && state.exit_requested.load(Ordering::Acquire) {
            commit_global_exit(&app, &state);
        }
    }
}

async fn initialize_global_window(
    app: AppHandle,
    state: GlobalRuntimeState,
    window: WebviewWindow,
    policy_readiness: desktop_webview_policy::WebviewPolicyReadiness,
) {
    let activation_pending = state.graphics_gate.has_pending_activation();
    if let Err(error) = policy_readiness.wait().await {
        tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            error = %error,
            event = "global_window_initialization_failed",
        );
        app.exit(1);
        return;
    }
    if !activation_pending {
        if let Err(error) = window.show() {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::Global.as_str(),
                error = %error,
                event = "global_window_show_failed",
            );
            app.exit(1);
        }
        return;
    }

    tokio::time::sleep(GRAPHICS_GATE_TIMEOUT).await;
    if state.graphics_gate.expire() {
        state.record_startup_failure(graphics_gate_timeout_failure());
        let _ = window.show();
    }
}

async fn run_scheduled_cleanup_background(
    service: CacheService,
    readiness: ScheduledCleanupGate,
) -> ScheduledCleanupResult {
    let result =
        match tauri::async_runtime::spawn_blocking(move || service.run_scheduled_cleanup()).await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(format!(
                "a tarefa de limpeza agendada não pôde ser concluída: {error}"
            )),
        };
    readiness.complete(result.clone());
    result
}

async fn initialize_global_runtime(
    app: AppHandle,
    state: GlobalRuntimeState,
    window: WebviewWindow,
    policy_readiness: desktop_webview_policy::WebviewPolicyReadiness,
    cache_service: CacheService,
) {
    let scheduled_cleanup = match run_scheduled_cleanup_background(
        cache_service,
        state.scheduled_cleanup.clone(),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::Global.as_str(),
                error,
                event = "scheduled_cache_cleanup_failed",
            );
            app.exit(1);
            return;
        }
    };
    tracing::info!(
        target: "myalbuns.desktop",
        outcome = ?scheduled_cleanup,
        event = "scheduled_cache_cleanup_checked",
    );
    if scheduled_cleanup == CacheScheduledCleanupOutcome::Deferred {
        tracing::warn!(
            target: "myalbuns.desktop",
            event = "scheduled_cache_cleanup_deferred",
        );
    }
    initialize_global_window(app, state, window, policy_readiness).await;
}

fn exit_global_after_handoff(app: &AppHandle) {
    let state = app.state::<GlobalRuntimeState>().inner();
    state.exit_requested.store(true, Ordering::Release);
    let can_exit = state
        .global_activation
        .as_ref()
        .is_none_or(|activation| activation.stop_accepting());
    if can_exit {
        commit_global_exit(app, state);
    }
}

fn commit_global_exit(app: &AppHandle, state: &GlobalRuntimeState) {
    if state.runtime_exiting.swap(true, Ordering::AcqRel) {
        return;
    }
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::Global.as_str(),
        process_id = std::process::id(),
        event = "global_exited_after_project_handoff",
    );
    app.exit(0);
}

#[cfg(windows)]
fn focus_existing_project_window(owner_process: ProcessInstanceId) -> bool {
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE,
                SetForegroundWindow, ShowWindow,
            },
        },
        core::BOOL,
    };

    struct Search {
        process_id: u32,
        window: HWND,
    }

    unsafe extern "system" fn find_window(window: HWND, parameter: LPARAM) -> BOOL {
        let search = unsafe { &mut *(parameter.0 as *mut Search) };
        let mut process_id = 0_u32;
        unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
        if process_id == search.process_id && unsafe { IsWindowVisible(window) }.as_bool() {
            search.window = window;
            return BOOL(0);
        }
        BOOL(1)
    }

    if owner_process.process_id() == std::process::id() {
        return false;
    }
    let Ok(process) = ProcessInstanceHandle::open(owner_process, 0) else {
        return false;
    };
    let mut search = Search {
        process_id: owner_process.process_id(),
        window: HWND::default(),
    };
    let parameter = LPARAM((&raw mut search).cast::<()>() as isize);
    let enumeration = unsafe { EnumWindows(Some(find_window), parameter) };
    if enumeration.is_err() && search.window.0.is_null() {
        return false;
    }
    if search.window.0.is_null() {
        return false;
    }
    let belongs_to_owner = || {
        let mut process_id = 0_u32;
        unsafe { GetWindowThreadProcessId(search.window, Some(&mut process_id)) };
        process_id == owner_process.process_id()
    };
    if !process.is_running().unwrap_or(false) || !belongs_to_owner() {
        return false;
    }
    unsafe {
        let _ = ShowWindow(search.window, SW_RESTORE);
    }
    if !process.is_running().unwrap_or(false) || !belongs_to_owner() {
        return false;
    }
    let foreground_confirmed = unsafe { SetForegroundWindow(search.window).as_bool() };
    if !process.is_running().unwrap_or(false) || !belongs_to_owner() {
        return false;
    }
    if !foreground_confirmed {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            owner_process_id = owner_process.process_id(),
            event = "existing_project_window_foreground_denied",
        );
    }
    // SetForegroundWindow is an OS policy request, not an ownership proof.
    // The exact visible owner has still been restored and signalled; treating
    // policy denial as a lock failure would reopen the welcome flow and invite
    // a competing edit attempt.
    true
}

#[cfg(not(windows))]
fn focus_existing_project_window(_owner_process: ProcessInstanceId) -> bool {
    false
}

#[cfg(debug_assertions)]
fn webdriver_project_path(automation_enabled: bool, candidate: Option<PathBuf>) -> Option<PathBuf> {
    candidate.filter(|path| {
        automation_enabled
            && path.is_absolute()
            && path
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("myalbuns"))
    })
}

#[cfg(debug_assertions)]
pub(crate) fn webdriver_automation_project() -> Option<PathBuf> {
    webdriver_project_path(
        std::env::var_os(TAURI_WEBVIEW_AUTOMATION_ENV).is_some(),
        std::env::var_os(WEBDRIVER_PROJECT_ENV).map(PathBuf::from),
    )
}

pub(crate) fn run(direct_projects: Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::discover()?;
    let primary_activation = match enter_global_activation(&app_paths, direct_projects)? {
        GlobalActivationEntry::Forwarded => return Ok(()),
        GlobalActivationEntry::Primary(primary) => Arc::new(primary),
    };
    let initial_projects = primary_activation.initial_projects().to_vec();
    let global_webview_data_directory =
        app_paths.webview_data_directory(GLOBAL_WEBVIEW_NAMESPACE)?;
    let state = GlobalRuntimeState::new(&app_paths, initial_projects, Some(primary_activation))?;
    let cache_service = CacheService::new(app_paths.clone());
    let setup_state = state.clone();
    let provisional_decoratives =
        crate::provisional_decoratives::ProvisionalDecorativeRegistry::default();
    let preview_registry = provisional_decoratives.clone();
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(
            crate::provisional_decoratives::PREVIEW_PROTOCOL_SCHEME,
            move |context, request, responder| {
                crate::provisional_decoratives::respond_to_preview_request(
                    preview_registry.clone(),
                    context,
                    request,
                    responder,
                );
            },
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .manage(cache_service)
        .manage(provisional_decoratives)
        .setup(move |app| {
            logging::initialize(app, &app_paths, ProcessRole::Global);
            let app_handle = app.handle().clone();
            // Both configured windows use `create: false`. Install the first
            // owned WebView before setup returns; the page-load terminal then
            // proves that Wry registered it before native policy is applied.
            let (window, policy_readiness) =
                build_global_window(&app_handle, global_webview_data_directory.clone())?;
            let managed_cache_service = app.state::<CacheService>().inner().clone();
            tauri::async_runtime::spawn(initialize_global_runtime(
                app_handle.clone(),
                setup_state.clone(),
                window,
                policy_readiness,
                managed_cache_service,
            ));
            tauri::async_runtime::spawn(listen_for_forwarded_activations(
                app_handle,
                setup_state.clone(),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            complete_graphics_gate,
            crate::native_dialog_window::dismiss_owned_dialog,
            crate::native_dialog_window::owned_window_content_ready,
            create_project,
            open_project,
            save_external_copy_as,
            recent_projects,
            open_recent_project,
            show_project_failure_dialog,
            startup_open_failure,
            latest_global_activation_terminal,
            crate::cache_service::cache_service_status,
            crate::cache_service::free_closed_project_cache,
            crate::cache_service::clear_all_cache,
            validate_project_configuration,
            crate::provisional_decoratives::choose_provisional_decorative,
            crate::provisional_decoratives::clear_provisional_decoratives,
            crate::provisional_decoratives::release_provisional_decorative,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn scheduled_cleanup_keeps_the_runtime_responsive_until_the_exact_writer_exits() {
        use std::{
            ffi::c_void,
            os::windows::io::AsRawHandle,
            process::{Command, Stdio},
            time::Instant,
        };

        use crate::{
            ipc_contract::CacheClearAllOutcome,
            operation_gate::{OperationGate, OperationGateError},
        };

        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().expect("the startup cleanup fixture exists");
            let app_paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            std::fs::create_dir_all(root.path().join("roaming")).unwrap();
            std::fs::create_dir_all(root.path().join("local")).unwrap();
            let cache = app_paths
                .project_cache("startup-delayed-writer")
                .expect("the Cache namespace is valid");
            drop(
                app_paths
                    .prepare_cache_storage(&cache)
                    .expect("the Cache namespace is prepared"),
            );
            std::fs::write(cache.media_directory().join("payload.bin"), b"Cache")
                .expect("the Cache payload is writable");
            let service = CacheService::new(app_paths.clone());
            let active_operation = OperationGate::new(&app_paths)
                .try_acquire()
                .expect("the scheduling fixture owns an active operation");
            assert_eq!(
                service
                    .clear_all_or_schedule()
                    .expect("cleanup is scheduled while an operation is active"),
                CacheClearAllOutcome::Scheduled
            );
            drop(active_operation);

            let mut writer =
                Command::new(std::env::current_exe().expect("the test executable is known"))
                    .arg("global_runtime::tests::delayed_cache_writer_process")
                    .args(["--ignored", "--exact", "--nocapture"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("the delayed Cache writer starts");
            let writer_identity = ProcessInstanceId::from_process_handle(
                writer.id(),
                writer.as_raw_handle().cast::<c_void>(),
            )
            .expect("the delayed writer has an exact identity");
            std::fs::write(
                cache.root().join(".processor-writer.v1.json"),
                serde_json::to_vec(&serde_json::json!({
                    "schemaVersion": 1,
                    "process": writer_identity,
                }))
                .expect("the exact writer claim serializes"),
            )
            .expect("the exact writer claim is published");

            let readiness = ScheduledCleanupGate::default();
            let cleanup = tauri::async_runtime::spawn(run_scheduled_cleanup_background(
                service,
                readiness.clone(),
            ));
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match OperationGate::new(&app_paths).try_acquire() {
                    Err(OperationGateError::Conflict) => break,
                    Ok(grant) => drop(grant),
                    Err(error) => panic!("the operation marker is unavailable: {error}"),
                }
                assert!(Instant::now() < deadline, "cleanup did not start in time");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(50), readiness.wait())
                    .await
                    .is_err(),
                "runtime timers remain responsive while readiness stays withheld"
            );

            writer.kill().expect("the delayed writer is released");
            writer.wait().expect("the delayed writer is reaped");
            assert_eq!(
                cleanup
                    .await
                    .expect("the cleanup task is joined")
                    .expect("the scheduled cleanup succeeds"),
                CacheScheduledCleanupOutcome::Cleared
            );
            assert_eq!(
                readiness
                    .wait()
                    .await
                    .expect("startup readiness observes the safe result"),
                CacheScheduledCleanupOutcome::Cleared
            );
            assert!(!cache.root().exists());
        });
    }

    #[test]
    #[ignore = "spawned by the scheduled startup cleanup responsiveness test"]
    fn delayed_cache_writer_process() {
        std::thread::sleep(Duration::from_secs(120));
    }

    #[cfg(windows)]
    #[test]
    fn a_reused_pid_never_focuses_the_visible_window_of_another_process_instance() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::windows::io::AsRawHandle,
            process::{Command, Stdio},
        };

        let mut alien = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-STA",
                "-Command",
                r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Threading;
using System.Windows.Forms;

public static class MyAlbunsFocusFixture
{
    private sealed class NonActivatingForm : Form
    {
        protected override bool ShowWithoutActivation { get { return true; } }

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams parameters = base.CreateParams;
                parameters.ExStyle |= 0x08000000;
                return parameters;
            }
        }
    }

    public static void Run()
    {
        using (var window = new NonActivatingForm())
        {
            window.Text = "MyAlbuns foreign process instance";
            window.Width = 320;
            window.Height = 200;
            window.Shown += delegate
            {
                Console.Out.WriteLine("ready");
                Console.Out.Flush();
            };
            var input = new Thread(new ThreadStart(delegate
            {
                Console.In.ReadLine();
                if (window.IsHandleCreated)
                {
                    window.BeginInvoke((Action)delegate { window.Close(); });
                }
            }));
            input.IsBackground = true;
            input.Start();
            Application.Run(window);
        }
    }
}
'@ -ReferencedAssemblies System.Windows.Forms.dll,System.Drawing.dll
[MyAlbunsFocusFixture]::Run()
"#,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the foreign process instance starts");
        let mut output = BufReader::new(
            alien
                .stdout
                .take()
                .expect("the foreign process exposes its readiness signal"),
        );
        let mut readiness = String::new();
        output
            .read_line(&mut readiness)
            .expect("the visible foreign window reports readiness");
        assert_eq!(readiness.trim(), "ready");

        let observed =
            ProcessInstanceId::from_process_handle(alien.id(), alien.as_raw_handle().cast())
                .expect("the foreign process instance is observed exactly");
        assert!(
            focus_existing_project_window(observed),
            "a visible window of the exact owner is reused even when Windows withholds foreground"
        );
        let reused_pid = ProcessInstanceId::from_wire(observed.process_id(), 1)
            .expect("the previous process instance token is structurally valid");

        let focused = focus_existing_project_window(reused_pid);

        writeln!(
            alien
                .stdin
                .as_mut()
                .expect("the fixture stdin remains open"),
            "close"
        )
        .expect("the foreign window receives its close signal");
        alien.wait().expect("the foreign process is reaped");
        assert_ne!(observed, reused_pid);
        assert!(
            !focused,
            "a PID reused by another process instance must never receive focus"
        );
    }

    #[test]
    fn webdriver_adapter_accepts_only_an_automated_absolute_project_path() {
        let project = PathBuf::from(r"C:\Projetos\Canvas real.myalbuns");

        assert_eq!(
            webdriver_project_path(true, Some(project.clone())),
            Some(project.clone())
        );
        assert_eq!(webdriver_project_path(false, Some(project)), None);
        assert_eq!(
            webdriver_project_path(true, Some(PathBuf::from("relative.myalbuns"))),
            None
        );
        assert_eq!(
            webdriver_project_path(true, Some(PathBuf::from(r"C:\Projetos\Canvas.png"))),
            None
        );
    }

    fn creation_launch() -> ConfirmedLaunch {
        ConfirmedLaunch::CreateNew {
            configuration: Box::new(
                serde_json::from_value(serde_json::json!({
                    "document": {
                        "displayUnit": "mm",
                        "sheetWidthUm": 600000,
                        "sheetHeightUm": 300000,
                        "dpi": 300,
                        "bleedUm": 3000,
                        "safetyUm": 3000
                    },
                    "structure": {
                        "sheetCount": 2,
                        "firstSheet": "double",
                        "lastSheet": "double"
                    },
                    "visualDefaults": {
                        "background": {
                            "scope": "bothSides",
                            "both": { "kind": "color", "rgb": "#FFFFFF" }
                        },
                        "overlay": { "scope": "bothSides", "both": null },
                        "frameBorder": { "kind": "none" }
                    }
                }))
                .expect("the creation launch fixture is valid"),
            ),
            authorization: CreateWriteAuthorization::CreateOnly,
        }
    }

    #[tokio::test]
    async fn graphics_gate_precedes_open_and_create_host_boundaries() {
        let directory = tempfile::tempdir().expect("temporary Global state root");
        let paths = AppPaths::from_roots(directory.path(), directory.path());
        let state =
            GlobalRuntimeState::new(&paths, Vec::new(), None).expect("Global state initializes");

        for (name, launch) in [
            ("Abrir.myalbuns", ConfirmedLaunch::OpenExisting),
            ("Criar.myalbuns", creation_launch()),
        ] {
            let project_path = directory.path().join(name);
            let outcome =
                launch_confirmed_project(state.clone(), project_path.clone(), launch).await;
            assert!(matches!(
                outcome,
                ProjectLaunchOutcome::Failed {
                    error: ProjectLaunchFailure { ref code, .. }
                } if code == "graphics_requirement_not_met"
            ));
            assert!(
                !project_path.exists(),
                "the rejected gate has no Project file or Host side effect"
            );
        }
    }

    #[test]
    fn validation_command_is_pure_and_returns_the_closed_wire_response() {
        let configuration = serde_json::from_value(serde_json::json!({
            "document": {
                "displayUnit": "cm",
                "sheetWidthUm": 508000,
                "sheetHeightUm": 254000,
                "dpi": 240,
                "bleedUm": 4000,
                "safetyUm": 7500
            },
            "structure": {
                "sheetCount": 3,
                "firstSheet": "singlePage",
                "lastSheet": "double"
            }
        }))
        .expect("the command accepts the closed configuration DTO");

        assert_eq!(
            serde_json::to_value(validate_project_configuration(configuration))
                .expect("the validation response serializes"),
            serde_json::json!({ "errors": [] })
        );
    }

    #[test]
    fn welcome_outcomes_never_contain_a_pathname() {
        assert_eq!(
            serde_json::to_value(ProjectLaunchOutcome::Opened).expect("outcome serializes"),
            serde_json::json!({ "status": "opened" })
        );
        assert_eq!(
            serde_json::to_value(ProjectLaunchOutcome::Focused).expect("outcome serializes"),
            serde_json::json!({ "status": "focused" })
        );
        assert_eq!(
            serde_json::to_value(ProjectLaunchOutcome::ExternalCopyNotWritable)
                .expect("outcome serializes"),
            serde_json::json!({ "status": "externalCopyNotWritable" })
        );
        assert_eq!(
            serde_json::to_value(ProjectLaunchOutcome::Cancelled).expect("outcome serializes"),
            serde_json::json!({ "status": "cancelled" })
        );
        let encoded = serde_json::to_value(ProjectLaunchOutcome::Failed {
            error: simple_failure("project_in_use", "Em uso.", "Use a outra janela."),
        })
        .expect("failure serializes");
        assert_eq!(encoded["status"], "failed");
        assert!(encoded.get("pathname").is_none());
        assert!(encoded["error"].get("pathname").is_none());
    }

    #[test]
    fn forwarded_activation_terminals_are_monotonic_and_snapshot_safe() {
        let terminals = GlobalActivationTerminalStore::default();
        assert_eq!(terminals.latest(), None);

        let first = terminals
            .record(ProjectLaunchOutcome::Failed {
                error: simple_failure("project_in_use", "Em uso.", "Use a outra janela."),
            })
            .expect("the first terminal is sequenced");
        let second = terminals
            .record(ProjectLaunchOutcome::ExternalCopyNotWritable)
            .expect("the second terminal is sequenced");

        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(terminals.latest(), Some(second.clone()));
        assert_eq!(
            serde_json::to_value(second).expect("activation terminal serializes"),
            serde_json::json!({
                "sequence": 2,
                "outcome": { "status": "externalCopyNotWritable" }
            })
        );
    }

    #[test]
    fn mixed_multi_file_batch_preserves_external_copy_continuations_in_fifo_order() {
        let pending = PendingExternalCopyCoordinator::default();
        pending.remember("primeira");
        pending.remember("segunda");

        let mut summary = ActivationBatchSummary::default();
        summary.observe(ProjectLaunchOutcome::ExternalCopyNotWritable);
        summary.observe(ProjectLaunchOutcome::Opened);
        summary.observe(ProjectLaunchOutcome::ExternalCopyNotWritable);
        summary.observe(ProjectLaunchOutcome::Focused);

        assert_eq!(
            pending.settle_activation_batch(&summary),
            ProjectLaunchOutcome::ExternalCopyNotWritable
        );
        assert_eq!(pending.take(), Some("primeira"));
        assert_eq!(
            pending.settle_external_copy_attempt(ProjectLaunchOutcome::Opened),
            (ProjectLaunchOutcome::ExternalCopyNotWritable, false)
        );
        assert_eq!(pending.take(), Some("segunda"));
        assert_eq!(
            pending.settle_external_copy_attempt(ProjectLaunchOutcome::Cancelled),
            (ProjectLaunchOutcome::Cancelled, true)
        );
        assert_eq!(pending.take(), None);
        assert!(summary.has_handoff());
    }

    #[test]
    fn later_activation_cannot_discard_an_earlier_external_copy_continuation() {
        let pending = PendingExternalCopyCoordinator::default();
        pending.remember("cópia pendente");

        let mut first_activation = ActivationBatchSummary::default();
        first_activation.observe(ProjectLaunchOutcome::ExternalCopyNotWritable);
        assert_eq!(
            pending.settle_activation_batch(&first_activation),
            ProjectLaunchOutcome::ExternalCopyNotWritable
        );

        let mut later_activation = ActivationBatchSummary::default();
        later_activation.observe(ProjectLaunchOutcome::Opened);
        assert_eq!(
            pending.settle_activation_batch(&later_activation),
            ProjectLaunchOutcome::ExternalCopyNotWritable
        );

        assert_eq!(pending.take(), Some("cópia pendente"));
        assert_eq!(
            pending.settle_external_copy_attempt(ProjectLaunchOutcome::Cancelled),
            (ProjectLaunchOutcome::Cancelled, true)
        );
    }

    #[test]
    fn mixed_batch_defers_its_first_failure_until_external_copies_are_resolved() {
        let pending = PendingExternalCopyCoordinator::default();
        pending.remember("cópia pendente");
        let expected_failure = simple_failure(
            "project_in_use",
            "Este Projeto já está aberto em outra janela.",
            "Use a janela já aberta ou feche-a antes de tentar novamente.",
        );

        let mut summary = ActivationBatchSummary::default();
        summary.observe(ProjectLaunchOutcome::Failed {
            error: expected_failure.clone(),
        });
        summary.observe(ProjectLaunchOutcome::ExternalCopyNotWritable);
        assert_eq!(
            pending.settle_activation_batch(&summary),
            ProjectLaunchOutcome::ExternalCopyNotWritable
        );

        assert_eq!(pending.take(), Some("cópia pendente"));
        assert_eq!(
            pending.settle_external_copy_attempt(ProjectLaunchOutcome::Cancelled),
            (
                ProjectLaunchOutcome::Failed {
                    error: expected_failure,
                },
                false,
            )
        );
    }

    #[test]
    fn startup_failure_reads_are_idempotent_for_strict_mode_mounts() {
        let directory = tempfile::tempdir().expect("temporary Global state root");
        let paths = AppPaths::from_roots(directory.path(), directory.path());
        let state =
            GlobalRuntimeState::new(&paths, Vec::new(), None).expect("Global state initializes");
        let failure = simple_failure("invalid_project_document", "Inválido.", "Escolha outro.");
        state.record_startup_failure(failure.clone());

        assert_eq!(state.startup_failure(), Some(failure.clone()));
        assert_eq!(state.startup_failure(), Some(failure));
    }

    #[test]
    fn path_failures_remain_actionable_without_exposing_the_native_pathname() {
        let cases = [
            (FailureCode::NotFound, "not_found"),
            (FailureCode::AccessDenied, "access_denied"),
            (FailureCode::Unavailable, "unavailable"),
            (FailureCode::InvalidPath, "invalid_path"),
            (FailureCode::UnexpectedObjectType, "unexpected_object_type"),
            (FailureCode::Conflict, "conflict"),
            (FailureCode::IoFailure, "io_failure"),
        ];

        for (failure_code, expected_public_code) in cases {
            let public_failure = bootstrap_failure(BootstrapFailure {
                kind: BootstrapFailureKind::HostFailed,
                stage: Some(crate::project_bootstrap::FailureStage::Open),
                code: Some(failure_code),
            });
            assert_eq!(public_failure.code, expected_public_code);
            assert_eq!(public_failure.stage, Some(FailureStage::Open));
            assert!(public_failure.action.is_some());

            let encoded =
                serde_json::to_value(public_failure).expect("the actionable failure serializes");
            assert!(encoded.get("pathname").is_none());
            assert!(encoded.get("path").is_none());
        }
    }

    #[test]
    fn root_binding_failures_keep_access_unavailable_and_invalid_target_distinct() {
        let cases = [
            (AppPathsError::InvalidOperationPath, "invalid_path"),
            (AppPathsError::UnsupportedOperationNamespace, "invalid_path"),
            (AppPathsError::OperationPathAccessDenied, "access_denied"),
            (AppPathsError::OperationPathUnavailable, "unavailable"),
            (
                AppPathsError::OperationPathIoFailure,
                "path_preparation_failed",
            ),
        ];

        for (binding_error, expected_public_code) in cases {
            let public_failure = binding_failure(binding_error);
            assert_eq!(public_failure.code, expected_public_code);
            assert_eq!(public_failure.stage, Some(FailureStage::Resolve));
            assert!(public_failure.action.is_some());
            let encoded =
                serde_json::to_value(public_failure).expect("the binding failure serializes");
            assert!(encoded.get("pathname").is_none());
            assert!(encoded.get("path").is_none());
        }
    }

    #[test]
    fn creation_failures_keep_conflict_invalid_state_and_indeterminate_state_distinct() {
        let cases = [
            (FailureCode::DestinationConflict, "destination_conflict"),
            (
                FailureCode::InvalidInitialProject,
                "invalid_initial_project",
            ),
            (
                FailureCode::CreateStateIndeterminate,
                "create_state_indeterminate",
            ),
        ];

        for (failure_code, expected_public_code) in cases {
            let failure = bootstrap_failure(BootstrapFailure {
                kind: BootstrapFailureKind::HostFailed,
                stage: Some(FailureStage::Create),
                code: Some(failure_code),
            });

            assert_eq!(failure.code, expected_public_code);
            assert_eq!(failure.stage, Some(FailureStage::Create));
            assert!(failure.action.is_some());
        }

        let indeterminate = bootstrap_failure(BootstrapFailure {
            kind: BootstrapFailureKind::HostFailed,
            stage: Some(FailureStage::Create),
            code: Some(FailureCode::CreateStateIndeterminate),
        });
        assert!(
            indeterminate
                .action
                .as_deref()
                .is_some_and(|action| action.contains("Não repita"))
        );
    }

    #[test]
    fn save_copy_failure_keeps_its_terminal_stage_and_safe_retry_instruction() {
        let failure = bootstrap_failure(BootstrapFailure {
            kind: BootstrapFailureKind::HostFailed,
            stage: Some(FailureStage::SaveCopy),
            code: Some(FailureCode::SaveCopyStateIndeterminate),
        });

        assert_eq!(failure.code, "save_copy_state_indeterminate");
        assert_eq!(failure.stage, Some(FailureStage::SaveCopy));
        assert!(
            failure
                .action
                .as_deref()
                .is_some_and(|action| action.contains("Não repita"))
        );
    }
}
