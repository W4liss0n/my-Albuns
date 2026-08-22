#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation, SaveAsAuthorization, SaveAsProjectError, SaveAsProjectRequest,
};
use myalbuns_paths::{ExpectedObject, OperationPathContext, PhysicalFileIdentity};

fn project_location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the public path seam captures the Project root");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project_core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

fn physical_identity(path: &Path) -> PhysicalFileIdentity {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the public path seam captures the existing file");
    context
        .freeze()
        .resolve_existing(path, ExpectedObject::RegularFile)
        .expect("the existing file resolves")
        .physical_identity()
        .expect("the local fixture exposes a physical identity")
}

#[test]
fn save_as_moves_the_live_session_to_an_independent_project_and_preserves_history() {
    let root = tempfile::tempdir().expect("temporary Salvar como fixture");
    let original_path = root.path().join("Original.myalbuns");
    let copy_path = root.path().join("Versão independente.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = project.project_id();

    project
        .apply(ProjectIntent::SetDpi { dpi: 300 })
        .expect("the original receives a saved creative change");
    project.save(1).expect("the original revision is saved");
    let original_bytes = fs::read(&original_path).expect("the saved original is readable");
    project
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("the visible Session diverges from the saved original");
    project
        .apply(ProjectIntent::SetDpi { dpi: 420 })
        .expect("the visible Session creates a Redo branch");
    project
        .undo()
        .expect("the visible Session retains a preexisting Redo entry");

    let outcome = project
        .save_as(SaveAsProjectRequest::new(
            2,
            project_location(&copy_path),
            SaveAsAuthorization::CreateOnly,
        ))
        .expect("Salvar como publishes and adopts an independent Project");

    assert_eq!(outcome.previous_project_id, original_id);
    assert_eq!(outcome.project_id, project.project_id());
    assert_ne!(outcome.project_id, original_id);
    assert_eq!(outcome.revision, 2);
    assert_eq!(project.project_path(), copy_path);
    let projected = serde_json::to_value(project.projection())
        .expect("the adopted Project projection serializes");
    assert!(
        projected["state"].get("projectLocationDisplay").is_none(),
        "the native Project pathname stays outside the WebView projection",
    );
    assert_eq!(project.saved_revision(), 2);
    assert!(!project.has_unsaved_changes());
    assert!(project.can_undo(), "Salvar como preserves the live History");
    assert!(
        project.can_redo(),
        "Salvar como preserves the live Redo branch"
    );
    assert_eq!(
        fs::read(&original_path).expect("the original remains readable"),
        original_bytes,
        "Salvar como never writes the visible unsaved state into the original",
    );

    let undone = project
        .undo()
        .expect("the preserved History remains usable");
    assert_eq!(undone.state.project_id, outcome.project_id.to_string());
    assert_eq!(undone.state.document.dpi, 300);
    assert!(undone.state.dirty);

    let mut original = core
        .open_editable(OpenProjectRequest::new(project_location(&original_path)))
        .expect("the released original opens beside the new Project");
    assert_eq!(original.project_id(), original_id);
    assert_eq!(original.project().document().dpi(), 300);
    assert!(!original.can_undo());
    assert!(!original.can_redo());

    let redone = project
        .redo()
        .expect("Redo remains usable in the copied live Session");
    assert_eq!(redone.state.project_id, outcome.project_id.to_string());
    assert_eq!(redone.state.document.dpi, 360);
    assert!(!redone.state.dirty);
    let redone_branch = project
        .redo()
        .expect("the Redo branch that existed before Save As remains usable");
    assert_eq!(
        redone_branch.state.project_id,
        outcome.project_id.to_string()
    );
    assert_eq!(redone_branch.state.document.dpi, 420);
    assert!(redone_branch.state.dirty);
    original
        .apply(ProjectIntent::SetDpi { dpi: 180 })
        .expect("the reopened original advances independently");
    original.save(2).expect("the original saves independently");
    let independently_saved_original =
        fs::read(&original_path).expect("the independently saved original is readable");
    let copy_before_next_save = fs::read(&copy_path).expect("the copied Project remains readable");
    project
        .apply(ProjectIntent::SetDpi { dpi: 480 })
        .expect("the copied Session advances independently");
    project.save(4).expect("the copy saves independently");

    assert_eq!(
        fs::read(&original_path).expect("the original remains isolated"),
        independently_saved_original
    );
    assert_ne!(
        fs::read(&copy_path).expect("the copy remains readable"),
        copy_before_next_save
    );
    assert_eq!(original.project().document().dpi(), 180);
    assert_eq!(project.project().document().dpi(), 480);
}

#[test]
fn replace_confirmed_overwrites_only_the_distinct_destination_and_adopts_it() {
    let root = tempfile::tempdir().expect("temporary replacement fixture");
    let original_path = root.path().join("Original.myalbuns");
    let destination_path = root.path().join("Destino existente.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    project
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("the visible state advances");
    let original_bytes = fs::read(&original_path).expect("the original baseline is readable");
    fs::write(&destination_path, b"replace-confirmed destination")
        .expect("a distinct regular file occupies the destination");

    let saved_as = project
        .save_as(SaveAsProjectRequest::new(
            1,
            project_location(&destination_path),
            SaveAsAuthorization::ReplaceConfirmed(physical_identity(&destination_path)),
        ))
        .expect("the explicitly confirmed distinct destination is replaced");

    assert_eq!(project.project_path(), destination_path);
    assert_eq!(project.project_id(), saved_as.project_id);
    assert_eq!(project.saved_revision(), 1);
    assert_eq!(project.project().document().dpi(), 360);
    assert_eq!(
        fs::read(&original_path).expect("the original remains readable"),
        original_bytes
    );
    assert_ne!(
        fs::read(&destination_path).expect("the replacement is readable"),
        b"replace-confirmed destination"
    );
}

#[test]
fn replace_confirmed_rejects_a_different_file_that_arrives_after_confirmation() {
    let root = tempfile::tempdir().expect("temporary replacement race fixture");
    let original_path = root.path().join("Original.myalbuns");
    let destination_path = root.path().join("Destino confirmado.myalbuns");
    let displaced_path = root.path().join("Destino confirmado anterior.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    project
        .apply(ProjectIntent::SetDpi { dpi: 360 })
        .expect("the visible state advances");
    let original_bytes = fs::read(&original_path).expect("the original baseline is readable");
    fs::write(&destination_path, b"confirmed physical destination")
        .expect("the user confirms an existing destination");
    let confirmed_identity = physical_identity(&destination_path);
    fs::rename(&destination_path, &displaced_path)
        .expect("the confirmed file is displaced after confirmation");
    fs::write(&destination_path, b"unexpected replacement")
        .expect("a different file arrives at the selected pathname");

    assert_eq!(
        project.save_as(SaveAsProjectRequest::new(
            1,
            project_location(&destination_path),
            SaveAsAuthorization::ReplaceConfirmed(confirmed_identity),
        )),
        Err(SaveAsProjectError::DestinationConflict)
    );
    assert_eq!(project.project_path(), original_path);
    assert_eq!(project.project().document().dpi(), 360);
    assert!(project.has_unsaved_changes());
    assert_eq!(
        fs::read(&original_path).expect("the original remains readable"),
        original_bytes
    );
    assert_eq!(
        fs::read(&destination_path).expect("the unexpected replacement remains readable"),
        b"unexpected replacement"
    );
    assert_eq!(
        fs::read(&displaced_path).expect("the confirmed destination remains readable"),
        b"confirmed physical destination"
    );
}

#[test]
fn save_as_rejects_the_current_file_and_its_physical_alias_without_touching_the_session() {
    let root = tempfile::tempdir().expect("temporary SameTarget fixture");
    let original_path = root.path().join("Original.myalbuns");
    let alias_path = root.path().join("Alias.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible Session becomes dirty");
    let original_bytes = fs::read(&original_path).expect("the original baseline is readable");
    fs::hard_link(&original_path, &alias_path).expect("the destination aliases the original");

    for destination in [&original_path, &alias_path] {
        assert_eq!(
            project.save_as(SaveAsProjectRequest::new(
                1,
                project_location(destination),
                SaveAsAuthorization::ReplaceConfirmed(physical_identity(destination)),
            )),
            Err(SaveAsProjectError::SameTarget)
        );
        assert_eq!(project.project_path(), original_path);
        assert_eq!(project.project().document().dpi(), 240);
        assert!(project.has_unsaved_changes());
        assert!(project.can_undo());
        assert_eq!(
            fs::read(&original_path).expect("the original remains readable"),
            original_bytes
        );
    }
}

#[test]
fn a_conclusive_destination_conflict_keeps_the_original_session_and_bytes() {
    let root = tempfile::tempdir().expect("temporary conflict fixture");
    let original_path = root.path().join("Original.myalbuns");
    let occupied_path = root.path().join("Ocupado.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible Session becomes dirty");
    let original_bytes = fs::read(&original_path).expect("the original baseline is readable");
    fs::write(&occupied_path, b"occupied destination")
        .expect("another object occupies the destination");

    assert_eq!(
        project.save_as(SaveAsProjectRequest::new(
            1,
            project_location(&occupied_path),
            SaveAsAuthorization::CreateOnly,
        )),
        Err(SaveAsProjectError::DestinationConflict)
    );
    assert_eq!(project.project_path(), original_path);
    assert_eq!(project.project().document().dpi(), 240);
    assert!(project.has_unsaved_changes());
    assert!(project.can_undo());
    assert_eq!(
        fs::read(&original_path).expect("the original remains readable"),
        original_bytes
    );
    assert_eq!(
        fs::read(&occupied_path).expect("the occupied destination remains readable"),
        b"occupied destination"
    );
}

#[test]
fn an_indeterminate_local_transition_keeps_the_previous_session_and_authority() {
    let root = tempfile::tempdir().expect("temporary transition fixture");
    let original_path = root.path().join("Original.myalbuns");
    let destination_path = root.path().join("Destino a reinspecionar.myalbuns");
    let core = project_core(root.path());
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = project.project_id();
    project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the visible Session becomes dirty");

    assert_eq!(
        project.save_as_with_transition(
            SaveAsProjectRequest::new(
                1,
                project_location(&destination_path),
                SaveAsAuthorization::CreateOnly,
            ),
            |authority, outcome| {
                assert_eq!(authority.project_id(), outcome.project_id);
                Err(())
            },
        ),
        Err(SaveAsProjectError::SaveAsStateIndeterminate)
    );

    assert_eq!(project.project_id(), original_id);
    assert_eq!(project.identity_authority().project_id(), original_id);
    assert_eq!(project.project_path(), original_path);
    assert_eq!(project.project().document().dpi(), 240);
    assert!(project.has_unsaved_changes());
    assert!(project.can_undo());
    assert!(
        destination_path.is_file(),
        "the indeterminate terminal truthfully requires destination reinspection"
    );
}
