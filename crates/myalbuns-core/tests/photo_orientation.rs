#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, EditableProject, ImportPhoto, InitialProject,
    OpenProjectRequest, PhotoOrientationAction, PhotoPlacementMode, PhotoSourceMetadata,
    ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

fn mixed_project(root: &Path) -> EditableProject {
    let mut project = core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Orientação.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let original = root.join("Foto.jpg");
    fs::write(&original, b"original unchanged").unwrap();
    let media_id = project
        .import_photo(ImportPhoto::new(
            original,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        ))
        .unwrap()
        .media_id;
    let sheet_id = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..2 {
        project
            .apply(ProjectIntent::AddPhoto {
                sheet_id: sheet_id.clone(),
                media_id,
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
    }
    project.apply(ProjectIntent::AddFrame { sheet_id }).unwrap();
    project
}

fn orient(project: &mut EditableProject, frame_ids: Vec<String>, action: PhotoOrientationAction) {
    project
        .apply(ProjectIntent::OrientPhotos { frame_ids, action })
        .unwrap();
}

#[test]
fn rotates_and_mirrors_only_selected_photos_as_one_reversible_edit() {
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: ids[0].clone(),
            delta_pan_x: 0.4,
            delta_pan_y: -0.3,
            delta_zoom: 0.5,
        })
        .unwrap();
    let before = project.projection();
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::RotateCounterClockwise,
    );
    let rotated = project.projection();
    assert_eq!(rotated.state.revision, before.state.revision + 1);
    let frames = &rotated.state.album.sheets[0].frames;
    assert_eq!(frames[0].photo.as_ref().unwrap().transform.quarter_turns, 3);
    assert_eq!(frames[1].photo.as_ref().unwrap().transform.quarter_turns, 3);
    assert_eq!(frames[2], before.state.album.sheets[0].frames[2]);
    for (actual, original) in frames.iter().zip(&before.state.album.sheets[0].frames) {
        assert_eq!(actual.rect, original.rect);
        assert_eq!(actual.id, original.id);
        assert_eq!(actual.z_index, original.z_index);
    }
    let transform = &frames[0].photo.as_ref().unwrap().transform;
    assert_eq!(
        (transform.pan_x, transform.pan_y, transform.user_zoom),
        (0.4, -0.3, 1.5)
    );
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, rotated.state.album);
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    let mirrored = project.projection();
    assert!(
        mirrored.state.album.sheets[0].frames[..2]
            .iter()
            .all(|frame| frame.photo.as_ref().unwrap().transform.mirror_x)
    );
    assert_eq!(mirrored.state.revision, rotated.state.revision + 1);
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    assert_eq!(project.projection().state.album, rotated.state.album);
    for _ in 0..3 {
        orient(
            &mut project,
            ids.clone(),
            PhotoOrientationAction::RotateCounterClockwise,
        );
    }
    assert_eq!(project.projection().state.album, before.state.album);
}

#[test]
fn orientation_survives_pan_zoom_save_reopen_and_export_without_writing_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let initial = project.projection();
    let id = initial.state.album.sheets[0].frames[0].id.clone();
    let frozen = project.render_snapshot();
    orient(
        &mut project,
        vec![id.clone()],
        PhotoOrientationAction::RotateCounterClockwise,
    );
    orient(
        &mut project,
        vec![id.clone()],
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: id,
            delta_pan_x: 0.5,
            delta_pan_y: -0.25,
            delta_zoom: 1.0,
        })
        .unwrap();
    let expected = project.projection();
    let photo = expected.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap();
    assert_eq!(photo.transform.quarter_turns, 3);
    assert!(photo.transform.mirror_x);
    assert_eq!(frozen.composition, initial.composition);
    assert_eq!(project.render_snapshot().composition, expected.composition);
    project.save(project.revision()).unwrap();
    let bytes = fs::read(root.path().join("Orientação.myalbuns")).unwrap();
    let dto: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(dto["schemaVersion"], 4);
    assert_eq!(
        dto["project"]["sheets"][0]["frames"][0]["photo"]["transform"]["quarterTurns"],
        3
    );
    drop(project);
    let mut reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(
            &root.path().join("Orientação.myalbuns"),
        )))
        .unwrap();
    reopened
        .observe_photo_source(
            photo.media_id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(reopened.projection().state.album, expected.state.album);
    assert_eq!(reopened.render_snapshot().composition, expected.composition);
    assert!(!reopened.has_unsaved_changes());
    assert_eq!(
        fs::read(root.path().join("Foto.jpg")).unwrap(),
        b"original unchanged"
    );
}

