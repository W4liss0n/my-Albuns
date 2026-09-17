use myalbuns_core::{
    AlbumInformation, AlbumInformationValidation, EditorProjection, PathFailure, PhotoDropTarget,
    ProjectIntent, ProjectMutationOutcome, SaveAsProjectError,
    SaveAsProjectOutcome as CoreSaveAsProjectOutcome, SaveProjectError,
    SaveProjectOutcome as CoreSaveProjectOutcome,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, AppPathsError};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    cache_engine::CacheEngine,
    cache_previews::CachePreviewRegistry,
    cache_service::ActiveCacheNamespace,
    image_processing::ImageProcessingBatch,
    ipc_contract::{
        ImportMediaResult, SaveAsProjectCommandError, SaveAsProjectOutcome, SaveAsProjectResult,
        SaveProjectCommandError, SaveProjectOutcome, SaveProjectResult,
    },
    logging::validate_optional_identifier,
    media_runtime::{MediaAvailability, MediaBinding, MediaResolver},
    native_project_dialog::{SaveAsDialogOutcome, choose_save_as_destination},
    product_runtime::PROJECT_WINDOW_LABEL,
    project_creative_commands,
    project_host::{ProjectHost, ProjectHostSaveAsError, ProjectHostSaveError},
    project_identity_transition::{self, IdentityTransitionError},
};

