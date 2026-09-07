#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, FrameGeometryEdit, FrameGeometryGesture,
    FrameResizeHandle, ImportPhoto, InitialProject, OpenProjectRequest, PhotoPlacementMode,
    PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation, RectUm,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project_with_frame(root: &Path) -> myalbuns_core::EditableProject {
    project_with_frame_on_sheet(root, 0, false)
}

fn project_with_frame_on_sheet(
    root: &Path,
    sheet_index: usize,
    single: bool,
) -> myalbuns_core::EditableProject {
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Frame.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    if single {
        project
            .apply(ProjectIntent::ConvertEdgeSheet {
                sheet_id: project.projection().state.album.sheets[sheet_index]
                    .id
                    .clone(),
            })
            .unwrap();
    }
    let photo = root.join("Foto.jpg");
    fs::write(&photo, b"original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(photo, photo_metadata()))
        .unwrap();
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: imported.projection.state.album.sheets[sheet_index]
                .id
                .clone(),
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    project
}

#[test]
fn geometry_stops_at_each_single_page_surface_even_for_extreme_pointer_deltas() {
    for sheet_index in [0, 1] {
        let root = tempfile::tempdir().unwrap();
        let project = project_with_frame_on_sheet(root.path(), sheet_index, true);
        let before = project.projection();
        let sheet = &before.state.album.sheets[sheet_index];
        let frame = &sheet.frames[0];
        let active_width = before.composition.sheets[sheet_index].width_um;
        assert_eq!(active_width, 300_000);
        for (delta, x, y) in [
            (i64::MIN, 0, 0),
            (
                i64::MAX,
                active_width - frame.rect.width,
                sheet.height_um - frame.rect.height,
            ),
        ] {
            let preview = project
                .preview_frame_geometry(&FrameGeometryEdit {
                    frame_id: frame.id.clone(),
                    expected_rect: frame.rect.clone(),
                    gesture: FrameGeometryGesture::Move {
                        delta_x_um: delta,
                        delta_y_um: delta,
                    },
                })
                .unwrap();
            assert_eq!(preview.clip_rect.x, x);
            assert_eq!(preview.clip_rect.y, y);
            assert_eq!(preview.clip_rect.width, frame.rect.width);
        }
        let preview = project
            .preview_frame_geometry(&FrameGeometryEdit {
                frame_id: frame.id.clone(),
                expected_rect: frame.rect.clone(),
                gesture: FrameGeometryGesture::Resize {
                    handle: FrameResizeHandle::BottomRight,
                    delta_x_um: i64::MAX,
                    delta_y_um: i64::MAX,
                    preserve_aspect_ratio: false,
                    from_center: false,
                },
            })
            .unwrap();
        assert_eq!(preview.clip_rect.x + preview.clip_rect.width, active_width);
        assert_eq!(
            preview.clip_rect.y + preview.clip_rect.height,
            sheet.height_um
        );
        assert_eq!(project.projection(), before);
    }
}

#[test]
fn a_geometry_gesture_preserves_adjacent_edits_rejects_stale_geometry_and_leaves_noop_history_untouched()
 {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frame(root.path());
    let initial = project.projection();
    let frame = &initial.state.album.sheets[0].frames[0];
    let mut edit = FrameGeometryEdit {
        frame_id: frame.id.clone(),
        expected_rect: frame.rect.clone(),
        gesture: FrameGeometryGesture::Move {
            delta_x_um: 0,
            delta_y_um: 0,
        },
    };
    assert_eq!(
        project
            .apply(ProjectIntent::EditFrameGeometry { edit: edit.clone() })
            .unwrap(),
        initial
    );
    edit.gesture = FrameGeometryGesture::Move {
        delta_x_um: 20_000,
        delta_y_um: 10_000,
    };
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: frame.id.clone(),
            delta_pan_x: 0.4,
            delta_pan_y: -0.25,
            delta_zoom: 0.5,
        })
        .unwrap();
    let adjacent = project.apply(ProjectIntent::SetDpi { dpi: 240 }).unwrap();
    let committed = project
        .apply(ProjectIntent::EditFrameGeometry { edit: edit.clone() })
        .unwrap();
    assert_eq!(committed.state.document.dpi, 240);
    assert_eq!(
        committed.state.album.sheets[0].frames[0].photo,
        adjacent.state.album.sheets[0].frames[0].photo
    );
    assert_eq!(
        project.preview_frame_geometry(&edit).unwrap_err(),
        myalbuns_core::CoreError::FrameGeometryChanged
    );
    assert_eq!(
        project
            .apply(ProjectIntent::EditFrameGeometry { edit: edit.clone() })
            .unwrap_err(),
        myalbuns_core::CoreError::FrameGeometryChanged
    );
    assert_eq!(project.projection(), committed);
    assert_eq!(project.undo().unwrap().state.album, adjacent.state.album);
    assert_eq!(project.projection().state.document.dpi, 240);
    edit.frame_id = "invalid-frame-id".into();
    assert!(project.preview_frame_geometry(&edit).is_err());
    edit.frame_id = uuid::Uuid::new_v4().to_string();
    assert!(project.preview_frame_geometry(&edit).is_err());
    assert_eq!(project.redo().unwrap().state.album, committed.state.album);
}

