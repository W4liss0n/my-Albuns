use super::*;
use myalbuns_core::{CreateAuthorization, CreateProjectRequest, InitialProject, ProjectLocation};
use std::sync::Mutex;

pub(crate) fn fixture(
    root: &std::path::Path,
    relative: &str,
) -> (ProjectCore, myalbuns_core::EditableProject) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let mut paths = myalbuns_paths::OperationPathContext::new();
    paths.capture(&path).unwrap();
    let editor = core
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(path, paths.freeze()),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    (core, editor)
}

#[test]
fn discovers_persisted_projects_recursively_and_preserves_destination_hierarchy() {
    let root = tempfile::tempdir().unwrap();
    let (core, mut first) = fixture(root.path(), "source/Escola/Turma/Album.myalbuns");
    let (_, _second) = fixture(root.path(), "source/Outro.myalbuns");
    first
        .apply(myalbuns_core::ProjectIntent::SetDpi { dpi: 240 })
        .unwrap();
    std::fs::write(root.path().join("source/ignore.txt"), b"not a project").unwrap();
    for alternate in [None, Some(root.path().join("delivery"))] {
        let batch = BatchRunner::discover(
            BatchConfiguration {
                source: root.path().join("source"),
                destination: alternate.clone(),
                format: myalbuns_core::ExportFormat::Png,
                mode: myalbuns_core::ExportMode::Sheet,
            },
            core.clone(),
            root.path().join("checkpoints"),
        )
        .unwrap();
        let view = batch.view();
        assert_eq!(view.items.len(), 2);
        assert!(view.items.iter().all(|item| item.problems.is_empty()));
        let album = view.items.iter().find(|item| item.name == "Album").unwrap();
        assert_eq!(
            Path::new(&album.destination),
            alternate
                .unwrap_or(root.path().join("source"))
                .join("Escola/Turma/Album")
        );
        assert!(
            !Path::new(&album.destination).exists(),
            "preflight does not create delivery folders"
        );
    }
    assert!(first.has_unsaved_changes());
}

#[derive(Default)]
struct RecordingTransport {
    names: Vec<String>,
    prior_outputs: Vec<PathBuf>,
    crash_during_preparation: bool,
    sources: Vec<Vec<PathBuf>>,
    dpis: Vec<u32>,
    storage_full_for: Option<String>,
    termination_unconfirmed_for: Option<String>,
}

#[test]
fn explicit_preflight_retry_recaptures_roots_instead_of_reusing_failed_bindings() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: myalbuns_core::ExportFormat::Png,
            mode: myalbuns_core::ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    let unavailable = tempfile::tempdir().unwrap();
    batch.paths = OperationPathContext::new();
    batch
        .paths
        .capture_with_binding(&batch.items[0].path, unavailable.path())
        .unwrap();
    batch.recheck();
    assert!(!batch.view().can_continue);
    batch.retry_preflight();
    assert!(batch.view().can_continue);
}

#[cfg(windows)]
#[test]
fn batch_conflicts_skip_and_orphan_cleanup_use_the_frozen_destination() {
    for policy in [ExportConflictPolicy::Skip, ExportConflictPolicy::Replace] {
        let root = tempfile::tempdir().unwrap();
        let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
        let mut batch = BatchRunner::discover(
            BatchConfiguration {
                source: root.path().join("source"),
                destination: None,
                format: myalbuns_core::ExportFormat::Png,
                mode: myalbuns_core::ExportMode::Sheet,
            },
            core,
            root.path().join("checkpoints"),
        )
        .unwrap();
        let bound = root.path().join("bound");
        let later = root.path().join("later");
        std::fs::create_dir_all(bound.join("Delivery")).unwrap();
        std::fs::create_dir_all(later.join("Delivery")).unwrap();
        std::fs::write(bound.join("Delivery/A_001.png"), b"bound-first").unwrap();
        std::fs::write(bound.join("Delivery/A_003.png"), b"bound-orphan").unwrap();
        std::fs::write(later.join("Delivery/A_002.png"), b"later-second").unwrap();
        let logical = PathBuf::from(r"Z:\Delivery");
        batch.items[0].destination = logical.clone();
        batch.paths.capture_with_binding(&logical, &bound).unwrap();
        batch.recheck();
        assert!(batch.view().has_conflicts);
        assert!(batch.paths.capture_with_binding(&logical, &later).is_err());
        let mut transport = RecordingTransport::default();
        let completed = tauri::async_runtime::block_on(batch.run(
            &mut transport,
            &BatchCancellation::default(),
            policy,
            &|_| {},
        ))
        .unwrap();
        assert_eq!(completed.view().items[0].status, BatchItemStatus::Completed);
        assert!(bound.join("Delivery/A_002.png").is_file());
        assert_eq!(
            bound.join("Delivery/A_003.png").exists(),
            policy == ExportConflictPolicy::Skip
        );
        if policy == ExportConflictPolicy::Skip {
            assert_eq!(
                std::fs::read(bound.join("Delivery/A_001.png")).unwrap(),
                b"bound-first"
            );
            assert_eq!(
                std::fs::read(bound.join("Delivery/A_003.png")).unwrap(),
                b"bound-orphan"
            );
        }
        assert_eq!(
            std::fs::read(later.join("Delivery/A_002.png")).unwrap(),
            b"later-second"
        );
        assert_eq!(
            std::fs::read_dir(later.join("Delivery")).unwrap().count(),
            1
        );
    }
}

