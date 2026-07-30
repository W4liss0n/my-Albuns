mod benchmark_corpus;
mod cache_engine;
mod export_pipeline;
mod imaging_processor;
#[cfg(test)]
mod imaging_recovery_integration;
mod logging;
mod project_host;
#[path = "../../tests/support/sample_project.rs"]
mod sample_project;
mod topology_benchmark;
mod topology_spike;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use myalbuns_core::{EditorProjection, ExportResult, ProjectIntent};
use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, OperationPathContext};
use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use cache_engine::CacheWork;
use imaging_processor::{InvocationContext, TauriImagingTransport};
use logging::{LoggingState, frontend_log};
use project_host::ProjectHost;
use topology_benchmark::{
    TopologyBenchmarkState, report_topology_benchmark_failure, report_topology_canvas_benchmark,
    report_topology_canvas_ready, topology_benchmark_config,
};
use topology_spike::TopologySpike;

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

const CACHE_PREVIEW_MAX_EDGE_PX: u32 = 1600;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaPreviewCatalog {
    previews: Vec<MediaPreview>,
    generated_count: usize,
    reused_count: usize,
    source_bytes: u64,
    preview_bytes: u64,
    elapsed_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaPreview {
    media_id: String,
    url: String,
    width_px: u32,
    height_px: u32,
}

#[tauri::command]
fn project_state(
    operation_id: String,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    logging::validate_optional_identifier("operationId", Some(&operation_id))?;
    let projection = state.projection(window.label())?;
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        operation_id = operation_id.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_state_read",
    );
    Ok(projection)
}

#[tauri::command]
fn apply_project_intent(
    intent: ProjectIntent,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let intent_kind = match &intent {
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::FillLeftmostPlaceholder { .. } => "fill_leftmost_placeholder",
    };
    let projection = state.apply(window.label(), intent).inspect_err(|_| {
        tracing::warn!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            window_label = window.label(),
            intent = intent_kind,
            event = "project_intent_rejected",
        );
    })?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        intent = intent_kind,
        event = "project_intent_applied",
    );
    Ok(projection)
}

#[tauri::command]
fn undo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.undo(window.label())?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_undo_completed",
    );
    Ok(projection)
}

#[tauri::command]
fn redo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.redo(window.label())?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_redo_completed",
    );
    Ok(projection)
}

