use std::path::PathBuf;

use myalbuns_core::{
    ProjectIdentityAuthority, ProjectLocation, SaveAsAuthorization, SaveAsProjectRequest,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPathsError, OperationPathContext};
use tauri::{Manager, WebviewWindow};
use uuid::Uuid;

use crate::{
    cache_activity_gate::CachePause,
    cache_engine::CacheEngine,
    cache_previews::CachePreviewRegistry,
    cache_service::{ActiveCacheNamespace, CacheService},
    product_runtime::project_window_title,
    project_host::{ProjectHost, ProjectHostSaveAsError, ProjectHostSaveAsResult},
    project_recovery::RecoveryCoordinator,
    project_webview_authority::{CommittedProjectWebview, ProjectWebviewAuthority},
};

#[derive(Debug)]
pub(crate) enum IdentityTransitionError {
    TitleRead,
    Path(AppPathsError),
    Worker,
    Save(ProjectHostSaveAsError),
}

/// Returns only after the old authority is retired and every staged owner is finalized.
pub(crate) async fn save_as(
    window: WebviewWindow,
    host: ProjectHost,
    expected_revision: u64,
    path: PathBuf,
    authorization: SaveAsAuthorization,
) -> Result<ProjectHostSaveAsResult, IdentityTransitionError> {
    let next_title = project_window_title(&path);
    let previous_title = window.title().map_err(|error| {
        tracing::error!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(), window_label = window.label(),
            error = %error, event = "project_save_as_title_read_failed");
        IdentityTransitionError::TitleRead
    })?;
    let cache_pause = window.state::<CacheEngine>().pause().await;
    let window_label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let mut paths = OperationPathContext::new();
        paths.capture(&path).map_err(IdentityTransitionError::Path)?;
        let request = SaveAsProjectRequest::new(
            expected_revision, ProjectLocation::new(path, paths.freeze()), authorization,
        );
        let app = window.app_handle();
        IdentityTransition {
            cache: &app.state::<CacheService>(),
            engine: &app.state::<CacheEngine>(),
            registry: &app.state::<CachePreviewRegistry>(),
            active_cache: &app.state::<ActiveCacheNamespace>(),
            recovery: &app.state::<RecoveryCoordinator>(),
        }.save_as(&host, request, cache_pause, &NativeIdentityPresentation {
            window: &window, previous_title, next_title,
        }).map_err(IdentityTransitionError::Save)
    }).await.map_err(|error| {
        tracing::error!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(),
            window_label, expected_revision, error = %error, event = "project_save_as_worker_failed");
        IdentityTransitionError::Worker
    })?
}

/// Only presentation is substituted in headless tests; persistence, Cache and Recovery are real.
trait IdentityPresentation {
    type Webview;
    fn replace_webview(
        &self,
        previous: Uuid,
        next: &ProjectIdentityAuthority,
    ) -> Result<Self::Webview, ()>;
    fn set_new_title(&self) -> Result<(), ()>;
    fn restore_title(&self) -> Result<(), ()>;
    fn rollback_webview(&self, webview: Self::Webview) -> Result<(), ()>;
    fn finalize_webview(&self, webview: Self::Webview);
    fn terminate_after_failed_rollback(&self);
}

struct IdentityTransition<'a> {
    cache: &'a CacheService,
    engine: &'a CacheEngine,
    registry: &'a CachePreviewRegistry,
    active_cache: &'a ActiveCacheNamespace,
    recovery: &'a RecoveryCoordinator,
}