impl crate::imaging_processor::ImagingTransport for RecordingTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a myalbuns_imaging_protocol::ImagingCommand,
        _context: &'a crate::imaging_processor::InvocationContext,
        _operation: crate::imaging_processor::ImagingOperation,
        _attempt: u8,
        control: crate::imaging_processor::InvocationControl<'a>,
    ) -> crate::imaging_processor::InvocationFuture<'a> {
        use myalbuns_imaging_protocol::{
            AlbumRenderCompletion, ImagingCommand, ImagingResponse, RenderCompletion,
        };
        use sha2::{Digest, Sha256};
        let ImagingCommand::RenderAlbum(request) = command else {
            panic!("batch uses the shared album protocol")
        };
        for path in &self.prior_outputs {
            assert!(path.exists(), "the preceding item has finished publication");
        }
        self.dpis.push(request.snapshot.dpi);
        self.names.push(request.snapshot.project_name.clone());
        self.sources.push(
            request
                .sources
                .iter()
                .map(|source| source.source_path().to_path_buf())
                .collect(),
        );
        let total_units = request
            .outputs
            .iter()
            .map(|output| output.units.len() as u32)
            .sum::<u32>();
        use myalbuns_imaging_protocol::{ImagingProgress, ImagingProgressStage};
        for (stage, completed, total) in [
            (
                ImagingProgressStage::LoadingSources,
                request.sources.len() as u32,
                request.sources.len() as u32,
            ),
            (
                ImagingProgressStage::Composing,
                total_units.min(1),
                total_units,
            ),
            (
                ImagingProgressStage::EncodingOutput,
                total_units,
                total_units,
            ),
        ] {
            if total > 0 {
                control.report(
                    ImagingProgress::new(&request.request_id, stage, completed, total).unwrap(),
                );
            }
        }
        let mut outputs = vec![];
        self.prior_outputs.clear();
        for (index, output) in request.outputs.iter().enumerate() {
            let path = request
                .root_bindings
                .resolve(output.prepared_path.as_path())
                .unwrap();
            let bytes = format!("{}-{index}", request.snapshot.project_name).into_bytes();
            std::fs::write(&path, &bytes).unwrap();
            let destination = path.parent().unwrap().parent().unwrap();
            self.prior_outputs
                .push(destination.join(path.file_name().unwrap()));
            outputs.push(RenderCompletion {
                width_px: 1,
                height_px: 1,
                dpi: 300,
                source_count: request.sources.len(),
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&bytes)),
            });
        }
        if self.crash_during_preparation {
            std::process::exit(91);
        }
        if self.termination_unconfirmed_for.as_deref() == Some(&request.snapshot.project_name) {
            self.prior_outputs.clear();
            return Box::pin(async {
                Err(
                    crate::imaging_processor::InvocationFailure::termination_unconfirmed(
                        4242,
                        UNCONFIRMED_TERMINATION,
                    ),
                )
            });
        }
        let response = if self.storage_full_for.as_deref() == Some(&request.snapshot.project_name) {
            self.prior_outputs.clear();
            ImagingResponse::failed(
                request.request_id.clone(),
                myalbuns_imaging_protocol::ImagingFailureCode::OutputStorageFull,
                None::<String>,
                None,
            )
        } else {
            ImagingResponse::AlbumCompleted {
                request_id: request.request_id.clone(),
                completion: AlbumRenderCompletion { outputs },
            }
        };
        Box::pin(async move { Ok(response) })
    }
}

#[test]
fn batch_progress_aggregates_pipeline_percentages_and_never_regresses_on_live_resume() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/B.myalbuns");
    let batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    let updates = Mutex::new(Vec::new());
    let report = |update: crate::ipc_contract::BatchExportProgress| {
        assert_eq!(update.total, 2);
        updates
            .lock()
            .unwrap()
            .push((update.completed, update.percent));
    };
    let fault =
        myalbuns_paths::test_support::DiskFull::on_rename(&root.path().join("source/A/A_002.png"));
    let mut transport = RecordingTransport::default();
    let mut paused = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &report,
    ))
    .unwrap();
    assert_eq!(fault.failure_count(), 1);
    assert_eq!(paused.view().phase, BatchPhase::StorageFull);
    assert_eq!(paused.progress().percent, 46.0);
    drop(fault);
    paused.retry_preflight();
    let completed = tauri::async_runtime::block_on(paused.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &report,
    ))
    .unwrap();
    assert_eq!(completed.view().phase, BatchPhase::Finished);
    assert_eq!(
        transport.names,
        ["A", "B"],
        "resuming publication does not render A again"
    );
    let updates = updates.into_inner().unwrap();
    for expected in [
        (0, 21.25),
        (0, 46.0),
        (0, 49.5),
        (1, 50.0),
        (1, 71.25),
        (1, 99.5),
        (2, 100.0),
    ] {
        assert!(
            updates.contains(&expected),
            "missing {expected:?} in {updates:?}"
        );
    }
    assert!(
        updates.windows(2).all(|pair| pair[0].1 <= pair[1].1),
        "{updates:?}"
    );
    assert_eq!(updates.last(), Some(&(2, 100.0)));
}

#[test]
fn disk_full_pauses_batch_before_the_next_project_and_preserves_completed_items() {
    for live in [false, true] {
        for publication_index in [None, Some(0), Some(1)] {
            let root = tempfile::tempdir().unwrap();
            let (core, _first) = fixture(root.path(), "source/A.myalbuns");
            let (_, _second) = fixture(root.path(), "source/B.myalbuns");
            let (_, _third) = fixture(root.path(), "source/C.myalbuns");
            let checkpoints = root.path().join("checkpoints");
            let batch = BatchRunner::discover(
                BatchConfiguration {
                    source: root.path().join("source"),
                    destination: None,
                    format: ExportFormat::Png,
                    mode: ExportMode::Sheet,
                },
                core.clone(),
                checkpoints.clone(),
            )
            .unwrap();
            let id = batch.view().id;
            let mut transport = RecordingTransport {
                storage_full_for: publication_index.is_none().then(|| "B".into()),
                ..Default::default()
            };
            let fault = publication_index.map(|index| {
                myalbuns_paths::test_support::DiskFull::on_rename(
                    &root.path().join(format!("source/B/B_{:03}.png", index + 1)),
                )
            });
            let paused = tauri::async_runtime::block_on(batch.run(
                &mut transport,
                &BatchCancellation::default(),
                ExportConflictPolicy::Ask,
                &|_| {},
            ))
            .unwrap();
            assert_eq!(transport.names, ["A", "B"], "disk full must stop before C");
            assert_eq!(paused.view().phase, BatchPhase::StorageFull);
            assert_eq!(
                paused.view().partial_publication,
                publication_index == Some(1)
            );
            assert_eq!(paused.view().items[0].status, BatchItemStatus::Completed);
            assert_eq!(paused.view().items[1].status, BatchItemStatus::Pending);
            assert_eq!(paused.view().items[2].status, BatchItemStatus::Pending);
            assert!(
                paused.view().items[1].problems.is_empty(),
                "disk full is not an item problem"
            );
            if let Some(fault) = &fault {
                assert_eq!(fault.failure_count(), 1);
            }
            drop(fault);
            assert_eq!(
                BatchRunner::recoveries(&checkpoints).unwrap()[0].remaining,
                2
            );
            let resumed = if live {
                let mut paused = paused;
                paused.retry_preflight();
                paused
            } else {
                drop(paused);
                BatchRunner::resume(&checkpoints, &id, core).unwrap()
            };
            let mut transport = RecordingTransport::default();
            let completed = tauri::async_runtime::block_on(resumed.run(
                &mut transport,
                &BatchCancellation::default(),
                ExportConflictPolicy::Replace,
                &|_| {},
            ))
            .unwrap();
            assert_eq!(
                transport.names,
                if live && publication_index.is_some() {
                    vec!["C"]
                } else {
                    vec!["B", "C"]
                }
            );
            assert!(
                completed
                    .view()
                    .items
                    .iter()
                    .all(|item| item.status == BatchItemStatus::Completed)
            );
            assert!(BatchRunner::recoveries(&checkpoints).unwrap().is_empty());
        }
    }
}

