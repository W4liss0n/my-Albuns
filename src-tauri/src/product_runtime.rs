use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use myalbuns_core::{EditableProject, MediaId, MediaKind, PhotoSourceMetadata};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, project_data_namespace};
use tauri::{Emitter, Manager, WebviewWindowBuilder};

use crate::{
    cache_engine::{CacheEngine, RecoveredCacheArtifact},
    cache_previews::CachePreviewRegistry,
    cache_service::{CacheNamespaceOwner, CacheService},
    desktop_webview_policy,
    export_attempts::ExportAttempts,
    imaging_processor::ImagingProcessor,
    ipc_contract::LinkedMediaChanged,
    logging,
    media_runtime::{MediaBinding, MediaMonitor, MediaResolver, MediaRuntime},
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
pub(crate) const LINKED_MEDIA_CHANGED_EVENT: &str = "myalbuns://linked-media-changed";
pub(crate) const CACHE_PROCESSOR_WARNING_EVENT: &str = "myalbuns://cache-processor-warning";

pub(crate) fn run(
    opened: BootstrappedHostProject,
    app_paths: AppPaths,
) -> Result<(), Box<dyn std::error::Error>> {
    let (request, mut project) = opened.into_parts();
    #[cfg(debug_assertions)]
    crate::dev_host_registration::register_from_environment(&request.launch_nonce)?;

    let cache_service = CacheService::new(app_paths.clone());
    let cache_namespace_owner = cache_service
        .reserve_namespace(project.identity_authority())
        .map_err(io::Error::other)?;
    hydrate_project_from_recovered_cache(&mut project, cache_namespace_owner.recovered_artifacts());
    let project_host = ProjectHost::new(project);
    let cache_previews = CachePreviewRegistry::new(PROJECT_WINDOW_LABEL);
    let media_protocol_registry = cache_previews.clone();
    let setup_paths = app_paths.clone();
    let startup_handshake = ProjectStartupHandshake::new(
        PendingHostTerminal::new(request),
        projection_identity(&project_host)?,
    );
    let mut context = tauri::generate_context!();

    // EdgeDriver's supported launch flow needs the single WebView2 instance to
    // be created by Tauri before the setup hook. Normal Project Hosts retain
    // their explicit, policy-gated window construction below.
    if desktop_webview_policy::automation_enabled() {
        let project_config = context
            .config_mut()
            .app
            .windows
            .iter_mut()
            .find(|window| window.label == PROJECT_WINDOW_LABEL)
            .ok_or_else(|| io::Error::other("the Project window configuration does not exist"))?;
        project_config.create = true;
        project_config.visible = true;
    }

    let run_result = tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(
            crate::cache_previews::CACHE_MEDIA_PROTOCOL_SCHEME,
            move |context, request, responder| {
                crate::cache_previews::respond_to_cache_media_request(
                    media_protocol_registry.clone(),
                    context,
                    request,
                    responder,
                );
            },
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(project_host)
        .manage(startup_handshake)
        .manage(cache_previews)
        .manage(cache_namespace_owner)
        .manage(CacheEngine::default())
        .manage(MediaRuntime::default())
        .manage(MediaMonitor::default())
        .manage(ExportAttempts::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != PROJECT_WINDOW_LABEL {
                    return;
                }
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    window_label = window.label(),
                    event = "project_window_close_requested",
                );
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
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    window_label = window.label(),
                    event = "project_window_destroyed",
                );
                request_window_export_cancellation(window);
            }
        })
        .setup(move |app| setup_host(app, setup_paths))
        .invoke_handler(tauri::generate_handler![
            crate::logging::frontend_log,
            project_ui_ready,
            crate::project_commands::project_state,
            crate::project_commands::apply_project_intent,
            crate::project_commands::import_photo,
            crate::project_commands::photo_drop_target,
            crate::project_commands::relink_media,
            crate::media_preview_commands::retry_unavailable_media,
            crate::project_commands::undo_project,
            crate::project_commands::redo_project,
            crate::project_commands::save_project,
            crate::project_close_commands::request_project_close,
            crate::project_close_commands::resolve_project_close,
            crate::media_preview_commands::prepare_media_previews,
            crate::export_commands::export_sheet,
            crate::export_commands::cancel_export,
        ])
        .run(context);
    run_result?;
    Ok(())
}