#[tauri::command]
async fn prepare_media_previews(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    app_paths: State<'_, AppPaths>,
    logging: State<'_, LoggingState>,
) -> Result<Option<MediaPreviewCatalog>, String> {
    let cache_sequence = CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("cache-{}-{cache_sequence}", std::process::id());
    let projection = state.projection(window.label())?;
    let project_id = projection.state.project_id;
    let cache_paths = app_paths
        .project_cache(&project_id)
        .map_err(|error| error.to_string())?;
    let Some(sources) = state.cache_sources(window.label())? else {
        return Ok(None);
    };
    let mut path_context = OperationPathContext::new();
    path_context
        .capture(cache_paths.root())
        .map_err(|error| error.to_string())?;
    for source in &sources {
        path_context
            .capture(source.source_path())
            .map_err(|error| error.to_string())?;
    }
    let work = CacheWork::new(
        request_id.clone(),
        project_id.clone(),
        cache_paths.clone(),
        sources,
        CACHE_PREVIEW_MAX_EDGE_PX,
        path_context.freeze(),
    );
    let safe_project_id = safe_log_identifier(&project_id);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        media_count = work.sources.len(),
        event = "media_cache_started",
    );
    let started = Instant::now();
    let context = InvocationContext::new(request_id.clone(), safe_project_id);
    let mut transport = TauriImagingTransport::new(&app, &logging);
    let execution = cache_engine::execute(&mut transport, &app_paths, work, &context)
        .await
        .map_err(|failure| {
            log_media_cache_failure(
                &request_id,
                safe_project_id,
                failure.stage.as_str(),
                failure.exit_code,
            );
            failure.message
        })?;
    let recovered_process_id = execution
        .recovery
        .map(|recovery| recovery.failed_process_id);
    let removed_recovery_temporary_count = execution
        .recovery
        .map_or(0, |recovery| recovery.removed_temporary_count);
    let completed = execution.completion;
    let catalog = MediaPreviewCatalog {
        previews: completed
            .artifacts
            .iter()
            .map(|artifact| -> Result<MediaPreview, String> {
                let preview_path = cache_paths
                    .preview_file(&artifact.media_id, &artifact.generation_id)
                    .map_err(|error| error.to_string())?;
                Ok(MediaPreview {
                    media_id: artifact.media_id.clone(),
                    url: app_paths
                        .cache_asset_url(&preview_path)
                        .map_err(|error| error.to_string())?,
                    width_px: artifact.width_px,
                    height_px: artifact.height_px,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        generated_count: completed.generated_count,
        reused_count: completed.reused_count,
        source_bytes: completed.source_bytes,
        preview_bytes: completed.preview_bytes,
        elapsed_ms: started.elapsed().as_millis(),
    };
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        generated_count = catalog.generated_count,
        reused_count = catalog.reused_count,
        source_bytes = catalog.source_bytes,
        preview_bytes = catalog.preview_bytes,
        recovered_process_id,
        removed_recovery_temporary_count,
        elapsed_ms = catalog.elapsed_ms,
        event = "media_cache_completed",
    );
    Ok(Some(catalog))
}

#[tauri::command]
async fn export_spike(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
) -> Result<ExportResult, String> {
    let export_sequence = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("export-{}-{export_sequence}", std::process::id());
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        window_label = window.label(),
        event = "export_started",
    );
    let snapshot = state
        .render_snapshot(window.label())
        .inspect_err(|_| log_export_failure(&request_id, None, "session_lock", None))?;

    let output_dir = std::env::temp_dir().join("MyAlbuns").join("spike");
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Não foi possível preparar o Destino da Exportação: {error}"))?;
    let output_path = output_dir.join(format!(
        "Album-Horizonte_{}_{export_sequence:03}.png",
        std::process::id()
    ));
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .ok_or_else(|| "O snapshot não contém Lâminas.".to_string())?
        .sheet_id
        .clone();
    let sources = state.export_sources(window.label(), &snapshot, &sheet_id)?;
    let dpi = if sources.is_some() { 300 } else { 25 };
    let project_id = safe_log_identifier(&snapshot.project_id).map(str::to_owned);
    let plan = export_pipeline::plan(
        snapshot,
        export_pipeline::ExportOptions::new(
            request_id.clone(),
            output_path,
            sheet_id,
            dpi,
            sources,
        ),
    )
    .map_err(|failure| {
        log_export_failure(
            &request_id,
            project_id.as_deref(),
            failure.stage.as_str(),
            failure.exit_code,
        );
        failure.message
    })?;
    let mut path_context = OperationPathContext::new();
    for path in plan.required_paths() {
        path_context
            .capture(path)
            .map_err(|error| error.to_string())?;
    }
    let root_bindings = path_context.freeze();

    let started = Instant::now();
    let context = InvocationContext::new(request_id.clone(), project_id.clone());
    let mut transport = TauriImagingTransport::new(&app, &logging);
    let cancellation = AtomicBool::new(false);
    let mut progress = |progress: export_pipeline::ExportProgress| {
        tracing::debug!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            operation_id = request_id.as_str(),
            project_id = project_id.as_deref(),
            stage = ?progress.stage,
            completed_units = progress.completed_units,
            total_units = progress.total_units,
            event = "export_progress",
        );
    };
    let published = export_pipeline::execute(
        &mut transport,
        plan,
        &root_bindings,
        &cancellation,
        &mut progress,
        &context,
    )
    .await
    .map_err(|failure| {
        log_export_failure(
            &request_id,
            project_id.as_deref(),
            failure.stage.as_str(),
            failure.exit_code,
        );
        failure.message
    })?;
    let completed = published.completion;
    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        process_id = std::process::id(),
        project_id = project_id.as_deref(),
        window_label = window.label(),
        width_px = completed.width_px,
        height_px = completed.height_px,
        dpi = completed.dpi,
        source_count = completed.source_count,
        source_bytes = completed.source_bytes,
        output_bytes = completed.output_bytes,
        output_sha256 = completed.output_sha256.as_str(),
        elapsed_ms,
        event = "export_completed",
    );

    Ok(ExportResult {
        output_path: published
            .output_path
            .to_str()
            .ok_or_else(|| {
                "o caminho da Exportação não pode ser representado pela interface".to_string()
            })?
            .to_owned(),
        width_px: completed.width_px,
        height_px: completed.height_px,
    })
}

fn log_export_failure(
    operation_id: &str,
    project_id: Option<&str>,
    stage: &str,
    exit_code: Option<i32>,
) {
    log_imaging_failure("export_failed", operation_id, project_id, stage, exit_code);
}

fn log_media_cache_failure(
    operation_id: &str,
    project_id: Option<&str>,
    stage: &str,
    exit_code: Option<i32>,
) {
    log_imaging_failure(
        "media_cache_failed",
        operation_id,
        project_id,
        stage,
        exit_code,
    );
}

fn log_imaging_failure(
    event: &str,
    operation_id: &str,
    project_id: Option<&str>,
    stage: &str,
    exit_code: Option<i32>,
) {
    tracing::error!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id,
        project_id,
        stage,
        exit_code,
        event,
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let topology = TopologySpike::from_environment()
        .unwrap_or_else(|error| panic!("configuração inválida do spike de topologia: {error}"));
    let project_host = topology
        .project_host()
        .unwrap_or_else(|error| panic!("corpus inválido do spike de topologia: {error}"));
    let topology_benchmark = TopologyBenchmarkState::from_environment(&topology)
        .unwrap_or_else(|error| panic!("benchmark de topologia inválido: {error}"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .manage(topology_benchmark)
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
            report_topology_benchmark_failure
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