#[test]
fn eight_resize_handles_keep_the_opposite_anchor_and_the_photo_filling_the_frame() {
    use FrameResizeHandle::*;
    let cases = [
        (TopLeft, [210_000, 90_000, 210_000, 140_000]),
        (Top, [180_000, 90_000, 240_000, 140_000]),
        (TopRight, [180_000, 90_000, 270_000, 140_000]),
        (Right, [180_000, 70_000, 270_000, 160_000]),
        (BottomRight, [180_000, 70_000, 270_000, 180_000]),
        (Bottom, [180_000, 70_000, 240_000, 180_000]),
        (BottomLeft, [210_000, 70_000, 210_000, 180_000]),
        (Left, [210_000, 70_000, 210_000, 160_000]),
    ];
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frame(root.path());
    let frame = project.projection().state.album.sheets[0].frames[0].clone();
    assert_eq!(
        frame.rect,
        RectUm {
            x: 180_000,
            y: 70_000,
            width: 240_000,
            height: 160_000
        }
    );
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: frame.id.clone(),
            delta_pan_x: 0.4,
            delta_pan_y: -0.25,
            delta_zoom: 0.5,
        })
        .unwrap();
    let before = project.projection();
    for (handle, [x, y, width, height]) in cases {
        let before_preview = project.projection();
        let edit = FrameGeometryEdit {
            frame_id: frame.id.clone(),
            expected_rect: frame.rect.clone(),
            gesture: FrameGeometryGesture::Resize {
                handle,
                delta_x_um: 30_000,
                delta_y_um: 20_000,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        };
        let preview = project.preview_frame_geometry(&edit).unwrap();
        assert_eq!(
            preview.clip_rect,
            RectUm {
                x,
                y,
                width,
                height
            },
            "{handle:?}"
        );
        assert_eq!(project.projection(), before_preview);
        let committed = project
            .apply(ProjectIntent::EditFrameGeometry { edit })
            .unwrap();
        assert_eq!(committed.composition.sheets[0].frames[0], preview);
        assert_eq!(
            committed.state.album.sheets[0].frames[0].photo,
            before.state.album.sheets[0].frames[0].photo
        );
        let photo = preview.photo.unwrap();
        assert!(photo.draw_rect.x <= x && photo.draw_rect.y <= y);
        assert!(photo.draw_rect.x + photo.draw_rect.width >= x + width);
        assert!(photo.draw_rect.y + photo.draw_rect.height >= y + height);
        project.undo().unwrap();
    }
}

fn photo_metadata() -> PhotoSourceMetadata {
    PhotoSourceMetadata::new(
        1_200,
        800,
        ["#FF0000".into(), "#00FF00".into(), "#0000FF".into()],
    )
    .unwrap()
}

