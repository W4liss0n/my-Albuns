mod cache_activity_gate;
mod cache_engine;
mod cache_previews;
mod desktop_webview_policy;
#[cfg(debug_assertions)]
mod dev_host_registration;
#[cfg(debug_assertions)]
mod dev_process_identity;
#[cfg(debug_assertions)]
mod dev_supervisor_protocol;
mod export_attempts;
mod export_commands;
mod export_pipeline;
mod global_runtime;
mod graphics_launch_gate;
mod imaging_processor;
#[cfg(test)]
mod imaging_recovery_integration;
pub mod ipc_contract;
mod logging;
mod media_preview_commands;
mod media_runtime;
mod native_project_dialog;
mod opaque_image_protocol;
mod operation_gate;
mod operation_lease;
mod path_io;
mod product_runtime;
mod project_bootstrap;
mod project_close_commands;
mod project_commands;
mod project_host;
mod project_window_lifecycle;
mod provisional_decoratives;
mod recent_projects;
mod runtime_role;
#[cfg(test)]
#[path = "../../tests/support/sample_project.rs"]
mod sample_project;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    let result = match global_runtime::webdriver_automation_project() {
        Some(project_path) => run_webdriver_project_host(project_path),
        None => run_selected_runtime_role(),
    };
    #[cfg(not(debug_assertions))]
    let result = run_selected_runtime_role();

    if let Err(error) = result {
        eprintln!("não foi possível executar o MyAlbuns: {error}");
    }
}

fn run_selected_runtime_role() -> Result<(), Box<dyn std::error::Error>> {
    match runtime_role::parse_runtime_role(std::env::args_os()) {
        runtime_role::RuntimeRole::Global { direct_project } => global_runtime::run(direct_project),
        runtime_role::RuntimeRole::ProjectHost => run_project_host(),
    }
}

#[cfg(debug_assertions)]
fn run_webdriver_project_host(
    project_path: std::path::PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    use myalbuns_paths::{NativePathDto, OperationPathContext};

    use project_bootstrap::{TargetAuthority, bootstrap_host_project, new_open_request};

    let app_paths = myalbuns_paths::AppPaths::discover()?;
    let mut path_context = OperationPathContext::new();
    path_context.capture(&project_path)?;
    let request = new_open_request(TargetAuthority {
        logical_target: NativePathDto::from(project_path),
        root_bindings: path_context.freeze(),
    })
    .map_err(|failure| std::io::Error::other(format!("bootstrap inválido: {failure:?}")))?;
    let opened = bootstrap_host_project(request, &app_paths)
        .map_err(|terminal| std::io::Error::other(format!("bootstrap recusado: {terminal:?}")))?;
    product_runtime::run(opened, app_paths)
}

fn run_project_host() -> Result<(), Box<dyn std::error::Error>> {
    use std::io;

    use project_bootstrap::{
        FailureCode, FailureStage, HostTerminal, bootstrap_host_project, read_bootstrap_request,
        write_host_terminal,
    };

    let request = match read_bootstrap_request(io::stdin().lock()) {
        Ok(request) => request,
        Err(_) => {
            write_host_terminal(
                io::stdout().lock(),
                &HostTerminal::uncorrelated_failure(
                    FailureStage::Decode,
                    FailureCode::InvalidRequest,
                ),
            )?;
            return Ok(());
        }
    };
    let app_paths = match myalbuns_paths::AppPaths::discover() {
        Ok(paths) => paths,
        Err(_) => {
            emit_host_start_failure(&request, FailureStage::Initialize, FailureCode::IoFailure)?;
            return Ok(());
        }
    };
    match bootstrap_host_project(request, &app_paths) {
        Ok(opened) => product_runtime::run(opened, app_paths),
        Err(terminal) => {
            write_host_terminal(io::stdout().lock(), &terminal)?;
            Ok(())
        }
    }
}

