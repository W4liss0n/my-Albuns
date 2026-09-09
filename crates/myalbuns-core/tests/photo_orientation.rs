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
fn black_and_white_is_per_photo_and_mixed_selection_makes_one_reversible_choice() {
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    let original = project.projection();
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![ids[0].clone()],
        })
        .unwrap();
    let mixed = project.projection();
    assert!(
        mixed.state.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .transform
            .black_and_white
    );
    assert!(
        !mixed.state.album.sheets[0].frames[1]
            .photo
            .as_ref()
            .unwrap()
            .transform
            .black_and_white
    );
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: ids.clone(),
        })
        .unwrap();
    let enabled = project.projection();
    assert_eq!(enabled.state.revision, mixed.state.revision + 1);
    for (old, new) in mixed.state.album.sheets[0]
        .frames
        .iter()
        .zip(&enabled.state.album.sheets[0].frames)
    {
        let mut expected = old.clone();
        if let Some(photo) = &mut expected.photo {
            photo.transform.black_and_white = true;
        }
        assert_eq!(&expected, new);
    }
    assert!(
        enabled.composition.sheets[0]
            .frames
            .iter()
            .filter_map(|frame| frame.photo.as_ref())
            .all(|photo| photo.black_and_white)
    );
    assert_eq!(project.undo().unwrap().state.album, mixed.state.album);
    assert_eq!(project.redo().unwrap().state.album, enabled.state.album);
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: ids.clone(),
        })
        .unwrap();
    assert_eq!(project.projection().state.album, original.state.album);
    project.undo().unwrap();
    let before_placeholder = project.projection();
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![ids[2].clone()],
        })
        .unwrap();
    assert_eq!(project.projection(), before_placeholder);
    assert_eq!(
        fs::read(root.path().join("Foto.jpg")).unwrap(),
        b"original unchanged"
    );
}

#[test]
fn fine_angle_preview_and_commit_share_the_same_composition_and_one_history_step() {
    use myalbuns_core::PhotoAngleEdit;
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
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: ids[0].clone(),
            delta_pan_x: 0.4,
            delta_pan_y: -0.3,
            delta_zoom: 0.5,
        })
        .unwrap();
    let before = project.projection();
    let edit = PhotoAngleEdit {
        frame_ids: ids,
        angle_tenths: 124,
    };
    let preview = project.preview_photo_angle(&edit).unwrap();
    assert_eq!(preview[1].photo.as_ref().unwrap().rotation_degrees, -12.4);
    assert_eq!(project.projection(), before);
    project
        .apply(ProjectIntent::SetPhotoAngle { edit: edit.clone() })
        .unwrap();
    let after = project.projection();
    assert_eq!(after.state.revision, before.state.revision + 1);
    assert_eq!(preview, after.composition.sheets[0].frames);
    for (old, new) in before.state.album.sheets[0]
        .frames
        .iter()
        .zip(&after.state.album.sheets[0].frames)
    {
        let mut expected = old.clone();
        if let Some(photo) = &mut expected.photo {
            photo.transform.fine_rotation_degrees = 12.4;
        }
        assert_eq!(&expected, new);
    }
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, after.state.album);
    project
        .apply(ProjectIntent::SetPhotoAngle { edit })
        .unwrap();
    assert_eq!(project.projection(), after);
}

fn record_angle_previews(
    state: &str,
    project: &EditableProject,
    frame_ids: &[String],
    previews: &mut Vec<serde_json::Value>,
) {
    for angle_tenths in [0, 1, 2, 10, 120, 123, 40, 450, -40, -450, -20, -230, -234] {
        let edit = myalbuns_core::PhotoAngleEdit {
            frame_ids: frame_ids.to_vec(),
            angle_tenths,
        };
        let before = project.projection();
        let frames = project.preview_photo_angle(&edit).unwrap();
        assert_eq!(project.projection(), before);
        previews.push(serde_json::json!({ "from": state, "edit": edit, "frames": frames }));
    }
}