impl IdentityTransition<'_> {
    fn save_as(
        &self,
        host: &ProjectHost,
        request: SaveAsProjectRequest,
        cache_pause: CachePause,
        presentation: &impl IdentityPresentation,
    ) -> Result<ProjectHostSaveAsResult, ProjectHostSaveAsError> {
        let mut staged = None;
        let saved = host.save_as_with_transition(request, |previous, authority, outcome| {
            let cache = self.cache.reserve_fresh_namespace(authority).map_err(|error| {
                tracing::error!(target: "myalbuns.desktop", error = %error,
                    event = "project_save_as_cache_stage_failed");
            })?;
            tracing::info!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(),
                project_id = safe_log_identifier(&authority.project_id().hyphenated().to_string()),
                cache_entry_count = 0, cache_byte_count = 0, event = "project_save_as_cache_staged_empty");
            let webview = presentation.replace_webview(outcome.previous_project_id, authority)?;
            if presentation.set_new_title().is_err() {
                rollback(presentation, webview, false);
                return Err(());
            }
            if let Err(error) = self.recovery.finish(previous) {
                tracing::error!(target: "myalbuns.desktop", error = %error,
                    event = "project_save_as_recovery_transition_failed");
                rollback(presentation, webview, true);
                return Err(());
            }
            tracing::info!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(),
                project_id = safe_log_identifier(&authority.project_id().hyphenated().to_string()),
                event = "project_save_as_previous_recovery_finished");
            staged = Some((cache, webview));
            Ok(())
        })?;
        let (cache, webview) = staged.expect("successful Save As stages its local authority");
        self.engine.retire_project_identity(
            &cache_pause,
            self.registry,
            &saved.outcome.previous_project_id.hyphenated().to_string(),
        );
        drop(self.active_cache.transition_to(cache));
        presentation.finalize_webview(webview);
        tracing::info!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(),
            project_id = safe_log_identifier(&saved.projection.state.project_id),
            event = "project_save_as_local_authority_transitioned");
        drop(cache_pause);
        Ok(saved)
    }
}

fn rollback<P: IdentityPresentation>(presentation: &P, webview: P::Webview, restore_title: bool) {
    let title_failed = restore_title && presentation.restore_title().is_err();
    let webview_failed = presentation.rollback_webview(webview).is_err();
    if title_failed || webview_failed {
        presentation.terminate_after_failed_rollback();
    }
}

struct NativeIdentityPresentation<'a> {
    window: &'a WebviewWindow,
    previous_title: String,
    next_title: String,
}

