use super::*;
use crate::{
    ipc_contract::{ExportConflictPolicy, NormalExportOptions},
    product_runtime::PROJECT_WINDOW_LABEL,
};
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};

#[derive(serde::Serialize)]
pub(crate) struct NormalExportError {
    #[serde(flatten)]
    error: Box<ExportCommandError>,
    conflicts: Vec<String>,
}
impl From<ExportCommandError> for NormalExportError {
    fn from(error: ExportCommandError) -> Self {
        Self {
            error: Box::new(error),
            conflicts: vec![],
        }
    }
}

#[tauri::command]
pub(crate) fn default_export_destination(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<String, String> {
    require_owner(&window)?;
    state.export_destination()
}

fn require_owner(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A Exportação pertence à Janela do Projeto.".into());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn choose_export_folder(
    app: AppHandle,
    window: WebviewWindow,
) -> Result<Option<String>, String> {
    let _operation = crate::project_ui_operations::begin(&app)?;
    require_owner(&window)?;
    let parent = app.get_webview_window("project-dialog").unwrap_or(window);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&parent)
        .set_title("Escolher pasta de destino da Exportação")
        .pick_folder(move |selection| {
            let _ = sender.send(selection);
        });
    Ok(match receiver.await.map_err(|error| error.to_string())? {
        Some(FilePath::Path(path)) => Some(path.to_string_lossy().into_owned()),
        _ => None,
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn export_project(
    app: AppHandle,
    window: WebviewWindow,
    options: NormalExportOptions,
    on_event: Channel<ExportEvent>,
    state: State<'_, ProjectHost>,
    logging: State<'_, LoggingState>,
    operation_gate: State<'_, OperationGate>,
    cache: State<'_, CacheEngine>,
    processor: State<'_, ImagingProcessor>,
    attempts: State<'_, ExportAttempts>,
) -> Result<Option<ExportResult>, NormalExportError> {
    let _operation =
        crate::project_ui_operations::begin(&app).map_err(ExportCommandError::failed)?;
    require_owner(&window).map_err(ExportCommandError::failed)?;
    options
        .format
        .validate()
        .map_err(ExportCommandError::failed)?;
    let layout = state
        .validate_export(&options.sheet_ids)
        .map_err(ExportCommandError::failed)?;
    if !layout.is_empty() {
        let mut error =
            ExportCommandError::failed("Preencha os Frames vazios antes de exportar a seleção.");
        error.code = ExportCommandErrorCode::UnfilledLayoutPositions;
        error.layout_problems = Some(layout);
        return Err(error.into());
    }
    let (snapshot, sources) = state
        .freeze_export(&options.sheet_ids)
        .map_err(ExportCommandError::failed)?;
    let protected_originals = state
        .authorized_media_catalog()
        .map_err(ExportCommandError::failed)?
        .bindings
        .into_iter()
        .map(|binding| binding.logical_path)
        .collect();
    let checking_host = state.inner().clone();
    let sheet_ids = options.sheet_ids.clone();
    let media = tauri::async_runtime::spawn_blocking(move || {
        crate::export_media::inspect_selection(&checking_host, &sheet_ids)
    })
    .await
    .map_err(|error| ExportCommandError::failed(error.to_string()))?
    .map_err(ExportCommandError::failed)?;
    if !media.is_empty() {
        let mut error = ExportCommandError::failed("Confira os Arquivos necessários à Exportação.");
        error.code = ExportCommandErrorCode::MediaProblems;
        error.media_problems = Some(media);
        return Err(error.into());
    }
    let current = state.projection().map_err(ExportCommandError::failed)?;
    if current.state.project_id != snapshot.project_id
        || current.state.revision != snapshot.revision
    {
        return Err(ExportCommandError::failed(
            "O Projeto mudou durante a verificação. Tente exportar novamente.",
        )
        .into());
    }
    let acquisition =
        OperationLease::begin(&operation_gate).map_err(ExportCommandError::from_gate)?;
    let request_id = format!("export-{}", uuid::Uuid::new_v4());
    let project_id = safe_log_identifier(&snapshot.project_id).map(str::to_owned);
    let destination = PathBuf::from(&options.destination);
    if !destination.is_absolute() {
        return Err(ExportCommandError::failed("Escolha uma pasta de destino absoluta.").into());
    }
    let conflict_policy = options.conflict_policy;
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let mut plan = export_pipeline::plan_album(
            snapshot,
            export_pipeline::AlbumExportOptions {
                protected_originals,
                sheet_ids: options.sheet_ids,
                whole_album: options.scope == crate::ipc_contract::ExportScope::Album,
                mode: options.mode,
                format: options.format,
                destination: destination.clone(),
                authorization: if conflict_policy == ExportConflictPolicy::Replace {
                    ExportWriteAuthorization::ReplaceConfirmed
                } else {
                    ExportWriteAuthorization::CreateOnly
                },
                sources,
                request_id,
            },
        )?;
        let conflicts = plan.conflicts()?;
        if conflicts.is_empty() || conflict_policy != ExportConflictPolicy::Ask {
            std::fs::create_dir_all(&destination).map_err(|error| {
                export_pipeline::ExportFailure::new(
                    export_pipeline::ExportFailureStage::Prepare,
                    format!("Não foi possível criar a pasta de destino: {error}"),
                )
            })?;
        }
        let has_outputs =
            conflict_policy != ExportConflictPolicy::Skip || plan.skip_existing_outputs()?;
        Ok::<_, export_pipeline::ExportFailure>((plan, conflicts, has_outputs))
    })
    .await
    .map_err(|error| ExportCommandError::failed(error.to_string()))?
    .map_err(ExportCommandError::from_pipeline)?;
    let (plan, conflicts, has_outputs) = plan;
    if !conflicts.is_empty() && conflict_policy == ExportConflictPolicy::Ask {
        let mut error = ExportCommandError::failed("Já existem arquivos no Destino da Exportação.");
        error.code = ExportCommandErrorCode::ExportConflict;
        return Err(NormalExportError {
            error: Box::new(error),
            conflicts,
        });
    }
    if !has_outputs {
        return Ok(None);
    }
    let request_id = plan.request_id().to_owned();
    let attempt = attempts
        .begin(request_id.clone(), window.label())
        .map_err(|error| ExportCommandError::failed(error.to_string()))?;
    let prepared = PreparedExportCommand {
        acquisition,
        attempt,
        operation_paths: plan.required_paths(),
        plan: ExportCommandPlan::Album(plan),
        project_id,
        request_id,
    };
    run_export(app, window, on_event, logging, cache, processor, prepared)
        .await
        .map(Some)
        .map_err(Into::into)
}