fn hydrate_project_from_recovered_cache(
    project: &mut EditableProject,
    artifacts: &[RecoveredCacheArtifact],
) -> usize {
    artifacts
        .iter()
        .filter(|recovered| {
            let artifact = recovered.artifact();
            let Ok(media_id) = artifact.media_id.parse::<MediaId>() else {
                return false;
            };
            let Some(media) = project
                .project()
                .media()
                .iter()
                .find(|media| media.id() == media_id.into_uuid())
            else {
                return false;
            };
            if media.kind() != MediaKind::Photo || !recovered.matches_source_path(media.path()) {
                return false;
            }
            let Ok(metadata) = PhotoSourceMetadata::new(
                artifact.width_px,
                artifact.height_px,
                ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
            ) else {
                return false;
            };
            project.observe_photo_source(media_id, metadata).is_ok()
        })
        .count()
}

fn setup_host(app: &mut tauri::App, app_paths: AppPaths) -> Result<(), Box<dyn std::error::Error>> {
    let projection = app
        .state::<ProjectHost>()
        .projection()
        .map_err(io::Error::other)?;
    logging::initialize(app, &app_paths, ProcessRole::DesktopHost);
    app.manage(OperationGate::new(&app_paths));
    app.manage(ImagingProcessor::default());

    let (project_window, policy_readiness) = if desktop_webview_policy::automation_enabled() {
        (
            app.get_webview_window(PROJECT_WINDOW_LABEL)
                .ok_or_else(|| io::Error::other("the automated Project WebView does not exist"))?,
            None,
        )
    } else {
        let webview_namespace = project_data_namespace(&projection.state.project_id);
        let webview_data_directory = app_paths.webview_data_directory(&webview_namespace)?;
        let project_config = app
            .config()
            .app
            .windows
            .iter()
            .find(|window| window.label == PROJECT_WINDOW_LABEL)
            .cloned()
            .ok_or_else(|| io::Error::other("the Project window configuration does not exist"))?;
        let (policy_signal, policy_readiness) = desktop_webview_policy::page_load_handshake();
        let window = WebviewWindowBuilder::from_config(app, &project_config)?
            .data_directory(webview_data_directory)
            .on_page_load(move |window, payload| {
                policy_signal.observe(&window, payload.event());
            })
            .build()?;
        (window, Some(policy_readiness))
    };
    app.manage(app_paths);
    let app_handle = app.handle().clone();
    let startup_handshake = app.state::<ProjectStartupHandshake>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let startup = async {
            if let Some(policy_readiness) = policy_readiness {
                policy_readiness.wait().await?;
            }
            project_window.show()?;
            let transition = startup_handshake.mark_host_ready()?;
            if transition.newly_observed {
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    process_id = std::process::id(),
                    window_label = PROJECT_WINDOW_LABEL,
                    project_id = safe_log_identifier(&projection.state.project_id),
                    revision = projection.state.revision,
                    event = "host_ready",
                );
            }
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
            startup_handshake.emit_failed(FailureStage::Initialize, FailureCode::IoFailure);
            app_handle.exit(1);
            return;
        }

        start_linked_media_monitor(app_handle.clone());
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

fn projection_identity(project_host: &ProjectHost) -> Result<(String, u64), io::Error> {
    let projection = project_host.projection().map_err(io::Error::other)?;
    Ok((projection.state.project_id, projection.state.revision))
}

#[tauri::command]
fn project_ui_ready(
    window: tauri::WebviewWindow,
    startup: tauri::State<'_, ProjectStartupHandshake>,
) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("UI confirmation belongs only to the Project window".into());
    }

    match startup.confirm_ui_ready() {
        Ok(transition) => {
            if transition.newly_observed {
                tracing::info!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    process_id = std::process::id(),
                    window_label = PROJECT_WINDOW_LABEL,
                    event = "project_ui_ready",
                );
            }
            Ok(())
        }
        Err(error) => {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                error = %error,
                event = "project_ui_ready_handshake_failed",
            );
            window.app_handle().exit(1);
            Err("could not complete the startup handshake".into())
        }
    }
}