#[tauri::command]
pub(crate) fn project_state(
    operation_id: String,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    validate_optional_identifier("operationId", Some(&operation_id))?;
    let projection = state.projection()?;
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
pub(crate) async fn apply_project_intent(
    intent: ProjectIntent,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<ProjectMutationOutcome, String> {
    let intent_kind = match &intent {
        ProjectIntent::RemoveMedia { .. } => "remove_media",
        ProjectIntent::EditMediaFolder { .. } => "edit_media_folder",
        ProjectIntent::ApplyDecorative { .. } => "apply_decorative",
        ProjectIntent::EditSheetVisual { .. } => "edit_sheet_visual",
        ProjectIntent::DropDecorative { .. } => "drop_decorative",
        ProjectIntent::CopyFrames { .. } => "copy_frames",
        ProjectIntent::ApplyLayout { .. } => "apply_layout",
        ProjectIntent::ToggleLayoutFavorite { .. } => "toggle_layout_favorite",
        ProjectIntent::LockLayout { .. } => "lock_layout",
        ProjectIntent::UnlockLayout { .. } => "unlock_layout",
        ProjectIntent::SetLayoutSettings { .. } => "set_layout_settings",
        ProjectIntent::PasteFrames { .. } => "paste_frames",
        ProjectIntent::AddFrame { .. } => "add_frame",
        ProjectIntent::DeleteFrames { .. } => "delete_frames",
        ProjectIntent::SwapFrameContents { .. } => "swap_frame_contents",
        ProjectIntent::SwapSheetSides { .. } => "swap_sheet_sides",
        ProjectIntent::ArrangeFrames { .. } => "arrange_frames",
        ProjectIntent::EditFrameGeometry { .. } => "edit_frame_geometry",
        ProjectIntent::SetAlbumInformation { .. } => "set_album_information",
        ProjectIntent::SetVisualDefaults { .. } => "set_visual_defaults",
        ProjectIntent::SetAlbumDesign { .. } => "set_album_design",
        ProjectIntent::SetDpi { .. } => "set_dpi",
        ProjectIntent::AddSheet { .. } => "add_sheet",
        ProjectIntent::DuplicateSheet { .. } => "duplicate_sheet",
        ProjectIntent::DeleteSheet { .. } => "delete_sheet",
        ProjectIntent::ConvertEdgeSheet { .. } => "convert_edge_sheet",
        ProjectIntent::ReorderSheet { .. } => "reorder_sheet",
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::OrientPhotos { .. } => "orient_photos",
        ProjectIntent::SetPhotoZoom { .. } => "set_photo_zoom",
        ProjectIntent::SetPhotoAngle { .. } => "set_photo_angle",
        ProjectIntent::SetFrameStyle { .. } => "set_frame_style",
        ProjectIntent::TogglePhotoBlackAndWhite { .. } => "toggle_photo_black_and_white",
        ProjectIntent::AddPhoto { .. } => "add_photo",
        ProjectIntent::DropPhoto { .. } => "drop_photo",
    };
    let process_id = std::process::id();
    let (mut outcome, projection) = project_creative_commands::execute_in_app(
        &app,
        &state,
        || {
            let outcome = state.apply_with_outcome(intent).inspect_err(|_| {
                tracing::warn!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    process_id = process_id,
                    window_label = window.label(),
                    intent = intent_kind,
                    event = "project_intent_rejected",
                );
            })?;
            tracing::info!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                process_id = process_id,
                window_label = window.label(),
                project_id = safe_log_identifier(&outcome.projection.state.project_id),
                revision = outcome.projection.state.revision,
                intent = intent_kind,
                event = "project_intent_applied",
            );
            Ok(outcome)
        },
        |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?;
    outcome.projection = projection;
    Ok(outcome)
}

#[tauri::command]
pub(crate) async fn import_media(
    app: AppHandle,
    selection: crate::ipc_contract::MediaImportSelection,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<ImportMediaResult, String> {
    let _operation = crate::project_ui_operations::begin(&app)?;
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A importação só está disponível na Janela do Projeto.".into());
    }
    let host = state.inner().clone();
    use crate::ipc_contract::MediaImportSource;
    let media_kind = selection.media_kind;
    let selected = match selection.source {
        MediaImportSource::Drop { drop_id } => Some(
            app.state::<crate::media_file_drop::NativeMediaDrops>()
                .take(&drop_id)?
                .into_iter()
                .map(FilePath::Path)
                .collect(),
        ),
        source => {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let dialog = app
                .dialog()
                .file()
                .set_parent(&window)
                .set_title(match media_kind {
                    myalbuns_core::MediaKind::Photo => "Importar Fotos",
                    myalbuns_core::MediaKind::Decorative => "Importar Decorativos",
                });
            match source {
                MediaImportSource::Folder => dialog.pick_folder(move |selection| {
                    let _ = sender.send(selection.map(|path| vec![path]));
                }),
                _ => dialog
                    .add_filter(
                        "Imagens JPEG, PNG e TIFF",
                        &["jpg", "jpeg", "png", "tif", "tiff"],
                    )
                    .pick_files(move |selection| {
                        let _ = sender.send(selection);
                    }),
            }
            receiver
                .await
                .map_err(|_| "Não foi possível concluir o diálogo de importação.".to_string())?
        }
    };
    let Some(selected) = selected else {
        return Ok(ImportMediaResult::Cancelled {
            projection: host.projection()?,
        });
    };
    let mut paths = Vec::new();
    let mut unsupported = Vec::new();
    for selected in selected {
        match selected {
            FilePath::Path(path) => paths.push(path),
            FilePath::Url(_) => unsupported.push(crate::ipc_contract::ImageProcessingProblem {
                file_name: "Local selecionado".into(),
                reason: "O local escolhido não é um Arquivo do Windows válido.".into(),
            }),
        }
    }
    let result = crate::photo_import::import_selected_media(
        &app,
        media_kind,
        paths,
        unsupported,
        move |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?;
    if let ImportMediaResult::Completed {
        projection,
        imported_count,
        media_ids,
        problems,
        operation_problem,
    } = &result
    {
        tracing::info!(
            target: "myalbuns.desktop",
            process_role = ProcessRole::DesktopHost.as_str(),
            window_label = window.label(),
            imported_count,
            media_id = safe_log_identifier(media_ids.last().map(String::as_str).unwrap_or("")),
            rejected_count = problems.len(),
            interrupted = operation_problem.is_some(),
            revision = projection.state.revision,
            event = "photos_imported",
        );
    }
    Ok(result)
}

#[tauri::command]
pub(crate) fn photo_drop_target(
    sheet_id: String,
    x_um: i64,
    y_um: i64,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<PhotoDropTarget, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O alvo da Foto só pode ser consultado na Janela do Projeto.".into());
    }
    state.project_photo_drop_target(&sheet_id, x_um, y_um)
}

#[tauri::command]
pub(crate) async fn preview_photo_zoom(
    edit: myalbuns_core::PhotoZoomEdit,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Vec<myalbuns_core::ComposedFrame>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O Zoom da Foto só pode ser consultado na Janela do Projeto.".into());
    }
    state.preview_photo_zoom(&edit)
}