#[test]
fn mixed_values_take_one_absolute_orientation_and_invalid_selections_are_atomic() {
    use myalbuns_core::CoreError;
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    orient(
        &mut project,
        vec![ids[0].clone()],
        PhotoOrientationAction::RotateCounterClockwise,
    );
    orient(
        &mut project,
        vec![ids[0].clone()],
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::RotateCounterClockwise,
    );
    assert!(
        project.projection().state.album.sheets[0].frames[..2]
            .iter()
            .all(|frame| frame.photo.as_ref().unwrap().transform.quarter_turns == 3)
    );
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    assert!(
        project.projection().state.album.sheets[0].frames[..2]
            .iter()
            .all(|frame| frame.photo.as_ref().unwrap().transform.mirror_x)
    );
    let other = project
        .apply_with_outcome(ProjectIntent::AddFrame {
            sheet_id: project.projection().state.album.sheets[1].id.clone(),
        })
        .unwrap()
        .affected_frame_id
        .unwrap();
    let before = project.projection();
    for invalid in [
        vec![],
        vec![ids[0].clone(), ids[0].clone()],
        vec![ids[0].clone(), other],
        vec![ids[0].clone(), "missing".into()],
    ] {
        assert_eq!(
            project.apply(ProjectIntent::OrientPhotos {
                frame_ids: invalid,
                action: PhotoOrientationAction::RotateCounterClockwise
            }),
            Err(CoreError::InvalidPhotoOrientationSelection)
        );
        assert_eq!(project.projection(), before);
    }
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::ResetRotation,
    );
    let reset = project.projection();
    orient(
        &mut project,
        ids.clone(),
        PhotoOrientationAction::ResetRotation,
    );
    assert_eq!(project.projection(), reset);
    project.undo().unwrap();
    let undone = project.projection();
    orient(
        &mut project,
        vec![ids[2].clone()],
        PhotoOrientationAction::ToggleHorizontalMirror,
    );
    assert_eq!(project.projection(), undone);
    assert_eq!(project.redo().unwrap().state.album, reset.state.album);
}

#[test]
fn v3_migrates_only_in_memory_and_explicit_save_matches_the_v4_golden_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Legado.myalbuns");
    let input = include_bytes!("fixtures/project_document_v3_photo_migration_input.myalbuns");
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v4_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    fs::write(&path, input).unwrap();
    let mut project = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), input);
    assert_eq!(project.revision(), 9);
    assert!(!project.has_unsaved_changes());
    let before = project.projection();
    assert_eq!(
        before.state.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .transform
            .quarter_turns,
        0
    );
    assert!(!before.state.can_undo);
    project.save(9).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(project.projection(), before);
}

