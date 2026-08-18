use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use myalbuns_logging::ProcessRole;
use myalbuns_paths::{AppPaths, AppPathsError, NativePathDto};
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    desktop_webview_policy,
    graphics_launch_gate::{
        GRAPHICS_GATE_TIMEOUT, GraphicsGateCompletion, GraphicsGateReport, GraphicsLaunchGate,
    },
    logging, native_project_dialog, path_io,
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
const GLOBAL_WEBVIEW_NAMESPACE: &str = "global";
const HOST_TERMINAL_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(debug_assertions)]
const WEBDRIVER_PROJECT_ENV: &str = "MYALBUNS_TAURI_WEBDRIVER_PROJECT";
#[cfg(debug_assertions)]
const TAURI_WEBVIEW_AUTOMATION_ENV: &str = "TAURI_WEBVIEW_AUTOMATION";

#[derive(Clone)]
struct GlobalRuntimeState {
    bootstrap: ProjectHostBootstrap,
    graphics_gate: GraphicsLaunchGate,
    recent_projects: RecentProjectsStore,
    startup_failure: Arc<Mutex<Option<ProjectLaunchFailure>>>,
    pending_external_copy: Arc<Mutex<Option<PendingExternalCopyProcess>>>,
}

impl GlobalRuntimeState {
    fn new(app_paths: &AppPaths, direct_project: Option<PathBuf>) -> Result<Self, std::io::Error> {
        Ok(Self {
            bootstrap: ProjectHostBootstrap::new(std::env::current_exe()?, HOST_TERMINAL_TIMEOUT),
            graphics_gate: GraphicsLaunchGate::new(direct_project),
            recent_projects: RecentProjectsStore::new(app_paths),
            startup_failure: Arc::new(Mutex::new(None)),
            pending_external_copy: Arc::new(Mutex::new(None)),
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
        self.pending_external_copy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    fn remember_external_copy(&self, pending: PendingExternalCopyProcess) {
        *self
            .pending_external_copy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(pending);
    }

    fn clear_external_copy(&self) {
        self.pending_external_copy
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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
    match state.graphics_gate.complete(report) {
        GraphicsGateCompletion::Ready(Some(project_path)) => {
            let outcome = launch_confirmed_project(
                state.clone(),
                project_path,
                ConfirmedLaunch::OpenExisting,
            )
            .await;
            match &outcome {
                ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused => {
                    exit_global_after_handoff(&app)
                }
                ProjectLaunchOutcome::Failed { error } => {
                    state.record_startup_failure(error.clone());
                    show_existing_global_window(&app);
                }
                ProjectLaunchOutcome::ExternalCopyNotWritable | ProjectLaunchOutcome::Cancelled => {
                    show_existing_global_window(&app)
                }
            }
            Some(outcome)
        }
        GraphicsGateCompletion::Rejected => {
            show_existing_global_window(&app);
            None
        }
        GraphicsGateCompletion::Ready(None) | GraphicsGateCompletion::AlreadyFinal => None,
    }
}

#[tauri::command]
async fn open_project(app: AppHandle) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
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

    let outcome = launch_confirmed_project(state, path, ConfirmedLaunch::OpenExisting).await;
    if matches!(
        outcome,
        ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
    ) {
        exit_global_after_handoff(&app);
    }
    outcome
}

#[tauri::command]
async fn save_external_copy_as(app: AppHandle) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
    if let Some(rejection) = state.project_host_gate_rejection() {
        return rejection;
    }
    let Some(pending) = state.take_external_copy() else {
        return ProjectLaunchOutcome::Failed {
            error: simple_failure(
                "external_copy_source_expired",
                "A cópia externa precisa ser validada novamente.",
                "Abra novamente a cópia somente leitura.",
            ),
        };
    };
    let (destination_path, authorization) =
        match native_project_dialog::choose_project_destination(&app).await {
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
                    event = "external_copy_destination_dialog_failed",
                );
                return ProjectLaunchOutcome::Failed {
                    error: simple_failure(
                        "dialog_unavailable",
                        "Não foi possível concluir o diálogo para salvar a cópia.",
                        "Tente novamente.",
                    ),
                };
            }
        };
    let root_bindings = match path_io::capture_root_bindings(vec![destination_path.clone()]).await {
        Ok(root_bindings) => root_bindings,
        Err(error) => {
            return ProjectLaunchOutcome::Failed {
                error: binding_failure(error),
            };
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
    match tauri::async_runtime::spawn_blocking(move || {
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
            state.clear_external_copy();
            if recent_result.is_some_and(|result| result.is_err()) {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id = ready.project_id,
                    event = "recent_project_promotion_failed",
                );
            }
            exit_global_after_handoff(&app);
            ProjectLaunchOutcome::Opened
        }
        Ok(Ok((
            BootstrapOutcome::FocusExisting {
                project_id,
                owner_process_id,
            },
            _,
        ))) => {
            if focus_existing_project_window(owner_process_id) {
                state.clear_external_copy();
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id,
                    owner_process_id,
                    event = "existing_project_window_focused",
                );
                exit_global_after_handoff(&app);
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
    }
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
    configuration: ProvisionalProjectCreationConfiguration,
) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
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
    let destination = match native_project_dialog::choose_project_destination(&app).await {
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

    let outcome = launch_confirmed_project(
        state,
        destination.0,
        ConfirmedLaunch::CreateNew {
            configuration: Box::new(configuration),
            authorization: destination.1,
        },
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
    let store = state.recent_projects.clone();
    tauri::async_runtime::spawn_blocking(move || store.list())
        .await
        .map_err(|_| state_failure())?
        .map_err(|_| state_failure())
}

#[tauri::command]
async fn open_recent_project(app: AppHandle, project_id: String) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
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
    let outcome = launch_confirmed_project(state, path, ConfirmedLaunch::OpenExisting).await;
    if matches!(
        outcome,
        ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused
    ) {
        exit_global_after_handoff(&app);
    }
    outcome
}

#[tauri::command]
fn startup_open_failure(
    state: tauri::State<'_, GlobalRuntimeState>,
) -> Option<ProjectLaunchFailure> {
    state.startup_failure()
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
            state.clear_external_copy();
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
                owner_process_id,
            },
            _,
        ))) => {
            if focus_existing_project_window(owner_process_id) {
                state.clear_external_copy();
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id,
                    owner_process_id,
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
            "O arquivo parece ser uma cópia externa de outro Projeto.",
            "A resolução interativa de cópias externas será disponibilizada em um fluxo próprio.",
        ),
        (_, Some(FailureCode::ExternalCopyNotWritable)) => (
            "external_copy_not_writable",
            "A cópia externa não pode receber uma nova Identidade neste local.",
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
    }
}

