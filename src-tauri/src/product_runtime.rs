use std::io;

use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, project_data_namespace};
use tauri::{Emitter, Manager, WebviewWindowBuilder};

use crate::{
    cache_activity_gate::CacheActivityGate,
    desktop_webview_policy,
    export_attempts::ExportAttempts,
    imaging_processor::ImagingProcessor,
    logging,
    operation_gate::OperationGate,
    project_bootstrap::{
        BootstrapRequest, BootstrappedHostProject, FailureCode, FailureStage, HostTerminal,
        write_host_terminal,
    },
    project_host::ProjectCloseRequestOutcome,
    project_host::ProjectHost,
    project_window_lifecycle::{
        PROJECT_CLOSE_CONFIRMATION_EVENT, complete_project_close,
        request_window_export_cancellation,
    },
};

pub(crate) const PROJECT_WINDOW_LABEL: &str = "project";

#[cfg(debug_assertions)]
const PROCESS_GATE_ROOT_ENV: &str = "MYALBUNS_PROCESS_GATE_DATA_ROOT";
#[cfg(debug_assertions)]
const PROCESS_GATE_HEADLESS_ENV: &str = "MYALBUNS_PROCESS_GATE_HEADLESS";

pub(crate) fn run(
    opened: BootstrappedHostProject,
    app_paths: AppPaths,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    if process_gate_headless_enabled() {
        return run_headless_process_gate(opened);
    }

    let (request, project) = opened.into_parts();
    let project_host = ProjectHost::new(project);
    let linked_media_previews =
        crate::linked_media_previews::LinkedMediaPreviewRegistry::new(PROJECT_WINDOW_LABEL);
    let media_protocol_registry = linked_media_previews.clone();
    let setup_paths = app_paths.clone();
    let terminal = PendingHostTerminal::new(request);

    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(
            crate::linked_media_previews::PROJECT_MEDIA_PROTOCOL_SCHEME,
            move |context, request, responder| {
                crate::linked_media_previews::respond_to_media_request(
                    media_protocol_registry.clone(),
                    context,
                    request,
                    responder,
                );
            },
        )
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .manage(linked_media_previews)
        .manage(ExportAttempts::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != PROJECT_WINDOW_LABEL {
                    return;
                }
                api.prevent_close();
                match window.state::<ProjectHost>().begin_close() {
                    Ok(ProjectCloseRequestOutcome::CloseImmediately) => {
                        complete_project_close(window);
                    }
                    Ok(ProjectCloseRequestOutcome::ConfirmationRequired) => {
                        if let Err(error) = window.emit(PROJECT_CLOSE_CONFIRMATION_EVENT, ()) {
                            let _ = window.state::<ProjectHost>().cancel_close();
                            tracing::error!(
                                target: "myalbuns.desktop",
                                process_role = ProcessRole::DesktopHost.as_str(),
                                window_label = window.label(),
                                error = %error,
                                event = "project_close_confirmation_emit_failed",
                            );
                        }
                    }
                    Err(error) => {
                        tracing::error!(
                            target: "myalbuns.desktop",
                            process_role = ProcessRole::DesktopHost.as_str(),
                            window_label = window.label(),
                            error = %error,
                            event = "project_close_state_unavailable",
                        );
                        complete_project_close(window);
                    }
                }
                return;
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                request_window_export_cancellation(window);
            }
        })
        .setup(move |app| setup_host(app, setup_paths, terminal))
        .invoke_handler(tauri::generate_handler![
            crate::logging::frontend_log,
            crate::project_commands::project_state,
            crate::project_commands::apply_project_intent,
            crate::project_commands::undo_project,
            crate::project_commands::redo_project,
            crate::project_commands::save_project,
            crate::project_close_commands::request_project_close,
            crate::project_close_commands::resolve_project_close,
            crate::media_preview_commands::prepare_media_previews,
            crate::export_commands::export_sheet,
            crate::export_commands::cancel_export,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

/// Exercises the real executable, bootstrap and Project ownership when the
/// caller itself runs in a restricted Windows environment that cannot create
/// WebView2. Release builds do not contain this path, and normal development
/// still reaches the productive Tauri composition root below.
#[cfg(debug_assertions)]
fn run_headless_process_gate(
    opened: BootstrappedHostProject,
) -> Result<(), Box<dyn std::error::Error>> {
    let (request, project) = opened.into_parts();
    let project_host = ProjectHost::new(project);
    let projection = project_host.projection().map_err(io::Error::other)?;
    let mut terminal = PendingHostTerminal::new(request);
    terminal.emit_ready(&projection.state.project_id, projection.state.revision)?;

    // The ProjectHost intentionally stays in this stack frame so its
    // EditableProject and identity lease live exactly as long as the process.
    let _project_host = project_host;
    loop {
        std::thread::park();
    }
}

#[cfg(debug_assertions)]
fn process_gate_headless_enabled() -> bool {
    std::env::var_os(PROCESS_GATE_ROOT_ENV).is_some()
        && std::env::var_os(PROCESS_GATE_HEADLESS_ENV).as_deref() == Some(std::ffi::OsStr::new("1"))
}

fn setup_host(
    app: &mut tauri::App,
    app_paths: AppPaths,
    mut terminal: PendingHostTerminal,
) -> Result<(), Box<dyn std::error::Error>> {
    let projection = app
        .state::<ProjectHost>()
        .projection()
        .map_err(io::Error::other)?;
    let webview_data_directory =
        app_paths.webview_data_directory(&project_data_namespace(&projection.state.project_id))?;
    logging::initialize(app, &app_paths, ProcessRole::DesktopHost);
    app.manage(OperationGate::new(&app_paths));
    app.manage(CacheActivityGate::default());
    app.manage(ImagingProcessor::default());
    app.manage(app_paths);

    let project_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == PROJECT_WINDOW_LABEL)
        .ok_or_else(|| io::Error::other("a configuração da janela do Projeto não existe"))?;
    let project_window = WebviewWindowBuilder::from_config(app, project_config)?
        .data_directory(webview_data_directory)
        .build()?;
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        let startup = async {
            desktop_webview_policy::enforce(&project_window).await?;
            project_window.show()?;
            terminal.emit_ready(&projection.state.project_id, projection.state.revision)?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        }
        .await;

        if let Err(error) = startup {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                error = %error,
                event = "project_host_initialization_failed",
            );
            terminal.emit_failed(FailureStage::Initialize, FailureCode::IoFailure);
            app_handle.exit(1);
            return;
        }

        tracing::info!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            process_id = std::process::id(),
            window_label = PROJECT_WINDOW_LABEL,
            project_id = safe_log_identifier(&projection.state.project_id),
            revision = projection.state.revision,
            event = "project_host_started",
        );
    });
    Ok(())
}

struct PendingHostTerminal {
    request: BootstrapRequest,
    emitted: bool,
}

impl PendingHostTerminal {
    fn new(request: BootstrapRequest) -> Self {
        Self {
            request,
            emitted: false,
        }
    }

    fn emit_ready(&mut self, project_id: &str, revision: u64) -> io::Result<()> {
        write_host_terminal(
            io::stdout().lock(),
            &HostTerminal::ready(&self.request, project_id.to_owned(), revision),
        )?;
        self.emitted = true;
        Ok(())
    }

    fn emit_failed(&mut self, stage: FailureStage, code: FailureCode) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        let _ = write_host_terminal(
            io::stdout().lock(),
            &HostTerminal::failed(&self.request, stage, code),
        );
    }
}

impl Drop for PendingHostTerminal {
    fn drop(&mut self) {
        if self.emitted {
            return;
        }
        self.emit_failed(FailureStage::Initialize, FailureCode::IoFailure);
    }
}

#[cfg(test)]
mod tests {
    use super::PROJECT_WINDOW_LABEL;

    #[test]
    fn productive_host_has_one_stable_project_window_label() {
        assert_eq!(PROJECT_WINDOW_LABEL, "project");
    }
}
