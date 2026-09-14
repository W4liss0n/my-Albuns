#![cfg(windows)]

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, ImportPhoto, InitialProject, LoadProjectRequest,
    PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;
use std::path::Path;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

#[test]
fn generated_projects_copy_unsaved_template_without_changing_the_model() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let original = root.path().join("Modelo.myalbuns");
    let mut model = core
        .create_editable(CreateProjectRequest::new(
            location(&original),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let saved_bytes = std::fs::read(&original).unwrap();
    model.apply(ProjectIntent::SetDpi { dpi: 420 }).unwrap();
    let template = model.freeze_template().unwrap();
    let photo = root.path().join("001.jpg");
    let destination = root.path().join("Turma.myalbuns");
    let generated = core
        .create_from_template(
            &template,
            location(&destination),
            CreateAuthorization::CreateOnly,
            vec![ImportPhoto::new(
                photo.clone(),
                PhotoSourceMetadata::new(
                    600,
                    400,
                    ["#FFFFFF".into(), "#808080".into(), "#000000".into()],
                )
                .unwrap(),
            )],
        )
        .unwrap();
    assert_ne!(generated.project_id(), model.project_id());
    assert_eq!(generated.project().document().dpi(), 420);
    assert_eq!(generated.project().sheets(), model.project().sheets());
    assert_eq!(generated.project().media().len(), 1);
    assert_eq!(generated.project().media()[0].path(), photo);
    assert!(!generated.has_unsaved_changes());
    assert!(model.has_unsaved_changes());
    assert_eq!(std::fs::read(&original).unwrap(), saved_bytes);
    drop(generated);
    let reopened = core
        .load_persisted_revision(LoadProjectRequest::new(location(&destination)))
        .unwrap();
    assert_eq!(reopened.project().document().dpi(), 420);
    model.apply(ProjectIntent::SetDpi { dpi: 240 }).unwrap();
    assert_eq!(reopened.project().document().dpi(), 420);
}

#[test]
fn generation_rejects_open_destinations_and_replacements_changed_since_confirmation() {
    use myalbuns_core::CreateProjectError;
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let destination = root.path().join("Existente.myalbuns");
    let existing = core
        .create_editable(CreateProjectRequest::new(
            location(&destination),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let template = existing.freeze_template().unwrap();
    assert_eq!(
        core.inspect_creation_destination(&location(&destination)),
        Err(CreateProjectError::ProjectInUse)
    );
    drop(existing);
    let confirmed = core
        .inspect_creation_destination(&location(&destination))
        .unwrap()
        .unwrap();
    std::fs::rename(&destination, root.path().join("Anterior.myalbuns")).unwrap();
    let replacement = core
        .create_editable(CreateProjectRequest::new(
            location(&destination),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    drop(replacement);
    let bytes = std::fs::read(&destination).unwrap();
    assert!(matches!(
        core.create_from_template(
            &template,
            location(&destination),
            CreateAuthorization::ReplaceTargetConfirmed(confirmed),
            vec![]
        ),
        Err(CreateProjectError::DestinationConflict)
    ));
    assert_eq!(std::fs::read(destination).unwrap(), bytes);
}

#[test]
fn a_populated_model_is_copied_as_a_whole_without_history_or_another_identity_reference() {
    use myalbuns_core::OpenProjectRequest;
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let model_path = root.path().join("Modelo.myalbuns");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/project_document_v11_photo_migration_expected.myalbuns"),
        &model_path,
    )
    .unwrap();
    let mut model = core
        .open_editable(OpenProjectRequest::new(location(&model_path)))
        .unwrap();
    model.apply(ProjectIntent::SetDpi { dpi: 420 }).unwrap();
    let destination = root.path().join("Completo.myalbuns");
    let generated = core
        .create_from_template(
            &model.freeze_template().unwrap(),
            location(&destination),
            CreateAuthorization::CreateOnly,
            vec![],
        )
        .unwrap();
    assert_eq!(generated.project(), model.project());
    assert_ne!(generated.project_id(), model.project_id());
    assert!(!generated.projection().state.can_undo);
    assert!(!generated.projection().state.can_redo);
    drop(generated);
    let reopened = core
        .load_persisted_revision(LoadProjectRequest::new(location(&destination)))
        .unwrap();
    assert_eq!(reopened.project(), model.project());
    assert!(model.has_unsaved_changes());
}