fn refresh_changed_photo_sources(host: &ProjectHost, changed_photos: Vec<MediaBinding>) -> usize {
    let mut refreshed = 0;
    for binding in changed_photos {
        match MediaResolver.inspect_photo_binding(&binding) {
            Ok(metadata) => match host.observe_photo_source(&binding, metadata) {
                Ok(()) => refreshed += 1,
                Err(error) => tracing::warn!(
                    target: "myalbuns.desktop",
                    media_id = safe_log_identifier(&binding.media_id),
                    error = %error,
                    event = "photo_source_refresh_rejected",
                ),
            },
            Err(error) => tracing::info!(
                target: "myalbuns.desktop",
                media_id = safe_log_identifier(&binding.media_id),
                error = %error,
                event = "photo_source_refresh_deferred",
            ),
        }
    }
    refreshed
}

pub(crate) fn refresh_project_photos_for_media_update(
    host: &ProjectHost,
    bindings: &[MediaBinding],
    update: &crate::media_runtime::MediaRuntimeUpdate,
) -> usize {
    let changed_photos = bindings
        .iter()
        .filter(|binding| {
            binding.kind == MediaKind::Photo
                && update
                    .changed_media_ids()
                    .iter()
                    .any(|media_id| media_id == &binding.media_id)
        })
        .cloned()
        .collect();
    refresh_changed_photo_sources(host, changed_photos)
}

fn start_linked_media_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if app.get_webview_window(PROJECT_WINDOW_LABEL).is_none() {
                break;
            }
            let catalog = match app.state::<ProjectHost>().authorized_media_catalog() {
                Ok(catalog) => catalog,
                Err(_) => break,
            };
            let monitor = app.state::<MediaMonitor>().inner().clone();
            let runtime = app.state::<MediaRuntime>().inner().clone();
            let bindings = catalog.bindings.clone();
            let poll = match spawn_media_io(move || monitor.poll(&runtime, &bindings)).await {
                Ok(poll) => poll,
                Err(error) => {
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "linked_media_monitor_poll_failed",
                    );
                    continue;
                }
            };
            let Some(update) = poll.update() else {
                continue;
            };
            let changed = update.changed_media_ids().to_vec();
            let invalidated = update.invalidated_media_ids().to_vec();
            if !changed.is_empty() {
                tracing::info!(
                    target: "myalbuns.desktop",
                    changed_media_count = changed.len(),
                    event = "linked_media_observation_applied",
                );
                let photo_bindings = catalog.bindings.clone();
                let photo_update = update.clone();
                if photo_bindings.iter().any(|binding| {
                    binding.kind == MediaKind::Photo
                        && changed.iter().any(|media_id| media_id == &binding.media_id)
                }) {
                    let host = app.state::<ProjectHost>().inner().clone();
                    match spawn_media_io(move || {
                        refresh_project_photos_for_media_update(
                            &host,
                            &photo_bindings,
                            &photo_update,
                        )
                    })
                    .await
                    {
                        Ok(_) => {}
                        Err(error) => tracing::warn!(
                            target: "myalbuns.desktop",
                            error = %error,
                            event = "photo_source_refresh_failed",
                        ),
                    }
                }
            }
            if !changed.is_empty() || !invalidated.is_empty() {
                let namespace = app.state::<CacheNamespaceOwner>();
                app.state::<CacheEngine>().apply_monitor_media_update(
                    namespace.namespace(),
                    app.state::<CachePreviewRegistry>().inner(),
                    update,
                );
                tracing::info!(
                    target: "myalbuns.desktop",
                    invalidated_media_count = invalidated.len(),
                    event = "linked_media_cache_invalidated",
                );
            }
            if !changed.is_empty()
                && let Some(window) = app.get_webview_window(PROJECT_WINDOW_LABEL)
                && let Err(error) = window.emit(
                    LINKED_MEDIA_CHANGED_EVENT,
                    LinkedMediaChanged { media_ids: changed },
                )
            {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    error = %error,
                    event = "linked_media_change_emit_failed",
                );
            }
        }
    });
}

