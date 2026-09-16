#![cfg(windows)]

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, EditableProject, ImportMedia, InitialProject,
    LoadProjectRequest, MediaFolderEdit, MediaId, MediaKind, MediaRemovalMode, OpenProjectRequest,
    PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;
use serde_json::{Value, json};
use std::{fs, path::Path};

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}
fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}
fn project(root: &Path) -> EditableProject {
    core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Álbum.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap()
}
fn import(project: &mut EditableProject, root: &Path, name: &str, kind: MediaKind) -> MediaId {
    let path = root.join(name);
    fs::write(&path, b"original").unwrap();
    project
        .import_media(
            kind,
            vec![ImportMedia::new(
                path,
                PhotoSourceMetadata::new(
                    600,
                    400,
                    ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
                )
                .unwrap(),
            )],
        )
        .unwrap()
        .media_ids[0]
}
fn edit(project: &mut EditableProject, edit: MediaFolderEdit) {
    project
        .apply(ProjectIntent::EditMediaFolder { edit })
        .unwrap();
}
fn create(project: &mut EditableProject, name: &str, kind: MediaKind) -> String {
    edit(
        project,
        MediaFolderEdit::Create {
            name: name.into(),
            media_kind: kind,
        },
    );
    project.project().media_folders().last().unwrap().id.clone()
}

#[test]
fn folder_operations_are_atomic_undoable_and_do_not_change_composition_or_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let a = import(&mut project, root.path(), "001.jpg", MediaKind::Photo);
    let b = import(&mut project, root.path(), "002.jpg", MediaKind::Photo);
    let composition = project.projection().composition;
    let first = create(&mut project, " Turma A ", MediaKind::Photo);
    let second = create(&mut project, "Turma B", MediaKind::Photo);
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![a, b],
            folder_id: Some(first.clone()),
        },
    );
    let before = project.projection();
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![a, b, a],
            folder_id: Some(first.clone()),
        },
    );
    assert_eq!(project.projection(), before, "same assignment is a no-op");
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![b],
            folder_id: Some(second.clone()),
        },
    );
    assert_eq!(project.project().media_folders()[0].media_ids, vec![a]);
    assert_eq!(project.project().media_folders()[1].media_ids, vec![b]);
    assert_eq!(project.revision(), before.state.revision + 1);
    project.undo().unwrap();
    assert_eq!(project.project().media_folders()[0].media_ids, vec![a, b]);
    project.redo().unwrap();
    edit(
        &mut project,
        MediaFolderEdit::Rename {
            folder_id: first.clone(),
            name: "Retratos".into(),
        },
    );
    let organized = project.project().media_folders().to_vec();
    edit(&mut project, MediaFolderEdit::Delete { folder_id: first });
    assert_eq!(project.project().media().len(), 2);
    project.undo().unwrap();
    assert_eq!(project.project().media_folders(), organized);
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![a, b],
            folder_id: None,
        },
    );
    assert!(
        project
            .project()
            .media_folders()
            .iter()
            .all(|folder| folder.media_ids.is_empty())
    );
    assert_eq!(project.projection().composition, composition);
    assert_eq!(fs::read(root.path().join("001.jpg")).unwrap(), b"original");
    assert!(project.has_unsaved_changes());
}

#[test]
fn invalid_names_members_and_cross_tab_moves_preserve_revision_and_redo() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let photo = import(&mut project, root.path(), "001.jpg", MediaKind::Photo);
    let decorative = import(
        &mut project,
        root.path(),
        "Fundo.jpg",
        MediaKind::Decorative,
    );
    let folder = create(&mut project, "Turma", MediaKind::Photo);
    create(&mut project, "Turma", MediaKind::Decorative);
    create(&mut project, "Extra", MediaKind::Photo);
    project.undo().unwrap();
    let before = project.projection();
    let mut invalid = vec![
        MediaFolderEdit::MoveMedia {
            media_ids: vec![decorative],
            folder_id: Some(folder.clone()),
        },
        MediaFolderEdit::MoveMedia {
            media_ids: vec![photo, decorative],
            folder_id: None,
        },
        MediaFolderEdit::MoveMedia {
            media_ids: vec![photo],
            folder_id: Some("missing".into()),
        },
        MediaFolderEdit::Delete {
            folder_id: "missing".into(),
        },
    ];
    for name in [
        "",
        "  ",
        "TURMA",
        "Todas",
        "Ausentes",
        "line\nbreak",
        &"x".repeat(81),
    ] {
        invalid.push(MediaFolderEdit::Create {
            name: name.into(),
            media_kind: MediaKind::Photo,
        });
    }
    for edit in invalid {
        assert!(
            project
                .apply(ProjectIntent::EditMediaFolder { edit })
                .is_err()
        );
        assert_eq!(project.projection(), before);
    }
}

#[test]
fn removal_cleans_membership_and_undo_restores_the_complete_catalog() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let media = import(&mut project, root.path(), "001.jpg", MediaKind::Photo);
    let folder = create(&mut project, "Turma", MediaKind::Photo);
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![media],
            folder_id: Some(folder),
        },
    );
    project
        .apply(ProjectIntent::RemoveMedia {
            media_ids: vec![media],
            mode: MediaRemovalMode::RemoveAll,
        })
        .unwrap();
    assert!(project.project().media_folders()[0].media_ids.is_empty());
    project.undo().unwrap();
    assert_eq!(project.project().media_folders()[0].media_ids, vec![media]);
}