#[tauri::command]
pub(crate) async fn preview_photo_angle(
    edit: myalbuns_core::PhotoAngleEdit,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Vec<myalbuns_core::ComposedFrame>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O Ângulo da Foto só pode ser consultada na Janela do Projeto.".into());
    }
    state.preview_photo_angle(&edit)
}

#[tauri::command]
pub(crate) fn preview_decorative_drop(
    request: myalbuns_core::DecorativeDropRequest,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Option<myalbuns_core::DecorativeDropPreview>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O Decorativo só pode ser consultado na Janela do Projeto.".into());
    }
    state.preview_decorative_drop(&request)
}

#[tauri::command]
pub(crate) async fn query_layouts(
    sheet_id: String,
    frame_request: Option<myalbuns_core::LayoutFrameRequest>,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    catalog: State<'_, crate::layout_catalog_store::LayoutCatalogStore>,
) -> Result<myalbuns_core::LayoutQueryResult, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("Os Layouts só podem ser consultados na Janela do Projeto.".into());
    }
    let snapshot = catalog
        .load()
        .map_err(|_| "Não foi possível ler os Layouts personalizados.".to_string())?;
    state.refresh_layout_catalog(snapshot)?;
    state.query_layouts(&sheet_id, frame_request)
}

#[tauri::command]
pub(crate) async fn preview_layout(
    selection: myalbuns_core::LayoutSelection,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Vec<myalbuns_core::ComposedFrame>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A prévia de Layout só está disponível na Janela do Projeto.".into());
    }
    state.preview_layout(&selection)
}

#[tauri::command]
pub(crate) async fn preview_frame_style(
    edit: myalbuns_core::FrameStyleEdit,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Vec<myalbuns_core::ComposedFrame>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O estilo do Frame só pode ser consultado na Janela do Projeto.".into());
    }
    state.preview_frame_style(&edit)
}

#[tauri::command]
pub(crate) fn slider_double_click_time(window: WebviewWindow) -> Result<u32, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O intervalo de dois cliques só está disponível na Janela do Projeto.".into());
    }
    #[cfg(windows)]
    {
        // Read the user's actual Windows double-click interval, without changing it.
        Ok(unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() })
    }
    #[cfg(not(windows))]
    Err("Os controles deslizantes requerem a plataforma Windows suportada.".into())
}

#[tauri::command]
pub(crate) async fn preview_frame_geometry(
    edit: myalbuns_core::FrameGeometryEdit,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<myalbuns_core::FrameGeometryPreview, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A geometria do Frame só pode ser consultada na Janela do Projeto.".into());
    }
    state.preview_frame_geometry(&edit)
}

#[tauri::command]
pub(crate) async fn frame_drag_threshold(
    window: WebviewWindow,
) -> Result<crate::ipc_contract::PointerDragThreshold, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O arraste de Frame só está disponível na Janela do Projeto.".into());
    }
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{
            HiDpi::GetSystemMetricsForDpi,
            WindowsAndMessaging::{SM_CXDRAG, SM_CYDRAG},
        };
        let dpi = (96.0 * scale).round() as u32;
        // These metrics are the distances on each side of the press point.
        // Convert device pixels to the CSS coordinates used by pointer events.
        let (x, y) = unsafe {
            (
                GetSystemMetricsForDpi(SM_CXDRAG, dpi),
                GetSystemMetricsForDpi(SM_CYDRAG, dpi),
            )
        };
        Ok(crate::ipc_contract::PointerDragThreshold {
            x: f64::from(x.unsigned_abs()) / scale,
            y: f64::from(y.unsigned_abs()) / scale,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = scale;
        Err("A edição de Frames requer a plataforma Windows suportada.".into())
    }
}

