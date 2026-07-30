mod benchmark_corpus;
mod cache_engine;
mod export_pipeline;
mod export_probe_commands;
pub(crate) mod global_process_spike;
mod imaging_processor;
#[cfg(test)]
mod imaging_recovery_integration;
mod logging;
mod media_preview_commands;
mod project_commands;
mod project_host;
#[path = "../../tests/support/sample_project.rs"]
mod sample_project;
mod topology_benchmark;
mod topology_fault_probe;
mod topology_spike;

use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::AppPaths;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use export_probe_commands::export_spike;
use logging::frontend_log;
use media_preview_commands::prepare_media_previews;
use project_commands::{apply_project_intent, project_state, redo_project, undo_project};
use topology_benchmark::{
    TopologyBenchmarkState, report_topology_benchmark_failure, report_topology_canvas_benchmark,
    report_topology_canvas_ready, topology_benchmark_config,
};
use topology_fault_probe::{
    TopologyFaultProbeState, persist_topology_fault_probe, report_topology_fault_probe_failure,
    topology_fault_probe_config,
};
use topology_spike::TopologySpike;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if global_process_spike::global_process_requested() {
        if let Err(error) = global_process_spike::run_global_process_spike_from_environment() {
            eprintln!("processo global do spike encerrado: {error}");
            std::process::exit(error.exit_code());
        }
        return;
    }

    let topology = TopologySpike::from_environment()
        .unwrap_or_else(|error| panic!("configuração inválida do spike de topologia: {error}"));
    let project_host = topology
        .project_host()
        .unwrap_or_else(|error| panic!("corpus inválido do spike de topologia: {error}"));
    let topology_benchmark = TopologyBenchmarkState::from_environment(&topology)
        .unwrap_or_else(|error| panic!("benchmark de topologia inválido: {error}"));
    let topology_fault_probe = TopologyFaultProbeState::from_environment(&topology)
        .unwrap_or_else(|error| panic!("probe de falhas de topologia inválido: {error}"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .manage(topology_benchmark)
        .manage(topology_fault_probe)
        .setup(move |app| {
            let app_paths = AppPaths::discover()?;
            let webview_data_directory =
                app_paths.webview_data_directory(topology.webview_data_namespace())?;
            logging::initialize(app, &app_paths);
            app.manage(app_paths);

            let main_config = app.config().app.windows.first().ok_or_else(|| {
                std::io::Error::other("a configuração da janela principal não existe")
            })?;
            WebviewWindowBuilder::from_config(app, main_config)?
                .title(topology.primary_title())
                .data_directory(webview_data_directory.clone())
                .build()?;
            if let Some(secondary) = topology.secondary_window() {
                WebviewWindowBuilder::new(
                    app,
                    secondary.label,
                    WebviewUrl::App("index.html".into()),
                )
                .title(secondary.title.as_str())
                .inner_size(1440.0, 900.0)
                .min_inner_size(1080.0, 720.0)
                .resizable(true)
                .data_directory(webview_data_directory)
                .build()?;
            }

            for window_label in topology.reopened_window_labels() {
                let projection = app
                    .state::<project_host::ProjectHost>()
                    .projection(window_label)
                    .map_err(std::io::Error::other)?;
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    process_id = std::process::id(),
                    run_id = topology.run_id(),
                    topology = topology.label(),
                    window_label,
                    project_id = safe_log_identifier(&projection.state.project_id),
                    revision = projection.state.revision,
                    event = "topology_project_reopened",
                );
            }

            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                process_id = std::process::id(),
                topology = topology.label(),
                session_count = topology.session_count(),
                event = "topology_host_started",
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            frontend_log,
            project_state,
            apply_project_intent,
            undo_project,
            redo_project,
            prepare_media_previews,
            export_spike,
            topology_benchmark_config,
            report_topology_canvas_ready,
            report_topology_canvas_benchmark,
            report_topology_benchmark_failure,
            topology_fault_probe_config,
            persist_topology_fault_probe,
            report_topology_fault_probe_failure
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o MyAlbuns");
}

#[cfg(test)]
mod tests {
    #[test]
    fn tauri_csp_allows_the_scoped_asset_protocol_for_pixi_texture_fetches() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let security = &config["app"]["security"];

        for policy_name in ["csp", "devCsp"] {
            let policy = security[policy_name]
                .as_str()
                .expect("the CSP policy is textual");
            let connect_sources = policy
                .split(';')
                .find(|directive| directive.trim_start().starts_with("connect-src "))
                .expect("the CSP defines connect-src");
            assert!(
                connect_sources
                    .split_whitespace()
                    .any(|value| value == "asset:")
                    && connect_sources
                        .split_whitespace()
                        .any(|value| value == "http://asset.localhost"),
                "{policy_name} must permit the two platform forms used by Tauri asset URLs"
            );
        }
    }

    #[test]
    fn production_csp_keeps_unsafe_eval_disabled_for_the_pixi_static_runtime() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let production_csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("the production CSP is textual");

        assert!(
            !production_csp.contains("'unsafe-eval'"),
            "the PixiJS static CSP runtime must not be replaced with unsafe-eval"
        );
    }

    #[test]
    fn main_window_is_created_with_the_central_webview_data_directory() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let main_window = &config["app"]["windows"][0];

        assert_eq!(main_window["label"], "main");
        assert_eq!(main_window["create"], false);
    }
}
