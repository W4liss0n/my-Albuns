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
    )
    .unwrap();
    assert!(batch.view().items[0].conflict);
    assert!(!batch.view().items[0].can_replace);
    assert!(!batch.view().can_continue);
    batch.decide(None, GenerationDecision::Replace).unwrap();
    assert!(!batch.view().can_continue);
    drop(existing);
    assert!(!batch.view().can_continue);
    batch.recheck().unwrap();
    assert!(batch.view().items[0].can_replace);
    assert!(!batch.view().can_continue);
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
    fn prepare(&self) -> Result<GenerationRunner, String> {
        GenerationRunner::prepare(
            self.options(),
            self.model.freeze_template().unwrap(),
            self.core.clone(),
        )
    }
}

#[test]
fn invalid_images_are_reported_before_writes_and_ignoring_problems_preserves_valid_items() {
    let fixture = Fixture::new();
    let invalid = fixture.photo("001");
    std::fs::write(invalid, "not an image").unwrap();
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    assert!(!runner.view().can_continue);
    assert!(!runner.view().items[0].problems.is_empty());
    runner.run(&AtomicBool::new(false), &|_| {
        panic!("unresolved preflight cannot start")
    });
    assert_eq!(std::fs::read_dir(&fixture.destination).unwrap().count(), 0);
    runner.decide(None, GenerationDecision::Ignore).unwrap();
    assert_eq!(runner.view().items[1].status, GenerationItemStatus::Pending);
    runner.run(&AtomicBool::new(false), &|_| {});
    assert_eq!(runner.view().items[0].status, GenerationItemStatus::Ignored);
    assert_eq!(
        runner.view().items[1].status,
        GenerationItemStatus::Completed
    );
}

#[test]
fn a_source_changed_after_preflight_fails_only_its_project() {
    let fixture = Fixture::new();
    let photo = fixture.photo("001");
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    std::fs::remove_file(photo).unwrap();
    runner.run(&AtomicBool::new(false), &|_| {});
    assert_eq!(runner.view().phase, GenerationPhase::Finished);
    assert_eq!(runner.view().items[0].status, GenerationItemStatus::Failed);
    assert_eq!(
        runner.view().items[1].status,
        GenerationItemStatus::Completed
    );
    assert!(!fixture.destination.join("001.myalbuns").exists());
    assert!(fixture.destination.join("002.myalbuns").is_file());
}

#[test]
fn cancellation_preserves_completed_projects_without_starting_the_next_one() {
    let fixture = Fixture::new();
    fixture.photo("001");
    fixture.photo("002");
    let mut runner = fixture.prepare().unwrap();
    let cancel = AtomicBool::new(false);
    runner.run(&cancel, &|progress| {
        if progress.completed == 1 {
            cancel.store(true, Ordering::Release);
        }
    });
    assert_eq!(runner.view().phase, GenerationPhase::Cancelled);
    assert_eq!(
        runner.view().items[0].status,
        GenerationItemStatus::Completed
    );
    assert_eq!(runner.view().items[1].status, GenerationItemStatus::Pending);
    assert!(fixture.destination.join("001.myalbuns").is_file());
    assert!(!fixture.destination.join("002.myalbuns").exists());
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
                fixture.core.clone()
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
