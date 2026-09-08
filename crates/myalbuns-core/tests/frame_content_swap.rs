#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, EditableProject, EditorProjection,
    FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle, ImportPhoto,
    InitialProject, OpenProjectRequest, PhotoPlacementMode, PhotoSourceMetadata, ProjectCore,
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

fn metadata(index: usize) -> PhotoSourceMetadata {
    let (width, height, palette) = if index == 0 {
        (600, 400, ["#173B4B", "#84AAA7", "#E7C58B"])
    } else {
        (300, 700, ["#66314A", "#D88370", "#EAD8B5"])
    };
    PhotoSourceMetadata::new(width, height, palette.map(String::from)).unwrap()
}

fn edit_geometry(project: &mut EditableProject, frame_id: &str, gesture: FrameGeometryGesture) {
    let projection = project.projection();
    let frame = projection.state.album.sheets[0]
        .frames
        .iter()
        .find(|frame| frame.id == frame_id)
        .unwrap();
    project
        .apply(ProjectIntent::EditFrameGeometry {
            edit: FrameGeometryEdit {
                frames: vec![FrameGeometryTarget {
                    frame_id: frame_id.into(),
                    expected_rect: frame.rect.clone(),
                }],
                gesture,
            },
        })
        .unwrap();
}

fn project_with_frames(root: &Path, same_media: bool) -> EditableProject {
    let mut project = core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Troca.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let media_ids = (0..2)
        .map(|index| {
            let path = root.join(format!("Foto-{index}.jpg"));
            fs::write(&path, format!("original-{index}")).unwrap();
            project
                .import_photo(ImportPhoto::new(path, metadata(index)))
                .unwrap()
                .media_id
        })
        .collect::<Vec<_>>();
    let sheet_id = project.projection().state.album.sheets[0].id.clone();
    for (index, [x, y, width, height]) in [
        [30_000, 90_000, 160_000, 100_000],
        [210_000, 40_000, 90_000, 200_000],
        [330_000, 100_000, 110_000, 100_000],
        [470_000, 100_000, 100_000, 100_000],
    ]
    .into_iter()
    .enumerate()
    {
        let added = project
            .apply_with_outcome(ProjectIntent::AddFrame {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
        let frame_id = added.affected_frame_id.unwrap();
        let rect = &added.projection.state.album.sheets[0]
            .frames
            .last()
            .unwrap()
            .rect;
        if index < 2 {
            project
                .apply(ProjectIntent::DropPhoto {
                    sheet_id: sheet_id.clone(),
                    media_id: media_ids[if same_media { 0 } else { index }],
                    x_um: rect.x + rect.width / 2,
                    y_um: rect.y + rect.height / 2,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
            let (pan_x, pan_y, zoom) = if index == 0 {
                (-0.6, 0.3, 0.6)
            } else {
                (0.5, -0.4, 1.1)
            };
            project
                .apply(ProjectIntent::TransformPhoto {
                    frame_id: frame_id.clone(),
                    delta_pan_x: pan_x,
                    delta_pan_y: pan_y,
                    delta_zoom: zoom,
                })
                .unwrap();
        }
        edit_geometry(
            &mut project,
            &frame_id,
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: width - rect.width,
                delta_y_um: height - rect.height,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        );
        edit_geometry(
            &mut project,
            &frame_id,
            FrameGeometryGesture::Move {
                delta_x_um: x - rect.x,
                delta_y_um: y - rect.y,
            },
        );
    }
    project
}

fn assert_photos_fill_frames(projection: &EditorProjection) {
    for frame in &projection.composition.sheets[0].frames {
        let Some(photo) = &frame.photo else { continue };
        let placement = &photo.placement.current;
        let epsilon = 0.001;
        assert!(placement.center.x - placement.size.width / 2.0 <= epsilon);
        assert!(placement.center.y - placement.size.height / 2.0 <= epsilon);
        assert!(
            placement.center.x + placement.size.width / 2.0 + epsilon
                >= frame.clip_rect.width as f64
        );
        assert!(
            placement.center.y + placement.size.height / 2.0 + epsilon
                >= frame.clip_rect.height as f64
        );
    }
}

#[test]
fn swapping_photos_or_a_placeholder_preserves_frame_identity_history_and_saved_export() {
    for destination in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let mut project = project_with_frames(root.path(), false);
        let before = project.projection();
        let frozen = project.render_snapshot();
        let frames = &before.state.album.sheets[0].frames;
        let swapped = project
            .apply(ProjectIntent::SwapFrameContents {
                frame_ids: vec![frames[destination].id.clone(), frames[0].id.clone()],
            })
            .unwrap();
        let mut expected = before.state.album.clone();
        expected.sheets[0].frames[0].photo = frames[destination].photo.clone();
        expected.sheets[0].frames[destination].photo = frames[0].photo.clone();
        assert_eq!(swapped.state.album, expected);
        assert_eq!(swapped.media_usage, before.media_usage);
        assert_eq!(swapped.state.revision, before.state.revision + 1);
        assert!(swapped.state.dirty);
        for (actual, original) in swapped.composition.sheets[0]
            .frames
            .iter()
            .zip(&before.composition.sheets[0].frames)
        {
            assert_eq!(actual.frame_id, original.frame_id);
            assert_eq!(actual.clip_rect, original.clip_rect);
            assert_eq!(actual.z_index, original.z_index);
            assert_eq!(actual.border_fill_rects, original.border_fill_rects);
        }
        assert_photos_fill_frames(&swapped);
        assert_eq!(frozen.composition, before.composition);
        assert_eq!(project.render_snapshot().composition, swapped.composition);
        assert_eq!(project.undo().unwrap().state.album, before.state.album);
        assert_eq!(project.redo().unwrap().state.album, swapped.state.album);
        project.save(project.revision()).unwrap();
        let saved = project.render_snapshot();
        drop(project);
        let mut reopened = core(root.path())
            .open_editable(OpenProjectRequest::new(location(
                &root.path().join("Troca.myalbuns"),
            )))
            .unwrap();
        for (index, media) in swapped.state.album.media.iter().enumerate() {
            reopened
                .observe_photo_source(media.id, metadata(index))
                .unwrap();
            assert_eq!(
                fs::read(root.path().join(format!("Foto-{index}.jpg"))).unwrap(),
                format!("original-{index}").as_bytes()
            );
        }
        assert_eq!(reopened.projection().state.album, swapped.state.album);
        assert_eq!(reopened.render_snapshot().composition, saved.composition);
    }
}

#[test]
fn two_occurrences_of_one_media_keep_their_independent_adjustments_and_swap_back() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), true);
    let before = project.projection();
    let frames = &before.state.album.sheets[0].frames;
    let selected = vec![frames[0].id.clone(), frames[1].id.clone()];
    let swapped = project
        .apply(ProjectIntent::SwapFrameContents {
            frame_ids: selected.clone(),
        })
        .unwrap();
    assert_eq!(
        swapped.state.album.sheets[0].frames[0].photo,
        frames[1].photo
    );
    assert_eq!(
        swapped.state.album.sheets[0].frames[1].photo,
        frames[0].photo
    );
    assert_eq!(swapped.media_usage, before.media_usage);
    let restored = project
        .apply(ProjectIntent::SwapFrameContents {
            frame_ids: selected,
        })
        .unwrap();
    assert_eq!(restored.state.album, before.state.album);
    assert_eq!(restored.composition, before.composition);
}