#[test]
fn fine_angle_rejects_invalid_ranges_and_selections_without_changing_history() {
    use myalbuns_core::{CoreError, PhotoAngleEdit};
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    let other = project
        .apply_with_outcome(ProjectIntent::AddFrame {
            sheet_id: project.projection().state.album.sheets[1].id.clone(),
        })
        .unwrap()
        .affected_frame_id
        .unwrap();
    let before = project.projection();
    for edit in [
        PhotoAngleEdit {
            frame_ids: vec![id.clone()],
            angle_tenths: -451,
        },
        PhotoAngleEdit {
            frame_ids: vec![id.clone()],
            angle_tenths: 451,
        },
        PhotoAngleEdit {
            frame_ids: vec![],
            angle_tenths: 0,
        },
        PhotoAngleEdit {
            frame_ids: vec![id.clone(), id.clone()],
            angle_tenths: 1,
        },
        PhotoAngleEdit {
            frame_ids: vec![id.clone(), other],
            angle_tenths: 1,
        },
        PhotoAngleEdit {
            frame_ids: vec![id.clone(), "missing".into()],
            angle_tenths: 1,
        },
    ] {
        let expected = if edit.angle_tenths.abs() > 450 {
            CoreError::InvalidPhotoAngle
        } else {
            CoreError::InvalidPhotoOrientationSelection
        };
        assert_eq!(project.preview_photo_angle(&edit).as_ref(), Err(&expected));
        assert_eq!(
            project.apply(ProjectIntent::SetPhotoAngle { edit }),
            Err(expected)
        );
        assert_eq!(project.projection(), before);
    }
    for angle_tenths in [-450, 450] {
        project
            .apply(ProjectIntent::SetPhotoAngle {
                edit: PhotoAngleEdit {
                    frame_ids: vec![id.clone()],
                    angle_tenths,
                },
            })
            .unwrap();
        assert_eq!(
            project.projection().state.album.sheets[0].frames[0]
                .photo
                .as_ref()
                .unwrap()
                .transform
                .fine_rotation_degrees,
            f32::from(angle_tenths) / 10.0
        );
    }
}

#[test]
fn v4_angle_migration_preserves_orientation_and_adds_only_neutral_angle_on_save() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Legado.myalbuns");
    let input = include_bytes!("fixtures/project_document_v4_angle_migration_input.myalbuns");
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v6_angle_migration_expected.myalbuns"
    ))
    .unwrap();
    fs::write(&path, input).unwrap();
    let mut project = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let before = project.projection();
    let transform = &before.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .transform;
    assert_eq!(
        (
            transform.quarter_turns,
            transform.mirror_x,
            transform.fine_rotation_degrees
        ),
        (3, true, 0.0)
    );
    assert_eq!(
        (transform.pan_x, transform.pan_y, transform.user_zoom),
        (0.25, -0.5, 1.75)
    );
    assert!(!project.has_unsaved_changes());
    assert!(!before.state.can_undo);
    assert_eq!(fs::read(&path).unwrap(), input);
    project.save(9).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(project.projection(), before);
}