#[tauri::command]
pub(crate) async fn relink_media(
    media_id: String,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<EditorProjection, String> {
    change_media_reference(
        media_id,
        app,
        window,
        state,
        on_progress,
        MediaChangeKind::Relink,
    )
    .await
}

#[tauri::command]
pub(crate) async fn replace_media(
    media_id: String,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<EditorProjection, String> {
    change_media_reference(
        media_id,
        app,
        window,
        state,
        on_progress,
        MediaChangeKind::Replace,
    )
    .await
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MediaChangeKind {
    Relink,
    Replace,
}

async fn change_media_reference(
    media_id: String,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
    kind: MediaChangeKind,
) -> Result<EditorProjection, String> {
    let _operation = crate::project_ui_operations::begin(&app)?;
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A alteração de imagem só está disponível na Janela do Projeto.".into());
    }
    let host = state.inner().clone();
    let binding = host
        .authorized_media_catalog()?
        .bindings
        .into_iter()
        .find(|binding| binding.media_id == media_id)
        .ok_or_else(|| "A ocorrência de mídia não pertence a este Projeto.".to_string())?;
    if kind == MediaChangeKind::Relink {
        let inspected_binding = binding.clone();
        let absent = tauri::async_runtime::spawn_blocking(move || {
            occurrence_is_authoritatively_absent(&inspected_binding)
        })
        .await
        .map_err(|_| "Não foi possível reinspecionar o Arquivo vinculado.".to_string())?;
        if !absent {
            return Err(
            "Somente um Arquivo comprovadamente ausente pode ser religado; tente novamente se a origem estiver indisponível."
                .into(),
        );
        }
    }

    let (sender, receiver) = tokio::sync::oneshot::channel();
    let dialog = app.dialog().file().set_parent(&window);
    match kind {
        MediaChangeKind::Relink => dialog
            .set_title("Escolher pasta para Religar Imagem")
            .pick_folder(move |selection| {
                let _ = sender.send(selection);
            }),
        MediaChangeKind::Replace => dialog
            .set_title("Substituir Imagem")
            .add_filter(
                "Imagens JPEG, PNG e TIFF",
                &["jpg", "jpeg", "png", "tif", "tiff"],
            )
            .pick_file(move |selection| {
                let _ = sender.send(selection);
            }),
    }
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível concluir a seleção da imagem.".to_string())?;
    let Some(selection) = selection else {
        return host.projection();
    };
    let FilePath::Path(path) = selection else {
        return Err("O local escolhido não é um caminho do Windows válido.".into());
    };
    if kind == MediaChangeKind::Replace && path == binding.logical_path {
        return host.projection();
    }

    let selected_media_id = binding.media_id.clone();
    let mut processing = ImageProcessingBatch::new(1, |progress| {
        let _ = on_progress.send(progress);
    });
    let mut paths = myalbuns_paths::OperationPathContext::new();
    let original_path = binding.logical_path.clone();
    let candidate_path = path;
    let search_binding = binding.clone();
    let catalog = host.authorized_media_catalog()?;
    let cache_root = app
        .state::<ActiveCacheNamespace>()
        .namespace()
        .paths()
        .root()
        .to_path_buf();
    let (path, roots) = tauri::async_runtime::spawn_blocking(move || {
        let _ = paths.capture(&cache_root);
        for media in &catalog.bindings {
            let _ = paths.capture(&media.logical_path);
        }
        // Replacement does not require the old storage root to be reachable.
        if kind == MediaChangeKind::Relink {
            paths.capture(&original_path).map_err(|error| error.to_string())?;
        }
        paths
            .capture(&candidate_path)
            .map_err(|error| error.to_string())?;
        let roots = paths.freeze();
        let path = match kind {
            MediaChangeKind::Replace => candidate_path,
            MediaChangeKind::Relink => MediaResolver.find_relink_candidates(
                &candidate_path, std::slice::from_ref(&search_binding), &roots,
            )?.remove(&search_binding.media_id).ok_or_else(||
                "A imagem com o mesmo nome e extensão não foi encontrada na pasta selecionada. Subpastas não são pesquisadas.".to_string())?,
        };
        Ok::<_, String>((path, roots))
    })
    .await
    .map_err(|error| error.to_string())??;
    let relinked = change_media_binding(&app, binding, path, roots.clone(), kind).await?;
    let relinked_binding = state
        .authorized_media_catalog()?
        .bindings
        .into_iter()
        .find(|binding| binding.media_id == selected_media_id)
        .ok_or_else(|| "A imagem não pertence mais ao Projeto.".to_string())?;
    processing
        .prepare_all_in_plan(&app, vec![relinked_binding], roots)
        .await;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        media_id = safe_log_identifier(&selected_media_id),
        revision = relinked.state.revision,
        event = if kind == MediaChangeKind::Relink { "linked_media_relinked" } else { "linked_media_replaced" },
    );
    state.projection()
}

pub(crate) async fn change_media_binding(
    app: &AppHandle,
    binding: MediaBinding,
    path: std::path::PathBuf,
    roots: myalbuns_paths::RootBindingPlan,
    kind: MediaChangeKind,
) -> Result<EditorProjection, String> {
    let cache_pause = app.state::<CacheEngine>().pause().await;
    let relink_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let estimate =
            crate::imaging_processor::ImageMemoryEstimate::in_plan(&roots, [path.as_path()]);
        let cancellation = crate::cache_activity_gate::CacheCancellation::default();
        let _reservation = tauri::async_runtime::block_on(
            relink_app
                .state::<crate::imaging_processor::ImagingProcessor>()
                .reserve_inspection(estimate, cancellation.flag()),
        )
        .map_err(|error| error.to_string())?;
        let proposal = match kind {
            MediaChangeKind::Relink => {
                MediaResolver.propose_relink_in_plan(&binding, path, &roots)?
            }
            MediaChangeKind::Replace => {
                MediaResolver.propose_replacement_in_plan(&binding, path, &roots)?
            }
        };
        let engine = relink_app.state::<CacheEngine>();
        // Reject the catalog's duplicate-reference constraint before discarding
        // the current Cache. The Core still validates the committed document.
        if relink_app
            .state::<ProjectHost>()
            .authorized_media_catalog()?
            .bindings
            .iter()
            .any(|other| {
                other.media_id != binding.media_id
                    && other.kind == binding.kind
                    && other.logical_path == proposal.replacement_path()
            })
        {
            return Err(
                "O arquivo escolhido já está vinculado a outra imagem deste Projeto.".into(),
            );
        }
        engine
            .invalidate_relinked_media(
                &cache_pause,
                relink_app.state::<AppPaths>().inner(),
                &relink_app.state::<ActiveCacheNamespace>().namespace(),
                relink_app.state::<CachePreviewRegistry>().inner(),
                &binding.media_id,
            )
            .map_err(|error| error.message)?;
        relink_app.state::<ProjectHost>().relink_media(proposal)
    })
    .await
    .map_err(|_| "Não foi possível atualizar a imagem.".to_string())?
}