async fn initialize_global_window(
    app: AppHandle,
    state: GlobalRuntimeState,
    window: WebviewWindow,
    policy_readiness: desktop_webview_policy::WebviewPolicyReadiness,
) {
    let direct_project_pending = state.graphics_gate.has_pending_direct_project();
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
    if !direct_project_pending {
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

fn exit_global_after_handoff(app: &AppHandle) {
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::Global.as_str(),
        process_id = std::process::id(),
        event = "global_exited_after_project_handoff",
    );
    app.exit(0);
}

#[cfg(windows)]
fn focus_existing_project_window(owner_process_id: u32) -> bool {
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

    if owner_process_id == 0 || owner_process_id == std::process::id() {
        return false;
    }
    let mut search = Search {
        process_id: owner_process_id,
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
    unsafe {
        let _ = ShowWindow(search.window, SW_RESTORE);
        SetForegroundWindow(search.window).as_bool()
    }
}

#[cfg(not(windows))]
fn focus_existing_project_window(_owner_process_id: u32) -> bool {
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

pub(crate) fn run(direct_project: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::discover()?;
    let global_webview_data_directory =
        app_paths.webview_data_directory(GLOBAL_WEBVIEW_NAMESPACE)?;
    let state = GlobalRuntimeState::new(&app_paths, direct_project)?;
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
        .manage(provisional_decoratives)
        .setup(move |app| {
            logging::initialize(app, &app_paths, ProcessRole::Global);
            let app_handle = app.handle().clone();
            // Both configured windows use `create: false`. Install the first
            // owned WebView before setup returns; the page-load terminal then
            // proves that Wry registered it before native policy is applied.
            let (window, policy_readiness) =
                build_global_window(&app_handle, global_webview_data_directory.clone())?;
            tauri::async_runtime::spawn(initialize_global_window(
                app_handle,
                setup_state.clone(),
                window,
                policy_readiness,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            complete_graphics_gate,
            create_project,
            open_project,
            save_external_copy_as,
            recent_projects,
            open_recent_project,
            startup_open_failure,
            validate_project_configuration,
            crate::provisional_decoratives::choose_provisional_decorative,
            crate::provisional_decoratives::release_provisional_decorative,
            crate::provisional_decoratives::clear_provisional_decoratives,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let state = GlobalRuntimeState::new(&paths, None).expect("Global state initializes");

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
    fn startup_failure_reads_are_idempotent_for_strict_mode_mounts() {
        let directory = tempfile::tempdir().expect("temporary Global state root");
        let paths = AppPaths::from_roots(directory.path(), directory.path());
        let state = GlobalRuntimeState::new(&paths, None).expect("Global state initializes");
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
