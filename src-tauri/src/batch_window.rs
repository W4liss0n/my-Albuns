use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use myalbuns_core::ProjectCore;
use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    batch_runner::{BatchCancellation, BatchConfiguration, BatchRunner},
    desktop_webview_policy,
    global_runtime::ProjectLaunchOutcome,
    ipc_contract::{
        BatchExportOptions, BatchExportProgress, BatchExportView, BatchPhase, BatchRecoverySummary,
        ExportConflictPolicy,
    },
    native_dialog_window::{self, HiddenOwnedWindowConfig},
};

pub(crate) const BATCH_WINDOW_LABEL: &str = "batch-export";
const PROGRESS_LABEL: &str = "batch-progress";
const PROGRESS_EVENT: &str = "myalbuns://batch-progress";

pub(crate) struct BatchWindowState {
    paths: AppPaths,
    window_serial: tokio::sync::Mutex<()>,
    runner: Arc<tokio::sync::Mutex<Option<BatchRunner>>>,
    view: Mutex<Option<BatchExportView>>,
    active: Mutex<Option<Arc<BatchCancellation>>>,
    progress: Mutex<Option<BatchExportProgress>>,
    result_ready: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl BatchWindowState {
    pub(crate) fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            window_serial: tokio::sync::Mutex::new(()),
            runner: Arc::new(tokio::sync::Mutex::new(None)),
            view: Mutex::new(None),
            active: Mutex::new(None),
            progress: Mutex::new(None),
            result_ready: Mutex::new(None),
        }
    }

    fn core(&self) -> ProjectCore {
        ProjectCore::new().with_identity_storage_roots(
            self.paths.project_identity_leases_dir(),
            self.paths.project_identities_dir(),
        )
    }

    fn recovery_root(&self) -> PathBuf {
        self.paths.recovery_dir().join("Batches")
    }

    fn require_idle(&self) -> Result<(), String> {
        if self
            .active
            .lock()
            .map_err(|_| "Lote indisponível.")?
            .is_some()
        {
            return Err("Aguarde o término da exportação em lote.".into());
        }
        Ok(())
    }

    fn publish(&self, view: Option<BatchExportView>) {
        *self
            .view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = view;
    }
}

