use std::fs;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectIntent, ProjectLocation, SaveProjectError,
    SaveProjectOutcome,
};
use myalbuns_paths::OperationPathContext;

fn project_location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the productive Project path is captured at the operation boundary");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

#[test]
fn productive_project_core_journey_rejects_stale_save_and_reopens_with_empty_history() {
    let root = tempfile::tempdir().expect("temporary productive journey root");
    let project_path = root.path().join("Jornada ProjectCore.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));

    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("CreateOnly publishes and owns one productive Project");
    assert_eq!(project.revision(), 0);
    assert_eq!(project.saved_revision(), 0);
    assert!(matches!(
        core.open_editable(OpenProjectRequest::new(project_location(&project_path))),
        Err(OpenProjectError::ProjectInUse)
    ));

    let applied = project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("DPI is applied through the public ProjectCore seam");
    assert_eq!(applied.state.revision, 1);
    assert!(applied.state.can_undo);
    let undone = project.undo().expect("the productive revision is undone");
    assert_eq!(undone.state.revision, 0);
    assert!(undone.state.can_redo);
    let redone = project.redo().expect("the productive revision is redone");
    assert_eq!(redone.state.revision, 1);

    let bytes_before_stale_save = fs::read(&project_path).expect("the saved Project is readable");
    assert_eq!(
        project
            .save(0)
            .expect_err("a stale visible revision is rejected"),
        SaveProjectError::StaleRevision {
            expected: 0,
            current: 1,
        }
    );
    assert_eq!(
        fs::read(&project_path).expect("the Project remains readable"),
        bytes_before_stale_save,
        "stale Save performs no file I/O"
    );
    assert!(project.has_unsaved_changes());
    assert!(project.can_undo());

    assert_eq!(
        project.save(1).expect("the visible revision is saved"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    assert!(!project.has_unsaved_changes());
    assert!(project.can_undo(), "Save preserves creative history");
    drop(project);

    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("a new Host can own the saved Project after the first owner closes");
    let projection = reopened.projection();
    assert_eq!(projection.state.revision, 1);
    assert_eq!(projection.state.saved_revision, 1);
    assert_eq!(projection.state.document.dpi, 240);
    assert!(!projection.state.dirty);
    assert!(!projection.state.can_undo);
    assert!(!projection.state.can_redo);
}
