use std::fs;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation, RecoveryCheckpoint, SaveProjectOutcome,
};
use myalbuns_paths::OperationPathContext;

fn project_location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the Project path is captured at the public boundary");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

#[test]
fn interrupted_session_restores_consolidated_state_without_history_or_autosave() {
    let root = tempfile::tempdir().expect("temporary Recuperação fixture");
    let project_path = root.path().join("Projeto recuperável.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the Project is created through ProjectCore");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the base action is applied");
    assert_eq!(
        project.save(1).expect("the base revision is saved"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    let saved_bytes = fs::read(&project_path).expect("the saved Project is readable");

    project
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("one completed action remains unsaved");
    let checkpoint_bytes = project
        .recovery_checkpoint()
        .expect("the consolidated state is captured")
        .to_bytes()
        .expect("the closed checkpoint serializes");
    let checkpoint_json: serde_json::Value =
        serde_json::from_slice(&checkpoint_bytes).expect("the checkpoint is JSON");
    assert_eq!(
        checkpoint_json
            .as_object()
            .expect("the checkpoint is an object")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "baseRevision",
            "creativeState",
            "projectId",
            "schemaVersion"
        ]
        .into_iter()
        .collect(),
        "the recovery envelope is closed and versioned"
    );
    let serialized = String::from_utf8(checkpoint_bytes.clone()).expect("checkpoint UTF-8");
    for forbidden in ["undo", "redo", "command", "cache", "pixel", "originalbytes"] {
        assert!(
            !serialized.to_ascii_lowercase().contains(forbidden),
            "the checkpoint must not persist {forbidden}"
        );
    }
    assert_eq!(
        fs::read(&project_path).expect("the Project remains readable"),
        saved_bytes,
        "capturing Recuperação is never an autosave"
    );
    drop(project);

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("another Host reopens the saved Project");
    reopened
        .restore_recovery(
            RecoveryCheckpoint::from_bytes(&checkpoint_bytes)
                .expect("the persisted checkpoint is accepted"),
        )
        .expect("the matching recovery is restored");
    let restored = reopened.projection();

    assert_eq!(restored.state.document.dpi, 360);
    assert_eq!(restored.state.saved_revision, 1);
    assert_eq!(restored.state.revision, 2);
    assert!(restored.state.dirty, "a restored Session remains unsaved");
    assert!(!restored.state.can_undo, "Undo starts empty after recovery");
    assert!(!restored.state.can_redo, "Redo starts empty after recovery");
    assert_eq!(
        fs::read(&project_path).expect("the original remains readable"),
        saved_bytes,
        "restoring Recuperação does not overwrite the original"
    );

    assert_eq!(
        reopened
            .save(restored.state.revision)
            .expect("the user explicitly saves the restored Session"),
        SaveProjectOutcome::Saved { revision: 2 }
    );
    drop(reopened);
    let saved_recovery = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the explicitly saved recovered Project reopens");
    assert_eq!(saved_recovery.projection().state.document.dpi, 360);
    assert!(!saved_recovery.projection().state.dirty);
}
