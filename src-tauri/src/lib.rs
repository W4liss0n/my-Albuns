mod cache_engine;
mod demo_project;
mod desktop_webview_policy;
mod export_attempts;
mod export_commands;
mod export_pipeline;
mod imaging_processor;
#[cfg(test)]
mod imaging_recovery_integration;
pub mod ipc_contract;
mod logging;
mod media_preview_commands;
mod operation_gate;
mod operation_lease;
mod path_io;
mod product_runtime;
mod project_commands;
mod project_host;
#[cfg(test)]
#[path = "../../tests/support/sample_project.rs"]
mod sample_project;

use myalbuns_logging::ProcessRole;
use tauri::Manager;

use export_attempts::ExportAttempts;
use export_commands::{cancel_export, export_preview};
use logging::frontend_log;
use media_preview_commands::prepare_media_previews;
use product_runtime::ProductRuntime;
use project_commands::{apply_project_intent, project_state, redo_project, undo_project};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ProductRuntime {
        project_host,
        app_paths,
    } = ProductRuntime::initialize()
        .unwrap_or_else(|error| panic!("não foi possível iniciar o runtime do produto: {error}"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
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
        .setup(move |app| product_runtime::setup(app, app_paths))
        .invoke_handler(tauri::generate_handler![
            frontend_log,
            project_state,
            apply_project_intent,
            undo_project,
            redo_project,
            prepare_media_previews,
            export_preview,
            cancel_export,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o MyAlbuns");
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    #[test]
    fn main_window_receives_only_product_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("valid project-window capability");
        let windows = capability["windows"]
            .as_array()
            .expect("the capability targets explicit windows")
            .iter()
            .map(|label| label.as_str().expect("window labels are textual"))
            .collect::<BTreeSet<_>>();
        let permission_manifest: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/project-window.json"))
                .expect("valid project-window permission manifest");
        let project_window_permission = &permission_manifest["permission"][0];
        let allowed_commands = project_window_permission["commands"]["allow"]
            .as_array()
            .expect("the project-window permission has an explicit command allow-list")
            .iter()
            .map(|command| command.as_str().expect("command names are textual"))
            .collect::<BTreeSet<_>>();

        assert_eq!(capability["local"], true);
        assert!(capability.get("remote").is_none());
        assert_eq!(windows, BTreeSet::from(["main"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["project-window-commands"])
        );
        assert_eq!(
            project_window_permission["identifier"],
            "project-window-commands"
        );
        assert_eq!(
            allowed_commands,
            BTreeSet::from([
                "apply_project_intent",
                "cancel_export",
                "export_preview",
                "frontend_log",
                "prepare_media_previews",
                "project_state",
                "redo_project",
                "undo_project",
            ])
        );
        assert_eq!(
            project_window_permission["commands"]["deny"],
            serde_json::json!([])
        );
    }

    #[test]
    fn windows_bundle_uses_current_user_nsis_and_evergreen_webview2() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let bundle = &config["bundle"];
        let windows = &bundle["windows"];

        assert_eq!(bundle["targets"], serde_json::json!(["nsis"]));
        assert_eq!(windows["nsis"]["installMode"], "currentUser");
        assert_eq!(
            windows["webviewInstallMode"],
            serde_json::json!({
                "type": "downloadBootstrapper",
                "silent": true
            })
        );
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
    fn csp_allows_scoped_assets_without_unsafe_eval_in_production() {
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
            );
            assert!(
                connect_sources
                    .split_whitespace()
                    .any(|value| value == "http://asset.localhost")
            );
        }
        assert!(
            !security["csp"]
                .as_str()
                .expect("the production CSP is textual")
                .contains("'unsafe-eval'")
        );
    }

    #[test]
    fn main_window_is_created_by_the_product_composition_root() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let windows = config["app"]["windows"]
            .as_array()
            .expect("the window list is explicit");

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0]["label"], "main");
        assert_eq!(windows[0]["create"], false);
    }

    #[test]
    fn main_window_disables_native_browser_controls() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let main_window = &config["app"]["windows"][0];

        assert_eq!(main_window["devtools"], false);
        assert_eq!(main_window["zoomHotkeysEnabled"], false);
    }

    #[test]
    fn product_runtime_applies_the_native_webview_policy() {
        let runtime_source = include_str!("product_runtime.rs");

        assert!(runtime_source.contains("desktop_webview_policy::enforce(&main_window)?;"));
    }
}
