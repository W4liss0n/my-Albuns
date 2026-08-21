use myalbuns_core::{
    EditorProjection, ImportPhotoDisposition, PathFailure, PhotoDropTarget, PhotoPlacementMode,
    ProjectIntent, ProjectMutationOutcome, SaveProjectError,
    SaveProjectOutcome as CoreSaveProjectOutcome,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::AppPaths;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    cache_engine::CacheEngine,
    cache_previews::CachePreviewRegistry,
    cache_service::CacheNamespaceOwner,
    ipc_contract::{
        ImportPhotoResult, SaveProjectCommandError, SaveProjectOutcome, SaveProjectResult,
    },
    logging::validate_optional_identifier,
    media_runtime::{MediaAvailability, MediaBinding, MediaResolver},
    product_runtime::PROJECT_WINDOW_LABEL,
    project_host::{ProjectHost, ProjectHostSaveError},
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
pub(crate) fn apply_project_intent(
    intent: ProjectIntent,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<ProjectMutationOutcome, String> {
    let intent_kind = match &intent {
        ProjectIntent::SetDpi { .. } => "set_dpi",
        ProjectIntent::TransformPhoto { .. } => "transform_photo",
        ProjectIntent::AddPhoto { .. } => "add_photo",
        ProjectIntent::DropPhoto { .. } => "drop_photo",
    };
    let outcome = state.apply_with_outcome(intent).inspect_err(|_| {
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
        project_id = safe_log_identifier(&outcome.projection.state.project_id),
        revision = outcome.projection.state.revision,
        intent = intent_kind,
        event = "project_intent_applied",
    );
    Ok(outcome)
}

#[tauri::command]
pub(crate) async fn import_photo(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<ImportPhotoResult, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A importação de Foto só está disponível na Janela do Projeto.".into());
    }
    let host = state.inner().clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .set_title("Importar Foto JPEG")
        .add_filter("Imagem JPEG", &["jpg", "jpeg"])
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível concluir o diálogo de importação de Foto.".to_string())?;
    let Some(selection) = selection else {
        return Ok(ImportPhotoResult::Cancelled {
            projection: host.projection()?,
        });
    };
    let FilePath::Path(path) = selection else {
        return Err("O local escolhido não é um Arquivo do Windows válido.".into());
    };
    let imported = tauri::async_runtime::spawn_blocking(move || {
        let proposal = MediaResolver.propose_photo_import(path)?;
        host.import_photo(proposal)
    })
    .await
    .map_err(|_| "Não foi possível concluir a importação da Foto.".to_string())??;
    let (event, result) = match imported.disposition {
        ImportPhotoDisposition::Imported => (
            "photo_imported",
            ImportPhotoResult::Imported {
                projection: imported.projection,
                media_id: imported.media_id.to_string(),
            },
        ),
        ImportPhotoDisposition::Existing => (
            "photo_import_existing_selected",
            ImportPhotoResult::Selected {
                projection: imported.projection,
                media_id: imported.media_id.to_string(),
            },
        ),
    };
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        media_id = safe_log_identifier(match &result {
            ImportPhotoResult::Imported { media_id, .. }
            | ImportPhotoResult::Selected { media_id, .. } => media_id,
            ImportPhotoResult::Cancelled { .. } => unreachable!("the native selection exists"),
        }),
        revision = match &result {
            ImportPhotoResult::Imported { projection, .. }
            | ImportPhotoResult::Selected { projection, .. }
            | ImportPhotoResult::Cancelled { projection } => projection.state.revision,
        },
        event,
    );
    Ok(result)
}

#[tauri::command]
pub(crate) fn photo_drop_target(
    sheet_id: String,
    x_um: i64,
    y_um: i64,
    mode: PhotoPlacementMode,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<PhotoDropTarget, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("O alvo da Foto só pode ser consultado na Janela do Projeto.".into());
    }
    state.project_photo_drop_target(&sheet_id, x_um, y_um, mode)
}

#[tauri::command]
pub(crate) async fn relink_media(
    media_id: String,
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A Religação só está disponível na Janela do Projeto.".into());
    }
    let host = state.inner().clone();
    let binding = host
        .authorized_media_catalog()?
        .bindings
        .into_iter()
        .find(|binding| binding.media_id == media_id)
        .ok_or_else(|| "A ocorrência de mídia não pertence a este Projeto.".to_string())?;
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

    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            "Imagens JPEG, PNG e TIFF",
            &["jpg", "jpeg", "png", "tif", "tiff"],
        )
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível concluir o diálogo de Religação.".to_string())?;
    let Some(selection) = selection else {
        return host.projection();
    };
    let FilePath::Path(path) = selection else {
        return Err("O local escolhido não é um Arquivo do Windows válido.".into());
    };

    let selected_media_id = binding.media_id.clone();
    let cache_pause = app.state::<CacheEngine>().pause().await;
    let relink_app = app.clone();
    let relinked = tauri::async_runtime::spawn_blocking(move || {
        if !occurrence_is_authoritatively_absent(&binding) {
            return Err(
                "O Arquivo original reapareceu durante a Religação; nenhuma referência foi alterada."
                    .to_string(),
            );
        }
        let proposal = MediaResolver.propose_relink(&binding, path)?;
        let engine = relink_app.state::<CacheEngine>();
        let namespace = relink_app.state::<CacheNamespaceOwner>();
        engine
            .invalidate_relinked_media(
                &cache_pause,
                relink_app.state::<AppPaths>().inner(),
                namespace.namespace(),
                relink_app.state::<CachePreviewRegistry>().inner(),
                &binding.media_id,
            )
            .map_err(|error| {
                format!(
                    "Não foi possível invalidar o Cache antes da Religação: {}",
                    error.message
                )
            })?;
        host.relink_media(proposal)
    })
    .await;
    let relinked = relinked.map_err(|_| "Não foi possível concluir a Religação.".to_string())??;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        window_label = window.label(),
        media_id = safe_log_identifier(&selected_media_id),
        revision = relinked.state.revision,
        event = "linked_media_relinked",
    );
    Ok(relinked)
}

fn occurrence_is_authoritatively_absent(binding: &MediaBinding) -> bool {
    MediaResolver
        .observe(0, std::slice::from_ref(binding))
        .observations()
        .first()
        .is_some_and(|observation| observation.availability == MediaAvailability::Absent)
}

#[tauri::command]
pub(crate) fn undo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.undo()?;
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
pub(crate) fn redo_project(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<EditorProjection, String> {
    let projection = state.redo()?;
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
pub(crate) async fn save_project(
    expected_revision: u64,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<SaveProjectResult, SaveProjectCommandError> {
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
        ProjectHostSaveError::SessionUnavailable => SaveProjectCommandError::SessionUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{PathFailure, SaveProjectError};
    use serde_json::json;

    use crate::project_host::ProjectHostSaveError;

    use super::map_save_project_error;

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
