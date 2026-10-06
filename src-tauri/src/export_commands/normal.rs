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
        return Err("A exportação pertence à Janela do projeto.".into());
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
        .set_title("Escolher pasta de destino da exportação")
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
    recovery_id: Option<String>,
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
    let recoveries = app.state::<crate::storage_recovery::StorageRecoveries>();
    if let Some(id) = recovery_id {
        let acquisition =
            OperationLease::begin(&operation_gate).map_err(ExportCommandError::from_gate)?;
        if let Some(recovery) = recoveries
            .take_export(&id)
            .map_err(ExportCommandError::failed)?
        {
            let request_id = recovery.request_id().to_owned();
            let attempt = attempts
                .begin(request_id.clone(), window.label())
                .map_err(|error| ExportCommandError::failed(error.to_string()))?;
            let prepared = PreparedExportCommand {
                acquisition,
                attempt,
                operation_paths: recovery.required_paths(),
                project_id: Some(recovery.project_id().to_owned()),
                request_id,
                plan: ExportCommandPlan::Resume(recovery),
            };
            return run_export(app, window, on_event, logging, cache, processor, prepared)
                .await
                .map(Some)
                .map_err(Into::into);
        }
        drop(acquisition);
    }
    recoveries.finish("export");
    options
        .format
        .validate()
        .map_err(ExportCommandError::failed)?;
    let layout = state
        .validate_export(&options.sheet_ids)
        .map_err(ExportCommandError::failed)?;
    if !layout.is_empty() {
        let mut error =
            ExportCommandError::failed("Preencha os quadros vazios antes de exportar a seleção.");
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
        let mut error = ExportCommandError::failed("Confira os arquivos necessários à exportação.");
        error.code = ExportCommandErrorCode::MediaProblems;
        error.media_problems = Some(media);
        return Err(error.into());
    }
    let current = state.projection().map_err(ExportCommandError::failed)?;
    if changed_since_snapshot(&snapshot, &current.state.project_id, current.state.revision) {
        return Err(ExportCommandError::failed(
            "O projeto mudou durante a verificação. Tente exportar novamente.",
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
    let destination_volume = myalbuns_paths::StorageVolume::containing(&destination);
    let planned = tauri::async_runtime::spawn_blocking(move || {
        plan_normal_export(
            snapshot,
            protected_originals,
            sources,
            options,
            destination,
            request_id,
        )
    })
    .await
    .map_err(|error| ExportCommandError::failed(error.to_string()))?
    .map_err(|failure| {
        if failure.is_storage_full() {
            recoveries.pause("export", destination_volume);
        }
        ExportCommandError::from_pipeline(failure)
    })?;
    let plan = match planned {
        PlannedNormalExport::Conflicts(conflicts) => {
            let mut error =
                ExportCommandError::failed("Já existem arquivos no destino da exportação.");
            error.code = ExportCommandErrorCode::ExportConflict;
            return Err(NormalExportError {
                error: Box::new(error),
                conflicts,
            });
        }
        PlannedNormalExport::NothingToExport => return Ok(None),
        PlannedNormalExport::Ready(plan) => plan,
    };
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

fn changed_since_snapshot(
    snapshot: &myalbuns_core::RenderSnapshot,
    current_project_id: &str,
    current_revision: u64,
) -> bool {
    current_project_id != snapshot.project_id || current_revision != snapshot.revision
}

/// What the chosen conflict policy allows once the Destination was inspected.
#[derive(Debug)]
enum PlannedNormalExport {
    /// The user has not chosen yet: nothing was created or written.
    Conflicts(Vec<String>),
    /// Every output already exists and the user chose to keep them.
    NothingToExport,
    Ready(Box<export_pipeline::AlbumExportPlan>),
}

fn plan_normal_export(
    snapshot: myalbuns_core::RenderSnapshot,
    protected_originals: Vec<PathBuf>,
    sources: Vec<myalbuns_imaging_protocol::RenderSource>,
    options: NormalExportOptions,
    destination: PathBuf,
    request_id: String,
) -> Result<PlannedNormalExport, export_pipeline::ExportFailure> {
    let conflict_policy = options.conflict_policy;
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
    if !conflicts.is_empty() && conflict_policy == ExportConflictPolicy::Ask {
        return Ok(PlannedNormalExport::Conflicts(conflicts));
    }
    std::fs::create_dir_all(&destination).map_err(|error| {
        tracing::warn!(target: "myalbuns.desktop", %error, event = "export_destination_creation_failed");
        let path_error = myalbuns_paths::AppPathsError::export_io(&error);
        let message = if path_error == myalbuns_paths::AppPathsError::ExportStorageFull {
            myalbuns_paths::AppPathsError::EXPORT_STORAGE_FULL_MESSAGE
        } else {
            "Não foi possível criar a pasta de destino. Escolha outra pasta ou confira as permissões."
        };
        export_pipeline::ExportFailure::from_path_error(
            export_pipeline::ExportFailureStage::Prepare,
            path_error,
            message,
        )
    })?;
    if conflict_policy == ExportConflictPolicy::Skip && !plan.skip_existing_outputs()? {
        return Ok(PlannedNormalExport::NothingToExport);
    }
    Ok(PlannedNormalExport::Ready(Box::new(plan)))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use myalbuns_core::{ExportFormat, ExportMode};
    use myalbuns_imaging_protocol::RenderSource;
    use myalbuns_paths::OperationPathContext;

    use super::*;
    use crate::{
        export_pipeline::{
            ExportExecutionControl, ExportFailure, ExportFailureStage, PublishedExport,
            tests::{AlbumTransport, productive_snapshot},
        },
        ipc_contract::ExportScope,
    };

    const FIRST: &str = "Album_001.png";
    const SECOND: &str = "Album_002.png";

    fn planned(
        root: &Path,
        project_name: &str,
        format: ExportFormat,
        conflict_policy: ExportConflictPolicy,
    ) -> Result<PlannedNormalExport, ExportFailure> {
        let source = root.join("original.jpg");
        if !source.exists() {
            image::RgbImage::from_pixel(4, 4, image::Rgb([20, 50, 90]))
                .save(&source)
                .unwrap();
        }
        let mut snapshot = productive_snapshot(source.clone());
        snapshot.project_name = project_name.into();
        snapshot.composition.sheets.truncate(2);
        let sheet_ids = snapshot
            .composition
            .sheets
            .iter()
            .map(|sheet| sheet.sheet_id.clone())
            .collect();
        let media_id = snapshot.composition.sheets[0]
            .referenced_media_ids()
            .next()
            .unwrap();
        let destination = root.join("out");
        plan_normal_export(
            snapshot,
            vec![source.clone()],
            vec![RenderSource::new(media_id, source).unwrap()],
            NormalExportOptions {
                scope: ExportScope::Range,
                sheet_ids,
                mode: ExportMode::Sheet,
                format,
                destination: destination.to_string_lossy().into_owned(),
                conflict_policy,
            },
            destination,
            "normal-export".into(),
        )
    }

    fn album(
        root: &Path,
        conflict_policy: ExportConflictPolicy,
    ) -> Result<PlannedNormalExport, ExportFailure> {
        planned(root, "Album", ExportFormat::Png, conflict_policy)
    }

    fn existing_output(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let destination = root.join("out");
        std::fs::create_dir_all(&destination).unwrap();
        let output = destination.join(name);
        std::fs::write(&output, bytes).unwrap();
        output
    }

    fn execute(
        plan: export_pipeline::AlbumExportPlan,
        untouched_while_preparing: (&Path, &[u8]),
    ) -> Result<PublishedExport, ExportFailure> {
        let mut roots = OperationPathContext::new();
        for path in plan.required_paths() {
            roots.capture(&path).unwrap();
        }
        let mut transport = AlbumTransport {
            fail: None,
            prior_output: untouched_while_preparing.0.to_path_buf(),
            prior_bytes: untouched_while_preparing.1.to_vec(),
        };
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(export_pipeline::execute_album(
                &mut transport,
                plan,
                &roots.freeze(),
                &ExportExecutionControl::default(),
                &|_| {},
                &InvocationContext::new("normal-export", None::<String>),
            ))
    }

    #[test]
    fn a_changed_project_or_revision_refuses_the_frozen_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = productive_snapshot(root.path().join("original.jpg"));
        let (project_id, revision) = (snapshot.project_id.clone(), snapshot.revision);
        for (current_project, current_revision, changed) in [
            (project_id.clone(), revision, false),
            (project_id.clone(), revision + 1, true),
            (format!("{project_id}-other"), revision, true),
            (format!("{project_id}-other"), revision + 1, true),
        ] {
            assert_eq!(
                changed_since_snapshot(&snapshot, &current_project, current_revision),
                changed,
                "{current_project} at revision {current_revision}"
            );
        }
    }

    #[test]
    fn without_conflicts_every_policy_creates_the_destination_and_is_ready() {
        for policy in [
            ExportConflictPolicy::Ask,
            ExportConflictPolicy::Skip,
            ExportConflictPolicy::Replace,
        ] {
            let root = tempfile::tempdir().unwrap();
            let planned = album(root.path(), policy).unwrap();
            let PlannedNormalExport::Ready(plan) = planned else {
                panic!("{policy:?} without conflicts must be ready: {planned:?}");
            };
            assert!(root.path().join("out").is_dir());
            let outputs = plan.required_paths();
            assert!(outputs.contains(&root.path().join("out").join(FIRST)));
            assert!(outputs.contains(&root.path().join("out").join(SECOND)));
        }
    }

    #[test]
    fn ask_returns_the_existing_files_and_leaves_them_untouched() {
        let root = tempfile::tempdir().unwrap();
        let second = existing_output(root.path(), SECOND, b"previous export");

        let planned = album(root.path(), ExportConflictPolicy::Ask).unwrap();

        let PlannedNormalExport::Conflicts(conflicts) = planned else {
            panic!("the user has not chosen yet: {planned:?}");
        };
        assert_eq!(conflicts, [SECOND]);
        assert_eq!(std::fs::read(&second).unwrap(), b"previous export");
        assert_eq!(
            std::fs::read_dir(root.path().join("out")).unwrap().count(),
            1,
            "asking prepares nothing in the destination"
        );
    }

    #[test]
    fn skip_all_with_every_output_present_exports_nothing_and_is_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let first = existing_output(root.path(), FIRST, b"keep-first");
        let second = existing_output(root.path(), SECOND, b"keep-second");

        let planned = album(root.path(), ExportConflictPolicy::Skip).unwrap();

        assert!(
            matches!(planned, PlannedNormalExport::NothingToExport),
            "{planned:?}"
        );
        assert_eq!(std::fs::read(&first).unwrap(), b"keep-first");
        assert_eq!(std::fs::read(&second).unwrap(), b"keep-second");
    }

    #[test]
    fn skip_exports_only_the_missing_files_and_keeps_the_existing_ones() {
        let root = tempfile::tempdir().unwrap();
        let first = existing_output(root.path(), FIRST, b"keep-first");
        let second = root.path().join("out").join(SECOND);

        let planned = album(root.path(), ExportConflictPolicy::Skip).unwrap();

        let PlannedNormalExport::Ready(plan) = planned else {
            panic!("the missing file is still exported: {planned:?}");
        };
        assert!(!plan.required_paths().contains(&first));
        assert!(plan.required_paths().contains(&second));
        execute(*plan, (&first, b"keep-first")).unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"keep-first");
        assert_eq!(std::fs::read(&second).unwrap(), b"prepared-0");
    }

    #[test]
    fn replace_is_the_only_policy_that_overwrites_an_existing_file() {
        let root = tempfile::tempdir().unwrap();
        let first = existing_output(root.path(), FIRST, b"previous export");
        let planned = album(root.path(), ExportConflictPolicy::Replace).unwrap();
        let PlannedNormalExport::Ready(plan) = planned else {
            panic!("the user confirmed the replacement: {planned:?}");
        };
        execute(*plan, (&first, b"previous export")).unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"prepared-0");

        // Ask and Skip stay create-only: a file that appears after the conflict
        // check was never shown to the user, so publication must refuse it.
        for policy in [ExportConflictPolicy::Ask, ExportConflictPolicy::Skip] {
            let root = tempfile::tempdir().unwrap();
            let planned = album(root.path(), policy).unwrap();
            let PlannedNormalExport::Ready(plan) = planned else {
                panic!("{policy:?} without conflicts must be ready: {planned:?}");
            };
            let late = existing_output(root.path(), FIRST, b"arrived-later");
            let failure = execute(*plan, (&late, b"arrived-later")).unwrap_err();
            assert!(
                matches!(failure.stage, ExportFailureStage::Publish { .. }),
                "{policy:?}: {:?}",
                failure.stage
            );
            assert_eq!(std::fs::read(&late).unwrap(), b"arrived-later");
        }
    }

    #[test]
    fn a_reserved_device_name_is_refused_at_planning_only_when_it_is_the_whole_file_stem() {
        for name in ["CON", "NUL", "COM1"] {
            let root = tempfile::tempdir().unwrap();
            assert_eq!(export_name(name), name);
            // `CON.pdf` would be a device alias; `CON_001.png` is an ordinary file.
            let pdf = planned(
                root.path(),
                name,
                ExportFormat::Pdf,
                ExportConflictPolicy::Ask,
            )
            .unwrap_err();
            assert_eq!(pdf.stage, ExportFailureStage::Plan, "{name}");
            assert!(!root.path().join("out").exists());
            let png = planned(
                root.path(),
                name,
                ExportFormat::Png,
                ExportConflictPolicy::Ask,
            )
            .unwrap();
            assert!(matches!(png, PlannedNormalExport::Ready(_)), "{name}");
        }
    }
}