fn require_configuration(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != BATCH_WINDOW_LABEL {
        return Err("Janela de lote inválida.".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn open_batch_export(app: AppHandle) -> Result<(), String> {
    let state = app.state::<BatchWindowState>();
    let _serial = state.window_serial.lock().await;
    if let Some(window) = app.get_webview_window(BATCH_WINDOW_LABEL) {
        return window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|error| error.to_string());
    }
    let owner = app
        .get_webview_window("global")
        .ok_or("Boas-vindas indisponível.")?;
    let profile = state
        .paths
        .webview_data_directory("global")
        .map_err(|error| error.to_string())?;
    let window = native_dialog_window::build_hidden_owned_window(
        &app,
        &owner,
        HiddenOwnedWindowConfig {
            label: BATCH_WINDOW_LABEL,
            url: "global.html?surface=batchExport",
            width: 800.0,
            height: 560.0,
            browser_arguments: None,
            browser_data_directory: Some(&profile),
        },
    )
    .await
    .map_err(|error| error.to_string())?;
    window
        .set_title("Exportação em lote")
        .map_err(|error| error.to_string())?;
    // Preflight remains nonmodal: projects can be opened, corrected and saved.
    window
        .show()
        .and_then(|_| window.set_focus())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn batch_choose_folder(window: WebviewWindow) -> Result<Option<String>, String> {
    require_configuration(&window)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .pick_folder(move |selection| {
            let _ = sender.send(selection);
        });
    match receiver
        .await
        .map_err(|_| "Não foi possível escolher a pasta.")?
    {
        Some(FilePath::Path(path)) => Ok(Some(path.to_string_lossy().into_owned())),
        Some(_) => Err("Escolha uma pasta do Windows.".into()),
        None => Ok(None),
    }
}

#[tauri::command]
pub(crate) async fn batch_count_projects(
    window: WebviewWindow,
    source: String,
) -> Result<usize, String> {
    require_configuration(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        BatchRunner::count_projects(&PathBuf::from(source))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn batch_current(app: AppHandle) -> Option<BatchExportView> {
    app.state::<BatchWindowState>()
        .view
        .lock()
        .ok()
        .and_then(|view| view.clone())
}

#[tauri::command]
pub(crate) fn batch_progress(app: AppHandle) -> Option<BatchExportProgress> {
    app.state::<BatchWindowState>()
        .progress
        .lock()
        .ok()
        .and_then(|progress| progress.clone())
}

#[tauri::command]
pub(crate) async fn batch_prepare(
    app: AppHandle,
    window: WebviewWindow,
    options: BatchExportOptions,
) -> Result<BatchExportView, String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    state.require_idle()?;
    let mut runner = state.runner.clone().lock_owned().await;
    state.require_idle()?;
    let core = state.core();
    let recovery = state.recovery_root();
    let view = tauri::async_runtime::spawn_blocking(move || {
        if runner
            .as_ref()
            .is_some_and(|batch| batch.view().phase == BatchPhase::Interrupted)
        {
            return Err("Retome ou encerre o lote interrompido antes de iniciar outro.".into());
        }
        let batch = BatchRunner::discover(
            BatchConfiguration {
                source: options.source_folder.into(),
                destination: options.destination_folder.map(Into::into),
                format: options.format,
                mode: options.mode,
            },
            core,
            recovery,
        )?;
        let view = batch.view();
        *runner = Some(batch);
        Ok::<_, String>(view)
    })
    .await
    .map_err(|error| error.to_string())??;
    state.publish(Some(view.clone()));
    Ok(view)
}

async fn update(
    app: &AppHandle,
    change: impl FnOnce(&mut BatchRunner) -> Result<(), String> + Send + 'static,
) -> Result<BatchExportView, String> {
    let state = app.state::<BatchWindowState>();
    state.require_idle()?;
    let mut runner = state.runner.clone().lock_owned().await;
    state.require_idle()?;
    let view = tauri::async_runtime::spawn_blocking(move || {
        let batch = runner.as_mut().ok_or("Verifique os Projetos primeiro.")?;
        change(batch)?;
        Ok::<_, String>(batch.view())
    })
    .await
    .map_err(|error| error.to_string())??;
    state.publish(Some(view.clone()));
    Ok(view)
}

#[tauri::command]
pub(crate) async fn batch_recheck(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<BatchExportView, String> {
    require_configuration(&window)?;
    update(&app, |batch| {
        batch.retry_preflight();
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn batch_ignore(
    app: AppHandle,
    window: WebviewWindow,
    item_id: String,
) -> Result<BatchExportView, String> {
    require_configuration(&window)?;
    update(&app, move |batch| batch.ignore(&item_id)).await
}

#[tauri::command]
pub(crate) async fn batch_relink(
    app: AppHandle,
    window: WebviewWindow,
    item_id: Option<String>,
) -> Result<Option<BatchExportView>, String> {
    let Some(folder) = batch_choose_folder(window).await? else {
        return Ok(None);
    };
    update(&app, move |batch| match item_id {
        Some(id) => batch.relink(&id, &PathBuf::from(folder)),
        None => batch.relink_all(&PathBuf::from(folder)),
    })
    .await
    .map(Some)
}

#[tauri::command]
pub(crate) async fn batch_open_project(
    app: AppHandle,
    window: WebviewWindow,
    item_id: String,
) -> Result<ProjectLaunchOutcome, String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    state.require_idle()?;
    let path = state
        .runner
        .lock()
        .await
        .as_ref()
        .ok_or("Lote indisponível.")?
        .project_path(&item_id)?;
    Ok(crate::global_runtime::open_batch_project(&app, path).await)
}

#[tauri::command]
pub(crate) async fn batch_recoveries(app: AppHandle) -> Result<Vec<BatchRecoverySummary>, String> {
    let root = app.state::<BatchWindowState>().recovery_root();
    tauri::async_runtime::spawn_blocking(move || BatchRunner::recoveries(&root))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn batch_resume(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<BatchExportView, String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    state.require_idle()?;
    let mut runner = state.runner.clone().lock_owned().await;
    state.require_idle()?;
    let root = state.recovery_root();
    let core = state.core();
    app.state::<crate::storage_recovery::StorageRecoveries>()
        .finish(&id);
    let view = tauri::async_runtime::spawn_blocking(move || {
        if let Some(batch) = runner.as_mut()
            && batch.view().id == id
            && batch.view().phase == crate::ipc_contract::BatchPhase::StorageFull
        {
            // Disk exhaustion may have prevented the last checkpoint write.
            // The live runner retains completed items and temporary relinks.
            batch.retry_preflight();
            return Ok(batch.view());
        }
        let batch = BatchRunner::resume(&root, &id, core)?;
        let view = batch.view();
        *runner = Some(batch);
        Ok::<_, String>(view)
    })
    .await
    .map_err(|error| error.to_string())??;
    state.publish(Some(view.clone()));
    Ok(view)
}

#[tauri::command]
pub(crate) async fn batch_end(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<(), String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    state.require_idle()?;
    let mut runner = state.runner.clone().lock_owned().await;
    state.require_idle()?;
    // A quarantined Processor cannot prove that its child stopped writing.
    // End still removes recovery, but defers preparation cleanup in that case.
    let processor = app.state::<crate::imaging_processor::ImagingProcessor>();
    let cleanup_preparation = processor.reserve().await.is_ok();
    let root = state.recovery_root();
    let core = state.core();
    app.state::<crate::storage_recovery::StorageRecoveries>()
        .finish(&id);
    tauri::async_runtime::spawn_blocking(move || {
        let batch = if runner.as_ref().is_some_and(|batch| batch.view().id == id) {
            runner.take().expect("matched batch exists")
        } else {
            BatchRunner::resume(&root, &id, core)?
        };
        batch.abandon(cleanup_preparation)
    })
    .await
    .map_err(|error| error.to_string())??;
    state.publish(None);
    Ok(())
}

#[tauri::command]
pub(crate) fn batch_cancel(app: AppHandle) {
    if let Ok(active) = app.state::<BatchWindowState>().active.lock()
        && let Some(cancel) = active.as_ref()
    {
        cancel.request();
    }
}

#[tauri::command]
pub(crate) fn batch_result_ready(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    require_configuration(&window)?;
    if let Some(sender) = app
        .state::<BatchWindowState>()
        .result_ready
        .lock()
        .map_err(|_| "Lote indisponível.")?
        .take()
    {
        let _ = sender.send(());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn close_batch_export(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<(), String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    let _serial = state.window_serial.lock().await;
    state.require_idle()?;
    let mut runner = state.runner.lock().await;
    state.require_idle()?;
    runner.take(); // Interrupted checkpoints survive closing.
    state.publish(None);
    window.destroy().map_err(|error| error.to_string())
}

pub(crate) mod execution;