#[test]
fn invalid_swap_selections_leave_the_project_and_redo_branch_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), false);
    let sheet_id = project.projection().state.album.sheets[1].id.clone();
    let other = project
        .apply_with_outcome(ProjectIntent::AddFrame { sheet_id })
        .unwrap()
        .affected_frame_id
        .unwrap();
    let frames = project.projection().state.album.sheets[0].frames.clone();
    project
        .apply(ProjectIntent::SwapFrameContents {
            frame_ids: vec![frames[0].id.clone(), frames[1].id.clone()],
        })
        .unwrap();
    project.undo().unwrap();
    let before = project.projection();
    for frame_ids in [
        vec![],
        vec![frames[0].id.clone()],
        vec![
            frames[0].id.clone(),
            frames[1].id.clone(),
            frames[2].id.clone(),
        ],
        vec![frames[0].id.clone(), frames[0].id.to_uppercase()],
        vec![frames[0].id.clone(), other],
        vec![
            frames[0].id.clone(),
            "00000000-0000-0000-0000-000000000000".into(),
        ],
        vec![frames[0].id.clone(), "invalid".into()],
        vec![frames[2].id.clone(), frames[3].id.clone()],
    ] {
        assert_eq!(
            project.apply(ProjectIntent::SwapFrameContents { frame_ids }),
            Err(CoreError::InvalidFrameContentSwapSelection)
        );
        assert_eq!(project.projection(), before);
    }
    assert!(project.redo().is_some());
}

