use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use myalbuns_core::ProjectIdentityAuthority;
use myalbuns_paths::{AppPaths, project_data_namespace};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, webview::WebviewBuilder};
use uuid::Uuid;

use crate::{desktop_webview_policy, product_runtime::PROJECT_WINDOW_LABEL};

const PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug)]
pub(crate) struct ProjectWebviewAuthority {
    app_paths: AppPaths,
    current_namespace: Arc<Mutex<String>>,
    transitioning: Arc<AtomicBool>,
}

pub(crate) struct StagedProjectWebview {
    owner: ProjectWebviewAuthority,
    next_namespace: String,
    previous_data_directory: PathBuf,
    next_data_directory: PathBuf,
    next_browser_arguments: Option<String>,
    automation: bool,
}

pub(crate) struct CommittedProjectWebview {
    staged: StagedProjectWebview,
}

impl ProjectWebviewAuthority {
    pub(crate) fn new(app_paths: AppPaths, project_id: &str) -> Self {
        Self {
            app_paths,
            current_namespace: Arc::new(Mutex::new(project_data_namespace(project_id))),
            transitioning: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn is_transitioning(&self) -> bool {
        self.transitioning.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn current_namespace(&self) -> String {
        self.current_namespace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn stage(
        &self,
        app: &tauri::AppHandle,
        previous_project_id: Uuid,
        authority: &ProjectIdentityAuthority,
    ) -> io::Result<StagedProjectWebview> {
        if self
            .transitioning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::other(
                "uma transição de autoridade do WebView já está em andamento",
            ));
        }
        let staged = self.stage_inner(app, previous_project_id, authority);
        if staged.is_err() {
            self.transitioning.store(false, Ordering::Release);
        }
        staged
    }

    fn stage_inner(
        &self,
        app: &tauri::AppHandle,
        previous_project_id: Uuid,
        authority: &ProjectIdentityAuthority,
    ) -> io::Result<StagedProjectWebview> {
        let previous_namespace =
            project_data_namespace(&previous_project_id.hyphenated().to_string());
        let current = self
            .current_namespace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if current != previous_namespace {
            return Err(io::Error::other(
                "a autoridade corrente do WebView não corresponde à Sessão anterior",
            ));
        }
        let next_namespace =
            project_data_namespace(&authority.project_id().hyphenated().to_string());
        if next_namespace == previous_namespace {
            return Err(io::Error::other(
                "a nova Identidade não produziu um namespace WebView independente",
            ));
        }
        let previous_data_directory = self
            .app_paths
            .webview_data_directory(&previous_namespace)
            .map_err(io::Error::other)?;
        let next_data_directory = self
            .app_paths
            .webview_data_directory(&next_namespace)
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
            next_namespace,
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
            )
        {
            let _ = replace_project_webview(app, self.previous_data_directory.clone(), None);
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
            replace_project_webview(app, self.staged.previous_data_directory.clone(), None)
        };
        self.staged
            .owner
            .transitioning
            .store(false, Ordering::Release);
        result
    }

    pub(crate) fn finalize(self) {
        *self
            .staged
            .owner
            .current_namespace
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            self.staged.next_namespace.clone();
        self.staged
            .owner
            .transitioning
            .store(false, Ordering::Release);
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
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut builder = WebviewBuilder::new(label, WebviewUrl::App("index.html".into()))
        .data_directory(data_directory)
        .focused(false);
    if let Some(arguments) = browser_arguments {
        builder = builder.additional_browser_args(arguments);
    }
    let webview = window
        .add_child(
            builder.on_page_load(move |webview, payload| {
                if payload.event() == tauri::webview::PageLoadEvent::Finished {
                    let _ = sender.send(desktop_webview_policy::enforce_webview(&webview));
                }
            }),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1, 1),
        )
        .map_err(io::Error::other)?;
    let ready = receiver
        .recv_timeout(PREFLIGHT_TIMEOUT)
        .map_err(|_| io::Error::other("o WebView do novo namespace não ficou pronto"));
    let close = webview.close().map_err(io::Error::other);
    ready??;
    close
}

fn replace_project_webview(
    app: &tauri::AppHandle,
    data_directory: PathBuf,
    browser_arguments: Option<&str>,
) -> io::Result<()> {
    let window = app
        .get_window(PROJECT_WINDOW_LABEL)
        .ok_or_else(|| io::Error::other("a janela nativa do Projeto não está disponível"))?;
    let size = window.inner_size().map_err(io::Error::other)?;
    if let Some(current) = app.get_webview(PROJECT_WINDOW_LABEL) {
        current.close().map_err(io::Error::other)?;
    }
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut builder =
        WebviewBuilder::new(PROJECT_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
            .data_directory(data_directory)
            .auto_resize();
    if let Some(arguments) = browser_arguments {
        builder = builder.additional_browser_args(arguments);
    }
    let webview = window
        .add_child(
            builder.on_page_load(move |webview, payload| {
                if payload.event() == tauri::webview::PageLoadEvent::Finished {
                    let _ = sender.send(desktop_webview_policy::enforce_webview(&webview));
                }
            }),
            PhysicalPosition::new(0, 0),
            size,
        )
        .map_err(io::Error::other)?;
    let ready = receiver
        .recv_timeout(PREFLIGHT_TIMEOUT)
        .map_err(|_| io::Error::other("o WebView da nova autoridade não ficou pronto"))
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
    webview.set_focus().map_err(io::Error::other)?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_id = std::process::id(),
        window_label = PROJECT_WINDOW_LABEL,
        event = "project_webview_authority_ready",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ProjectWebviewAuthority;
    use myalbuns_paths::{AppPaths, project_data_namespace};

    #[test]
    fn tracked_namespace_is_an_opaque_project_key() {
        let root = tempfile::tempdir().expect("temporary WebView authority fixture");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let project_id = "4b594571-6b51-4cad-a37c-8fd8cedb7dd2";
        let authority = ProjectWebviewAuthority::new(paths, project_id);

        assert_eq!(
            authority.current_namespace(),
            project_data_namespace(project_id)
        );
        assert!(!authority.current_namespace().contains(project_id));
    }
}