#[test]
fn disk_full_in_the_next_album_does_not_reuse_previous_partial_publication_notice() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/B.myalbuns");
    let batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    let first_fault =
        myalbuns_paths::test_support::DiskFull::on_rename(&root.path().join("source/A/A_002.png"));
    let mut first_transport = RecordingTransport::default();
    let mut paused = tauri::async_runtime::block_on(batch.run(
        &mut first_transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(first_fault.failure_count(), 1);
    assert_eq!(first_transport.names, ["A"]);
    assert!(paused.view().partial_publication);
    assert!(root.path().join("source/A/A_001.png").is_file());
    assert!(!root.path().join("source/A/A_002.png").exists());
    drop(first_fault);

    paused.retry_preflight();
    let second_destination = root.path().join("source/B");
    let second_fault =
        myalbuns_paths::test_support::DiskFullAcrossThreads::on_create(&second_destination);
    let mut resumed_transport = RecordingTransport::default();
    let paused_again = tauri::async_runtime::block_on(paused.run(
        &mut resumed_transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(second_fault.failure_count(), 1);
    assert!(resumed_transport.names.is_empty());
    assert!(root.path().join("source/A/A_002.png").is_file());
    assert!(!second_destination.exists());
    let view = paused_again.view();
    assert_eq!(view.phase, BatchPhase::StorageFull);
    assert_eq!(view.items[0].status, BatchItemStatus::Completed);
    assert_eq!(view.items[1].status, BatchItemStatus::Pending);
    assert!(view.items[1].problems.is_empty());
    assert!(
        !view.partial_publication,
        "the next album has not published anything and must not inherit the previous album's notice"
    );
}

#[test]
fn disk_full_checkpoint_keeps_live_progress_and_the_previous_atomic_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    let checkpoints = root.path().join("checkpoints");
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        checkpoints.clone(),
    )
    .unwrap();
    batch.save_checkpoint().unwrap();
    let path = checkpoints.join(format!("{}.json", batch.id));
    let before = std::fs::read(&path).unwrap();
    batch.items[0].status = BatchItemStatus::Completed;
    let fault = myalbuns_paths::test_support::DiskFull::on_create(&path);
    batch.checkpoint_or_pause().unwrap();
    assert_eq!(fault.failure_count(), 1);
    assert_eq!(batch.view().phase, BatchPhase::StorageFull);
    assert_eq!(batch.view().items[0].status, BatchItemStatus::Completed);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(fault);
    batch.retry_preflight();
    let mut transport = RecordingTransport::default();
    let finished = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert!(
        transport.names.is_empty(),
        "retaking the live runner must not export completed items again"
    );
    assert_eq!(finished.view().phase, BatchPhase::Finished);
    assert!(!path.exists());
}

#[test]
fn disk_full_live_retry_does_not_revive_an_ignored_project_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/B.myalbuns");
    let (_, third) = fixture(root.path(), "source/C.myalbuns");
    let third_bytes = std::fs::read(third.project_path()).unwrap();
    let checkpoints = root.path().join("checkpoints");
    let batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core.clone(),
        checkpoints.clone(),
    )
    .unwrap();
    let id = batch.id.clone();
    let mut transport = RecordingTransport {
        storage_full_for: Some("B".into()),
        ..Default::default()
    };
    let mut paused = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    paused.retry_preflight();
    paused.ignore(&paused.items[1].id.clone()).unwrap();
    std::fs::write(third.project_path(), b"changed after preflight").unwrap();
    let finished = tauri::async_runtime::block_on(paused.run(
        &mut RecordingTransport::default(),
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(finished.items[1].status, BatchItemStatus::Ignored);
    assert_eq!(finished.items[2].status, BatchItemStatus::Failed);
    std::fs::write(third.project_path(), third_bytes).unwrap();
    let recovered = BatchRunner::resume(&checkpoints, &id, core).unwrap();
    assert_eq!(recovered.items[1].status, BatchItemStatus::Ignored);
}

#[test]
fn disk_full_pause_keeps_temporary_relinks_for_an_explicit_live_retry() {
    let root = tempfile::tempdir().unwrap();
    let (core, editor, original) = background_fixture(root.path(), "A");
    let before = std::fs::read(editor.project_path()).unwrap();
    let replacement = root.path().join("relocated");
    std::fs::create_dir_all(&replacement).unwrap();
    std::fs::rename(&original, replacement.join("001.jpg")).unwrap();
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    batch
        .relink(&batch.items[0].id.clone(), &replacement)
        .unwrap();
    let mut full = RecordingTransport {
        storage_full_for: Some("A".into()),
        ..Default::default()
    };
    let mut paused = tauri::async_runtime::block_on(batch.run(
        &mut full,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    paused.retry_preflight();
    assert!(
        paused.view().can_continue,
        "a live pause preserves temporary relinks"
    );
    let mut transport = RecordingTransport::default();
    tauri::async_runtime::block_on(paused.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Replace,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(transport.sources, [vec![replacement.join("001.jpg")]]);
    assert_eq!(std::fs::read(editor.project_path()).unwrap(), before);
}

#[test]
fn exports_items_serially_and_keeps_explicitly_ignored_problems_in_the_result() {
    let root = tempfile::tempdir().unwrap();
    let (core, mut editor) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/nested/B.myalbuns");
    editor
        .apply(myalbuns_core::ProjectIntent::SetDpi { dpi: 240 })
        .unwrap();
    std::fs::write(root.path().join("source/Invalid.myalbuns"), b"invalid").unwrap();
    let checkpoint_root = root.path().join("checkpoints");
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        checkpoint_root.clone(),
    )
    .unwrap();
    let invalid = batch
        .view()
        .items
        .iter()
        .find(|item| item.name == "Invalid")
        .unwrap()
        .id
        .clone();
    assert!(!batch.view().can_continue);
    batch.ignore(&invalid).unwrap();
    assert!(batch.view().can_continue);
    let mut transport = RecordingTransport::default();
    let batch = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(transport.names, ["A", "B"]);
    assert_eq!(
        transport.dpis,
        [300, 300],
        "unsaved editor changes are ignored"
    );
    assert_eq!(batch.view().phase, BatchPhase::Finished);
    assert_eq!(
        batch
            .view()
            .items
            .iter()
            .filter(|item| item.status == BatchItemStatus::Completed)
            .count(),
        2
    );
    let ignored = batch
        .view()
        .items
        .into_iter()
        .find(|item| item.id == invalid)
        .unwrap();
    assert_eq!(ignored.status, BatchItemStatus::Ignored);
    assert!(!ignored.problems.is_empty());
    assert!(
        BatchRunner::recoveries(&checkpoint_root)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read(root.path().join("source/A/A_001.png")).unwrap(),
        b"A-0"
    );
    assert_eq!(
        std::fs::read(root.path().join("source/nested/B/B_002.png")).unwrap(),
        b"B-1"
    );
    assert!(editor.has_unsaved_changes());
}

#[test]
fn cancellation_between_items_resumes_only_pending_projects_with_a_new_preflight() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/B.myalbuns");
    let checkpoints = root.path().join("checkpoints");
    let batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core.clone(),
        checkpoints.clone(),
    )
    .unwrap();
    let id = batch.view().id;
    let cancel = BatchCancellation::default();
    let mut transport = RecordingTransport::default();
    let interrupted = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &cancel,
        ExportConflictPolicy::Ask,
        &|progress| {
            if progress.completed == 1 {
                cancel.request();
            }
        },
    ))
    .unwrap();
    assert_eq!(transport.names, ["A"]);
    assert_eq!(interrupted.view().phase, BatchPhase::Interrupted);
    let recoveries = BatchRunner::recoveries(&checkpoints).unwrap();
    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0].remaining, 1);
    drop(interrupted);
    let resumed = BatchRunner::resume(&checkpoints, &id, core).unwrap();
    assert!(resumed.view().can_continue);
    let mut second_transport = RecordingTransport::default();
    let finished = tauri::async_runtime::block_on(resumed.run(
        &mut second_transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(second_transport.names, ["B"]);
    assert_eq!(finished.view().phase, BatchPhase::Finished);
    assert!(BatchRunner::recoveries(&checkpoints).unwrap().is_empty());
}

#[test]
#[ignore = "invoked by the isolated batch crash test"]
fn batch_crash_child() {
    let root = PathBuf::from(
        std::env::var_os("MYALBUNS_BATCH_CRASH_ROOT").expect("isolated crash fixture"),
    );
    let phase = std::env::var("MYALBUNS_BATCH_CRASH_PHASE").unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.join("checkpoints"),
    )
    .unwrap();
    let mut transport = RecordingTransport {
        crash_during_preparation: phase == "prepare",
        ..Default::default()
    };
    tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Replace,
        &|_| {
            let first = std::fs::read(root.join("source/A/A_001.png")).ok();
            let second = std::fs::read(root.join("source/A/A_002.png")).ok();
            let first_is_new = first.as_deref() == Some(b"A-0");
            let second_is_new = second.as_deref() == Some(b"A-1");
            if (phase == "publish" && first_is_new && !second_is_new)
                || (phase == "published"
                    && first_is_new
                    && second_is_new
                    && !root.join("source/A/A_003.png").exists())
            {
                std::process::exit(91);
            }
        },
    ))
    .unwrap();
    panic!("the crash trigger was not reached");
}

