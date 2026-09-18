use super::*;
use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, LoadProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;
use std::{path::Path, sync::atomic::AtomicBool};

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

#[test]
fn conflicts_require_explicit_decisions_and_recheck_after_closing_a_project() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("origem");
    let destination = root.path().join("destino");
    std::fs::create_dir_all(source.join("001")).unwrap();
    std::fs::create_dir(&destination).unwrap();
    image::RgbImage::new(8, 6)
        .save(source.join("001/foto.png"))
        .unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let model = core
        .create_editable(CreateProjectRequest::new(
            location(&root.path().join("modelo.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let existing_path = destination.join("001.myalbuns");
    let existing = core
        .create_editable(CreateProjectRequest::new(
            location(&existing_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let original = std::fs::read(&existing_path).unwrap();
    let mut batch = GenerationRunner::prepare(
        GenerationOptions {
            source_folder: source.to_string_lossy().into(),
            destination_folder: destination.to_string_lossy().into(),
        },
        model.freeze_template().unwrap(),
        core,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(batch.view().items[0].conflict);
    assert!(!batch.view().items[0].can_replace);
    assert!(!batch.view().can_continue);
    batch.decide(None, GenerationDecision::Replace).unwrap();
    assert!(!batch.view().can_continue);
    drop(existing);
    assert!(!batch.view().can_continue);
    batch.recheck(&AtomicBool::new(false)).unwrap();
    assert!(batch.view().items[0].can_replace);
    assert!(!batch.view().can_continue);
    batch.decide(None, GenerationDecision::Ignore).unwrap();
    assert_eq!(
        batch.view().items[0].problems,
        ["Já existe um projeto no destino."]
    );
    batch.decide(None, GenerationDecision::Replace).unwrap();
    assert!(batch.view().can_continue);
    assert_eq!(batch.view().phase, GenerationPhase::Prepared);
    assert_eq!(std::fs::read(&existing_path).unwrap(), original);
    batch.run(&AtomicBool::new(false), &|_| {});
    assert_eq!(
        batch.view().items[0].status,
        GenerationItemStatus::Completed
    );
    assert_ne!(std::fs::read(existing_path).unwrap(), original);
}

#[test]
fn nested_folders_generate_independent_projects_from_the_unsaved_model_after_preflight() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("origem");
    let destination = root.path().join("destino");
    std::fs::create_dir_all(source.join("Turma/001/sub")).unwrap();
    std::fs::create_dir(&destination).unwrap();
    for folder in ["Turma/001", "Turma/001/sub"] {
        image::RgbImage::from_pixel(8, 6, image::Rgb([120, 30, 40]))
            .save(source.join(folder).join("foto.png"))
            .unwrap();
    }
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let model_path = root.path().join("Modelo.myalbuns");
    let mut model = core
        .create_editable(CreateProjectRequest::new(
            location(&model_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    model.apply(ProjectIntent::SetDpi { dpi: 420 }).unwrap();
    let mut batch = GenerationRunner::prepare(
        GenerationOptions {
            source_folder: source.to_string_lossy().into(),
            destination_folder: destination.to_string_lossy().into(),
        },
        model.freeze_template().unwrap(),
        core.clone(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(batch.view().items.len(), 2);
    assert!(batch.view().can_continue);
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
    batch.run(&AtomicBool::new(false), &|_| {});
    assert!(
        batch
            .view()
            .items
            .iter()
            .all(|item| item.status == GenerationItemStatus::Completed)
    );
    let first = core
        .load_persisted_revision(LoadProjectRequest::new(location(
            &destination.join("Turma/001.myalbuns"),
        )))
        .unwrap();
    let second = core
        .load_persisted_revision(LoadProjectRequest::new(location(
            &destination.join("Turma/001/sub.myalbuns"),
        )))
        .unwrap();
    assert_ne!(first.project_id(), second.project_id());
    assert_ne!(first.project_id(), model.project_id());
    assert_eq!(first.project().document().dpi(), 420);
    assert_eq!(first.project().sheets(), model.project().sheets());
    assert_eq!(first.project().media().len(), 1);
    assert_eq!(
        second.project().media()[0].path(),
        source.join("Turma/001/sub/foto.png")
    );
    assert!(model.has_unsaved_changes());
}

struct Fixture {
    _root: tempfile::TempDir,
    source: PathBuf,
    destination: PathBuf,
    core: ProjectCore,
    model: myalbuns_core::EditableProject,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("origem");
        let destination = root.path().join("destino");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&destination).unwrap();
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let model = core
            .create_editable(CreateProjectRequest::new(
                location(&root.path().join("modelo.myalbuns")),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        Self {
            _root: root,
            source,
            destination,
            core,
            model,
        }
    }
    fn photo(&self, folder: &str) -> PathBuf {
        std::fs::create_dir_all(self.source.join(folder)).unwrap();
        let photo = self.source.join(folder).join("foto.png");
        image::RgbImage::new(8, 6).save(&photo).unwrap();
        photo
    }
    fn options(&self) -> GenerationOptions {
        GenerationOptions {
            source_folder: self.source.to_string_lossy().into(),
            destination_folder: self.destination.to_string_lossy().into(),
        }
    }
    fn prepare(&self) -> Result<GenerationRunner, GenerationPreparationError> {
        GenerationRunner::prepare(
            self.options(),
            self.model.freeze_template().unwrap(),
            self.core.clone(),
            &AtomicBool::new(false),
        )
    }
}

#[test]
fn image_content_is_validated_only_when_opening_the_generated_project() {
    let fixture = Fixture::new();
    let invalid = fixture.photo("001");
    std::fs::write(&invalid, "not an image").unwrap();
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    assert!(runner.view().can_continue);
    assert!(
        runner
            .view()
            .items
            .iter()
            .all(|item| item.problems.is_empty())
    );
    assert_eq!(std::fs::read_dir(&fixture.destination).unwrap().count(), 0);
    runner.run(&AtomicBool::new(false), &|_| {});
    assert!(
        runner
            .view()
            .items
            .iter()
            .all(|item| item.status == GenerationItemStatus::Completed)
    );
    let generated = fixture
        .core
        .load_persisted_revision(LoadProjectRequest::new(location(
            &fixture.destination.join("001.myalbuns"),
        )))
        .unwrap();
    let media = &generated.project().media()[0];
    assert_eq!(media.path(), invalid);
    let mut paths = OperationPathContext::new();
    paths.capture(media.path()).unwrap();
    let binding = crate::media_runtime::MediaBinding {
        media_id: media.id().to_string(),
        kind: media.kind(),
        logical_path: media.path().to_path_buf(),
    };
    // This is the same inspector used by project opening and Cache preparation.
    assert!(
        crate::media_runtime::MediaResolver
            .inspect_media_binding_in_plan(&binding, &paths.freeze())
            .is_err()
    );
}

#[test]
fn a_source_removed_after_discovery_remains_linked_for_opening_to_resolve() {
    let fixture = Fixture::new();
    let photo = fixture.photo("001");
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    std::fs::remove_file(&photo).unwrap();
    runner.run(&AtomicBool::new(false), &|_| {});
    assert_eq!(runner.view().phase, GenerationPhase::Finished);
    assert_eq!(
        runner.view().items[0].status,
        GenerationItemStatus::Completed
    );
    assert_eq!(
        runner.view().items[1].status,
        GenerationItemStatus::Completed
    );
    let generated = fixture
        .core
        .load_persisted_revision(LoadProjectRequest::new(location(
            &fixture.destination.join("001.myalbuns"),
        )))
        .unwrap();
    assert_eq!(generated.project().media()[0].path(), photo);
    assert!(fixture.destination.join("002.myalbuns").is_file());
}

#[cfg(windows)]
#[test]
fn generation_does_not_require_read_access_to_the_photo_contents() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let photo = fixture.photo("001");
    let _locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x2 | 0x4)
        .open(&photo)
        .unwrap();
    assert!(
        std::fs::File::open(&photo).is_err(),
        "The fixture must deny other content readers"
    );
    let mut runner = fixture.prepare().unwrap();
    assert!(runner.view().can_continue);
    runner.run(&AtomicBool::new(false), &|_| {});
    assert_eq!(
        runner.view().items[0].status,
        GenerationItemStatus::Completed
    );
    assert!(fixture.destination.join("001.myalbuns").is_file());
}

#[test]
fn cancellation_before_generation_does_not_start_any_project() {
    let fixture = Fixture::new();
    fixture.photo("001");
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    let cancel = AtomicBool::new(true);
    runner.run(&cancel, &|_| {});
    assert_eq!(runner.view().phase, GenerationPhase::Cancelled);
    assert_eq!(runner.view().items[0].status, GenerationItemStatus::Pending);
    assert_eq!(runner.view().items[1].status, GenerationItemStatus::Pending);
    assert!(!fixture.destination.join("001.myalbuns").exists());
    assert!(!fixture.destination.join("002.myalbuns").exists());
}

#[test]
fn parallel_cancellation_stops_admission_and_waits_for_every_active_publication() {
    use std::{sync::mpsc, time::Duration};
    let fixture = Fixture::new();
    for index in 0..8 {
        fixture.photo(&format!("Turma/{index:03}"));
    }
    let mut runner = fixture.prepare().unwrap();
    let operations = crate::project_ui_operations::ProjectUiOperations::default();
    let pause = operations.pause_for_batch().unwrap();
    let lifetime = std::sync::Arc::new(crate::generation_window::lifetime::Lifetime::default());
    let accepted = lifetime.begin().unwrap();
    let cancellation = lifetime.cancel.clone();
    let (started, starts) = mpsc::channel();
    let (updates, progress) = mpsc::channel();
    let (release, waits): (Vec<_>, Vec<_>) = (0..8)
        .map(|_| {
            let (sender, receiver) = mpsc::channel::<()>();
            (sender, Mutex::new(receiver))
        })
        .unzip();
    let timeout = Duration::from_secs(10);
    std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            let _accepted = accepted;
            generate_items(
                &mut runner.items,
                PROJECT_GENERATION_CONCURRENCY,
                &cancellation,
                &|value| {
                    updates.send(value).unwrap();
                },
                |item| {
                    let index: usize = item.name.parse().unwrap();
                    started.send(index).unwrap();
                    waits[index].lock().unwrap().recv_timeout(timeout).unwrap();
                    generate_item(
                        &runner.core,
                        &runner.template,
                        &runner.roots,
                        &runner.options,
                        item,
                    )
                },
            )
        });
        let mut admitted = (0..4)
            .map(|_| starts.recv_timeout(timeout).unwrap())
            .collect::<Vec<_>>();
        admitted.sort_unstable();
        assert_eq!(admitted, [0, 1, 2, 3]);
        assert!(starts.try_recv().is_err());
        assert_eq!(progress.recv_timeout(timeout).unwrap().completed, 0);
        let recovery = pause.recover().unwrap();
        assert!(lifetime.close());
        assert!(lifetime.begin().is_err());
        let retirement_wait = scope.spawn(|| lifetime.wait_until_drained(timeout));
        // Force completion out of discovery order. The remaining admitted item
        // keeps the operation alive even after three results have been reported.
        for (completed, index) in [3, 2, 1].into_iter().enumerate() {
            release[index].send(()).unwrap();
            let update = progress.recv_timeout(timeout).unwrap();
            assert_eq!(update.completed, completed as u32 + 1);
            assert_eq!(update.total, Some(8));
            assert!(!running.is_finished());
            assert!(!retirement_wait.is_finished());
            assert!(lifetime.wait_until_drained(Duration::ZERO).is_err());
            assert!(operations.begin().is_err());
        }
        release[0].send(()).unwrap();
        assert_eq!(progress.recv_timeout(timeout).unwrap().completed, 4);
        assert_eq!(running.join().unwrap(), GenerationPhase::Cancelled);
        retirement_wait.join().unwrap().unwrap();
        lifetime.wait_until_drained(Duration::ZERO).unwrap();
        drop(pause);
        assert!(recovery.is_drained());
        assert!(operations.begin().is_err());
        drop(recovery);
        assert!(operations.begin().is_ok());
    });
    assert!(
        starts.try_recv().is_err(),
        "Cancelled queued items must never start"
    );
    for (index, item) in runner.view().items.iter().enumerate() {
        assert_eq!(item.name, format!("{index:03}"));
        assert_eq!(
            item.status,
            if index < 4 {
                GenerationItemStatus::Completed
            } else {
                GenerationItemStatus::Pending
            }
        );
        assert_eq!(Path::new(&item.destination).is_file(), index < 4);
    }
}

#[test]
fn parallel_generation_shares_parents_isolates_failures_and_reports_each_terminal_item_once() {
    use std::cell::RefCell;
    let fixture = Fixture::new();
    for index in 0..16 {
        fixture.photo(&format!("Turma/{index:03}"));
    }
    std::fs::create_dir(fixture.destination.join("Turma")).unwrap();
    let ignored_path = fixture.destination.join("Turma/000.myalbuns");
    std::fs::write(&ignored_path, "preserve ignored destination").unwrap();
    let mut runner = fixture.prepare().unwrap();
    let ignored_id = runner.view().items[0].id.clone();
    runner
        .decide(Some(&ignored_id), GenerationDecision::Ignore)
        .unwrap();
    assert!(runner.view().can_continue);
    // A destination can change after verification. Only its item may fail.
    std::fs::create_dir(fixture.destination.join("Turma/001.myalbuns")).unwrap();
    let updates = RefCell::new(Vec::new());
    let coordinator = std::thread::current().id();
    runner.run(&AtomicBool::new(false), &|value| {
        assert_eq!(std::thread::current().id(), coordinator);
        assert_eq!(value.total, Some(16));
        updates.borrow_mut().push(value.completed);
    });
    assert_eq!(runner.view().phase, GenerationPhase::Finished);
    assert_eq!(*updates.borrow(), (0..=16).collect::<Vec<_>>());
    assert_eq!(runner.view().items[0].status, GenerationItemStatus::Ignored);
    assert_eq!(
        std::fs::read_to_string(&ignored_path).unwrap(),
        "preserve ignored destination"
    );
    assert_eq!(runner.view().items[1].status, GenerationItemStatus::Failed);
    let mut identities = HashSet::new();
    for item in runner.view().items.iter().skip(2) {
        assert_eq!(
            item.status,
            GenerationItemStatus::Completed,
            "{}: {:?}",
            item.name,
            item.problems
        );
        let generated = fixture
            .core
            .load_persisted_revision(LoadProjectRequest::new(location(Path::new(
                &item.destination,
            ))))
            .unwrap();
        assert!(identities.insert(generated.project_id()));
        assert_ne!(generated.project_id(), fixture.model.project_id());
        assert_eq!(generated.project().media().len(), 1);
    }
}

#[test]
fn cancellation_after_the_last_publication_keeps_the_complete_result() {
    let fixture = Fixture::new();
    fixture.photo("001");
    let mut runner = fixture.prepare().unwrap();
    let cancellation = AtomicBool::new(false);
    runner.run(&cancellation, &|value| {
        if value.completed == 1 {
            cancellation.store(true, Ordering::Release);
        }
    });
    assert_eq!(runner.view().phase, GenerationPhase::Finished);
    assert_eq!(
        runner.view().items[0].status,
        GenerationItemStatus::Completed
    );
}

#[test]
fn unavailable_roots_and_destinations_inside_source_are_rejected_before_any_write() {
    let fixture = Fixture::new();
    fixture.photo("001");
    for target in [&fixture.source, &fixture.source.join("001")] {
        let mut options = fixture.options();
        options.destination_folder = target.to_string_lossy().into();
        assert!(
            GenerationRunner::prepare(
                options,
                fixture.model.freeze_template().unwrap(),
                fixture.core.clone(),
                &AtomicBool::new(false),
            )
            .is_err()
        );
    }
    std::fs::remove_dir(&fixture.destination).unwrap();
    assert!(fixture.prepare().is_err());
    assert!(!fixture.destination.exists());
}

#[test]
fn a_file_blocking_the_mirrored_parent_is_found_in_preflight() {
    let fixture = Fixture::new();
    fixture.photo("Turma/001");
    std::fs::write(fixture.destination.join("Turma"), "preserve").unwrap();
    let runner = fixture.prepare().unwrap();
    assert!(!runner.view().can_continue);
    assert!(!runner.view().items[0].problems.is_empty());
    assert_eq!(
        std::fs::read_to_string(fixture.destination.join("Turma")).unwrap(),
        "preserve"
    );
}

#[test]
fn cancelled_preflight_and_recheck_do_not_publish_projects() {
    let fixture = Fixture::new();
    fixture.photo("001");
    let cancel = AtomicBool::new(true);
    assert!(
        GenerationRunner::prepare(
            fixture.options(),
            fixture.model.freeze_template().unwrap(),
            fixture.core.clone(),
            &cancel,
        )
        .is_err()
    );
    let mut runner = fixture.prepare().unwrap();
    assert!(runner.recheck(&cancel).is_err());
    assert_eq!(std::fs::read_dir(&fixture.destination).unwrap().count(), 0);
}
