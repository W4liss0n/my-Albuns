use super::*;
use crate::{
    cache_engine::CacheEngine,
    imaging_processor::{ImagingProcessor, TauriImagingTransport},
    logging::LoggingState,
    operation_gate::OperationGate,
    operation_lease::OperationLease,
};

struct ActiveAttempt(AppHandle);
impl Drop for ActiveAttempt {
    fn drop(&mut self) {
        let state = self.0.state::<BatchWindowState>();
        state
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        state
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        state
            .result_ready
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

struct ProgressSurface {
    window: WebviewWindow,
    owner: WebviewWindow,
}
impl Drop for ProgressSurface {
    fn drop(&mut self) {
        let _ = self.window.destroy();
        native_dialog_window::restore_owner(&self.owner);
    }
}

#[tauri::command]
pub(crate) async fn batch_run(
    app: AppHandle,
    window: WebviewWindow,
    policy: ExportConflictPolicy,
) -> Result<BatchExportView, String> {
    require_configuration(&window)?;
    let state = app.state::<BatchWindowState>();
    let cancel = Arc::new(BatchCancellation::default());
    {
        let mut active = state.active.lock().map_err(|_| "Lote indisponível.")?;
        if active.is_some() {
            return Err("O lote já está em execução.".into());
        }
        *active = Some(cancel.clone());
    }
    // The owner task outlives the WebView invocation. Losing an interface never
    // drops a live publication or its exclusivity reservation.
    let attempt = ActiveAttempt(app.clone());
    tauri::async_runtime::spawn(async move {
        let _attempt = attempt;
        execute(&app, window, cancel, policy).await
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn execute(
    app: &AppHandle,
    owner: WebviewWindow,
    cancel: Arc<BatchCancellation>,
    policy: ExportConflictPolicy,
) -> Result<BatchExportView, String> {
    let state = app.state::<BatchWindowState>();
    let mut runner = state.runner.lock().await;
    let initial = runner
        .as_ref()
        .ok_or("Verifique os Projetos primeiro.")?
        .view();
    if !initial.can_continue {
        return Err("Resolva ou ignore os problemas antes de exportar.".into());
    }
    if initial.has_conflicts && policy == ExportConflictPolicy::Ask {
        return Err("Escolha como tratar a exportação existente.".into());
    }
    let mut visible = initial.clone();
    visible.phase = BatchPhase::Running;
    visible.can_continue = false;
    *state.progress.lock().map_err(|_| "Lote indisponível.")? =
        runner.as_ref().map(BatchRunner::progress);
    state.publish(Some(visible));
    let result = run_attempt(app, &owner, &mut runner, &cancel, policy).await;
    let view = match &result {
        Ok(view) => view.clone(),
        Err(_) => runner.as_ref().map(BatchRunner::view).unwrap_or_else(|| {
            let mut interrupted = initial;
            interrupted.phase = BatchPhase::Interrupted;
            interrupted.can_continue = false;
            interrupted
        }),
    };
    state.publish(Some(view.clone()));
    let _ = app.emit("myalbuns://batch-view", &view);
    result
}

async fn run_attempt(
    app: &AppHandle,
    owner: &WebviewWindow,
    runner: &mut Option<BatchRunner>,
    cancel: &BatchCancellation,
    policy: ExportConflictPolicy,
) -> Result<BatchExportView, String> {
    let state = app.state::<BatchWindowState>();
    let gate = app.state::<OperationGate>();
    let acquisition = OperationLease::begin(&gate).map_err(|error| error.to_string())?;
    let profile = state
        .paths
        .webview_data_directory("global-progress")
        .map_err(|error| error.to_string())?;
    let arguments = format!(
        "{} --disable-gpu",
        desktop_webview_policy::WRY_DEFAULT_DISABLED_FEATURES
    );
    let progress_window = native_dialog_window::build_hidden_owned_window(
        app,
        owner,
        HiddenOwnedWindowConfig {
            label: PROGRESS_LABEL,
            url: "global.html?surface=batchProgress",
            width: 400.0,
            height: 184.0,
            browser_arguments: Some(&arguments),
            browser_data_directory: Some(&profile),
        },
    )
    .await
    .map_err(|error| error.to_string())?;
    native_dialog_window::display_transition_dialog(owner, &progress_window)
        .map_err(|error| error.to_string())?;
    let _surface = ProgressSurface {
        window: progress_window,
        owner: owner.clone(),
    };
    // Prevent launches from racing participant discovery. Existing launches can
    // finish before the mode blocks their new Project windows.
    let launches = crate::global_runtime::reserve_batch_launches(app);
    tokio::pin!(launches);
    let _launches = loop {
        tokio::select! {
            permit = &mut launches => break permit,
            _ = tokio::time::sleep(std::time::Duration::from_millis(40)) => {
                if cancel.is_requested() { return Err("Exportação cancelada.".into()); }
            }
        }
    };
    let mode = crate::batch_exclusivity::acquire(&state.paths, cancel).await?;
    crate::application_modality::synchronize(app).await?;
    let cache = app.state::<CacheEngine>();
    let processor = app.state::<ImagingProcessor>();
    let lease = acquisition
        .complete(&cache, &processor)
        .await
        .map_err(|error| error.to_string())?;
    let logging = app.state::<LoggingState>();
    let mut transport = TauriImagingTransport::new(app, &logging, lease.processor_reservation());
    let batch = runner.take().ok_or("Lote indisponível.")?;
    app.state::<crate::storage_recovery::StorageRecoveries>()
        .finish(&batch.view().id);
    let outcome = batch
        .run(&mut transport, cancel, policy, &|progress| {
            *state
                .progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(progress.clone());
            let _ = app.emit(PROGRESS_EVENT, progress);
        })
        .await;
    drop(lease);
    drop(mode);
    let batch = outcome?;
    let view = batch.view();
    let recoveries = app.state::<crate::storage_recovery::StorageRecoveries>();
    if view.phase == crate::ipc_contract::BatchPhase::StorageFull {
        recoveries.pause(&view.id, batch.storage_volume());
    } else {
        recoveries.finish(&view.id);
    }
    *runner = Some(batch);
    let (sender, ready) = tokio::sync::oneshot::channel();
    *state
        .result_ready
        .lock()
        .map_err(|_| "Lote indisponível.")? = Some(sender);
    state.publish(Some(view.clone()));
    let _ = app.emit("myalbuns://batch-view", &view);
    // The hidden configuration paints its terminal state before returning. This
    // is a content handshake, not an animation or a visible intermediate line.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), ready).await;
    Ok(view)
}
