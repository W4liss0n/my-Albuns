#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, EditableProject, EditorProjection,
    FrameSnapshot, ImportPhoto, InitialProject, PhotoDropTarget, PhotoOrientationAction,
    PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

// The automatic Layouts keep a margin, so this corner of a Sheet is free.
const FREE_POINT: (i64, i64) = (1_000, 1_000);

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

/// Two Photos on the first Sheet, one on the second, placed in normal mode.
fn project(root: &Path) -> (EditableProject, Vec<String>) {
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Mover.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let media_ids = [(600, 400), (300, 700), (640, 480)]
        .into_iter()
        .enumerate()
        .map(|(index, (width, height))| {
            let path = root.join(format!("Foto-{index}.jpg"));
            fs::write(&path, format!("original-{index}")).unwrap();
            let palette = ["#173B4B", "#84AAA7", "#E7C58B"].map(String::from);
            project
                .import_photo(ImportPhoto::new(
                    path,
                    PhotoSourceMetadata::new(width, height, palette).unwrap(),
                ))
                .unwrap()
                .media_id
        })
        .collect::<Vec<_>>();
    let sheets = sheet_ids(&project.projection());
    for (media_id, sheet_id) in media_ids.iter().zip([&sheets[0], &sheets[0], &sheets[1]]) {
        project
            .apply(ProjectIntent::DropPhoto {
                sheet_id: sheet_id.clone(),
                media_id: *media_id,
                x_um: 300_000,
                y_um: 150_000,
                mode: PhotoPlacementMode::Normal,
            })
            .unwrap();
    }
    (project, sheets)
}

fn sheet_ids(projection: &EditorProjection) -> Vec<String> {
    projection
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect()
}

fn frames(projection: &EditorProjection, sheet: usize) -> &[FrameSnapshot] {
    &projection.state.album.sheets[sheet].frames
}

fn frame_showing(projection: &EditorProjection, sheet: usize, media: usize) -> &FrameSnapshot {
    let media_id = projection.state.album.media[media].id;
    frames(projection, sheet)
        .iter()
        .find(|frame| {
            frame
                .photo
                .as_ref()
                .is_some_and(|photo| photo.media_id == media_id)
        })
        .unwrap()
}

fn move_intent(frame_id: &str, sheet_id: &str, (x_um, y_um): (i64, i64)) -> ProjectIntent {
    ProjectIntent::MoveFrameToSheet {
        frame_id: frame_id.into(),
        sheet_id: sheet_id.into(),
        x_um,
        y_um,
    }
}

fn lock_layout(project: &mut EditableProject, sheet_id: &str) {
    let query = project.query_layouts(sheet_id).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: myalbuns_core::LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
}

#[test]
fn a_photo_dropped_on_the_free_area_of_another_sheet_moves_there_with_its_adjustments() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheets) = project(root.path());
    let source = frame_showing(&project.projection(), 0, 0).id.clone();
    for intent in [
        ProjectIntent::OrientPhotos {
            frame_ids: vec![source.clone()],
            action: PhotoOrientationAction::RotateCounterClockwise,
        },
        ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![source.clone()],
        },
    ] {
        project.apply(intent).unwrap();
    }
    let before = project.projection();
    let adjusted = frame_showing(&before, 0, 0)
        .photo
        .clone()
        .unwrap()
        .transform;
    assert_eq!(
        project.photo_drop_target(&sheets[1], FREE_POINT.0, FREE_POINT.1),
        Ok(PhotoDropTarget::Sheet {
            sheet_id: sheets[1].clone()
        })
    );

    let moved = project
        .apply_with_outcome(move_intent(&source, &sheets[1], FREE_POINT))
        .unwrap();
    let after = &moved.projection;
    assert_eq!(frames(after, 0).len(), 1);
    assert_eq!(
        frame_showing(after, 0, 1).id,
        frame_showing(&before, 0, 1).id
    );
    assert_eq!(frames(after, 1).len(), 2);
    let arrived = frame_showing(after, 1, 0);
    assert_eq!(
        moved.affected_frame_id.as_deref(),
        Some(arrived.id.as_str())
    );
    assert_eq!(moved.affected_sheet_id.as_deref(), Some(sheets[1].as_str()));
    assert_eq!(arrived.photo.as_ref().unwrap().transform, adjusted);
    // The landscape Photo was turned a quarter, so it arrives in a vertical Frame.
    assert!(arrived.rect.height > arrived.rect.width);

    // One History entry restores both Sheets.
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, after.state.album);
}

#[test]
fn a_locked_origin_keeps_the_frame_as_a_placeholder() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheets) = project(root.path());
    lock_layout(&mut project, &sheets[0]);
    let before = project.projection();
    let source = frame_showing(&before, 0, 0).id.clone();

    let after = project
        .apply(move_intent(&source, &sheets[1], FREE_POINT))
        .unwrap();
    let origin = frames(&after, 0);
    assert_eq!(origin.len(), frames(&before, 0).len());
    let emptied = origin.iter().find(|frame| frame.id == source).unwrap();
    assert!(emptied.photo.is_none());
    assert_eq!(emptied.rect, frame_showing(&before, 0, 0).rect);
    frame_showing(&after, 1, 0);
}

#[test]
fn moving_the_only_photo_of_a_sheet_leaves_it_empty() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheets) = project(root.path());
    let source = frame_showing(&project.projection(), 1, 2).id.clone();

    let after = project
        .apply(move_intent(&source, &sheets[0], FREE_POINT))
        .unwrap();
    assert!(frames(&after, 1).is_empty());
    assert_eq!(frames(&after, 0).len(), 3);
    frame_showing(&after, 0, 2);
}

#[test]
fn frames_same_sheet_outside_points_and_locked_destinations_do_not_receive_a_move() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheets) = project(root.path());
    let before = project.projection();
    let source = frame_showing(&before, 0, 0).id.clone();
    let occupied = &frame_showing(&before, 1, 2).rect;
    let over_frame = (
        occupied.x + occupied.width / 2,
        occupied.y + occupied.height / 2,
    );
    for (sheet, point) in [
        (&sheets[1], over_frame),
        (&sheets[0], FREE_POINT),
        (&sheets[1], (-1, FREE_POINT.1)),
    ] {
        assert_eq!(
            project.apply(move_intent(&source, sheet, point)),
            Err(CoreError::InvalidFrameMove)
        );
    }
    lock_layout(&mut project, &sheets[1]);
    let locked = project.projection();
    assert_eq!(
        project.apply(move_intent(&source, &sheets[1], FREE_POINT)),
        Err(CoreError::InvalidFrameMove)
    );
    assert_eq!(project.projection().state.album, locked.state.album);
    assert_eq!(locked.state.album.sheets[0], before.state.album.sheets[0]);
}
