use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use myalbuns_paths::AppPaths;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, webview::WebviewBuilder};
use uuid::Uuid;

use crate::{
    desktop_webview_policy,
    named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant},
    product_runtime::PROJECT_WINDOW_LABEL,
};

const PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(15);
/// Upper bound on Project WebView profiles held at once, across processes.
const MAX_PROJECT_WEBVIEW_SLOTS: u32 = 64;
const PROJECT_SLOT_PREFIX: &str = "project-";

/// A reusable WebView2 profile for one Project Host. Profiles hold only
/// browser caches, so a Host takes the first free slot instead of a profile
/// per Project Identity: the number of profiles stays bounded by the Hosts
/// open at once, and two processes never share a profile.
struct WebviewSlot {
    namespace: String,
    _reservation: NamedMutexGrant,
}

impl WebviewSlot {
    fn acquire(app_paths: &AppPaths) -> io::Result<Self> {
        for index in 1..=MAX_PROJECT_WEBVIEW_SLOTS {
            let slot = NamedMutex::scoped(
                app_paths,
                "ProjectWebviewSlot",
                &index.to_string(),
                "project-webview-slot",
            );
            match slot.try_acquire() {
                Ok(reservation) => {
                    return Ok(Self {
                        namespace: format!("{PROJECT_SLOT_PREFIX}{index}"),
                        _reservation: reservation,
                    });
                }
                Err(NamedMutexError::Conflict) => continue,
                Err(NamedMutexError::Unavailable(reason)) => return Err(io::Error::other(reason)),
            }
        }
        Err(io::Error::other("todos os perfis do WebView estão em uso"))
    }
}