#[test]
fn a_small_persisted_placeholder_can_move_and_grow_without_being_enlarged_implicitly() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frame(root.path());
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.path().join("Frame.myalbuns");
    let mut payload: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let frame = &mut payload["project"]["sheets"][0]["frames"][0];
    frame["rect"]["width"] = 4_000.into();
    frame["rect"]["height"] = 8_000.into();
    frame["photo"] = serde_json::Value::Null;
    fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let before = reopened.projection();
    let frame = &before.state.album.sheets[0].frames[0];
    let mut edit = FrameGeometryEdit {
        frame_id: frame.id.clone(),
        expected_rect: frame.rect.clone(),
        gesture: FrameGeometryGesture::Resize {
            handle: FrameResizeHandle::BottomRight,
            delta_x_um: -100_000,
            delta_y_um: -100_000,
            preserve_aspect_ratio: false,
            from_center: false,
        },
    };
    assert_eq!(
        reopened
            .apply(ProjectIntent::EditFrameGeometry { edit: edit.clone() })
            .unwrap(),
        before
    );
    edit.gesture = FrameGeometryGesture::Move {
        delta_x_um: 10_000,
        delta_y_um: 20_000,
    };
    let moved = reopened.preview_frame_geometry(&edit).unwrap();
    assert_eq!(moved.clip_rect.width, 4_000);
    assert_eq!(moved.clip_rect.height, 8_000);
    assert!(moved.photo.is_none());
    edit.gesture = FrameGeometryGesture::Resize {
        handle: FrameResizeHandle::BottomRight,
        delta_x_um: 5_000,
        delta_y_um: 5_000,
        preserve_aspect_ratio: false,
        from_center: false,
    };
    let grown = reopened
        .apply(ProjectIntent::EditFrameGeometry { edit })
        .unwrap();
    assert_eq!(grown.state.album.sheets[0].frames[0].rect.width, 9_000);
    assert_eq!(grown.state.album.sheets[0].frames[0].rect.height, 13_000);
    assert_eq!(reopened.undo().unwrap().state.album, before.state.album);
}

#[test]
fn headless_frame_geometry_corpus_matches_the_core_previews() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frame(root.path());
    let frame = project.projection().state.album.sheets[0].frames[0].clone();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: frame.id.clone(),
            delta_pan_x: 0.35,
            delta_pan_y: -0.2,
            delta_zoom: 0.3,
        })
        .unwrap();
    let cases = [
        (
            "selected",
            FrameGeometryGesture::Move {
                delta_x_um: 0,
                delta_y_um: 0,
            },
        ),
        (
            "moved",
            FrameGeometryGesture::Move {
                delta_x_um: 90_000,
                delta_y_um: -30_000,
            },
        ),
        (
            "resized",
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: 80_000,
                delta_y_um: -40_000,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        ),
        (
            "proportional-centered",
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: 60_000,
                delta_y_um: 0,
                preserve_aspect_ratio: true,
                from_center: true,
            },
        ),
        (
            "minimum",
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::TopLeft,
                delta_x_um: 9_000_000,
                delta_y_um: 9_000_000,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        ),
    ];
    let cases = cases
        .into_iter()
        .map(|(name, gesture)| {
            let mut preview = project
                .preview_frame_geometry(&FrameGeometryEdit {
                    frame_id: frame.id.clone(),
                    expected_rect: frame.rect.clone(),
                    gesture: gesture.clone(),
                })
                .unwrap();
            preview.frame_id = "geometry-frame".into();
            preview.photo.as_mut().unwrap().media_id =
                "00000000-0000-4000-8000-000000000001".parse().unwrap();
            serde_json::json!({ "name": name, "gesture": gesture, "frame": preview })
        })
        .collect::<Vec<_>>();
    let actual = serde_json::json!({ "cases": cases });
    let serialized = format!("{}\n", serde_json::to_string_pretty(&actual).unwrap());
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/frame-geometry-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_GEOMETRY_FIXTURE").is_some() {
        fs::write(&path, &serialized).unwrap();
    }
    let expected = fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    assert_eq!(
        serialized, expected,
        "the rendered visual corpus must use the current Core composition"
    );
}