fn occurrence_is_authoritatively_absent(binding: &MediaBinding) -> bool {
    MediaResolver
        .observe(0, std::slice::from_ref(binding))
        .observations()
        .first()
        .is_some_and(|observation| observation.availability == MediaAvailability::Absent)
}

#[tauri::command]
pub(crate) fn validate_album_information(
    information: AlbumInformation,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<AlbumInformationValidation, String> {
    let validation = state.validate_album_information(&information)?;
    tracing::debug!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        error_count = validation.errors.len(),
        event = "album_information_validated",
    );
    Ok(validation)
}

#[tauri::command]
pub(crate) async fn undo_project(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<EditorProjection, String> {
    let (projection, completed) = project_creative_commands::execute_in_app(
        &app,
        &state,
        || state.undo(),
        |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_undo_completed",
    );
    Ok(completed)
}

#[tauri::command]
pub(crate) async fn redo_project(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<EditorProjection, String> {
    let (projection, completed) = project_creative_commands::execute_in_app(
        &app,
        &state,
        || state.redo(),
        |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&projection.state.project_id),
        revision = projection.state.revision,
        event = "project_redo_completed",
    );
    Ok(completed)
}

#[tauri::command]
pub(crate) async fn save_project(
    expected_revision: u64,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<SaveProjectResult, SaveProjectCommandError> {
    let _operation = crate::project_ui_operations::begin(window.app_handle())
        .map_err(|_| SaveProjectCommandError::SessionUnavailable)?;
    let host = state.inner().clone();
    let window_label = window.label().to_owned();
    let save = tauri::async_runtime::spawn_blocking(move || host.save(expected_revision))
        .await
        .map_err(|error| {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window_label.as_str(),
                expected_revision,
                error = %error,
                event = "project_save_worker_failed",
            );
            SaveProjectCommandError::SessionUnavailable
        })?;
    let saved = save.map_err(|error| {
        let indeterminate = matches!(
            &error,
            ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate)
        );
        if indeterminate {
            tracing::error!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                expected_revision,
                error = ?error,
                event = "project_save_state_indeterminate",
            );
        } else {
            tracing::warn!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                expected_revision,
                error = ?error,
                event = "project_save_rejected",
            );
        }
        map_save_project_error(error)
    })?;
    let outcome = map_save_project_outcome(saved.outcome);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&saved.projection.state.project_id),
        revision = saved.projection.state.revision,
        save_outcome = match outcome {
            SaveProjectOutcome::Saved { .. } => "saved",
            SaveProjectOutcome::AlreadyCurrent { .. } => "already_current",
        },
        event = "project_save_completed",
    );
    Ok(SaveProjectResult {
        outcome,
        projection: saved.projection,
    })
}