#[test]
fn process_exit_before_during_and_after_publication_replays_the_whole_interrupted_item() {
    for (phase, ignore_recovered) in [
        ("prepare", false),
        ("publish", false),
        ("published", false),
        ("prepare", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (core, _first) = fixture(root.path(), "source/A.myalbuns");
        let (_, _second) = fixture(root.path(), "source/B.myalbuns");
        let destination = root.path().join("source/A");
        std::fs::create_dir(&destination).unwrap();
        for (name, bytes) in [
            ("A_001.png", "old-first"),
            ("A_002.png", "old-second"),
            ("A_003.png", "orphan"),
        ] {
            std::fs::write(destination.join(name), bytes).unwrap();
        }
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "batch_runner::tests::batch_crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("MYALBUNS_BATCH_CRASH_ROOT", root.path())
            .env("MYALBUNS_BATCH_CRASH_PHASE", phase);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            child.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        let result = child.output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(91),
            "{phase}: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(destination.join("A_003.png").exists(), phase != "published");
        let checkpoint_root = root.path().join("checkpoints");
        let summaries = BatchRunner::recoveries(&checkpoint_root).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].remaining, 2);
        let mut resumed = BatchRunner::resume(&checkpoint_root, &summaries[0].id, core).unwrap();
        if ignore_recovered {
            resumed.ignore(&resumed.view().items[0].id).unwrap();
        }
        let mut transport = RecordingTransport::default();
        let completed = tauri::async_runtime::block_on(resumed.run(
            &mut transport,
            &BatchCancellation::default(),
            ExportConflictPolicy::Replace,
            &|_| {},
        ))
        .unwrap();
        assert_eq!(
            transport.names,
            if ignore_recovered {
                vec!["B"]
            } else {
                vec!["A", "B"]
            }
        );
        assert!(completed.view().items.iter().all(|item| matches!(
            item.status,
            BatchItemStatus::Completed | BatchItemStatus::Ignored
        )));
        assert_eq!(
            std::fs::read_dir(&destination).unwrap().count(),
            if ignore_recovered { 3 } else { 2 },
            "no interrupted preparation or orphan remains"
        );
        if ignore_recovered {
            assert_eq!(
                std::fs::read(destination.join("A_001.png")).unwrap(),
                b"old-first"
            );
            assert_eq!(
                std::fs::read(destination.join("A_003.png")).unwrap(),
                b"orphan"
            );
        }
        assert!(
            BatchRunner::recoveries(&checkpoint_root)
                .unwrap()
                .is_empty()
        );
    }
}