#[test]
fn resize_modifiers_use_the_gesture_baseline_and_stop_at_surface_and_minimum_size() {
    use FrameResizeHandle::*;
    let root = tempfile::tempdir().unwrap();
    let project = project_with_frame(root.path());
    let before = project.projection();
    let frame = &before.state.album.sheets[0].frames[0];
    let cases = [
        (
            BottomRight,
            60_000,
            0,
            true,
            false,
            [180_000, 70_000, 300_000, 200_000],
        ),
        (
            BottomRight,
            60_000,
            0,
            false,
            true,
            [120_000, 70_000, 360_000, 160_000],
        ),
        (
            BottomRight,
            60_000,
            0,
            true,
            true,
            [120_000, 30_000, 360_000, 240_000],
        ),
        (
            Right,
            60_000,
            90_000,
            true,
            true,
            [120_000, 70_000, 360_000, 160_000],
        ),
        (
            BottomRight,
            -60_000,
            0,
            true,
            false,
            [180_000, 70_000, 180_000, 120_000],
        ),
        (
            BottomRight,
            9_000_000,
            9_000_000,
            true,
            false,
            [180_000, 70_000, 345_000, 230_000],
        ),
        (
            BottomRight,
            9_000_000,
            9_000_000,
            true,
            true,
            [75_000, 0, 450_000, 300_000],
        ),
        (
            TopLeft,
            9_000_000,
            9_000_000,
            true,
            false,
            [402_000, 218_000, 18_000, 12_000],
        ),
        (
            TopLeft,
            9_000_000,
            9_000_000,
            false,
            false,
            [408_000, 218_000, 12_000, 12_000],
        ),
    ];
    for (
        handle,
        delta_x_um,
        delta_y_um,
        preserve_aspect_ratio,
        from_center,
        [x, y, width, height],
    ) in cases
    {
        let edit = FrameGeometryEdit {
            frame_id: frame.id.clone(),
            expected_rect: frame.rect.clone(),
            gesture: FrameGeometryGesture::Resize {
                handle,
                delta_x_um,
                delta_y_um,
                preserve_aspect_ratio,
                from_center,
            },
        };
        assert_eq!(
            project.preview_frame_geometry(&edit).unwrap().clip_rect,
            RectUm {
                x,
                y,
                width,
                height
            },
            "{edit:?}"
        );
    }
    assert_eq!(project.projection(), before);
}

#[test]
fn moving_a_frame_previews_without_history_and_commits_once_through_save_and_reopen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Geometria.myalbuns");
    let original = root.path().join("Foto.jpg");
    fs::write(&original, b"unchanged original").unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(original.clone(), photo_metadata()))
        .unwrap();
    let added = project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: imported.projection.state.album.sheets[0].id.clone(),
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    project.save(added.state.revision).unwrap();
    let before = project.projection();
    let frame = &before.state.album.sheets[0].frames[0];
    let edit = FrameGeometryEdit {
        frame_id: frame.id.clone(),
        expected_rect: frame.rect.clone(),
        gesture: FrameGeometryGesture::Move {
            delta_x_um: 40_000,
            delta_y_um: -30_000,
        },
    };
    let expected = RectUm {
        x: frame.rect.x + 40_000,
        y: frame.rect.y - 30_000,
        width: frame.rect.width,
        height: frame.rect.height,
    };

    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(preview.clip_rect, expected);
    assert_eq!(
        project.projection(),
        before,
        "preview cannot mutate the creative revision or History"
    );

    let changed = project
        .apply(ProjectIntent::EditFrameGeometry { edit })
        .unwrap();
    assert_eq!(changed.state.revision, before.state.revision + 1);
    assert!(changed.state.dirty);
    assert_eq!(changed.state.album.sheets[0].frames[0].rect, expected);
    assert_eq!(changed.state.album.sheets[0].frames[0].photo, frame.photo);
    assert_eq!(changed.composition.sheets[0].frames[0], preview);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert!(!project.projection().state.dirty);
    assert_eq!(project.redo().unwrap().state.album, changed.state.album);
    project.save(changed.state.revision).unwrap();
    let saved = project.render_snapshot();
    drop(project);

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    reopened
        .observe_photo_source(imported.media_id, photo_metadata())
        .unwrap();
    assert_eq!(
        reopened.projection().state.album.sheets[0].frames[0].rect,
        expected
    );
    assert_eq!(reopened.render_snapshot().composition, saved.composition);
    assert_eq!(fs::read(original).unwrap(), b"unchanged original");
}