#[tauri::command]
pub(crate) async fn save_project_as(
    expected_revision: u64,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<SaveAsProjectResult, SaveAsProjectCommandError> {
    let _operation = crate::project_ui_operations::begin(window.app_handle())
        .map_err(|_| SaveAsProjectCommandError::SessionUnavailable)?;
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(SaveAsProjectCommandError::SessionUnavailable);
    }
    let host = state.inner().clone();
    let before = host
        .projection()
        .map_err(|_| SaveAsProjectCommandError::SessionUnavailable)?;
    let suggested_filename = format!("{}.myalbuns", before.state.project_name);
    let selection = choose_save_as_destination(&window, suggested_filename)
        .await
        .map_err(|error| {
            tracing::warn!(
                target: "myalbuns.desktop",
                process_role = ProcessRole::DesktopHost.as_str(),
                window_label = window.label(),
                expected_revision,
                error = %error,
                event = "project_save_as_dialog_failed",
            );
            SaveAsProjectCommandError::DialogUnavailable
        })?;
    let (path, authorization) = match selection {
        SaveAsDialogOutcome::Cancelled => {
            return Ok(SaveAsProjectResult {
                outcome: SaveAsProjectOutcome::Cancelled,
                projection: before,
            });
        }
        SaveAsDialogOutcome::ReplacementIdentityIndeterminate => {
            return Err(SaveAsProjectCommandError::IdentityIndeterminate);
        }
        SaveAsDialogOutcome::Selected {
            path,
            authorization,
        } => (path, authorization),
    };

    let saved = project_identity_transition::save_as(
        window.clone(),
        host,
        expected_revision,
        path,
        authorization,
    )
    .await
    .map_err(|error| match error {
        IdentityTransitionError::TitleRead => SaveAsProjectCommandError::IoFailure,
        IdentityTransitionError::Path(error) => map_save_as_operation_path_error(error),
        IdentityTransitionError::Worker => SaveAsProjectCommandError::SessionUnavailable,
        IdentityTransitionError::Save(error) => map_save_as_project_error(error),
    })?;

    let outcome = map_save_as_project_outcome(saved.outcome);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        project_id = safe_log_identifier(&saved.projection.state.project_id),
        revision = saved.projection.state.revision,
        event = "project_save_as_completed",
    );
    Ok(SaveAsProjectResult {
        outcome,
        projection: saved.projection,
    })
}

fn map_save_as_project_outcome(outcome: CoreSaveAsProjectOutcome) -> SaveAsProjectOutcome {
    SaveAsProjectOutcome::SavedAs {
        previous_project_id: outcome.previous_project_id.hyphenated().to_string(),
        project_id: outcome.project_id.hyphenated().to_string(),
        revision: outcome.revision,
    }
}

