#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, EditableProject, EditorProjection,
    FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, ImportPhoto, InitialProject,
    OpenProjectRequest, PhotoDropTarget, PhotoPlacementMode, PhotoSourceMetadata, ProjectCore,
    ProjectIntent, ProjectLocation,
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

fn project_with_frames(root: &Path) -> EditableProject {
    let mut project = core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Exclusão.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let photo = root.join("Foto.jpg");
    fs::write(&photo, b"original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(
            photo,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
            )
            .unwrap(),
        ))
        .unwrap();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    for (index, delta_x_um) in [-90_000, -30_000, 30_000, 90_000].into_iter().enumerate() {
        let added = project
            .apply_with_outcome(ProjectIntent::AddFrame {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
        let frame_id = added.affected_frame_id.unwrap();
        let frame = added.projection.state.album.sheets[0]
            .frames
            .iter()
            .find(|frame| frame.id == frame_id)
            .unwrap();
        if index % 2 == 0 {
            project
                .apply(ProjectIntent::DropPhoto {
                    sheet_id: sheet_id.clone(),
                    media_id: imported.media_id,
                    x_um: frame.rect.x + frame.rect.width / 2,
                    y_um: frame.rect.y + frame.rect.height / 2,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
        }
        project
            .apply(ProjectIntent::EditFrameGeometry {
                edit: FrameGeometryEdit {
                    frames: vec![FrameGeometryTarget {
                        frame_id,
                        expected_rect: frame.rect.clone(),
                    }],
                    gesture: FrameGeometryGesture::Move {
                        delta_x_um,
                        delta_y_um: 0,
                    },
                },
            })
            .unwrap();
    }
    project
}

#[test]
fn deleting_a_mixed_selection_preserves_remaining_frames_media_and_frozen_export() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: id,
            delta_pan_x: 0.2,
            delta_pan_y: -0.1,
            delta_zoom: 0.5,
        })
        .unwrap();
    let before = project.projection();
    let frozen = project.render_snapshot();
    let frames = &before.state.album.sheets[0].frames;
    let deleted = project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: vec![frames[2].id.clone(), frames[1].id.clone()],
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    assert_eq!(deleted.state.revision, before.state.revision + 1);
    assert!(deleted.state.dirty);
    assert_eq!(deleted.state.album.media, before.state.album.media);
    assert_eq!(deleted.state.album.sheets[1], before.state.album.sheets[1]);
    let remaining = &deleted.state.album.sheets[0].frames;
    assert_eq!(remaining.len(), 2);
    for (index, original_index) in [0, 3].into_iter().enumerate() {
        let mut expected = frames[original_index].clone();
        expected.z_index = index as u32;
        assert_eq!(remaining[index], expected);
        let mut composed = before.composition.sheets[0].frames[original_index].clone();
        composed.z_index = index as u32;
        assert_eq!(deleted.composition.sheets[0].frames[index], composed);
    }
    assert_eq!(deleted.media_usage[0].count, 1);
    assert_eq!(frozen.composition, before.composition);
    assert_eq!(project.render_snapshot().composition, deleted.composition);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, deleted.state.album);
    project.save(project.revision()).unwrap();
    let saved = project.render_snapshot();
    drop(project);
    let mut reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(
            &root.path().join("Exclusão.myalbuns"),
        )))
        .unwrap();
    reopened
        .observe_photo_source(
            deleted.state.album.media[0].id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(reopened.projection().state.album, deleted.state.album);
    assert_eq!(reopened.render_snapshot().composition, saved.composition);
    assert_eq!(fs::read(root.path().join("Foto.jpg")).unwrap(), b"original");
}

#[test]
fn deleting_the_last_frames_keeps_the_sheet_and_imported_photo_available() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let before = project.projection();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    let deleted = project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: ids,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    assert_eq!(
        deleted.state.album.sheets.len(),
        before.state.album.sheets.len()
    );
    assert!(deleted.state.album.sheets[0].frames.is_empty());
    assert!(deleted.composition.sheets[0].frames.is_empty());
    assert_eq!(deleted.state.album.media, before.state.album.media);
    assert_eq!(deleted.media_usage[0].count, 0);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, deleted.state.album);
    let reused = project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: before.state.album.sheets[0].id.clone(),
            media_id: before.state.album.media[0].id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    assert_eq!(reused.state.album.sheets[0].frames.len(), 1);
    assert_eq!(reused.media_usage[0].count, 1);
}

