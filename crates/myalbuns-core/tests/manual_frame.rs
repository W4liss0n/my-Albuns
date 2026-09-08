#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, DisplayUnit, EditableProject,
    EndSheetFormat, ImportPhoto, InitialProject, InitialProjectConfiguration, OpenProjectRequest,
    PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project(core: &ProjectCore, path: &Path, width: i64, height: i64) -> EditableProject {
    core.create_editable(CreateProjectRequest::new(
        location(path),
        InitialProject::configured(InitialProjectConfiguration::new(
            DisplayUnit::Mm,
            width,
            height,
            300,
            0,
            0,
            3,
            EndSheetFormat::SinglePage,
            EndSheetFormat::SinglePage,
        )),
        CreateAuthorization::CreateOnly,
    ))
    .unwrap()
}

#[test]
fn manual_frame_is_an_empty_centered_placeholder_created_in_one_history_action() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&root.path().join("Manual.myalbuns")),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                3_000,
                3_000,
                3,
                EndSheetFormat::SinglePage,
                EndSheetFormat::SinglePage,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let before = project.projection();
    let result = project
        .apply_with_outcome(ProjectIntent::AddFrame {
            sheet_id: before.state.album.sheets[1].id.clone(),
        })
        .unwrap();
    let frame = &result.projection.state.album.sheets[1].frames[0];
    assert_eq!(result.affected_frame_id.as_deref(), Some(frame.id.as_str()));
    assert!(frame.photo.is_none());
    assert_eq!(
        (
            frame.rect.x,
            frame.rect.y,
            frame.rect.width,
            frame.rect.height
        ),
        (180_000, 70_000, 240_000, 160_000)
    );
    assert_eq!(result.projection.state.revision, before.state.revision + 1);
    assert_eq!(
        result.projection.state.album.media,
        before.state.album.media
    );
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert!(project.undo().is_none());
    assert_eq!(
        project.redo().unwrap().state.album,
        result.projection.state.album
    );
}

#[test]
fn manual_frames_fit_each_active_surface_preserve_existing_frames_and_survive_reopening() {
    for (width, height, sheet_index, expected) in [
        (600_000, 300_000, 0, (90_000, 110_000, 120_000, 80_000)),
        (600_000, 300_000, 2, (90_000, 110_000, 120_000, 80_000)),
        (1_200_000, 100_000, 1, (525_000, 0, 150_000, 100_000)),
    ] {
        let root = tempfile::tempdir().unwrap();
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let path = root.path().join("Superfície.myalbuns");
        let mut project = project(&core, &path, width, height);
        let sheet_id = project.projection().state.album.sheets[sheet_index]
            .id
            .clone();
        let first = project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
        let added = project
            .apply_with_outcome(ProjectIntent::AddFrame { sheet_id })
            .unwrap();
        let sheet = &added.projection.state.album.sheets[sheet_index];
        assert_eq!(sheet.frames.len(), 2);
        assert_eq!(
            sheet.frames[0],
            first.state.album.sheets[sheet_index].frames[0]
        );
        let frame = &sheet.frames[1];
        assert_ne!(frame.id, sheet.frames[0].id);
        assert_eq!(added.affected_frame_id.as_deref(), Some(frame.id.as_str()));
        assert_eq!(frame.z_index, 1);
        assert_eq!(
            (
                frame.rect.x,
                frame.rect.y,
                frame.rect.width,
                frame.rect.height
            ),
            expected
        );
        assert!(frame.photo.is_none());
        assert_eq!(project.undo().unwrap().state.album, first.state.album);
        let redone = project.redo().unwrap();
        project.save(project.revision()).unwrap();
        drop(project);
        let reopened = core
            .open_editable(OpenProjectRequest::new(location(&path)))
            .unwrap();
        assert_eq!(reopened.projection().state.album, redone.state.album);
        assert_eq!(reopened.projection().composition, redone.composition);
    }
}

#[test]
fn unknown_sheet_cannot_create_history_or_clear_redo() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = project(
        &core,
        &root.path().join("Inválido.myalbuns"),
        600_000,
        300_000,
    );
    let sheet_id = project.projection().state.album.sheets[1].id.clone();
    project.apply(ProjectIntent::AddFrame { sheet_id }).unwrap();
    let before = project.undo().unwrap();
    for sheet_id in ["invalid", "00000000-0000-0000-0000-000000000000"] {
        assert_eq!(
            project.apply(ProjectIntent::AddFrame {
                sheet_id: sheet_id.into()
            }),
            Err(CoreError::SheetNotFound(sheet_id.into()))
        );
        assert_eq!(project.projection(), before);
    }
    assert!(project.redo().is_some());
}

#[test]
fn adding_a_photo_fills_the_manual_placeholder_and_keeps_its_geometry() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = project(
        &core,
        &root.path().join("Preencher.myalbuns"),
        600_000,
        300_000,
    );
    let sheet_id = project.projection().state.album.sheets[1].id.clone();
    let empty = project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet_id.clone(),
        })
        .unwrap();
    let source = root.path().join("Foto.jpg");
    fs::write(&source, b"original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(
            source,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
            )
            .unwrap(),
        ))
        .unwrap();
    let filled = project
        .apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id,
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    let frames = &filled.projection.state.album.sheets[1].frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].rect, empty.state.album.sheets[1].frames[0].rect);
    assert_eq!(frames[0].id, empty.state.album.sheets[1].frames[0].id);
    assert_eq!(
        filled.affected_frame_id.as_deref(),
        Some(frames[0].id.as_str())
    );
    assert_eq!(
        frames[0].photo.as_ref().unwrap().media_id,
        imported.media_id
    );
    assert!(
        project.undo().unwrap().state.album.sheets[1].frames[0]
            .photo
            .is_none()
    );
}

#[test]
fn manual_creation_preview_corpus_matches_the_core() {
    let mut cases = Vec::new();
    for (name, width, height, sheet_index) in [
        ("double", 600_000, 300_000, 1),
        ("right", 600_000, 300_000, 0),
        ("left", 600_000, 300_000, 2),
        ("height-limited", 1_200_000, 100_000, 1),
    ] {
        let root = tempfile::tempdir().unwrap();
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = project(
            &core,
            &root.path().join("Criação manual.myalbuns"),
            width,
            height,
        );
        let mut before = project.projection();
        let created = project
            .apply(ProjectIntent::AddFrame {
                sheet_id: before.state.album.sheets[sheet_index].id.clone(),
            })
            .unwrap();
        before.state.project_id = "manual-frame-project".into();
        for (index, sheet) in before.state.album.sheets.iter_mut().enumerate() {
            sheet.id = format!("sheet-{:03}", index + 1);
        }
        for (index, sheet) in before.composition.sheets.iter_mut().enumerate() {
            sheet.sheet_id = format!("sheet-{:03}", index + 1);
        }
        let mut frame = created.state.album.sheets[sheet_index].frames[0].clone();
        let mut composed_frame = created.composition.sheets[sheet_index].frames[0].clone();
        frame.id = "manual-frame".into();
        composed_frame.frame_id = frame.id.clone();
        cases.push(serde_json::json!({ "name": name, "before": before,
            "sheetId": format!("sheet-{:03}", sheet_index + 1), "frame": frame, "composedFrame": composed_frame }));
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({ "cases": cases })).unwrap()
    );
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/manual-frame-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_MANUAL_FRAME_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