fn map_save_as_path_failure(path: PathFailure) -> SaveAsProjectCommandError {
    match path {
        PathFailure::NotFound => SaveAsProjectCommandError::NotFound,
        PathFailure::Unavailable => SaveAsProjectCommandError::Unavailable,
        PathFailure::AccessDenied => SaveAsProjectCommandError::AccessDenied,
        PathFailure::InvalidPath => SaveAsProjectCommandError::InvalidPath,
        PathFailure::UnexpectedObjectType => SaveAsProjectCommandError::UnexpectedObjectType,
        PathFailure::Conflict => SaveAsProjectCommandError::Conflict,
        PathFailure::IoFailure => SaveAsProjectCommandError::IoFailure,
    }
}

fn map_save_as_operation_path_error(error: AppPathsError) -> SaveAsProjectCommandError {
    match error {
        AppPathsError::OperationPathAccessDenied => SaveAsProjectCommandError::AccessDenied,
        AppPathsError::OperationPathUnavailable => SaveAsProjectCommandError::Unavailable,
        AppPathsError::OperationPathIoFailure | AppPathsError::KnownFoldersUnavailable => {
            SaveAsProjectCommandError::IoFailure
        }
        _ => SaveAsProjectCommandError::InvalidPath,
    }
}

pub(crate) fn map_save_as_project_error(
    error: ProjectHostSaveAsError,
) -> SaveAsProjectCommandError {
    match error {
        ProjectHostSaveAsError::Project(SaveAsProjectError::StaleRevision {
            expected,
            current,
        }) => SaveAsProjectCommandError::StaleRevision {
            expected_revision: expected,
            current_revision: current,
        },
        ProjectHostSaveAsError::Project(SaveAsProjectError::SameTarget) => {
            SaveAsProjectCommandError::SameTarget
        }
        ProjectHostSaveAsError::Project(SaveAsProjectError::DestinationConflict) => {
            SaveAsProjectCommandError::DestinationConflict
        }
        ProjectHostSaveAsError::Project(SaveAsProjectError::ProjectInUse) => {
            SaveAsProjectCommandError::ProjectInUse
        }
        ProjectHostSaveAsError::Project(SaveAsProjectError::IdentityIndeterminate) => {
            SaveAsProjectCommandError::IdentityIndeterminate
        }
        ProjectHostSaveAsError::Project(SaveAsProjectError::Path(path)) => {
            map_save_as_path_failure(path)
        }
        ProjectHostSaveAsError::Project(SaveAsProjectError::SaveAsStateIndeterminate) => {
            SaveAsProjectCommandError::SaveAsStateIndeterminate
        }
        ProjectHostSaveAsError::SessionUnavailable => SaveAsProjectCommandError::SessionUnavailable,
    }
}

fn map_save_project_outcome(outcome: CoreSaveProjectOutcome) -> SaveProjectOutcome {
    match outcome {
        CoreSaveProjectOutcome::Saved { revision } => SaveProjectOutcome::Saved { revision },
        CoreSaveProjectOutcome::AlreadyCurrent { revision } => {
            SaveProjectOutcome::AlreadyCurrent { revision }
        }
    }
}