#[test]
fn swapped_photos_fill_extreme_frame_proportions_even_at_pan_limits() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), false);
    for (index, (dx, dy)) in [(-145_000, 100_000), (250_000, -185_000)]
        .into_iter()
        .enumerate()
    {
        let frame_id = project.projection().state.album.sheets[0].frames[index]
            .id
            .clone();
        edit_geometry(
            &mut project,
            &frame_id,
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: dx,
                delta_y_um: dy,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        );
        project
            .apply(ProjectIntent::TransformPhoto {
                frame_id,
                delta_pan_x: if index == 0 { -10.0 } else { 10.0 },
                delta_pan_y: if index == 0 { 10.0 } else { -10.0 },
                delta_zoom: -10.0,
            })
            .unwrap();
    }
    let before = project.projection();
    let swapped = project
        .apply(ProjectIntent::SwapFrameContents {
            frame_ids: before.state.album.sheets[0].frames[..2]
                .iter()
                .map(|frame| frame.id.clone())
                .collect(),
        })
        .unwrap();
    assert_photos_fill_frames(&swapped);
    for (index, frame) in swapped.state.album.sheets[0].frames[..2].iter().enumerate() {
        assert_eq!(
            frame.photo,
            before.state.album.sheets[0].frames[1 - index].photo
        );
    }
}

#[test]
fn swap_preview_corpus_matches_the_public_core() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), false);
    let before = project.projection();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    let normalized_media = |id| {
        let index = before
            .state
            .album
            .media
            .iter()
            .position(|media| media.id == id)
            .unwrap();
        format!("00000000-0000-4000-8000-{:012}", index + 1)
            .parse()
            .unwrap()
    };
    let normalize = |value: &EditorProjection| {
        let mut value = value.clone();
        value.state.project_id = "frame-content-swap-project".into();
        for (index, sheet) in value.state.album.sheets.iter_mut().enumerate() {
            sheet.id = format!("sheet-{:03}", index + 1);
            for frame in &mut sheet.frames {
                frame.id = format!(
                    "swap-frame-{}",
                    ids.iter().position(|id| id == &frame.id).unwrap()
                );
                if let Some(photo) = &mut frame.photo {
                    photo.media_id = normalized_media(photo.media_id);
                }
            }
        }
        for (index, sheet) in value.composition.sheets.iter_mut().enumerate() {
            sheet.sheet_id = format!("sheet-{:03}", index + 1);
            for frame in &mut sheet.frames {
                frame.frame_id = format!(
                    "swap-frame-{}",
                    ids.iter().position(|id| id == &frame.frame_id).unwrap()
                );
                if let Some(photo) = &mut frame.photo {
                    photo.media_id = normalized_media(photo.media_id);
                }
            }
        }
        for media in &mut value.state.album.media {
            media.id = normalized_media(media.id);
        }
        for usage in &mut value.media_usage {
            usage.media_id = normalized_media(usage.media_id);
        }
        value
    };
    let mut cases = Vec::new();
    for (name, destination) in [("photos", 1), ("placeholder", 2)] {
        let after = project
            .apply(ProjectIntent::SwapFrameContents {
                frame_ids: vec![ids[0].clone(), ids[destination].clone()],
            })
            .unwrap();
        cases.push(serde_json::json!({
            "name": name, "selectedFrameIds": ["swap-frame-0", &format!("swap-frame-{destination}")],
            "after": normalize(&after),
        }));
        project.undo().unwrap();
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({
            "before": normalize(&before), "cases": cases,
        }))
        .unwrap()
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/frame-content-swap-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_CONTENT_SWAP_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