pub(crate) fn background_fixture(
    root: &Path,
    name: &str,
) -> (ProjectCore, myalbuns_core::EditableProject, PathBuf) {
    use myalbuns_core::{
        InitialBackground, InitialBackgroundContent, InitialFrameBorder, InitialOverlay,
        InitialProjectPersonalization,
    };
    let path = root.join("source").join(format!("{name}.myalbuns"));
    let original = root.join("originals").join(name).join("001.jpg");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(original.parent().unwrap()).unwrap();
    image::RgbImage::new(2, 2).save(&original).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let mut paths = OperationPathContext::new();
    paths.capture(&path).unwrap();
    let project = core
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(path, paths.freeze()),
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: original.clone(),
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    (core, project, original)
}

#[test]
fn global_relink_respects_a_prior_claim_through_another_file_alias() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first, original_a) = background_fixture(root.path(), "A");
    let (_, _second, original_b) = background_fixture(root.path(), "B");
    let individual = root.path().join("individual");
    let global = root.path().join("global");
    std::fs::create_dir(&individual).unwrap();
    std::fs::create_dir_all(global.join("B")).unwrap();
    let replacement = individual.join("001.jpg");
    std::fs::copy(&original_a, &replacement).unwrap();
    std::fs::hard_link(&replacement, global.join("B/001.jpg")).unwrap();
    std::fs::remove_file(original_a).unwrap();
    std::fs::remove_file(original_b).unwrap();
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    let first_id = batch.view().items[0].id.clone();
    batch.relink(&first_id, &individual).unwrap();
    batch.relink_all(&global).unwrap();
    let view = batch.view();
    assert!(view.items[0].problems.is_empty());
    assert!(
        view.items[1]
            .problems
            .iter()
            .any(|problem| problem.kind == BatchProblemKind::MissingMedia)
    );
}

#[test]
fn changed_persisted_revision_fails_only_that_item_and_an_explicit_recheck_uses_the_saved_revision()
{
    for same_revision_rewrite in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (core, mut editor) = fixture(root.path(), "source/A.myalbuns");
        let (_, _second) = fixture(root.path(), "source/B.myalbuns");
        let batch = BatchRunner::discover(
            BatchConfiguration {
                source: root.path().join("source"),
                destination: None,
                format: ExportFormat::Png,
                mode: ExportMode::Sheet,
            },
            core,
            root.path().join("checkpoints"),
        )
        .unwrap();
        if same_revision_rewrite {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(editor.project_path())
                .unwrap()
                .write_all(b"\n ")
                .unwrap();
        } else {
            editor
                .apply(myalbuns_core::ProjectIntent::SetDpi { dpi: 240 })
                .unwrap();
            editor.save(editor.revision()).unwrap();
        }
        let mut transport = RecordingTransport::default();
        let mut batch = tauri::async_runtime::block_on(batch.run(
            &mut transport,
            &BatchCancellation::default(),
            ExportConflictPolicy::Ask,
            &|_| {},
        ))
        .unwrap();
        assert_eq!(transport.names, ["B"]);
        let failed = batch
            .view()
            .items
            .into_iter()
            .find(|item| item.name == "A")
            .unwrap();
        assert_eq!(failed.status, BatchItemStatus::Failed);
        assert!(
            failed
                .problems
                .iter()
                .any(|problem| problem.kind == BatchProblemKind::Changed)
        );
        batch.recheck();
        assert!(batch.view().can_continue);
        tauri::async_runtime::block_on(batch.run(
            &mut transport,
            &BatchCancellation::default(),
            ExportConflictPolicy::Ask,
            &|_| {},
        ))
        .unwrap();
        assert_eq!(transport.names, ["B", "A"]);
        assert_eq!(
            transport.dpis.last(),
            Some(&(if same_revision_rewrite { 300 } else { 240 }))
        );
    }
}