#[test]
fn saved_folders_survive_reopen_and_missing_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let media = import(&mut project, root.path(), "001.jpg", MediaKind::Photo);
    let folder = create(&mut project, "Formandos", MediaKind::Photo);
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![media],
            folder_id: Some(folder),
        },
    );
    let expected = project.project().media_folders().to_vec();
    let path = project.project_path().to_path_buf();
    project.save(project.revision()).unwrap();
    drop(project);
    fs::remove_file(root.path().join("001.jpg")).unwrap();
    let reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.project().media_folders(), expected);
    assert_eq!(
        reopened.projection().state.album.media_folders,
        Some(expected)
    );
    assert!(!reopened.has_unsaved_changes());
    assert!(!reopened.projection().state.can_undo);
}

#[test]
fn save_as_and_external_copy_preserve_folders_and_keep_edits_independent() {
    use myalbuns_core::{SaveAsAuthorization, SaveAsProjectRequest};
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let media = import(&mut project, root.path(), "001.jpg", MediaKind::Photo);
    let folder = create(&mut project, "Turma", MediaKind::Photo);
    edit(
        &mut project,
        MediaFolderEdit::MoveMedia {
            media_ids: vec![media],
            folder_id: Some(folder.clone()),
        },
    );
    project.save(project.revision()).unwrap();
    let path = project.project_path().to_path_buf();
    let original_id = project.project_id();
    let folders = project.project().media_folders().to_vec();
    let copy_path = root.path().join("Cópia.myalbuns");
    fs::copy(&path, &copy_path).unwrap();
    let mut copy = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&copy_path)))
        .unwrap();
    assert_ne!(copy.project_id(), original_id);
    assert_eq!(copy.project().media_folders(), folders);
    edit(&mut copy, MediaFolderEdit::Delete { folder_id: folder });
    assert_eq!(project.project().media_folders(), folders);
    let save_as_path = root.path().join("Salvar como.myalbuns");
    project
        .save_as(SaveAsProjectRequest::new(
            project.revision(),
            location(&save_as_path),
            SaveAsAuthorization::CreateOnly,
        ))
        .unwrap();
    assert_ne!(project.project_id(), original_id);
    drop(project);
    let saved_as = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&save_as_path)))
        .unwrap();
    assert_eq!(saved_as.project().media_folders(), folders);
}

#[test]
fn v11_migration_is_read_only_until_save_and_matches_v12_golden() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Legado.myalbuns");
    let bytes = include_bytes!("fixtures/project_document_v11_migration_expected.myalbuns");
    fs::write(&path, bytes).unwrap();
    let mut project = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert!(project.project().media_folders().is_empty());
    assert!(!project.has_unsaved_changes());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    project.save(project.revision()).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        include_bytes!("fixtures/project_document_v12_migration_expected.myalbuns")
    );
}

#[test]
fn v12_closed_schema_rejects_invalid_organization_without_writing() {
    let root = tempfile::tempdir().unwrap();
    let mut valid: Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v12_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    let media = valid["project"]["media"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "photo")
        .unwrap()["id"]
        .clone();
    let folder = json!({"id":"ab000000-0000-4000-8000-000000000001","name":"Turma A","kind":"photo","mediaIds":[media]});
    valid["mediaFolders"] = json!([folder]);
    let valid_path = root.path().join("Válido.myalbuns");
    fs::write(&valid_path, serde_json::to_vec(&valid).unwrap()).unwrap();
    core(root.path())
        .load_persisted_revision(LoadProjectRequest::new(location(&valid_path)))
        .unwrap();
    let mut cases = Vec::new();
    for (key, value) in [
        ("id", json!("bad")),
        ("kind", json!("decorative")),
        ("name", json!("")),
        ("name", json!(" Todas ")),
        ("extra", json!(true)),
        ("mediaIds", json!([media, media])),
        ("mediaIds", json!(["ab000000-0000-4000-8000-000000000002"])),
    ] {
        let mut case = valid.clone();
        case["mediaFolders"][0][key] = value;
        cases.push(case);
    }
    let mut duplicate = valid.clone();
    duplicate["mediaFolders"]
        .as_array_mut()
        .unwrap()
        .push(folder.clone());
    cases.push(duplicate);
    let mut repeated_member = valid.clone();
    let mut second = folder;
    second["id"] = json!("ab000000-0000-4000-8000-000000000003");
    second["name"] = json!("Turma B");
    repeated_member["mediaFolders"]
        .as_array_mut()
        .unwrap()
        .push(second);
    cases.push(repeated_member);
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("mediaFolders");
    cases.push(missing);
    for (i, case) in cases.into_iter().enumerate() {
        let path = root.path().join(format!("Inválido-{i}.myalbuns"));
        let bytes = serde_json::to_vec(&case).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(
            core(root.path())
                .load_persisted_revision(LoadProjectRequest::new(location(&path)))
                .is_err(),
            "case {i}"
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}
