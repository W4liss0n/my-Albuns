use std::fs;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation, RecoveryCheckpoint, RecoveryCheckpointError,
    SaveProjectOutcome,
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

#[test]
fn recovering_the_saved_revision_still_requires_an_explicit_save() {
    let root = tempfile::tempdir().expect("temporary saved-revision Recuperação fixture");
    let project_path = root.path().join("Projeto recuperado na base.myalbuns");
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
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("one creative action is completed");
    let returned_to_base = project
        .undo()
        .expect("Undo returns the live Session to its saved revision");
    assert_eq!(returned_to_base.state.revision, 0);
    assert!(!returned_to_base.state.dirty);
    let checkpoint = project
        .recovery_checkpoint()
        .expect("the completed Undo is consolidated");
    drop(project);

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("another Host reopens the saved Project");
    let recovered = reopened
        .restore_recovery(checkpoint)
        .expect("the matching saved-revision checkpoint is restored");

    assert_eq!(recovered.state.revision, recovered.state.saved_revision);
    assert!(
        recovered.state.dirty,
        "choosing recovery always creates an unsaved Session"
    );
    assert!(reopened.has_unsaved_changes());
    assert!(!recovered.state.can_undo);
    assert!(!recovered.state.can_redo);

    assert_eq!(
        reopened
            .save(recovered.state.revision)
            .expect("the recovered Session is explicitly saved"),
        SaveProjectOutcome::Saved {
            revision: recovered.state.revision
        }
    );
    assert!(!reopened.projection().state.dirty);
}

// ---------------------------------------------------------------------------
// Rejected checkpoints

struct UnsavedProject {
    core: ProjectCore,
    path: std::path::PathBuf,
    project: myalbuns_core::EditableProject,
}

/// A Project saved at revision 1 with one unsaved action after it.
fn unsaved_project(root: &std::path::Path, name: &str) -> UnsavedProject {
    let path = root.join(format!("{name}.myalbuns"));
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the Project is created through ProjectCore");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the base action is applied");
    project.save(1).expect("the base revision is saved");
    project
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("one completed action remains unsaved");
    UnsavedProject {
        core,
        path,
        project,
    }
}

#[test]
fn a_checkpoint_changed_after_it_was_written_is_rejected_as_invalid() {
    const ANOTHER_PROJECT_ID: &str = "c0000000-0000-4000-8000-000000000001";
    let root = tempfile::tempdir().expect("temporary rejected checkpoint fixture");
    let bytes = unsaved_project(root.path(), "Projeto")
        .project
        .recovery_checkpoint()
        .expect("the consolidated state is captured")
        .to_bytes()
        .expect("the checkpoint serializes");
    let written: serde_json::Value =
        serde_json::from_slice(&bytes).expect("the checkpoint is JSON");
    let project_id = written["projectId"]
        .as_str()
        .expect("the checkpoint names its Project")
        .to_owned();
    assert_ne!(project_id, ANOTHER_PROJECT_ID);
    let changed = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut checkpoint = written.clone();
        change(&mut checkpoint);
        serde_json::to_vec(&checkpoint).expect("the changed checkpoint serializes")
    };

    // Rewriting the same content is accepted, so each rejection below comes
    // from its one change.
    RecoveryCheckpoint::from_bytes(&changed(&|_| {}))
        .expect("the unchanged checkpoint is accepted");

    let rejected = [
        ("a truncated file", bytes[..bytes.len() / 2].to_vec()),
        (
            "a field outside the closed envelope",
            changed(&|checkpoint| checkpoint["undo"] = serde_json::json!([])),
        ),
        (
            "another envelope version",
            changed(&|checkpoint| checkpoint["schemaVersion"] = serde_json::json!(2)),
        ),
        (
            "a base revision beyond the safe integer range",
            changed(&|checkpoint| {
                checkpoint["baseRevision"]["revision"] =
                    serde_json::json!(9_007_199_254_740_992_u64)
            }),
        ),
        (
            "a Project identity that is not canonical",
            changed(&|checkpoint| {
                checkpoint["projectId"] = serde_json::json!(project_id.to_uppercase());
                checkpoint["baseRevision"]["projectId"] =
                    serde_json::json!(project_id.to_uppercase());
            }),
        ),
        (
            "a base revision of another Project",
            changed(&|checkpoint| {
                checkpoint["baseRevision"]["projectId"] = serde_json::json!(ANOTHER_PROJECT_ID)
            }),
        ),
        (
            "a creative state that is not a Project document",
            changed(&|checkpoint| {
                checkpoint["creativeState"]["documentType"] = serde_json::json!("other")
            }),
        ),
        (
            "a creative state of another Project",
            changed(&|checkpoint| {
                checkpoint["creativeState"]["projectId"] = serde_json::json!(ANOTHER_PROJECT_ID)
            }),
        ),
    ];
    let not_rejected: Vec<&str> = rejected
        .iter()
        .filter(|(_, bytes)| {
            RecoveryCheckpoint::from_bytes(bytes).err()
                != Some(RecoveryCheckpointError::InvalidCheckpoint)
        })
        .map(|(case, _)| *case)
        .collect();
    assert!(not_rejected.is_empty(), "{not_rejected:?}");
}

#[test]
fn a_checkpoint_of_another_project_is_not_restored() {
    let root = tempfile::tempdir().expect("temporary identity mismatch fixture");
    let mut first = unsaved_project(root.path(), "Primeiro");
    let second = unsaved_project(root.path(), "Segundo");
    let foreign = second
        .project
        .recovery_checkpoint()
        .expect("the other Project is consolidated");
    drop(first.project);
    first.project = first
        .core
        .open_editable(OpenProjectRequest::new(project_location(&first.path)))
        .expect("the first Project reopens at its saved revision");
    let before = first.project.projection();

    assert_eq!(
        first.project.restore_recovery(foreign).err(),
        Some(RecoveryCheckpointError::IdentityMismatch)
    );
    assert_eq!(first.project.projection(), before);
}

#[test]
fn a_checkpoint_is_restored_only_over_the_saved_revision_it_derives_from() {
    let root = tempfile::tempdir().expect("temporary baseline mismatch fixture");
    let mut fixture = unsaved_project(root.path(), "Projeto");
    let from_revision_1 = fixture
        .project
        .recovery_checkpoint()
        .expect("the unsaved action is consolidated");
    assert_eq!(from_revision_1.base_saved_revision(), 1);

    // The live Session already has unsaved work over the same saved revision.
    let unsaved = fixture.project.projection();
    assert_eq!(
        fixture
            .project
            .restore_recovery(from_revision_1.clone())
            .err(),
        Some(RecoveryCheckpointError::BaselineMismatch)
    );
    assert_eq!(fixture.project.projection(), unsaved);

    // The saved revision advanced after the checkpoint was written.
    fixture
        .project
        .save(2)
        .expect("the saved revision advances");
    drop(fixture.project);
    let mut reopened = fixture
        .core
        .open_editable(OpenProjectRequest::new(project_location(&fixture.path)))
        .expect("the Project reopens at the later saved revision");
    let saved = reopened.projection();
    assert_eq!(
        reopened.restore_recovery(from_revision_1).err(),
        Some(RecoveryCheckpointError::BaselineMismatch)
    );
    assert_eq!(reopened.projection(), saved);
}