#[test]
fn v5_requires_an_integer_angle_in_range_and_keeps_legacy_dtos_closed() {
    use myalbuns_core::{DocumentFailure, LoadProjectError, LoadProjectRequest};
    let valid: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v5_angle_migration_expected.myalbuns"
    ))
    .unwrap();
    let transform = "/project/sheets/0/frames/0/photo/transform";
    let mut cases = Vec::new();
    for angle in [
        serde_json::json!(-451),
        serde_json::json!(451),
        serde_json::json!(12.5),
        serde_json::json!("125"),
        serde_json::Value::Null,
    ] {
        let mut invalid = valid.clone();
        invalid.pointer_mut(transform).unwrap()["angleTenths"] = angle;
        cases.push(invalid);
    }
    let mut missing = valid.clone();
    missing
        .pointer_mut(transform)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("angleTenths");
    cases.push(missing);
    let mut unknown = valid.clone();
    unknown.pointer_mut(transform).unwrap()["angleDegrees"] = serde_json::json!(0);
    cases.push(unknown);
    let mut legacy = valid.clone();
    legacy["schemaVersion"] = serde_json::json!(4);
    cases.push(legacy);
    let root = tempfile::tempdir().unwrap();
    for (index, value) in cases.into_iter().enumerate() {
        let path = root.path().join(format!("Invalid-{index}.myalbuns"));
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            core(root.path()).load_persisted_revision(LoadProjectRequest::new(location(&path))),
            Err(LoadProjectError::Document(
                DocumentFailure::InvalidProjectDocument
            ))
        ));
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn v5_migration_preserves_photo_adjustments_and_v6_requires_a_boolean_effect() {
    use myalbuns_core::{DocumentFailure, LoadProjectError, LoadProjectRequest};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Efeito.myalbuns");
    let input = include_bytes!("fixtures/project_document_v5_effect_migration_input.myalbuns");
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v6_effect_migration_expected.myalbuns"
    ))
    .unwrap();
    fs::write(&path, input).unwrap();
    let mut project = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let before = project.projection();
    let photo = before.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap();
    assert!(!photo.transform.black_and_white);
    assert_eq!(photo.transform.fine_rotation_degrees, -12.3);
    assert!(!before.state.can_undo);
    assert!(!project.has_unsaved_changes());
    assert_eq!(fs::read(&path).unwrap(), input);
    project.save(9).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(project.projection(), before);
    drop(project);
    let transform = "/project/sheets/0/frames/0/photo/transform";
    let mut cases = Vec::new();
    for value in [
        serde_json::json!(1),
        serde_json::json!("true"),
        serde_json::Value::Null,
    ] {
        let mut invalid = expected.clone();
        invalid.pointer_mut(transform).unwrap()["blackAndWhite"] = value;
        cases.push(invalid);
    }
    let mut missing = expected.clone();
    missing
        .pointer_mut(transform)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("blackAndWhite");
    cases.push(missing);
    let mut legacy = expected.clone();
    legacy["schemaVersion"] = serde_json::json!(5);
    cases.push(legacy);
    let mut unknown = expected.clone();
    unknown.pointer_mut(transform).unwrap()["saturation"] = serde_json::json!(0);
    cases.push(unknown);
    for (index, value) in cases.into_iter().enumerate() {
        let path = root.path().join(format!("Invalid-effect-{index}.myalbuns"));
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            core(root.path()).load_persisted_revision(LoadProjectRequest::new(location(&path))),
            Err(LoadProjectError::Document(
                DocumentFailure::InvalidProjectDocument
            ))
        ));
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn fine_angles_fill_every_frame_corner_at_pan_limits_with_any_rotation_and_mirror() {
    use myalbuns_core::{
        FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
        PhotoAngleEdit,
    };
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0].frames[..2]
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    for (id, (width, height)) in ids.iter().zip([(170_000, 125_000), (130_000, 190_000)]) {
        let rect = project.projection().state.album.sheets[0]
            .frames
            .iter()
            .find(|frame| &frame.id == id)
            .unwrap()
            .rect
            .clone();
        project
            .apply(ProjectIntent::EditFrameGeometry {
                edit: FrameGeometryEdit {
                    frames: vec![FrameGeometryTarget {
                        frame_id: id.clone(),
                        expected_rect: rect.clone(),
                    }],
                    gesture: FrameGeometryGesture::Resize {
                        handle: FrameResizeHandle::BottomRight,
                        delta_x_um: width - rect.width,
                        delta_y_um: height - rect.height,
                        preserve_aspect_ratio: false,
                        from_center: false,
                    },
                },
            })
            .unwrap();
    }
    for _ in 0..4 {
        for _ in 0..2 {
            for angle_tenths in [-450, -123, 0, 123, 450] {
                project
                    .apply(ProjectIntent::SetPhotoAngle {
                        edit: PhotoAngleEdit {
                            frame_ids: ids.clone(),
                            angle_tenths,
                        },
                    })
                    .unwrap();
                for zoom in [1.0, 2.0] {
                    for (x, y) in [
                        (-1.0, -1.0),
                        (-1.0, 1.0),
                        (0.0, 0.0),
                        (1.0, -1.0),
                        (1.0, 1.0),
                    ] {
                        for id in &ids {
                            let snapshot = project.projection();
                            let old = &snapshot.state.album.sheets[0]
                                .frames
                                .iter()
                                .find(|frame| &frame.id == id)
                                .unwrap()
                                .photo
                                .as_ref()
                                .unwrap()
                                .transform;
                            project
                                .apply(ProjectIntent::TransformPhoto {
                                    frame_id: id.clone(),
                                    delta_pan_x: x - old.pan_x,
                                    delta_pan_y: y - old.pan_y,
                                    delta_zoom: zoom - old.user_zoom,
                                })
                                .unwrap();
                        }
                        for frame in &project.projection().composition.sheets[0].frames[..2] {
                            let photo = frame.photo.as_ref().unwrap();
                            let angle = f64::from(photo.rotation_degrees).to_radians();
                            let placement = &photo.placement.current;
                            for (corner_x, corner_y) in [
                                (0.0, 0.0),
                                (frame.clip_rect.width as f64, 0.0),
                                (0.0, frame.clip_rect.height as f64),
                                (frame.clip_rect.width as f64, frame.clip_rect.height as f64),
                            ] {
                                let dx = (corner_x - placement.center.x)
                                    * if photo.mirror_x { -1.0 } else { 1.0 };
                                let dy = corner_y - placement.center.y;
                                // Independently invert the renderer's rotation-then-mirror transform.
                                let local_x = dx * angle.cos() + dy * angle.sin();
                                let local_y = -dx * angle.sin() + dy * angle.cos();
                                assert!(
                                    local_x.abs() <= placement.size.width / 2.0 + 0.001
                                        && local_y.abs() <= placement.size.height / 2.0 + 0.001,
                                    "uncovered corner at {} degrees, mirror {}, pan ({x},{y}), zoom {zoom}",
                                    photo.rotation_degrees,
                                    photo.mirror_x
                                );
                            }
                        }
                    }
                }
            }
            orient(
                &mut project,
                ids.clone(),
                PhotoOrientationAction::ToggleHorizontalMirror,
            );
        }
        orient(
            &mut project,
            ids.clone(),
            PhotoOrientationAction::RotateCounterClockwise,
        );
    }
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
fn photo_adjustments_survive_pan_zoom_save_reopen_and_export_without_writing_originals() {
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let initial = project.projection();
    let id = initial.state.album.sheets[0].frames[0].id.clone();
    let frozen = project.render_snapshot();
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![id.clone()],
        })
        .unwrap();
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
    project
        .apply(ProjectIntent::SetPhotoAngle {
            edit: myalbuns_core::PhotoAngleEdit {
                frame_ids: vec![initial.state.album.sheets[0].frames[0].id.clone()],
                angle_tenths: -123,
            },
        })
        .unwrap();
    let expected = project.projection();
    let photo = expected.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap();
    assert_eq!(photo.transform.quarter_turns, 3);
    assert!(photo.transform.mirror_x);
    assert!(photo.transform.black_and_white);
    assert_eq!(frozen.composition, initial.composition);
    assert_eq!(project.render_snapshot().composition, expected.composition);
    project.save(project.revision()).unwrap();
    let bytes = fs::read(root.path().join("Orientação.myalbuns")).unwrap();
    let dto: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(dto["schemaVersion"], 6);
    assert_eq!(
        dto["project"]["sheets"][0]["frames"][0]["photo"]["transform"]["blackAndWhite"],
        true
    );
    assert_eq!(
        dto["project"]["sheets"][0]["frames"][0]["photo"]["transform"]["angleTenths"],
        -123
    );
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
                frame_ids: invalid.clone(),
                action: PhotoOrientationAction::RotateCounterClockwise
            }),
            Err(CoreError::InvalidPhotoOrientationSelection)
        );
        assert_eq!(project.projection(), before);
        assert_eq!(
            project.apply(ProjectIntent::TogglePhotoBlackAndWhite { frame_ids: invalid }),
            Err(CoreError::InvalidPhotoEffectSelection)
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
fn v3_migrates_only_in_memory_and_explicit_save_matches_the_v6_golden_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Legado.myalbuns");
    let input = include_bytes!("fixtures/project_document_v3_photo_migration_input.myalbuns");
    let expected: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v6_photo_migration_expected.myalbuns"
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
    future["schemaVersion"] = serde_json::json!(7);
    let path = root.path().join("future.myalbuns");
    let bytes = serde_json::to_vec(&future).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        core(root.path())
            .load_persisted_revision(LoadProjectRequest::new(location(&path)))
            .unwrap_err(),
        LoadProjectError::Document(DocumentFailure::UnsupportedFutureSchema { version: 7 })
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
    let mut angle_transitions = Vec::new();
    let mut angle_previews = Vec::new();
    record_angle_previews("neutral", &project, &single, &mut angle_previews);
    let keyboard_edit = myalbuns_core::PhotoAngleEdit {
        frame_ids: single.clone(),
        angle_tenths: 1,
    };
    project
        .apply(ProjectIntent::SetPhotoAngle {
            edit: keyboard_edit.clone(),
        })
        .unwrap();
    states.insert(
        "keyboard-angle".into(),
        serde_json::to_value(project.projection()).unwrap(),
    );
    angle_transitions.push(
        serde_json::json!({ "from": "neutral", "to": "keyboard-angle", "edit": keyboard_edit }),
    );
    project.undo().unwrap();
    for (from, to, angle_tenths) in [
        ("neutral", "single-angle", 123),
        ("single-angle", "single-angle-max", 450),
        ("single-angle-max", "single-angle-min", -450),
        ("single-angle-min", "neutral", 0),
        ("neutral", "single-angle", 123),
    ] {
        let edit = myalbuns_core::PhotoAngleEdit {
            frame_ids: single.clone(),
            angle_tenths,
        };
        let preview = project.preview_photo_angle(&edit).unwrap();
        project
            .apply(ProjectIntent::SetPhotoAngle { edit: edit.clone() })
            .unwrap();
        let next = serde_json::to_value(project.projection()).unwrap();
        assert_eq!(
            preview[0],
            project.projection().composition.sheets[0].frames[0]
        );
        if let Some(existing) = states.get(to) {
            assert_eq!(existing["state"]["album"], next["state"]["album"]);
        } else {
            states.insert(to.into(), next);
            record_angle_previews(to, &project, &single, &mut angle_previews);
        }
        let transition = serde_json::json!({ "from": from, "to": to, "edit": edit });
        if !angle_transitions.contains(&transition) {
            angle_transitions.push(transition);
        }
    }
    let reset_edit = myalbuns_core::PhotoAngleEdit {
        frame_ids: single.clone(),
        angle_tenths: 0,
    };
    project
        .apply(ProjectIntent::SetPhotoAngle {
            edit: reset_edit.clone(),
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(project.projection().state.album).unwrap(),
        states["neutral"]["state"]["album"]
    );
    angle_transitions
        .push(serde_json::json!({ "from": "single-angle", "to": "neutral", "edit": reset_edit }));
    project.undo().unwrap();
    for (from, to, action) in [
        ("single-angle", "single-angle-rotated", Rotate),
        ("single-angle-rotated", "single-angle-both", Mirror),
    ] {
        orient(&mut project, single.clone(), action);
        states.insert(
            to.into(),
            serde_json::to_value(project.projection()).unwrap(),
        );
        transitions.push(
            serde_json::json!({ "from": from, "to": to, "frameIds": single, "action": action }),
        );
    }
    record_angle_previews("single-angle-both", &project, &ids, &mut angle_previews);
    for (from, to, angle_tenths) in [
        ("single-angle-both", "group-angle", -234),
        ("group-angle", "single-both", 0),
    ] {
        let edit = myalbuns_core::PhotoAngleEdit {
            frame_ids: ids.clone(),
            angle_tenths,
        };
        project
            .apply(ProjectIntent::SetPhotoAngle { edit: edit.clone() })
            .unwrap();
        let next = serde_json::to_value(project.projection()).unwrap();
        if let Some(existing) = states.get(to) {
            assert_eq!(existing["state"]["album"], next["state"]["album"]);
        } else {
            states.insert(to.into(), next);
        }
        angle_transitions.push(serde_json::json!({ "from": from, "to": to, "edit": edit }));
        record_angle_previews(to, &project, &ids, &mut angle_previews);
    }
    let mut effect_transitions = Vec::new();
    for (from, to, selected) in [
        ("single-both", "single-both-black-white", &single),
        ("single-both-black-white", "group-both-black-white", &ids),
        ("group-both-black-white", "single-both", &ids),
    ] {
        project
            .apply(ProjectIntent::TogglePhotoBlackAndWhite {
                frame_ids: selected.clone(),
            })
            .unwrap();
        let next = serde_json::to_value(project.projection()).unwrap();
        if let Some(existing) = states.get(to) {
            assert_eq!(existing["state"]["album"], next["state"]["album"]);
        } else {
            states.insert(to.into(), next);
        }
        effect_transitions
            .push(serde_json::json!({ "from": from, "to": to, "frameIds": selected }));
    }
    orient(&mut project, single.clone(), Reset);
    orient(&mut project, single.clone(), Mirror);
    for (from, to, selected) in [
        ("neutral", "single-black-white", &single),
        ("single-black-white", "group-black-white", &ids),
        ("group-black-white", "neutral", &ids),
        ("neutral", "group-black-white", &ids),
    ] {
        project
            .apply(ProjectIntent::TogglePhotoBlackAndWhite {
                frame_ids: selected.clone(),
            })
            .unwrap();
        let next = serde_json::to_value(project.projection()).unwrap();
        if let Some(existing) = states.get(to) {
            assert_eq!(existing["state"]["album"], next["state"]["album"]);
        } else {
            states.insert(to.into(), next);
        }
        effect_transitions
            .push(serde_json::json!({ "from": from, "to": to, "frameIds": selected }));
    }
    let mut text = serde_json::to_string(
        &serde_json::json!({"states": states, "transitions": transitions,
        "angleTransitions": angle_transitions, "anglePreviews": angle_previews,
        "effectTransitions": effect_transitions,
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