#[test]
fn global_relink_does_not_share_a_candidate_between_projects_or_match_a_different_folder_case() {
    let root = tempfile::tempdir().unwrap();
    let (core, _a, original_a) = background_fixture(root.path(), "A");
    let (_, _b, original_b) = background_fixture(root.path(), "B");
    let photos = root.path().join("photos");
    std::fs::create_dir_all(photos.join("A")).unwrap();
    std::fs::create_dir_all(photos.join("b")).unwrap();
    std::fs::copy(&original_a, photos.join("A/001.jpg")).unwrap();
    std::fs::copy(&original_b, photos.join("b/001.jpg")).unwrap();
    std::fs::remove_file(original_a).unwrap();
    std::fs::remove_file(original_b).unwrap();
    // A second stored project named A deliberately competes for the global folder.
    let nested = root.path().join("nested-fixture");
    let (_, second_a, second_original) = background_fixture(&nested, "A");
    std::fs::remove_file(second_original).unwrap();
    std::fs::create_dir_all(root.path().join("source/other")).unwrap();
    std::fs::rename(
        second_a.project_path(),
        root.path().join("source/other/A.myalbuns"),
    )
    .unwrap();
    drop(second_a);
    let mut batch = BatchRunner::discover(
        BatchConfiguration {
            source: root.path().join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.path().join("checkpoints"),
    )
    .unwrap();
    batch.relink_all(&photos).unwrap();
    assert!(
        batch
            .view()
            .items
            .iter()
            .all(|item| !item.problems.is_empty())
    );
    let b = batch
        .view()
        .items
        .iter()
        .find(|item| item.name == "B")
        .unwrap()
        .id
        .clone();
    batch.relink(&b, &photos.join("b")).unwrap();
    assert!(
        batch
            .view()
            .items
            .iter()
            .find(|item| item.id == b)
            .unwrap()
            .problems
            .is_empty()
    );
}

#[test]
fn global_relink_requires_unique_exact_names_in_each_project_folder_and_never_saves_the_project() {
    let root = tempfile::tempdir().unwrap();
    let (core, first, original_a) = background_fixture(root.path(), "A");
    let (_, second, original_b) = background_fixture(root.path(), "B");
    let bytes_a = std::fs::read(first.project_path()).unwrap();
    let bytes_b = std::fs::read(second.project_path()).unwrap();
    let photos = root.path().join("photos");
    for folder in ["A/deep", "A/duplicate", "B/deep", "other"] {
        std::fs::create_dir_all(photos.join(folder)).unwrap();
        std::fs::copy(&original_a, photos.join(folder).join("001.jpg")).unwrap();
    }
    std::fs::remove_file(original_a).unwrap();
    std::fs::remove_file(original_b).unwrap();
    let options = BatchConfiguration {
        source: root.path().join("source"),
        destination: None,
        format: ExportFormat::Png,
        mode: ExportMode::Sheet,
    };
    let mut batch = BatchRunner::discover(
        options.clone(),
        core.clone(),
        root.path().join("checkpoints"),
    )
    .unwrap();
    batch.relink_all(&photos).unwrap();
    let initial = batch.view();
    assert!(!initial.can_continue);
    assert!(
        !initial
            .items
            .iter()
            .find(|item| item.name == "A")
            .unwrap()
            .problems
            .is_empty()
    );
    assert!(
        initial
            .items
            .iter()
            .find(|item| item.name == "B")
            .unwrap()
            .problems
            .is_empty()
    );
    std::fs::remove_file(photos.join("A/duplicate/001.jpg")).unwrap();
    batch.relink_all(&photos).unwrap();
    assert!(batch.view().can_continue);
    let mut transport = RecordingTransport::default();
    tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(
        transport.sources,
        [
            vec![photos.join("A/deep/001.jpg")],
            vec![photos.join("B/deep/001.jpg")]
        ]
    );
    assert_eq!(std::fs::read(first.project_path()).unwrap(), bytes_a);
    assert_eq!(std::fs::read(second.project_path()).unwrap(), bytes_b);
    assert!(!first.has_unsaved_changes());
    assert!(!second.has_unsaved_changes());
    let next = BatchRunner::discover(options, core, root.path().join("checkpoints")).unwrap();
    assert!(
        !next.view().can_continue,
        "temporary links do not survive a batch"
    );
}

#[test]
#[ignore = "executed with a matching Processor by the integration gates"]
fn real_processor_exports_persisted_batches_in_every_format() {
    use crate::{
        cache_engine::CacheEngine, imaging_processor::ImagingProcessor,
        imaging_recovery_integration::RealProcessTransport, operation_gate::OperationGate,
        operation_lease::OperationLease,
    };
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os("MYALBUNS_TEST_IMAGING_PROCESSOR")
                .or_else(|| std::env::var_os("MYALBUNS_REAL_IMAGING_PROCESSOR"))
                .expect("the integration gate provides a matching Processor"),
        );
        let root = tempfile::tempdir().unwrap();
        let (core, mut first, _) = background_fixture(root.path(), "A");
        let (_, mut second, _) = background_fixture(root.path(), "B");
        for editor in [&mut first, &mut second] {
            editor
                .apply(myalbuns_core::ProjectIntent::SetDpi { dpi: 72 })
                .unwrap();
            editor.save(editor.revision()).unwrap();
        }
        let paths = myalbuns_paths::AppPaths::from_roots(
            &root.path().join("roaming"),
            &root.path().join("local"),
        );
        for (format, extension) in [
            (ExportFormat::Jpeg { quality: 100 }, "jpg"),
            (ExportFormat::Png, "png"),
            (ExportFormat::Pdf, "pdf"),
        ] {
            let output = root.path().join(extension);
            let logs = root.path().join(format!("logs-{extension}"));
            std::fs::create_dir_all(&logs).unwrap();
            let batch = BatchRunner::discover(
                BatchConfiguration {
                    source: root.path().join("source"),
                    destination: Some(output.clone()),
                    format,
                    mode: ExportMode::Sheet,
                },
                core.clone(),
                root.path().join("checkpoints"),
            )
            .unwrap();
            assert!(batch.view().can_continue);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let gate = OperationGate::new(&paths);
            let lease = OperationLease::acquire(&gate, &cache, &processor)
                .await
                .unwrap();
            let mut transport = RealProcessTransport::in_data_root(executable.clone(), logs);
            let result = batch
                .run(
                    &mut transport,
                    &BatchCancellation::default(),
                    ExportConflictPolicy::Ask,
                    &|_| {
                        assert!(
                            gate.try_acquire().is_err(),
                            "one grant stays held between all items"
                        );
                    },
                )
                .await
                .unwrap();
            assert!(
                result
                    .view()
                    .items
                    .iter()
                    .all(|item| item.status == BatchItemStatus::Completed),
                "{:?}",
                result.view()
            );
            for name in ["A", "B"] {
                let directory = output.join(name);
                let files = std::fs::read_dir(&directory)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect::<Vec<_>>();
                assert_eq!(files.len(), if extension == "pdf" { 1 } else { 2 });
                for path in files {
                    if extension == "pdf" {
                        assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF"));
                    } else {
                        let image = image::open(&path).unwrap();
                        assert!(image.width() > 0 && image.height() > 0);
                    }
                }
            }
            drop(lease);
            gate.try_acquire().unwrap();
        }
    });
}

const UNCONFIRMED_TERMINATION: &str = "injected unconfirmed termination";

fn discovered(root: &Path, core: ProjectCore) -> BatchRunner {
    BatchRunner::discover(
        BatchConfiguration {
            source: root.join("source"),
            destination: None,
            format: ExportFormat::Png,
            mode: ExportMode::Sheet,
        },
        core,
        root.join("checkpoints"),
    )
    .unwrap()
}

fn preparation_directories(destination: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(destination)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".myalbuns-export-")
        })
        .collect()
}