#[test]
fn v4_rejects_unknown_missing_invalid_orientation_and_future_schemas_without_writing() {
    use myalbuns_core::{DocumentFailure, LoadProjectError, LoadProjectRequest};
    let valid: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v4_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    let transform = "/project/sheets/0/frames/0/photo/transform";
    let mut cases = Vec::new();
    for value in [
        serde_json::json!(-1),
        serde_json::json!(4),
        serde_json::json!(1.5),
        serde_json::json!("1"),
        serde_json::Value::Null,
    ] {
        let mut invalid = valid.clone();
        invalid.pointer_mut(transform).unwrap()["quarterTurns"] = value;
        cases.push(invalid);
    }
    for field in ["quarterTurns", "mirrorX"] {
        let mut invalid = valid.clone();
        invalid
            .pointer_mut(transform)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        cases.push(invalid);
    }
    let mut unknown = valid.clone();
    unknown.pointer_mut(transform).unwrap()["angle"] = serde_json::json!(90);
    cases.push(unknown);
    let mut invalid_mirror = valid.clone();
    invalid_mirror.pointer_mut(transform).unwrap()["mirrorX"] = serde_json::json!(1);
    cases.push(invalid_mirror);
    let root = tempfile::tempdir().unwrap();
    for (index, invalid) in cases.iter().enumerate() {
        let path = root.path().join(format!("invalid-{index}.myalbuns"));
        let bytes = serde_json::to_vec(invalid).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            core(root.path()).load_persisted_revision(LoadProjectRequest::new(location(&path))),
            Err(LoadProjectError::Document(
                DocumentFailure::InvalidProjectDocument
            ))
        ));
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    let mut future = valid.clone();
    future["schemaVersion"] = serde_json::json!(5);
    let path = root.path().join("future.myalbuns");
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        core(root.path())
            .load_persisted_revision(LoadProjectRequest::new(location(&path)))
            .unwrap_err(),
        LoadProjectError::Document(DocumentFailure::UnsupportedFutureSchema { version: 5 })
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn public_orientation_projections_match_the_visual_corpus() {
    use myalbuns_core::{
        FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
    };
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    for (id, [x, y, width, height]) in ids.iter().zip([
        [35_000, 55_000, 170_000, 125_000],
        [350_000, 40_000, 130_000, 190_000],
        [235_000, 185_000, 85_000, 65_000],
    ]) {
        for resize in [true, false] {
            let rect = project.projection().state.album.sheets[0]
                .frames
                .iter()
                .find(|frame| &frame.id == id)
                .unwrap()
                .rect
                .clone();
            let gesture = if resize {
                FrameGeometryGesture::Resize {
                    handle: FrameResizeHandle::BottomRight,
                    delta_x_um: width - rect.width,
                    delta_y_um: height - rect.height,
                    preserve_aspect_ratio: false,
                    from_center: false,
                }
            } else {
                FrameGeometryGesture::Move {
                    delta_x_um: x - rect.x,
                    delta_y_um: y - rect.y,
                }
            };
            project
                .apply(ProjectIntent::EditFrameGeometry {
                    edit: FrameGeometryEdit {
                        frames: vec![FrameGeometryTarget {
                            frame_id: id.clone(),
                            expected_rect: rect,
                        }],
                        gesture,
                    },
                })
                .unwrap();
        }
    }
    project.save(project.revision()).unwrap();
    let before = project.projection();
    let mut states = serde_json::Map::new();
    states.insert("neutral".into(), serde_json::to_value(&before).unwrap());
    let mut transitions = Vec::new();
    let single = vec![ids[0].clone()];
    use PhotoOrientationAction::{
        ResetRotation as Reset, RotateCounterClockwise as Rotate, ToggleHorizontalMirror as Mirror,
    };
    for (from, to, selected, action) in [
        ("neutral", "single-rotated", &single, Rotate),
        ("single-rotated", "single-both", &single, Mirror),
        ("single-both", "group-rotated", &ids, Rotate),
        ("group-rotated", "group-both", &ids, Mirror),
        ("group-both", "group-mirrored", &ids, Reset),
        ("group-mirrored", "neutral", &ids, Mirror),
        ("neutral", "single-rotated", &single, Rotate),
        ("single-rotated", "single-both", &single, Mirror),
        ("single-both", "single-mirrored", &single, Reset),
        ("single-mirrored", "neutral", &single, Mirror),
    ] {
        assert_eq!(
            serde_json::to_value(project.projection().state.album).unwrap(),
            states[from]["state"]["album"]
        );
        orient(&mut project, selected.clone(), action);
        let next = serde_json::to_value(project.projection()).unwrap();
        if let Some(existing) = states.get(to) {
            assert_eq!(existing["state"]["album"], next["state"]["album"]);
        } else {
            states.insert(to.into(), next);
        }
        let transition =
            serde_json::json!({"from": from, "to": to, "frameIds": selected, "action": action});
        if !transitions.contains(&transition) {
            transitions.push(transition);
        }
    }
    let mut text = serde_json::to_string(
        &serde_json::json!({"states": states, "transitions": transitions,
        "single": single, "group": ids, "placeholders": [ids[2]]}),
    )
    .unwrap()
    .replace(&before.state.project_id, "photo-orientation-project");
    for (index, sheet) in before.state.album.sheets.iter().enumerate() {
        text = text.replace(&sheet.id, &format!("sheet-{:03}", index + 1));
        for (frame_index, frame) in sheet.frames.iter().enumerate() {
            text = text.replace(&frame.id, &format!("orientation-frame-{frame_index}"));
        }
    }
    text = text.replace(
        &before.state.album.media[0].id.to_string(),
        "00000000-0000-4000-8000-000000000001",
    );
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&text).unwrap())
            .unwrap()
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/photo-orientation-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_PHOTO_ORIENTATION_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