pub(crate) fn map_save_project_error(error: ProjectHostSaveError) -> SaveProjectCommandError {
    match error {
        ProjectHostSaveError::Project(SaveProjectError::StaleRevision { expected, current }) => {
            SaveProjectCommandError::StaleRevision {
                expected_revision: expected,
                current_revision: current,
            }
        }
        ProjectHostSaveError::Project(SaveProjectError::PersistedBaselineConflict) => {
            SaveProjectCommandError::PersistedBaselineConflict
        }
        ProjectHostSaveError::Project(SaveProjectError::Path(path)) => match path {
            PathFailure::NotFound => SaveProjectCommandError::NotFound,
            PathFailure::Unavailable => SaveProjectCommandError::Unavailable,
            PathFailure::AccessDenied => SaveProjectCommandError::AccessDenied,
            PathFailure::InvalidPath => SaveProjectCommandError::InvalidPath,
            PathFailure::UnexpectedObjectType => SaveProjectCommandError::UnexpectedObjectType,
            PathFailure::Conflict => SaveProjectCommandError::Conflict,
            PathFailure::IoFailure => SaveProjectCommandError::IoFailure,
        },
        ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate) => {
            SaveProjectCommandError::SaveStateIndeterminate
        }
        ProjectHostSaveError::RecoveryCleanupFailed => {
            SaveProjectCommandError::RecoveryCleanupFailed
        }
        ProjectHostSaveError::SessionUnavailable => SaveProjectCommandError::SessionUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{PathFailure, SaveAsProjectError, SaveProjectError};
    use serde_json::json;

    use crate::project_host::{ProjectHostSaveAsError, ProjectHostSaveError};

    use super::{map_save_as_project_error, map_save_project_error};

    #[test]
    fn maps_every_save_as_failure_to_stable_wire_data_without_messages() {
        let stale = serde_json::to_value(map_save_as_project_error(
            ProjectHostSaveAsError::Project(SaveAsProjectError::StaleRevision {
                expected: 3,
                current: 4,
            }),
        ))
        .expect("the stale Save As error serializes");
        assert_eq!(
            stale,
            json!({
                "code": "stale_revision",
                "expectedRevision": 3,
                "currentRevision": 4
            })
        );

        let cases = [
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::SameTarget),
                "same_target",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::DestinationConflict),
                "destination_conflict",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::ProjectInUse),
                "project_in_use",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::IdentityIndeterminate),
                "identity_indeterminate",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(PathFailure::NotFound)),
                "not_found",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(PathFailure::Unavailable)),
                "unavailable",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(
                    PathFailure::AccessDenied,
                )),
                "access_denied",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(PathFailure::InvalidPath)),
                "invalid_path",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(
                    PathFailure::UnexpectedObjectType,
                )),
                "unexpected_object_type",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(PathFailure::Conflict)),
                "conflict",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::Path(PathFailure::IoFailure)),
                "io_failure",
            ),
            (
                ProjectHostSaveAsError::Project(SaveAsProjectError::SaveAsStateIndeterminate),
                "save_as_state_indeterminate",
            ),
            (
                ProjectHostSaveAsError::SessionUnavailable,
                "session_unavailable",
            ),
        ];
        for (error, expected_code) in cases {
            let value = serde_json::to_value(map_save_as_project_error(error))
                .expect("the Save As command error serializes");
            assert_eq!(value, json!({ "code": expected_code }));
            assert!(value.get("message").is_none());
        }
    }

    #[test]
    fn maps_every_save_failure_to_stable_wire_data_without_messages() {
        let stale = serde_json::to_value(map_save_project_error(ProjectHostSaveError::Project(
            SaveProjectError::StaleRevision {
                expected: 3,
                current: 4,
            },
        )))
        .expect("the stale-revision command error serializes");
        assert_eq!(
            stale,
            json!({
                "code": "stale_revision",
                "expectedRevision": 3,
                "currentRevision": 4
            })
        );

        let cases = [
            (
                ProjectHostSaveError::Project(SaveProjectError::PersistedBaselineConflict),
                "persisted_baseline_conflict",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::NotFound)),
                "not_found",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::Unavailable)),
                "unavailable",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::AccessDenied)),
                "access_denied",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::InvalidPath)),
                "invalid_path",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(
                    PathFailure::UnexpectedObjectType,
                )),
                "unexpected_object_type",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::Conflict)),
                "conflict",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::Path(PathFailure::IoFailure)),
                "io_failure",
            ),
            (
                ProjectHostSaveError::Project(SaveProjectError::SaveStateIndeterminate),
                "save_state_indeterminate",
            ),
            (
                ProjectHostSaveError::RecoveryCleanupFailed,
                "recovery_cleanup_failed",
            ),
            (
                ProjectHostSaveError::SessionUnavailable,
                "session_unavailable",
            ),
        ];

        for (error, expected_code) in cases {
            let value = serde_json::to_value(map_save_project_error(error))
                .expect("the command error serializes");
            assert_eq!(value, json!({ "code": expected_code }));
            assert!(value.get("message").is_none());
        }
    }
}
