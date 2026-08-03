use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, project_data_namespace};
use tauri::{Manager, WebviewWindowBuilder};

use crate::{
    cache_engine::CacheEngine, demo_project, imaging_processor::ImagingProcessor, logging,
    operation_gate::OperationGate, project_host::ProjectHost,
};

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";

pub(crate) struct ProductRuntime {
    pub(crate) project_host: ProjectHost,
    pub(crate) app_paths: AppPaths,
}

/// Creates the only Project Host owned by this process.
///
/// The demo source is temporary until create/open enters the product flow; the
/// one-host/one-session shape is the definitive topology decision.
impl ProductRuntime {
    pub(crate) fn initialize() -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_paths(AppPaths::discover()?)
    }

    fn from_paths(app_paths: AppPaths) -> Result<Self, Box<dyn std::error::Error>> {
        let project_host = demo_project::open(&app_paths).map_err(std::io::Error::other)?;
        Ok(Self {
            project_host,
            app_paths,
        })
    }
}

pub(crate) fn setup(
    app: &mut tauri::App,
    app_paths: AppPaths,
) -> Result<(), Box<dyn std::error::Error>> {
    let projection = app
        .state::<ProjectHost>()
        .projection()
        .map_err(std::io::Error::other)?;
    let webview_data_directory =
        app_paths.webview_data_directory(&project_data_namespace(&projection.state.project_id))?;
    logging::initialize(app, &app_paths);
    app.manage(OperationGate::new(&app_paths));
    app.manage(CacheEngine::default());
    app.manage(ImagingProcessor::default());
    app.manage(app_paths);

    let main_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW_LABEL)
        .ok_or_else(|| std::io::Error::other("a configuração da janela principal não existe"))?;
    WebviewWindowBuilder::from_config(app, main_config)?
        .data_directory(webview_data_directory)
        .build()?;

    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        window_label = MAIN_WINDOW_LABEL,
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_host_started",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use myalbuns_paths::AppPaths;

    use super::{MAIN_WINDOW_LABEL, ProductRuntime};

    #[test]
    fn product_runtime_has_one_stable_window() {
        assert_eq!(MAIN_WINDOW_LABEL, "main");
        let directory = tempfile::tempdir().expect("the temporary root exists");
        let paths = AppPaths::from_known_folders(directory.path(), directory.path());
        assert!(ProductRuntime::from_paths(paths).is_ok());
    }
}
