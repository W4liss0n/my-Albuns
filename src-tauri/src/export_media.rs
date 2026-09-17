use myalbuns_paths::OperationPathContext;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    ipc_contract::{
        ExportMediaProblem, ExportMediaState, ExportRelinkResult, ImageProcessingProblem,
    },
    media_runtime::{MediaAvailability, MediaBinding, MediaResolver},
    product_runtime::PROJECT_WINDOW_LABEL,
    project_host::ProjectHost,
};

fn required_bindings_for(
    host: &ProjectHost,
    sheet_ids: &[String],
) -> Result<Vec<MediaBinding>, String> {
    let (_, sources) = host.freeze_export(sheet_ids)?;
    let catalog = host.authorized_media_catalog()?;
    Ok(catalog
        .bindings
        .into_iter()
        .filter(|binding| {
            sources
                .iter()
                .any(|source| source.media_id().to_string() == binding.media_id)
        })
        .collect())
}

pub(crate) fn inspect_selection(
    host: &ProjectHost,
    sheet_ids: &[String],
) -> Result<Vec<ExportMediaProblem>, String> {
    let bindings = required_bindings_for(host, sheet_ids)?;
    Ok(MediaResolver
        .observe(0, &bindings)
        .observations()
        .iter()
        .filter_map(|observation| {
            let state = match observation.availability {
                MediaAvailability::Candidate => return None,
                MediaAvailability::Absent => ExportMediaState::Absent,
                MediaAvailability::Unavailable => ExportMediaState::Unavailable,
            };
            Some(ExportMediaProblem {
                media_id: observation.media_id.clone(),
                file_name: observation
                    .logical_path()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                state,
            })
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn inspect_export_media(
    sheet_id: String,
    sheet_ids: Option<Vec<String>>,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
) -> Result<Vec<ExportMediaProblem>, String> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A Exportação pertence à Janela do Projeto.".into());
    }
    let host = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        inspect_selection(&host, &sheet_ids.unwrap_or_else(|| vec![sheet_id]))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn relink_export_media(
    sheet_id: String,
    sheet_ids: Option<Vec<String>>,
    app: AppHandle,
    window: WebviewWindow,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<ExportRelinkResult, String> {
    let _operation = crate::project_ui_operations::begin(&app)?;
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err("A Religação pertence à Janela do Projeto.".into());
    }
    let host = app.state::<ProjectHost>().inner().clone();
    let inspecting_host = host.clone();
    let sheet_ids = sheet_ids.unwrap_or_else(|| vec![sheet_id]);
    let inspecting_sheets = sheet_ids.clone();
    let missing = tauri::async_runtime::spawn_blocking(move || {
        let problems = inspect_selection(&inspecting_host, &inspecting_sheets)?;
        Ok::<_, String>(
            required_bindings_for(&inspecting_host, &inspecting_sheets)?
                .into_iter()
                .filter(|binding| {
                    problems.iter().any(|problem| {
                        problem.media_id == binding.media_id
                            && problem.state == ExportMediaState::Absent
                    })
                })
                .collect::<Vec<_>>(),
        )
    })
    .await
    .map_err(|error| error.to_string())??;
    let mut notes = Vec::new();
    if !missing.is_empty() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let parent = app
            .get_webview_window("project-dialog")
            .unwrap_or_else(|| window.clone());
        app.dialog()
            .file()
            .set_parent(&parent)
            .set_title("Escolher pasta das Fotos para Relinkar")
            .pick_folder(move |selection| {
                let _ = sender.send(selection);
            });
        let selection = receiver.await.map_err(|error| error.to_string())?;
        if let Some(FilePath::Path(folder)) = selection {
            let catalog = host.authorized_media_catalog()?;
            let cache_root = app
                .state::<crate::cache_service::ActiveCacheNamespace>()
                .namespace()
                .paths()
                .root()
                .to_path_buf();
            let (candidates, roots, missing) = tauri::async_runtime::spawn_blocking(move || {
                let mut context = OperationPathContext::new();
                let _ = context.capture(&cache_root);
                for media in &catalog.bindings {
                    let _ = context.capture(&media.logical_path);
                }
                context
                    .capture(&folder)
                    .map_err(|error| error.to_string())?;
                for binding in &missing {
                    context
                        .capture(&binding.logical_path)
                        .map_err(|error| error.to_string())?;
                }
                let roots = context.freeze();
                let candidates = MediaResolver.find_relink_candidates(&folder, &missing, &roots)?;
                Ok::<_, String>((candidates, roots, missing))
            })
            .await
            .map_err(|error| error.to_string())??;
            let mut processing = crate::image_processing::ImageProcessingBatch::new(
                missing.len() as u32,
                |progress| {
                    if let Some(problem) = progress.problem.as_ref() {
                        notes.push(problem.clone());
                    }
                    if let Some(reason) = progress.operation_problem.as_ref()
                        && !notes.iter().any(|note| note.reason == *reason)
                    {
                        notes.push(ImageProcessingProblem {
                            file_name: "Processamento".into(),
                            reason: reason.clone(),
                        });
                    }
                    let _ = on_progress.send(progress);
                },
            );
            for binding in missing {
                let file_name = binding
                    .logical_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let result = match candidates.get(&binding.media_id) {
                    Some(path) => crate::project_commands::change_media_binding(
                        &app,
                        binding.clone(),
                        path.clone(),
                        roots.clone(),
                        crate::project_commands::MediaChangeKind::Relink,
                    )
                    .await
                    .map(|_| ()),
                    None => Err(
                        "Nenhuma correspondência única de nome e extensão foi encontrada na pasta."
                            .into(),
                    ),
                };
                match result {
                    Ok(()) => {
                        let replacement = MediaBinding {
                            logical_path: candidates[&binding.media_id].clone(),
                            ..binding
                        };
                        processing
                            .prepare_all_in_plan(&app, vec![replacement], roots.clone())
                            .await;
                    }
                    Err(reason) => {
                        processing.complete(Some(ImageProcessingProblem { file_name, reason }))
                    }
                }
            }
        }
    }
    let inspecting_host = host.clone();
    let problems = tauri::async_runtime::spawn_blocking(move || {
        inspect_selection(&inspecting_host, &sheet_ids)
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(ExportRelinkResult {
        projection: host.projection()?,
        problems,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, PhotoPlacementMode, ProjectCore,
        ProjectIntent, ProjectLocation,
    };

    #[test]
    fn recovery_scopes_dependencies_and_exports_unsaved_relinks_with_undo() {
        let root = tempfile::tempdir().unwrap();
        let project_path = root.path().join("Projeto.myalbuns");
        let original = root.path().join("Usada.png");
        let panel_only = root.path().join("Painel.png");
        for path in [&original, &panel_only] {
            RgbImage::from_pixel(30, 20, Rgb([40, 80, 160]))
                .save_with_format(path, ImageFormat::Png)
                .unwrap();
        }
        let mut context = OperationPathContext::new();
        context.capture(&project_path).unwrap();
        let roots = context.freeze();
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), roots.clone()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let host = ProjectHost::new(project);
        host.import_photos(vec![original.clone(), panel_only.clone()], |_| {})
            .unwrap();
        let sheet_id = host.projection().unwrap().state.album.sheets[0].id.clone();
        let binding = host
            .authorized_media_catalog()
            .unwrap()
            .bindings
            .into_iter()
            .find(|binding| binding.logical_path == original)
            .unwrap();
        host.apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id: sheet_id.clone(),
            media_id: binding.media_id.parse().unwrap(),
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
        host.save(host.projection().unwrap().state.revision)
            .unwrap();
        let persisted = std::fs::read(&project_path).unwrap();
        std::fs::remove_file(panel_only).unwrap();
        assert!(
            inspect_selection(&host, std::slice::from_ref(&sheet_id))
                .unwrap()
                .is_empty(),
            "a Panel-only missing file cannot block the selection"
        );
        std::fs::remove_file(&original).unwrap();
        let problems = inspect_selection(&host, std::slice::from_ref(&sheet_id)).unwrap();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].state, ExportMediaState::Absent);
        std::fs::create_dir(&original).unwrap();
        assert_eq!(
            inspect_selection(&host, std::slice::from_ref(&sheet_id)).unwrap()[0].state,
            ExportMediaState::Unavailable
        );
        std::fs::remove_dir(&original).unwrap();
        let folder = root.path().join("Restauradas");
        std::fs::create_dir_all(folder.join("subpasta")).unwrap();
        let replacement = folder.join("subpasta/Usada.png");
        RgbImage::from_pixel(40, 30, Rgb([60, 80, 160]))
            .save_with_format(&replacement, ImageFormat::Png)
            .unwrap();
        let candidates = MediaResolver
            .find_relink_candidates(&folder, std::slice::from_ref(&binding), &roots)
            .unwrap();
        assert!(
            candidates.is_empty(),
            "Export recovery must not search subfolders"
        );
        let direct_replacement = folder.join("Usada.png");
        std::fs::copy(&replacement, &direct_replacement).unwrap();
        let candidates = MediaResolver
            .find_relink_candidates(&folder, std::slice::from_ref(&binding), &roots)
            .unwrap();
        let proposal = MediaResolver
            .propose_relink_in_plan(&binding, candidates[&binding.media_id].clone(), &roots)
            .unwrap();
        host.relink_media(proposal).unwrap();
        assert!(host.projection().unwrap().state.dirty);
        assert_eq!(
            std::fs::read(&project_path).unwrap(),
            persisted,
            "recovery does not save the Project"
        );
        assert!(
            inspect_selection(&host, std::slice::from_ref(&sheet_id))
                .unwrap()
                .is_empty()
        );
        let exported = host.freeze_sheet_export(&sheet_id).unwrap();
        assert_eq!(exported.sources.len(), 1);
        assert_eq!(exported.sources[0].source_path(), direct_replacement);
        host.undo().unwrap();
        assert_eq!(
            inspect_selection(&host, std::slice::from_ref(&sheet_id)).unwrap()[0].state,
            ExportMediaState::Absent
        );
        host.redo().unwrap();
        assert!(
            inspect_selection(&host, std::slice::from_ref(&sheet_id))
                .unwrap()
                .is_empty()
        );
        assert_eq!(std::fs::read(&project_path).unwrap(), persisted);
    }
}