impl IdentityPresentation for NativeIdentityPresentation<'_> {
    type Webview = CommittedProjectWebview;

    fn replace_webview(
        &self,
        previous: Uuid,
        next: &ProjectIdentityAuthority,
    ) -> Result<Self::Webview, ()> {
        let app = self.window.app_handle();
        app.state::<ProjectWebviewAuthority>()
            .stage(app, previous, next)
            .map_err(|error| {
                tracing::error!(target: "myalbuns.desktop", error = %error,
                event = "project_save_as_webview_stage_failed");
            })?
            .commit(app)
            .map_err(|error| {
                tracing::error!(target: "myalbuns.desktop", error = %error,
                event = "project_save_as_webview_transition_failed");
            })
    }

    fn set_new_title(&self) -> Result<(), ()> {
        self.window.set_title(&self.next_title).map_err(|error| {
            tracing::error!(target: "myalbuns.desktop", process_role = ProcessRole::DesktopHost.as_str(), window_label = self.window.label(),
                error = %error, event = "project_save_as_title_update_failed");
        })
    }

    fn restore_title(&self) -> Result<(), ()> {
        self.window
            .set_title(&self.previous_title)
            .map_err(|error| {
                tracing::error!(target: "myalbuns.desktop", error = %error,
                event = "project_save_as_title_rollback_failed");
            })
    }

    fn rollback_webview(&self, webview: Self::Webview) -> Result<(), ()> {
        webview.rollback(self.window.app_handle()).map_err(|error| {
            tracing::error!(target: "myalbuns.desktop", error = %error,
                event = "project_save_as_webview_rollback_failed");
        })
    }

    fn finalize_webview(&self, webview: Self::Webview) {
        webview.finalize();
    }
    fn terminate_after_failed_rollback(&self) {
        self.window.app_handle().exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_recovery::RecoveryStore;
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectIntent,
        SaveAsProjectError,
    };
    use myalbuns_paths::AppPaths;
    use std::{
        cell::{Cell, RefCell},
        fs,
    };

    struct Fixture {
        host: ProjectHost,
        cache: CacheService,
        active: ActiveCacheNamespace,
        engine: CacheEngine,
        registry: CachePreviewRegistry,
        recovery: RecoveryCoordinator,
        checkpoint: PathBuf,
        original: PathBuf,
        paths: AppPaths,
        root: tempfile::TempDir,
    }

    fn location(path: PathBuf) -> ProjectLocation {
        let mut paths = OperationPathContext::new();
        paths.capture(&path).unwrap();
        ProjectLocation::new(path, paths.freeze())
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            fs::create_dir_all(root.path().join("roaming")).unwrap();
            fs::create_dir_all(root.path().join("local")).unwrap();
            let original = root.path().join("Original.myalbuns");
            let mut project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    location(original.clone()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            project.apply(ProjectIntent::SetDpi { dpi: 360 }).unwrap();
            let cache = CacheService::new(paths.clone());
            let active = ActiveCacheNamespace::new(
                cache
                    .reserve_namespace(project.identity_authority())
                    .unwrap(),
            );
            let store = RecoveryStore::new(paths.clone());
            let checkpoint = store.checkpoint_path(project.identity_authority()).unwrap();
            store
                .publish(
                    project.identity_authority(),
                    &project.recovery_checkpoint().unwrap(),
                )
                .unwrap();
            Self {
                host: ProjectHost::new(project),
                cache,
                active,
                engine: CacheEngine::default(),
                registry: CachePreviewRegistry::new("project"),
                recovery: RecoveryCoordinator::new(store),
                checkpoint,
                original,
                paths,
                root,
            }
        }

        fn transition(&self) -> IdentityTransition<'_> {
            IdentityTransition {
                cache: &self.cache,
                engine: &self.engine,
                registry: &self.registry,
                active_cache: &self.active,
                recovery: &self.recovery,
            }
        }

        fn request(&self) -> SaveAsProjectRequest {
            SaveAsProjectRequest::new(
                self.host.projection().unwrap().state.revision,
                location(self.root.path().join("Copia.myalbuns")),
                SaveAsAuthorization::CreateOnly,
            )
        }
    }

    struct Presentation {
        failures: Vec<&'static str>,
        events: RefCell<Vec<&'static str>>,
        identity: Cell<Uuid>,
        previous: Uuid,
        title: Cell<&'static str>,
        terminated: Cell<bool>,
    }

    impl Presentation {
        fn new(fixture: &Fixture, failures: Vec<&'static str>) -> Self {
            let previous = fixture.host.identity_authority().unwrap().project_id();
            Self {
                failures,
                events: RefCell::new(vec![]),
                identity: Cell::new(previous),
                previous,
                title: Cell::new("original"),
                terminated: Cell::new(false),
            }
        }

        fn perform(&self, event: &'static str) -> Result<(), ()> {
            self.events.borrow_mut().push(event);
            if self.failures.contains(&event) {
                Err(())
            } else {
                Ok(())
            }
        }
    }

    impl IdentityPresentation for Presentation {
        type Webview = Uuid;
        fn replace_webview(
            &self,
            previous: Uuid,
            next: &ProjectIdentityAuthority,
        ) -> Result<Uuid, ()> {
            assert_eq!(previous, self.previous);
            self.perform("webview")?;
            self.identity.set(next.project_id());
            Ok(next.project_id())
        }
        fn set_new_title(&self) -> Result<(), ()> {
            self.perform("title")?;
            self.title.set("copy");
            Ok(())
        }
        fn restore_title(&self) -> Result<(), ()> {
            self.perform("restore-title")?;
            self.title.set("original");
            Ok(())
        }
        fn rollback_webview(&self, _: Uuid) -> Result<(), ()> {
            self.perform("restore-webview")?;
            self.identity.set(self.previous);
            Ok(())
        }
        fn finalize_webview(&self, next: Uuid) {
            assert_eq!(self.identity.get(), next);
            self.perform("finalize").unwrap();
        }
        fn terminate_after_failed_rollback(&self) {
            self.terminated.set(true);
        }
    }

    #[test]
    fn returns_only_after_all_authorities_are_adopted_and_preserves_history_and_original() {
        tauri::async_runtime::block_on(async {
            let fixture = Fixture::new();
            let original = fs::read(&fixture.original).unwrap();
            let previous_cache = fixture.active.namespace();
            let presentation = Presentation::new(&fixture, vec![]);
            let saved = fixture
                .transition()
                .save_as(
                    &fixture.host,
                    fixture.request(),
                    fixture.engine.pause().await,
                    &presentation,
                )
                .unwrap();
            assert_ne!(saved.outcome.project_id, saved.outcome.previous_project_id);
            assert_eq!(presentation.identity.get(), saved.outcome.project_id);
            assert_eq!(
                fixture.active.namespace().project_id(),
                saved.projection.state.project_id
            );
            assert_ne!(
                fixture.active.namespace().paths().root(),
                previous_cache.paths().root()
            );
            assert_eq!(
                fixture
                    .paths
                    .inspect_cache_namespace(fixture.active.namespace().paths())
                    .unwrap()
                    .unwrap()
                    .bytes(),
                0
            );
            assert!(!fixture.checkpoint.exists());
            assert_eq!(fs::read(&fixture.original).unwrap(), original);
            assert_eq!(presentation.title.get(), "copy");
            assert_eq!(
                *presentation.events.borrow(),
                ["webview", "title", "finalize"]
            );
            let undone = fixture.host.undo().unwrap();
            assert_eq!(undone.state.project_id, saved.projection.state.project_id);
            assert_eq!(undone.state.document.dpi, 300);
            assert_eq!(fixture.host.redo().unwrap().state.document.dpi, 360);
            let _released = fixture.engine.pause().await;
        });
    }

    #[test]
    fn failures_preserve_the_previous_session_cache_and_checkpoint() {
        tauri::async_runtime::block_on(async {
            for failure in ["cache", "webview", "title", "recovery", "destination"] {
                let fixture = Fixture::new();
                let previous = fixture.host.projection().unwrap();
                let previous_cache = fixture.active.namespace();
                let original = fs::read(&fixture.original).unwrap();
                if failure == "recovery" {
                    fs::remove_file(&fixture.checkpoint).unwrap();
                    fs::create_dir(&fixture.checkpoint).unwrap();
                }
                if failure == "destination" {
                    fs::write(fixture.root.path().join("Copia.myalbuns"), b"occupied").unwrap();
                }
                let blocked = fixture.root.path().join("blocked");
                fs::write(&blocked, b"not a storage directory").unwrap();
                let unavailable_cache = CacheService::new(AppPaths::from_roots(&blocked, &blocked));
                let mut transition = fixture.transition();
                if failure == "cache" {
                    transition.cache = &unavailable_cache;
                }
                let presentation = Presentation::new(&fixture, vec![failure]);
                let result = transition.save_as(
                    &fixture.host,
                    fixture.request(),
                    fixture.engine.pause().await,
                    &presentation,
                );
                assert!(result.is_err(), "{failure}");
                if failure != "destination" {
                    assert!(
                        matches!(
                            result,
                            Err(ProjectHostSaveAsError::Project(
                                SaveAsProjectError::SaveAsStateIndeterminate
                            ))
                        ),
                        "{failure}"
                    );
                }
                assert_eq!(fixture.host.projection().unwrap(), previous, "{failure}");
                assert_eq!(
                    fixture.active.namespace().project_id(),
                    previous_cache.project_id()
                );
                assert!(fixture.checkpoint.exists());
                assert_eq!(fs::read(&fixture.original).unwrap(), original);
                assert_eq!(presentation.identity.get(), presentation.previous);
                assert_eq!(presentation.title.get(), "original");
                assert!(!presentation.terminated.get());
                assert!(!presentation.events.borrow().contains(&"finalize"));
                let _released = fixture.engine.pause().await;
            }
        });
    }

    #[test]
    fn failed_compensation_attempts_both_restorations_before_terminating() {
        tauri::async_runtime::block_on(async {
            for failures in [
                vec!["restore-title"],
                vec!["restore-webview"],
                vec!["restore-title", "restore-webview"],
            ] {
                let fixture = Fixture::new();
                fs::remove_file(&fixture.checkpoint).unwrap();
                fs::create_dir(&fixture.checkpoint).unwrap();
                let previous = fixture.host.projection().unwrap();
                let presentation = Presentation::new(&fixture, failures);
                assert!(
                    fixture
                        .transition()
                        .save_as(
                            &fixture.host,
                            fixture.request(),
                            fixture.engine.pause().await,
                            &presentation
                        )
                        .is_err()
                );
                assert_eq!(fixture.host.projection().unwrap(), previous);
                assert_eq!(
                    *presentation.events.borrow(),
                    ["webview", "title", "restore-title", "restore-webview"]
                );
                assert!(presentation.terminated.get());
                let _released = fixture.engine.pause().await;
            }
        });
    }
}
