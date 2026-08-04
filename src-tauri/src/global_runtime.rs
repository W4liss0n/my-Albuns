use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use myalbuns_logging::ProcessRole;
use myalbuns_paths::{AppPaths, AppPathsError, NativePathDto};
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    desktop_webview_policy, logging, native_project_dialog, path_io,
    project_bootstrap::{
        BootstrapFailure, BootstrapFailureKind, CreateWriteAuthorization, FailureCode,
        FailureStage, InitialProjectConfiguration, InitialProjectCreationConfiguration,
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
const HOST_TERMINAL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct GlobalRuntimeState {
    bootstrap: ProjectHostBootstrap,
    recent_projects: RecentProjectsStore,
    startup_failure: Arc<Mutex<Option<ProjectLaunchFailure>>>,
}

impl GlobalRuntimeState {
    fn new(app_paths: &AppPaths) -> Result<Self, std::io::Error> {
        Ok(Self {
            bootstrap: ProjectHostBootstrap::new(std::env::current_exe()?, HOST_TERMINAL_TIMEOUT),
            recent_projects: RecentProjectsStore::new(app_paths),
            startup_failure: Arc::new(Mutex::new(None)),
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
async fn open_project(app: AppHandle) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
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
    if outcome == ProjectLaunchOutcome::Opened {
        app.exit(0);
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
    configuration: ProvisionalProjectCreationConfiguration,
) -> ProjectLaunchOutcome {
    let state = app.state::<GlobalRuntimeState>().inner().clone();
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
    if outcome == ProjectLaunchOutcome::Opened {
        provisional_decoratives.clear();
        app.exit(0);
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
    if outcome == ProjectLaunchOutcome::Opened {
        app.exit(0);
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
    let bootstrap = state.bootstrap;
    let recent_projects = state.recent_projects;
    match tauri::async_runtime::spawn_blocking(move || {
        let ready = match launch {
            ConfirmedLaunch::OpenExisting => bootstrap.open(authority),
            ConfirmedLaunch::CreateNew {
                configuration,
                authorization,
            } => bootstrap.create(authority, configuration, authorization),
        }?;
        Ok::<_, BootstrapFailure>({
            let recent_result = recent_projects.promote(&ready.project_id, recent_path);
            (ready, recent_result)
        })
    })
    .await
    {
        Ok(Ok((ready, recent_result))) => {
            if recent_result.is_err() {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::Global.as_str(),
                    project_id = ready.project_id,
                    event = "recent_project_promotion_failed",
                );
            }
            ProjectLaunchOutcome::Opened
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

async fn show_global_window(app: &AppHandle) -> Result<(), std::io::Error> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == GLOBAL_WINDOW_LABEL)
        .ok_or_else(|| std::io::Error::other("a configuração da janela global não existe"))?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .map_err(std::io::Error::other)?
        .build()
        .map_err(std::io::Error::other)?;
    desktop_webview_policy::enforce(&window)
        .await
        .map_err(std::io::Error::other)?;
    window.show().map_err(std::io::Error::other)
}

async fn show_global_window_or_exit(app: AppHandle) {
    if let Err(error) = show_global_window(&app).await {
        tracing::error!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::Global.as_str(),
            error = %error,
            event = "global_window_initialization_failed",
        );
        app.exit(1);
    }
}

pub(crate) fn run(direct_project: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let app_paths = AppPaths::discover()?;
    let state = GlobalRuntimeState::new(&app_paths)?;
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
            if let Some(project_path) = direct_project {
                let direct_state = setup_state.clone();
                tauri::async_runtime::spawn(async move {
                    match launch_confirmed_project(
                        direct_state.clone(),
                        project_path,
                        ConfirmedLaunch::OpenExisting,
                    )
                    .await
                    {
                        ProjectLaunchOutcome::Opened => app_handle.exit(0),
                        ProjectLaunchOutcome::Failed { error } => {
                            direct_state.record_startup_failure(error);
                            show_global_window_or_exit(app_handle).await;
                        }
                        ProjectLaunchOutcome::Cancelled => {
                            show_global_window_or_exit(app_handle).await;
                        }
                    }
                });
            } else {
                tauri::async_runtime::spawn(show_global_window_or_exit(app_handle));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_project,
            open_project,
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
        let paths = AppPaths::from_roots(directory.path(), directory.path(), directory.path());
        let state = GlobalRuntimeState::new(&paths).expect("Global state initializes");
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
}