/// Removes WebView2 profiles that the current layout never uses: one profile
/// per Project Identity and prototype profiles from development builds. Only
/// plain directories directly under `State/WebView2` are touched; a profile
/// still held by a running WebView fails to delete and is left in place.
pub(crate) fn prune_retired_webview_profiles(app_paths: &AppPaths) -> usize {
    let Ok(root) = app_paths.webview_data_directory("global").map(|path| {
        path.parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or(path)
    }) else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let plain_directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !plain_directory || is_current_profile(name) {
            continue;
        }
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn is_current_profile(name: &str) -> bool {
    matches!(
        name,
        crate::global_runtime::GLOBAL_WEBVIEW_NAMESPACE
            | crate::native_dialog_window::PROGRESS_WEBVIEW_NAMESPACE
    ) || name
        .strip_prefix(PROJECT_SLOT_PREFIX)
        .and_then(|index| index.parse::<u32>().ok())
        .is_some_and(|index| (1..=MAX_PROJECT_WEBVIEW_SLOTS).contains(&index))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectWebviewStartupTerminal {
    SaveAsStateIndeterminate,
}

#[derive(Clone)]
pub(crate) struct ProjectWebviewAuthority {
    app_paths: AppPaths,
    current: Arc<Mutex<WebviewSlot>>,
    transitioning: Arc<AtomicBool>,
}

pub(crate) struct StagedProjectWebview {
    owner: ProjectWebviewAuthority,
    next: WebviewSlot,
    previous_data_directory: PathBuf,
    next_data_directory: PathBuf,
    next_browser_arguments: Option<String>,
    automation: bool,
}

pub(crate) struct CommittedProjectWebview {
    staged: StagedProjectWebview,
}

pub(crate) struct ProjectWebviewRecoveryReservation(ProjectWebviewAuthority);

impl Drop for ProjectWebviewRecoveryReservation {
    fn drop(&mut self) {
        self.0.transitioning.store(false, Ordering::Release);
    }
}

impl ProjectWebviewAuthority {
    pub(crate) fn new(app_paths: AppPaths) -> io::Result<Self> {
        let slot = WebviewSlot::acquire(&app_paths)?;
        Ok(Self {
            app_paths,
            current: Arc::new(Mutex::new(slot)),
            transitioning: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn is_transitioning(&self) -> bool {
        self.transitioning.load(Ordering::Acquire)
    }

    pub(crate) fn try_reserve_recovery(&self) -> Option<ProjectWebviewRecoveryReservation> {
        self.transitioning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ProjectWebviewRecoveryReservation(self.clone()))
    }

    pub(crate) fn current_namespace(&self) -> String {
        self.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .namespace
            .clone()
    }

    pub(crate) fn current_data_directory(&self) -> io::Result<PathBuf> {
        self.app_paths
            .webview_data_directory(&self.current_namespace())
            .map_err(io::Error::other)
    }

    /// Prepares a fresh WebView for a new Identity in another free slot. The
    /// replacement still reloads the Project UI under the new authority.
    pub(crate) fn stage(&self, app: &tauri::AppHandle) -> io::Result<StagedProjectWebview> {
        if self
            .transitioning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::other(
                "uma transição de autoridade do WebView já está em andamento",
            ));
        }
        let staged = self.stage_inner(app);
        if staged.is_err() {
            self.transitioning.store(false, Ordering::Release);
        }
        staged
    }

    fn stage_inner(&self, app: &tauri::AppHandle) -> io::Result<StagedProjectWebview> {
        let previous_data_directory = self.current_data_directory()?;
        let next = WebviewSlot::acquire(&self.app_paths)?;
        let next_data_directory = self
            .app_paths
            .webview_data_directory(&next.namespace)
            .map_err(io::Error::other)?;
        #[cfg(debug_assertions)]
        let next_browser_arguments = desktop_webview_policy::replacement_webview_debug_arguments(
            std::env::var_os(desktop_webview_policy::SAVE_AS_WEBVIEW_DEBUG_PORT_ENV),
        )?;
        #[cfg(not(debug_assertions))]
        let next_browser_arguments = None;
        let automation = desktop_webview_policy::automation_enabled();
        if !automation {
            preflight(
                app,
                next_data_directory.clone(),
                next_browser_arguments.as_deref(),
            )?;
        }
        Ok(StagedProjectWebview {
            owner: self.clone(),
            next,
            previous_data_directory,
            next_data_directory,
            next_browser_arguments,
            automation,
        })
    }
}

impl StagedProjectWebview {
    pub(crate) fn commit(self, app: &tauri::AppHandle) -> io::Result<CommittedProjectWebview> {
        if !self.automation
            && let Err(error) = replace_project_webview(
                app,
                self.next_data_directory.clone(),
                self.next_browser_arguments.as_deref(),
                None,
            )
        {
            if let Err(restore_error) = replace_project_webview(
                app,
                self.previous_data_directory.clone(),
                None,
                Some(ProjectWebviewStartupTerminal::SaveAsStateIndeterminate),
            ) {
                tracing::error!(
                    target: "myalbuns.desktop",
                    error = %restore_error,
                    event = "project_save_as_previous_webview_restore_failed",
                );
                app.exit(1);
            }
            self.owner.transitioning.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(CommittedProjectWebview { staged: self })
    }
}

impl CommittedProjectWebview {
    pub(crate) fn rollback(self, app: &tauri::AppHandle) -> io::Result<()> {
        let result = if self.staged.automation {
            Ok(())
        } else {
            replace_project_webview(
                app,
                self.staged.previous_data_directory.clone(),
                None,
                Some(ProjectWebviewStartupTerminal::SaveAsStateIndeterminate),
            )
        };
        self.staged
            .owner
            .transitioning
            .store(false, Ordering::Release);
        result
    }

    /// Adopts the new slot; the previous one is released for another Host.
    pub(crate) fn finalize(self) {
        let StagedProjectWebview { owner, next, .. } = self.staged;
        *owner
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
        owner.transitioning.store(false, Ordering::Release);
    }
}

fn preflight(
    app: &tauri::AppHandle,
    data_directory: PathBuf,
    browser_arguments: Option<&str>,
) -> io::Result<()> {
    let window = app
        .get_window(PROJECT_WINDOW_LABEL)
        .ok_or_else(|| io::Error::other("a janela nativa do Projeto não está disponível"))?;
    let label = format!("project-save-as-preflight-{}", Uuid::new_v4().simple());
    let mut builder = WebviewBuilder::new(label, project_webview_url(None))
        .data_directory(data_directory)
        .focused(false);
    if let Some(arguments) = browser_arguments {
        builder = builder.additional_browser_args(arguments);
    }
    let webview = add_ready_webview(
        &window,
        builder,
        PhysicalSize::new(1, 1),
        "o WebView do novo namespace não ficou pronto",
        browser_arguments,
    )?;
    webview.close().map_err(io::Error::other)
}

fn replace_project_webview(
    app: &tauri::AppHandle,
    data_directory: PathBuf,
    browser_arguments: Option<&str>,
    startup_terminal: Option<ProjectWebviewStartupTerminal>,
) -> io::Result<()> {
    let window = app
        .get_window(PROJECT_WINDOW_LABEL)
        .ok_or_else(|| io::Error::other("a janela nativa do Projeto não está disponível"))?;
    let size = window.inner_size().map_err(io::Error::other)?;
    if let Some(current) = app.get_webview(PROJECT_WINDOW_LABEL) {
        current.close().map_err(io::Error::other)?;
    }
    let mut builder =
        WebviewBuilder::new(PROJECT_WINDOW_LABEL, project_webview_url(startup_terminal))
            .data_directory(data_directory)
            .focused(false)
            .auto_resize();
    if let Some(arguments) = browser_arguments {
        builder = builder.additional_browser_args(arguments);
    }
    let webview = add_ready_webview(
        &window,
        builder,
        size,
        "o WebView da nova autoridade não ficou pronto",
        browser_arguments,
    )?;
    webview.set_focus().map_err(io::Error::other)?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_id = std::process::id(),
        window_label = PROJECT_WINDOW_LABEL,
        event = "project_webview_authority_ready",
    );
    Ok(())
}

fn project_webview_url(startup_terminal: Option<ProjectWebviewStartupTerminal>) -> WebviewUrl {
    let path = match startup_terminal {
        Some(ProjectWebviewStartupTerminal::SaveAsStateIndeterminate) => {
            "index.html#save-as-state-indeterminate"
        }
        None => "index.html",
    };
    WebviewUrl::App(path.into())
}

fn add_ready_webview(
    window: &tauri::Window<tauri::Wry>,
    builder: WebviewBuilder<tauri::Wry>,
    size: PhysicalSize<u32>,
    timeout_message: &'static str,
    browser_arguments: Option<&str>,
) -> io::Result<tauri::Webview<tauri::Wry>> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let arguments = desktop_webview_policy::browser_arguments(browser_arguments);
    let webview = window
        .add_child(
            builder.on_page_load(move |webview, payload| {
                if payload.event() == tauri::webview::PageLoadEvent::Finished {
                    let _ = sender.send(desktop_webview_policy::enforce_webview_with_arguments(
                        &webview,
                        arguments.clone(),
                    ));
                }
            }),
            PhysicalPosition::new(0, 0),
            size,
        )
        .map_err(io::Error::other)?;
    let ready = receiver
        .recv_timeout(PREFLIGHT_TIMEOUT)
        .map_err(|_| io::Error::other(timeout_message))
        .and_then(|result| result);
    if let Err(error) = ready {
        if let Err(close_error) = webview.close() {
            tracing::error!(
                target: "myalbuns.desktop",
                error = %close_error,
                event = "project_webview_authority_failed_webview_close_failed",
            );
        }
        return Err(error);
    }
    Ok(webview)
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectWebviewAuthority, ProjectWebviewStartupTerminal, project_webview_url,
        prune_retired_webview_profiles,
    };
    use myalbuns_paths::AppPaths;
    use tauri::WebviewUrl;

    fn paths(root: &std::path::Path) -> AppPaths {
        AppPaths::from_roots(&root.join("roaming"), &root.join("local"))
    }

    #[test]
    fn hosts_take_distinct_slots_and_a_released_slot_is_reused() {
        let root = tempfile::tempdir().expect("temporary WebView authority fixture");
        let first = ProjectWebviewAuthority::new(paths(root.path())).unwrap();
        let second = ProjectWebviewAuthority::new(paths(root.path())).unwrap();
        assert_eq!(first.current_namespace(), "project-1");
        assert_eq!(second.current_namespace(), "project-2");
        drop(first);
        let third = ProjectWebviewAuthority::new(paths(root.path())).unwrap();
        assert_eq!(third.current_namespace(), "project-1");
        drop(second);
    }

    #[test]
    fn retired_profiles_are_pruned_and_current_ones_are_kept() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let profiles = paths
            .webview_data_directory("global")
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        for name in [
            "global",
            "global-progress",
            "project-1",
            "project-64",
            "project-65",
            "project-0c200b7131342e81a0c080b00dcb6a9335f8ac2d8c2997fb85aec1d742f20e9a",
            "topology-multiwindow",
        ] {
            std::fs::create_dir_all(profiles.join(name).join("EBWebView")).unwrap();
        }
        std::fs::write(profiles.join("notes.txt"), b"not a profile").unwrap();

        assert_eq!(prune_retired_webview_profiles(&paths), 3);
        let mut remaining = std::fs::read_dir(&profiles)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        remaining.sort();
        assert_eq!(
            remaining,
            [
                "global",
                "global-progress",
                "notes.txt",
                "project-1",
                "project-64"
            ]
        );
    }

    #[test]
    fn recovery_reserves_the_same_transition_until_its_owner_is_dropped() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let authority = ProjectWebviewAuthority::new(paths).unwrap();
        let reservation = authority.try_reserve_recovery().unwrap();
        assert!(authority.is_transitioning());
        assert!(authority.clone().try_reserve_recovery().is_none());
        drop(reservation);
        assert!(!authority.is_transitioning());
        assert!(authority.try_reserve_recovery().is_some());
    }

    #[test]
    fn restored_previous_webview_bootstraps_with_the_indeterminate_terminal() {
        let WebviewUrl::App(path) = project_webview_url(Some(
            ProjectWebviewStartupTerminal::SaveAsStateIndeterminate,
        )) else {
            panic!("the restored Project remains on the bundled application URL");
        };
        assert_eq!(
            path,
            std::path::PathBuf::from("index.html#save-as-state-indeterminate")
        );
    }
}