#[test]
fn cancellation_during_an_item_publishes_nothing_and_replays_that_item_on_resume() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first) = fixture(root.path(), "source/A.myalbuns");
    let (_, _second) = fixture(root.path(), "source/B.myalbuns");
    let checkpoints = root.path().join("checkpoints");
    let batch = discovered(root.path(), core.clone());
    let id = batch.view().id;
    let cancel = BatchCancellation::default();
    let mut transport = RecordingTransport::default();
    let interrupted = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &cancel,
        ExportConflictPolicy::Ask,
        &|progress| {
            // Only the Processor rendering the current item moves the percentage
            // while no item has been recorded yet.
            if progress.completed == 0 && progress.percent > 0.0 {
                cancel.request();
            }
        },
    ))
    .unwrap();
    assert_eq!(transport.names, ["A"], "B must not start after the cancel");
    let view = interrupted.view();
    assert_eq!(view.phase, BatchPhase::Interrupted);
    assert_eq!(view.items[0].status, BatchItemStatus::Pending);
    assert!(
        !view.items[0].problems.is_empty(),
        "the interrupted project says it will be redone"
    );
    assert_eq!(view.items[1].status, BatchItemStatus::Pending);
    assert!(view.items[1].problems.is_empty());
    assert_eq!(
        std::fs::read_dir(root.path().join("source/A"))
            .unwrap()
            .count(),
        0,
        "the cancelled item publishes nothing and discards its preparation"
    );
    assert_eq!(
        BatchRunner::recoveries(&checkpoints).unwrap()[0].remaining,
        2
    );
    drop(interrupted);

    let resumed = BatchRunner::resume(&checkpoints, &id, core).unwrap();
    let mut second_transport = RecordingTransport::default();
    let finished = tauri::async_runtime::block_on(resumed.run(
        &mut second_transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(second_transport.names, ["A", "B"]);
    assert_eq!(finished.view().phase, BatchPhase::Finished);
    assert_eq!(
        std::fs::read(root.path().join("source/A/A_001.png")).unwrap(),
        b"A-0"
    );
}

/// A is published, B stops with an unconfirmed Processor termination and C never starts.
fn interrupted_by_unconfirmed_termination(root: &Path) -> BatchRunner {
    let (core, _first) = fixture(root, "source/A.myalbuns");
    let (_, _second) = fixture(root, "source/B.myalbuns");
    let (_, _third) = fixture(root, "source/C.myalbuns");
    let batch = discovered(root, core);
    let mut transport = RecordingTransport {
        termination_unconfirmed_for: Some("B".into()),
        ..Default::default()
    };
    let interrupted = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ))
    .unwrap();
    assert_eq!(
        transport.names,
        ["A", "B"],
        "a Processor that may still be running must stop the batch before C"
    );
    interrupted
}

#[test]
fn unconfirmed_processor_termination_interrupts_the_batch_and_keeps_the_item_pending() {
    let root = tempfile::tempdir().unwrap();
    let interrupted = interrupted_by_unconfirmed_termination(root.path());
    let view = interrupted.view();
    assert_eq!(view.phase, BatchPhase::Interrupted);
    assert_eq!(view.items[0].status, BatchItemStatus::Completed);
    assert_eq!(view.items[1].status, BatchItemStatus::Pending);
    assert_eq!(view.items[1].problems.len(), 1);
    assert_eq!(view.items[1].problems[0].message, UNCONFIRMED_TERMINATION);
    assert_eq!(view.items[2].status, BatchItemStatus::Pending);
    assert!(view.items[2].problems.is_empty());
    assert!(!root.path().join("source/B/B_001.png").exists());
    assert_eq!(
        preparation_directories(&root.path().join("source/B")).len(),
        1,
        "files a live Processor may still write are preserved for recovery"
    );
    let recoveries = BatchRunner::recoveries(&root.path().join("checkpoints")).unwrap();
    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0].remaining, 2);
}

#[test]
fn abandoning_a_batch_ends_its_recovery_and_cleans_only_its_preparation() {
    for cleanup_preparation in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let interrupted = interrupted_by_unconfirmed_termination(root.path());
        let project = std::fs::read(root.path().join("source/B.myalbuns")).unwrap();

        interrupted.abandon(cleanup_preparation).unwrap();

        assert!(
            BatchRunner::recoveries(&root.path().join("checkpoints"))
                .unwrap()
                .is_empty(),
            "an abandoned batch is never offered for recovery again"
        );
        assert_eq!(
            preparation_directories(&root.path().join("source/B")).len(),
            usize::from(!cleanup_preparation)
        );
        assert_eq!(
            std::fs::read(root.path().join("source/A/A_001.png")).unwrap(),
            b"A-0",
            "published outputs are never touched"
        );
        assert_eq!(
            std::fs::read(root.path().join("source/B.myalbuns")).unwrap(),
            project
        );
    }
}

#[test]
fn run_refuses_to_start_while_a_pending_project_has_problems() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    std::fs::write(root.path().join("source/Invalid.myalbuns"), b"invalid").unwrap();
    let batch = discovered(root.path(), core);
    assert!(!batch.view().can_continue);
    let mut transport = RecordingTransport::default();
    let refused = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Replace,
        &|_| {},
    ));
    assert!(refused.is_err());
    assert!(transport.names.is_empty());
    assert!(!root.path().join("source/A").exists());
    assert!(
        BatchRunner::recoveries(&root.path().join("checkpoints"))
            .unwrap()
            .is_empty(),
        "a refused run records no recovery"
    );
}