fn spawn_media_io<F, R>(operation: F) -> tauri::async_runtime::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
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
        if self.emitted {
            return Ok(());
        }
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

#[derive(Clone)]
struct ProjectStartupHandshake {
    state: Arc<Mutex<ProjectStartupState>>,
}

struct ProjectStartupState {
    terminal: PendingHostTerminal,
    project_id: String,
    revision: u64,
    readiness: StartupReadiness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StartupTransition {
    newly_observed: bool,
    ready_emitted: bool,
}

#[derive(Default)]
struct StartupReadiness {
    host_ready: bool,
    ui_ready: bool,
    emitted: bool,
}

impl StartupReadiness {
    fn observe(&mut self, signal: StartupSignal) -> StartupTransition {
        let target = match signal {
            StartupSignal::HostReady => &mut self.host_ready,
            StartupSignal::UiReady => &mut self.ui_ready,
        };
        let newly_observed = !*target;
        *target = true;
        let ready_emitted = self.host_ready && self.ui_ready && !self.emitted;
        self.emitted |= ready_emitted;
        StartupTransition {
            newly_observed,
            ready_emitted,
        }
    }
}

#[derive(Clone, Copy)]
enum StartupSignal {
    HostReady,
    UiReady,
}

impl ProjectStartupHandshake {
    fn new(terminal: PendingHostTerminal, identity: (String, u64)) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProjectStartupState {
                terminal,
                project_id: identity.0,
                revision: identity.1,
                readiness: StartupReadiness::default(),
            })),
        }
    }

    fn mark_host_ready(&self) -> io::Result<StartupTransition> {
        self.transition(StartupSignal::HostReady)
    }

    fn confirm_ui_ready(&self) -> io::Result<StartupTransition> {
        self.transition(StartupSignal::UiReady)
    }

    fn transition(&self, signal: StartupSignal) -> io::Result<StartupTransition> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("the startup handshake is unavailable"))?;
        let transition = state.readiness.observe(signal);
        if transition.ready_emitted {
            let project_id = state.project_id.clone();
            let revision = state.revision;
            state.terminal.emit_ready(&project_id, revision)?;
        }
        Ok(transition)
    }

    fn emit_failed(&self, stage: FailureStage, code: FailureCode) {
        if let Ok(mut state) = self.state.lock() {
            state.terminal.emit_failed(stage, code);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, ImportPhoto, InitialProject, OpenProjectRequest,
        PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
        SaveProjectOutcome,
    };
    use myalbuns_imaging_protocol::{
        CacheArtifact, CacheArtifactFormat, CacheBasicColorProfile, CacheFingerprint,
    };
    use myalbuns_paths::OperationPathContext;

    use super::{
        PROJECT_WINDOW_LABEL, StartupReadiness, StartupSignal,
        hydrate_project_from_recovered_cache, refresh_changed_photo_sources,
        refresh_project_photos_for_media_update, spawn_media_io,
    };
    use crate::{
        cache_engine::{CacheSourceBinding, RecoveredCacheArtifact},
        media_runtime::{MediaMonitor, MediaRuntime},
        project_host::ProjectHost,
    };

    #[test]
    fn productive_host_has_one_stable_project_window_label() {
        assert_eq!(PROJECT_WINDOW_LABEL, "project");
    }

    #[test]
    fn ready_is_emitted_once_only_after_native_and_ui_readiness() {
        for order in [
            [StartupSignal::HostReady, StartupSignal::UiReady],
            [StartupSignal::UiReady, StartupSignal::HostReady],
        ] {
            let mut readiness = StartupReadiness::default();
            let first = readiness.observe(order[0]);
            let second = readiness.observe(order[1]);
            let duplicate = readiness.observe(order[1]);

            assert!(!first.ready_emitted);
            assert!(second.ready_emitted);
            assert!(!duplicate.ready_emitted);
            assert!(!duplicate.newly_observed);
        }
    }

    #[test]
    fn blocked_original_inspection_runs_outside_the_product_runtime_caller() {
        let (inspection_started_tx, inspection_started_rx) = mpsc::sync_channel(0);
        let (release_inspection_tx, release_inspection_rx) = mpsc::sync_channel(0);

        let inspection = spawn_media_io(move || {
            inspection_started_tx
                .send(())
                .expect("the caller observes the blocked Original inspection");
            release_inspection_rx
                .recv()
                .expect("the blocked Original inspection is released");
            17
        });

        inspection_started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("scheduling returned before the external Original I/O completed");
        release_inspection_tx
            .send(())
            .expect("the Original inspection can finish");
        assert_eq!(
            tauri::async_runtime::block_on(inspection)
                .expect("the scheduled Original inspection joins cleanly"),
            17
        );
    }

    #[test]
    fn a_stale_photo_observation_does_not_abort_the_rest_of_the_monitor_batch() {
        let root = tempfile::tempdir().expect("temporary Monitor batch fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let first_path = root.path().join("Primeira.jpg");
        let second_path = root.path().join("Segunda.jpg");
        let stale_path = root.path().join("Vinculo-stale.jpg");
        RgbImage::from_pixel(5, 7, Rgb([20, 30, 40]))
            .save_with_format(&first_path, ImageFormat::Jpeg)
            .expect("the first Original is a JPEG");
        RgbImage::from_pixel(17, 11, Rgb([50, 60, 70]))
            .save_with_format(&second_path, ImageFormat::Jpeg)
            .expect("the second Original is a JPEG");
        RgbImage::from_pixel(3, 2, Rgb([80, 90, 100]))
            .save_with_format(&stale_path, ImageFormat::Jpeg)
            .expect("the stale binding still points to an inspectable JPEG");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Project root is captured");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the Monitor batch Project is created");
        let initial_metadata =
            PhotoSourceMetadata::new(1, 1, ["#102030".into(), "#405060".into(), "#708090".into()])
                .expect("the initial runtime metadata is valid");
        let first = project
            .import_photo(ImportPhoto::new(first_path, initial_metadata.clone()))
            .expect("the first Photo is imported");
        let second = project
            .import_photo(ImportPhoto::new(second_path, initial_metadata))
            .expect("the second Photo is imported");
        let host = ProjectHost::new(project);
        let catalog = host
            .authorized_media_catalog()
            .expect("the Monitor receives the authorized catalog");
        let mut stale_first = catalog.bindings[0].clone();
        stale_first.logical_path = stale_path;

        assert_eq!(
            refresh_changed_photo_sources(&host, vec![stale_first, catalog.bindings[1].clone()]),
            1,
            "one stale occurrence cannot consume the valid observation that follows it"
        );
        let projection = host.projection().expect("the batch result is projected");
        let first_projection = projection
            .state
            .album
            .media
            .iter()
            .find(|media| media.id == first.media_id)
            .expect("the first occurrence remains present");
        let second_projection = projection
            .state
            .album
            .media
            .iter()
            .find(|media| media.id == second.media_id)
            .expect("the second occurrence remains present");
        assert_eq!(first_projection.source_width_px, Some(1));
        assert_eq!(first_projection.source_height_px, Some(1));
        assert_eq!(second_projection.source_width_px, Some(17));
        assert_eq!(second_projection.source_height_px, Some(11));
    }

    #[test]
    fn a_confirmation_consumed_by_preview_demand_refreshes_photo_geometry_without_history() {
        let root = tempfile::tempdir().expect("temporary preview demand fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let photo_path = root.path().join("Foto.jpg");
        RgbImage::from_pixel(19, 7, Rgb([20, 30, 40]))
            .save_with_format(&photo_path, ImageFormat::Jpeg)
            .expect("the demanded Original is a JPEG");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Project root is captured");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the preview demand Project is created");
        let imported = project
            .import_photo(ImportPhoto::new(
                photo_path,
                PhotoSourceMetadata::new(
                    1,
                    1,
                    ["#102030".into(), "#405060".into(), "#708090".into()],
                )
                .expect("the fallback metadata is valid"),
            ))
            .expect("the demanded Photo is imported");
        let host = ProjectHost::new(project);
        let catalog = host
            .authorized_media_catalog()
            .expect("the demand receives the authorized catalog");
        let monitor = MediaMonitor::default();
        let runtime = MediaRuntime::default();
        assert!(monitor.poll(&runtime, &catalog.bindings).update().is_none());
        let confirmed = monitor.poll(&runtime, &catalog.bindings);
        let update = confirmed
            .update()
            .expect("the stable demand confirmation is adopted");
        let before = host.projection().expect("the fallback is projected");

        assert_eq!(
            refresh_project_photos_for_media_update(&host, &catalog.bindings, update),
            1
        );

        let after = host.projection().expect("the demand refresh is projected");
        let photo = after
            .state
            .album
            .media
            .iter()
            .find(|media| media.id == imported.media_id)
            .expect("the demanded Photo remains projected");
        assert_eq!(photo.source_width_px, Some(19));
        assert_eq!(photo.source_height_px, Some(7));
        assert_eq!(after.state.revision, before.state.revision);
        assert_eq!(after.state.dirty, before.state.dirty);
        assert_eq!(after.state.can_undo, before.state.can_undo);
        assert_eq!(after.state.can_redo, before.state.can_redo);
    }

    #[test]
    fn retrying_an_unavailable_photo_refreshes_changed_geometry_without_history() {
        let root = tempfile::tempdir().expect("temporary retry fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let photo_path = root.path().join("Foto.jpg");
        RgbImage::from_pixel(3, 2, Rgb([20, 30, 40]))
            .save_with_format(&photo_path, ImageFormat::Jpeg)
            .expect("the initial Original is a JPEG");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Project root is captured");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the retry Project is created");
        let imported = project
            .import_photo(ImportPhoto::new(
                photo_path.clone(),
                PhotoSourceMetadata::new(
                    1,
                    1,
                    ["#102030".into(), "#405060".into(), "#708090".into()],
                )
                .expect("the fallback metadata is valid"),
            ))
            .expect("the retry Photo is imported");
        let host = ProjectHost::new(project);
        let catalog = host
            .authorized_media_catalog()
            .expect("the retry receives the authorized catalog");
        let binding = catalog.bindings[0].clone();
        std::fs::remove_file(&photo_path).expect("the initial Original is removed");
        std::fs::create_dir(&photo_path)
            .expect("an unexpected object makes the binding unavailable");
        let monitor = MediaMonitor::default();
        let runtime = MediaRuntime::default();
        assert!(monitor.poll(&runtime, &catalog.bindings).update().is_none());
        assert!(monitor.poll(&runtime, &catalog.bindings).update().is_some());
        std::fs::remove_dir(&photo_path).expect("the unavailable object is removed");
        RgbImage::from_pixel(23, 5, Rgb([50, 60, 70]))
            .save_with_format(&photo_path, ImageFormat::Jpeg)
            .expect("the replacement Original is a JPEG");
        let before = host.projection().expect("the fallback is projected");
        let inspection = monitor
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect("the unavailable occurrence is retried");

        assert_eq!(
            refresh_project_photos_for_media_update(
                &host,
                std::slice::from_ref(&binding),
                inspection.update(),
            ),
            1
        );

        let after = host.projection().expect("the retry refresh is projected");
        let photo = after
            .state
            .album
            .media
            .iter()
            .find(|media| media.id == imported.media_id)
            .expect("the retried Photo remains projected");
        assert_eq!(photo.source_width_px, Some(23));
        assert_eq!(photo.source_height_px, Some(5));
        assert_eq!(after.state.revision, before.state.revision);
        assert_eq!(after.state.dirty, before.state.dirty);
        assert_eq!(after.state.can_undo, before.state.can_undo);
        assert_eq!(after.state.can_redo, before.state.can_redo);
    }

    #[test]
    fn reopening_with_a_missing_original_restores_oriented_geometry_from_verified_cache() {
        let root = tempfile::tempdir().expect("temporary missing Original Project");
        let project_path = root.path().join("Projeto com contexto de Cache.myalbuns");
        let original_path = root.path().join("Foto orientada ausente.jpg");
        std::fs::write(&original_path, b"linked Original")
            .expect("the linked Original initially exists");
        let mut create_context = OperationPathContext::new();
        create_context
            .capture(&project_path)
            .expect("the Project root is captured");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), create_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the productive Project is created");
        let source_metadata = PhotoSourceMetadata::new(
            800,
            1_200,
            ["#102030".into(), "#405060".into(), "#708090".into()],
        )
        .expect("the oriented source metadata is valid");
        let imported = project
            .import_photo(ImportPhoto::new(original_path.clone(), source_metadata))
            .expect("the linked Photo is imported");
        let sheet_id = imported.projection.state.album.sheets[0].id.clone();
        let placed = project
            .apply_with_outcome(ProjectIntent::AddPhoto {
                sheet_id,
                media_id: imported.media_id,
                mode: PhotoPlacementMode::Normal,
            })
            .expect("the linked Photo is placed");
        let expected_base_fill_zoom = placed.projection.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the placed Photo is composed")
            .placement
            .base_fill_zoom;
        assert_eq!(
            project.save(2).expect("the Photo composition is saved"),
            SaveProjectOutcome::Saved { revision: 2 }
        );
        drop(project);
        std::fs::remove_file(&original_path).expect("the Original becomes absent before reopening");

        let mut open_context = OperationPathContext::new();
        open_context
            .capture(&project_path)
            .expect("the reopened Project root is captured");
        let mut reopened = core
            .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                project_path,
                open_context.freeze(),
            )))
            .expect("the Project reopens without reading the absent Original");
        assert_eq!(
            reopened.projection().state.album.media[0].source_width_px,
            Some(1),
            "without runtime context the Core has only its non-authoritative fallback"
        );
        let artifact = CacheArtifact {
            media_id: imported.media_id.to_string(),
            generation_id: "g-verified-context".into(),
            width_px: 800,
            height_px: 1_200,
            preview_bytes: 1,
            format: CacheArtifactFormat::Jpeg,
            exif_orientation: Some(6),
            source_page_count: None,
            basic_color_profile: CacheBasicColorProfile::Srgb,
            fingerprint: CacheFingerprint::sha256_full_file(1, "a".repeat(64))
                .expect("the recovered fingerprint is valid"),
        };

        let stale_path = root.path().join("Outro vinculo.jpg");
        let stale_recovery = RecoveredCacheArtifact::new(
            artifact.clone(),
            CacheSourceBinding::for_path(&stale_path),
        );
        assert_eq!(
            hydrate_project_from_recovered_cache(&mut reopened, &[stale_recovery]),
            0,
            "a Cache generation from a relinked path cannot hydrate the restored binding"
        );
        assert_eq!(
            reopened.projection().state.album.media[0].source_width_px,
            Some(1)
        );

        let recovered =
            RecoveredCacheArtifact::new(artifact, CacheSourceBinding::for_path(&original_path));
        assert_eq!(
            hydrate_project_from_recovered_cache(&mut reopened, &[recovered]),
            1
        );
        let restored = reopened.projection();
        assert_eq!(restored.state.album.media[0].source_width_px, Some(800));
        assert_eq!(restored.state.album.media[0].source_height_px, Some(1_200));
        assert_eq!(
            restored.composition.sheets[0].frames[0]
                .photo
                .as_ref()
                .expect("the cached Photo context remains composed")
                .placement
                .base_fill_zoom,
            expected_base_fill_zoom,
            "the Cache dimensions are already oriented and must not be swapped again"
        );
        assert_eq!(restored.state.revision, 2);
        assert!(!restored.state.dirty);
        assert!(!restored.state.can_undo);
        assert_eq!(reopened.project().media()[0].path(), original_path);
    }
}