#[test]
fn invalid_deletion_never_removes_part_of_a_selection_or_discards_redo() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let sheet_id = project.projection().state.album.sheets[1].id.clone();
    let other = project
        .apply_with_outcome(ProjectIntent::AddFrame { sheet_id })
        .unwrap()
        .affected_frame_id
        .unwrap();
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: vec![id.clone()],
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    project.undo().unwrap();
    let before = project.projection();
    for frame_ids in [
        vec![],
        vec![id.clone(), id.to_uppercase()],
        vec![id.clone(), other],
        vec![id.clone(), "00000000-0000-0000-0000-000000000000".into()],
        vec![id, "invalid".into()],
    ] {
        assert_eq!(
            project.apply(ProjectIntent::DeleteFrames {
                frame_ids,
                mode: PhotoPlacementMode::Edit
            }),
            Err(CoreError::InvalidFrameDeletionSelection)
        );
        assert_eq!(project.projection(), before);
    }
    assert!(project.redo().is_some());
}

#[test]
fn deleting_the_top_frame_exposes_the_remaining_photo_drop_target() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let before = project.projection();
    let sheet_id = &before.state.album.sheets[0].id;
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        project
            .photo_drop_target(sheet_id, 300_000, 150_000)
            .unwrap(),
        PhotoDropTarget::Frame {
            frame_id: ids[3].clone()
        }
    );
    project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: vec![ids[3].clone()],
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    assert_eq!(
        project
            .photo_drop_target(sheet_id, 300_000, 150_000)
            .unwrap(),
        PhotoDropTarget::Frame {
            frame_id: ids[2].clone()
        }
    );
}

#[test]
fn deletion_preview_corpus_matches_the_core() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let before = project.projection();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    let normalize = |value: &EditorProjection| {
        let mut value = value.clone();
        let media_id = "00000000-0000-4000-8000-000000000001".parse().unwrap();
        value.state.project_id = "frame-deletion-project".into();
        for (index, sheet) in value.state.album.sheets.iter_mut().enumerate() {
            sheet.id = format!("sheet-{:03}", index + 1);
            for frame in &mut sheet.frames {
                frame.id = format!(
                    "delete-frame-{}",
                    ids.iter().position(|id| id == &frame.id).unwrap()
                );
                if let Some(photo) = &mut frame.photo {
                    photo.media_id = media_id;
                }
            }
        }
        for (index, sheet) in value.composition.sheets.iter_mut().enumerate() {
            sheet.sheet_id = format!("sheet-{:03}", index + 1);
            for frame in &mut sheet.frames {
                frame.frame_id = format!(
                    "delete-frame-{}",
                    ids.iter().position(|id| id == &frame.frame_id).unwrap()
                );
                if let Some(photo) = &mut frame.photo {
                    photo.media_id = media_id;
                }
            }
        }
        for media in &mut value.state.album.media {
            media.id = media_id;
        }
        for usage in &mut value.media_usage {
            usage.media_id = media_id;
        }
        value
    };
    let mut cases = Vec::new();
    for (name, selected) in [
        ("single", vec![3]),
        ("group", vec![1, 2]),
        ("all", vec![0, 1, 2, 3]),
    ] {
        let after = project
            .apply(ProjectIntent::DeleteFrames {
                frame_ids: selected.iter().map(|index| ids[*index].clone()).collect(),
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
        cases.push(serde_json::json!({"name": name, "selectedFrameIds": selected.iter().map(|index| format!("delete-frame-{index}")).collect::<Vec<_>>(), "after": normalize(&after)}));
        project.undo().unwrap();
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(
            &serde_json::json!({"before": normalize(&before), "cases": cases})
        )
        .unwrap()
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/frame-deletion-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_DELETION_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