#[test]
fn run_refuses_the_ask_policy_while_a_pending_project_has_conflicts() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    let existing = root.path().join("source/A/A_001.png");
    std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
    std::fs::write(&existing, b"previous export").unwrap();
    let batch = discovered(root.path(), core);
    let view = batch.view();
    assert!(view.can_continue && view.has_conflicts);
    let mut transport = RecordingTransport::default();
    let refused = tauri::async_runtime::block_on(batch.run(
        &mut transport,
        &BatchCancellation::default(),
        ExportConflictPolicy::Ask,
        &|_| {},
    ));
    assert!(
        refused.is_err(),
        "existing files need the user's choice before any project is exported"
    );
    assert!(transport.names.is_empty());
    assert_eq!(std::fs::read(&existing).unwrap(), b"previous export");
    assert_eq!(
        std::fs::read_dir(existing.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
    assert!(
        BatchRunner::recoveries(&root.path().join("checkpoints"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn resume_rejects_a_corrupt_tampered_or_incompatible_checkpoint() {
    use serde_json::{Value, json};
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    let checkpoints = root.path().join("checkpoints");
    let batch = discovered(root.path(), core.clone());
    let id = batch.id.clone();
    batch.save_checkpoint().unwrap();
    let file = checkpoints.join(format!("{id}.json"));
    let valid = std::fs::read(&file).unwrap();
    let resumes = |bytes: &[u8]| {
        std::fs::write(&file, bytes).unwrap();
        BatchRunner::resume(&checkpoints, &id, core.clone()).is_ok()
    };
    let tampered = |change: &dyn Fn(&mut Value)| {
        let mut value: Value = serde_json::from_slice(&valid).unwrap();
        change(&mut value);
        resumes(&serde_json::to_vec(&value).unwrap())
    };
    let native =
        |path: PathBuf| serde_json::to_value(myalbuns_paths::NativePathDto::from(path)).unwrap();
    let source = root.path().join("source");
    let preparation = |folder: &str| {
        json!({
            "requestId": "batch-interrupted",
            "path": native(source.join("A").join(folder)),
        })
    };
    assert!(resumes(&valid), "the untouched checkpoint is the baseline");
    {
        let mut value: Value = serde_json::from_slice(&valid).unwrap();
        value["items"][0]["preparation"] = preparation(".myalbuns-export-batch-interrupted.tmp");
        std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
        BatchRunner::resume(&checkpoints, &id, core.clone())
            .map(drop)
            .expect("a preparation folder derived from its request is accepted");
    }

    let other_id = uuid::Uuid::new_v4().to_string();
    type Tamper<'a> = &'a dyn Fn(&mut Value);
    let changes: [(&str, Tamper); 13] = [
        ("another version", &|value| value["version"] = json!(2)),
        ("an id that is not the file name", &|value| {
            value["id"] = json!(other_id)
        }),
        ("no items", &|value| value["items"] = json!([])),
        ("an unknown field", &|value| value["extra"] = json!(true)),
        ("an unknown item status", &|value| {
            value["items"][0]["status"] = json!("exploded")
        }),
        ("an item id that is not a UUID", &|value| {
            value["items"][0]["id"] = json!("first")
        }),
        ("a repeated item", &|value| {
            let item = value["items"][0].clone();
            value["items"].as_array_mut().unwrap().push(item);
        }),
        ("a project outside the source folder", &|value| {
            value["items"][0]["path"] = native(root.path().join("elsewhere").join("A.myalbuns"));
        }),
        ("a file that is not a project", &|value| {
            value["items"][0]["path"] = native(source.join("A.txt"));
        }),
        ("a relative source folder", &|value| {
            value["source"] = native(PathBuf::from("source"));
            value["items"][0]["path"] = native(PathBuf::from("source").join("A.myalbuns"));
        }),
        ("a relative destination folder", &|value| {
            value["destination"] = native(PathBuf::from("delivery"))
        }),
        ("an interrupted item that is not in the batch", &|value| {
            value["current"] = json!(other_id)
        }),
        ("a preparation folder chosen by the file", &|value| {
            value["items"][0]["preparation"] = preparation("A_001.png");
        }),
    ];
    let mut accepted = changes
        .iter()
        .filter(|(_, change)| tampered(change))
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    for (name, bytes) in [
        ("truncated JSON", &valid[..valid.len() / 2]),
        ("bytes that are not JSON", b"\0\0not json".as_slice()),
        ("an empty file", b"".as_slice()),
    ] {
        if resumes(bytes) {
            accepted.push(name);
        }
    }
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    if BatchRunner::resume(&checkpoints, &id, core.clone()).is_ok() {
        accepted.push("a folder in place of the file");
    }
    std::fs::remove_dir(&file).unwrap();
    std::fs::write(&file, &valid).unwrap();
    if BatchRunner::resume(&checkpoints, &id.to_uppercase(), core.clone()).is_ok() {
        accepted.push("an id that is not in canonical form");
    }
    assert!(accepted.is_empty(), "accepted: {accepted:?}");
    assert!(
        BatchRunner::resume(&checkpoints, &id, core).is_ok(),
        "the restored checkpoint still resumes"
    );
}

#[test]
fn one_corrupt_checkpoint_file_hides_every_batch_recovery() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/A.myalbuns");
    let checkpoints = root.path().join("checkpoints");
    for _ in 0..2 {
        discovered(root.path(), core.clone())
            .save_checkpoint()
            .unwrap();
    }
    assert_eq!(BatchRunner::recoveries(&checkpoints).unwrap().len(), 2);

    // Files that are not `.json` are not checkpoints and are skipped.
    std::fs::write(checkpoints.join("notes.txt"), b"not a checkpoint").unwrap();
    assert_eq!(BatchRunner::recoveries(&checkpoints).unwrap().len(), 2);

    let corrupt = checkpoints.join(format!("{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&corrupt, b"{ truncated").unwrap();
    assert!(
        BatchRunner::recoveries(&checkpoints).is_err(),
        "today the two valid recoveries are not listed either"
    );
    std::fs::remove_file(&corrupt).unwrap();

    // The same happens for any `.json` whose name is not a batch id.
    let stray = checkpoints.join("notes.json");
    std::fs::write(&stray, b"{}").unwrap();
    assert!(BatchRunner::recoveries(&checkpoints).is_err());
    std::fs::remove_file(&stray).unwrap();
    assert_eq!(BatchRunner::recoveries(&checkpoints).unwrap().len(), 2);
}

#[test]
fn a_project_reached_through_two_hard_links_is_discovered_once() {
    let root = tempfile::tempdir().unwrap();
    let (core, editor) = fixture(root.path(), "source/A.myalbuns");
    std::fs::create_dir(root.path().join("source/copies")).unwrap();
    std::fs::hard_link(
        editor.project_path(),
        root.path().join("source/copies/Alias.myalbuns"),
    )
    .unwrap();
    // A separate file with identical bytes is another project file, not an alias.
    std::fs::create_dir(root.path().join("source/other")).unwrap();
    std::fs::copy(
        editor.project_path(),
        root.path().join("source/other/Copy.myalbuns"),
    )
    .unwrap();

    let batch = discovered(root.path(), core);

    let mut names = batch
        .view()
        .items
        .into_iter()
        .map(|item| item.name)
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"Copy".to_owned()), "{names:?}");
    assert_eq!(
        names.iter().filter(|name| *name != "Copy").count(),
        1,
        "the two links to one file are a single item: {names:?}"
    );
}

#[cfg(windows)]
#[test]
fn a_folder_that_contains_a_junction_to_its_ancestor_is_walked_once() {
    use std::os::windows::process::CommandExt;
    let root = tempfile::tempdir().unwrap();
    let (core, _editor) = fixture(root.path(), "source/real/A.myalbuns");
    // The junction leads back to the folder being walked.
    let junction = root.path().join("source").join("real").join("again");
    let created = std::process::Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(root.path().join("source"))
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .output()
        .expect("the junction command starts");
    assert!(
        created.status.success(),
        "the junction is created: stdout={}, stderr={}",
        String::from_utf8_lossy(&created.stdout),
        String::from_utf8_lossy(&created.stderr)
    );

    let found = discover_projects(
        &root.path().join("source"),
        &mut OperationPathContext::new(),
    );
    std::fs::remove_dir(&junction).expect("the junction is removed");

    assert_eq!(
        found.unwrap().len(),
        1,
        "the cycle ends at the folder already visited"
    );
    drop(core);
}
