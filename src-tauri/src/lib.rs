mod batch_lease_probe;
mod batch_runner;
mod benchmark_corpus;
mod cache_engine;
mod export_attempts;
mod export_pipeline;
mod export_probe_commands;
mod export_terminal_probe;
pub(crate) mod global_process_spike;
mod imaging_processor;
#[cfg(test)]
mod imaging_recovery_integration;
mod logging;
mod media_preview_commands;
mod operation_gate;
mod operation_gate_probe;
mod operation_lease;
mod path_io;
mod probe_support;
mod project_commands;
mod project_core_probe;
mod project_host;
mod project_open_probe;
mod project_opening_guard;
#[path = "../../tests/support/sample_project.rs"]
mod sample_project;
mod topology_benchmark;
mod topology_fault_probe;
mod topology_spike;

use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::AppPaths;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use batch_lease_probe::BatchLeaseProbe;
use cache_engine::CacheEngine;
use export_attempts::ExportAttempts;
use export_probe_commands::{cancel_export_spike, export_spike};
use export_terminal_probe::ExportTerminalProbe;
use imaging_processor::ImagingProcessor;
use logging::frontend_log;
use media_preview_commands::prepare_media_previews;
use operation_gate::OperationGate;
use operation_gate_probe::OperationGateProbe;
use project_commands::{apply_project_intent, project_state, redo_project, undo_project};
use project_open_probe::ProjectOpenProbe;
use topology_benchmark::{
    TopologyBenchmarkState, report_topology_benchmark_failure, report_topology_canvas_benchmark,
    report_topology_canvas_ready, topology_benchmark_config,
};
use topology_fault_probe::{
    TopologyFaultProbeState, persist_topology_fault_probe, report_topology_fault_probe_failure,
    topology_fault_probe_config,
};
use topology_spike::TopologySpike;

enum ExclusiveProbe {
    OperationGate(OperationGateProbe),
    ExportTerminal(ExportTerminalProbe),
    BatchLease(BatchLeaseProbe),
    ProjectOpen(ProjectOpenProbe),
}

impl ExclusiveProbe {
    fn start(self, app: &tauri::AppHandle) -> Result<(), String> {
        match self {
            Self::OperationGate(probe) => probe.start(app),
            Self::ExportTerminal(probe) => probe.start(app),
            Self::BatchLease(probe) => probe.start(app),
            Self::ProjectOpen(probe) => probe.start(app),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if project_core_probe::requested() {
        if let Err(error) = project_core_probe::run_from_environment() {
            eprintln!("probe de ProjectCore encerrado: {error}");
            std::process::exit(project_core_probe::failure_exit_code());
        }
        return;
    }

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
    let operation_gate_probe = OperationGateProbe::from_environment(&topology)
        .unwrap_or_else(|error| panic!("probe de OperationGate inválido: {error}"));
    let export_terminal_probe = ExportTerminalProbe::from_environment(&topology)
        .unwrap_or_else(|error| panic!("probe terminal de Exportação inválido: {error}"));
    let batch_lease_probe = BatchLeaseProbe::from_environment(&topology)
        .unwrap_or_else(|error| panic!("probe de lease do lote inválido: {error}"));
    let project_open_probe = ProjectOpenProbe::from_environment(&topology)
        .unwrap_or_else(|error| panic!("probe de abertura inválido: {error}"));
    let exclusive_probes = [
        operation_gate_probe.map(ExclusiveProbe::OperationGate),
        export_terminal_probe.map(ExclusiveProbe::ExportTerminal),
        batch_lease_probe.map(ExclusiveProbe::BatchLease),
        project_open_probe.map(ExclusiveProbe::ProjectOpen),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if exclusive_probes.len() > 1 {
        panic!(
            "os probes de OperationGate, de terminais de Exportação, de lease do lote e de abertura são exclusivos"
        );
    }
    let (topology_benchmark, topology_fault_probe) = if !exclusive_probes.is_empty() {
        (
            TopologyBenchmarkState::disabled(&topology),
            TopologyFaultProbeState::disabled(&topology),
        )
    } else {
        (
            TopologyBenchmarkState::from_environment(&topology)
                .unwrap_or_else(|error| panic!("benchmark de topologia inválido: {error}")),
            TopologyFaultProbeState::from_environment(&topology)
                .unwrap_or_else(|error| panic!("probe de falhas de topologia inválido: {error}")),
        )
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .manage(topology_benchmark)
        .manage(topology_fault_probe)
        .manage(ExportAttempts::default())
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let cancelled_attempts = window
                    .state::<ExportAttempts>()
                    .request_cancel_for_window(window.label());
                if cancelled_attempts > 0 {
                    tracing::info!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        window_label = window.label(),
                        cancelled_attempts,
                        event = "window_export_attempts_cancelled",
                    );
                }
            }
        })
        .setup(move |app| {
            let app_paths = AppPaths::discover()?;
            let webview_data_directory =
                app_paths.webview_data_directory(topology.webview_data_namespace())?;
            logging::initialize(app, &app_paths);
            app.manage(OperationGate::new(&app_paths));
            app.manage(CacheEngine::default());
            app.manage(ImagingProcessor::default());
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
            for probe in exclusive_probes {
                probe.start(app.handle()).map_err(std::io::Error::other)?;
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
            cancel_export_spike,
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
    use std::collections::BTreeSet;

    #[test]
    fn project_windows_receive_only_the_explicit_frontend_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("valid project-window capability");
        let windows = capability["windows"]
            .as_array()
            .expect("the capability targets explicit windows")
            .iter()
            .map(|label| label.as_str().expect("window labels are textual"))
            .collect::<BTreeSet<_>>();
        let permissions = capability["permissions"]
            .as_array()
            .expect("the capability has an explicit permission list")
            .iter()
            .map(|permission| {
                permission
                    .as_str()
                    .expect("scoped permission objects are not needed by the frontend")
            })
            .collect::<BTreeSet<_>>();
        let permission_manifest: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/project-window.json"))
                .expect("valid project-window permission manifest");
        let project_window_permission = &permission_manifest["permission"][0];
        let allowed_commands = project_window_permission["commands"]["allow"]
            .as_array()
            .expect("the project-window permission has an explicit command allow-list");

        assert_eq!(capability["local"], true);
        assert!(capability.get("remote").is_none());
        assert_eq!(windows, BTreeSet::from(["main", "project-b"]));
        assert_eq!(permissions, BTreeSet::from(["project-window-commands"]));
        assert_eq!(
            project_window_permission["identifier"],
            "project-window-commands"
        );
        assert_eq!(
            project_window_permission["commands"]["deny"],
            serde_json::json!([])
        );
        assert!(!allowed_commands.is_empty());
        assert!(allowed_commands.iter().all(|command| {
            command
                .as_str()
                .is_some_and(|command| !command.contains(':'))
        }));
    }

    #[test]
    fn asset_protocol_serves_only_published_media_previews() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let scope = config["app"]["security"]["assetProtocol"]["scope"]
            .as_array()
            .expect("the asset protocol has an explicit scope")
            .iter()
            .map(|entry| entry.as_str().expect("asset scopes are textual"))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            scope,
            BTreeSet::from([
                "$LOCALDATA/MyAlbuns2/Cache/*/Media/*.jpg",
                "$LOCALDATA/MyAlbuns2/Cache/*/Media/*.png",
            ])
        );
        assert!(scope.iter().all(|entry| !entry.contains("**")));
    }

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