fn emit_host_start_failure(
    request: &project_bootstrap::BootstrapRequest,
    stage: project_bootstrap::FailureStage,
    code: project_bootstrap::FailureCode,
) -> Result<(), std::io::Error> {
    project_bootstrap::write_host_terminal(
        std::io::stdout().lock(),
        &project_bootstrap::HostTerminal::failed(request, stage, code),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    fn config() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config")
    }

    fn allowed_commands(manifest: &serde_json::Value) -> BTreeSet<&str> {
        manifest["permission"][0]["commands"]["allow"]
            .as_array()
            .expect("the permission has an explicit allow-list")
            .iter()
            .map(|command| command.as_str().expect("commands are textual"))
            .collect()
    }

    #[test]
    fn global_and_project_windows_have_disjoint_minimal_capabilities() {
        let project_capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("valid project capability");
        let global_capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/global.json"))
                .expect("valid global capability");
        let project_permission: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/project-window.json"))
                .expect("valid project permission");
        let global_permission: serde_json::Value =
            serde_json::from_str(include_str!("../permissions/global-window.json"))
                .expect("valid global permission");

        assert_eq!(
            project_capability["windows"],
            serde_json::json!(["project"])
        );
        assert_eq!(global_capability["windows"], serde_json::json!(["global"]));
        assert_eq!(
            project_capability["permissions"],
            serde_json::json!([
                "project-window-commands",
                "core:event:allow-listen",
                "core:event:allow-unlisten"
            ])
        );
        assert_eq!(
            global_capability["permissions"],
            serde_json::json!(["global-window-commands"])
        );
        assert!(
            allowed_commands(&project_permission)
                .is_disjoint(&allowed_commands(&global_permission))
        );
        let global_commands = allowed_commands(&global_permission);
        let project_commands = allowed_commands(&project_permission);
        for command in [
            "choose_provisional_decorative",
            "release_provisional_decorative",
            "clear_provisional_decoratives",
        ] {
            assert!(global_commands.contains(command));
            assert!(!project_commands.contains(command));
        }
    }

    #[test]
    fn windows_bundle_uses_current_user_nsis_and_evergreen_webview2() {
        let config = config();
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
    fn bundle_associates_only_the_product_project_extension() {
        assert_eq!(
            config()["bundle"]["fileAssociations"],
            serde_json::json!([{
                "ext": ["myalbuns"],
                "name": "Projeto MyAlbuns",
                "description": "Projeto MyAlbuns",
                "role": "Editor"
            }])
        );
    }

    #[test]
    fn asset_protocol_serves_only_published_media_previews() {
        let config = config();
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
        let config = config();
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
            assert!(
                !connect_sources.contains("myalbuns-preview"),
                "the provisional preview protocol remains image-only"
            );
            for cache_origin in ["http://myalbuns-cache.localhost", "myalbuns-cache:"] {
                assert!(
                    connect_sources
                        .split_whitespace()
                        .any(|value| value == cache_origin),
                    "Pixi Assets.load fetches opaque Cache textures"
                );
            }
            let image_sources = policy
                .split(';')
                .find(|directive| directive.trim_start().starts_with("img-src "))
                .expect("the CSP defines img-src");
            for preview_origin in ["http://myalbuns-preview.localhost", "myalbuns-preview:"] {
                assert!(
                    image_sources
                        .split_whitespace()
                        .any(|value| value == preview_origin),
                    "the global preview protocol is allowed only as an image source"
                );
            }
            for media_origin in ["http://myalbuns-cache.localhost", "myalbuns-cache:"] {
                assert!(
                    image_sources
                        .split_whitespace()
                        .any(|value| value == media_origin),
                    "the registered Project Cache protocol is allowed as an image source"
                );
            }
        }
        assert!(
            !security["csp"]
                .as_str()
                .expect("the production CSP is textual")
                .contains("'unsafe-eval'")
        );
    }

    #[test]
    fn composition_roots_create_hidden_global_and_project_windows() {
        let windows = config()["app"]["windows"]
            .as_array()
            .expect("the window list is explicit")
            .clone();

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0]["label"], "global");
        assert_eq!(windows[0]["url"], "global.html");
        assert_eq!(windows[1]["label"], "project");
        assert_eq!(windows[1]["url"], "index.html");
        assert!(windows.iter().all(|window| window["create"] == false));
        assert!(windows.iter().all(|window| window["visible"] == false));
    }

    #[test]
    fn project_window_disables_native_browser_controls() {
        let config = config();
        let project_window = config["app"]["windows"]
            .as_array()
            .expect("the windows are explicit")
            .iter()
            .find(|window| window["label"] == "project")
            .expect("project window exists");

        assert_eq!(project_window["devtools"], false);
        assert_eq!(project_window["zoomHotkeysEnabled"], false);
    }
}
