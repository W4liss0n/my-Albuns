use std::path::Path;

use image::{Rgb, RgbImage};
use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, DisplayUnit, EditableProject, EndSheetFormat,
    ExportMode, FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
    FrameStackAction, ImportPhoto, InitialProject, InitialProjectConfiguration, OpenProjectRequest,
    PhotoOrientationAction, PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent,
    ProjectLocation, RenderSnapshot,
};
use myalbuns_imaging_protocol::{
    AlbumRenderOutput, AlbumRenderRequest, IMAGING_PROTOCOL_VERSION, ImagingCommand,
    ImagingResponse, RenderFormat, RenderSource, decode_event_stream,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), paths.freeze())
}

fn edit_geometry(
    project: &mut EditableProject,
    frame_ids: &[String],
    gesture: FrameGeometryGesture,
) {
    let before = project.projection();
    let frames = &before.state.album.sheets[0].frames;
    let edit = FrameGeometryEdit {
        frames: frame_ids
            .iter()
            .map(|id| FrameGeometryTarget {
                frame_id: id.clone(),
                expected_rect: frames
                    .iter()
                    .find(|frame| &frame.id == id)
                    .unwrap()
                    .rect
                    .clone(),
            })
            .collect(),
        gesture,
        snap: None,
    };
    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(
        project.projection(),
        before,
        "preview must not enter History"
    );
    let after = project
        .apply(ProjectIntent::EditFrameGeometry { edit })
        .unwrap();
    for expected in preview.frames {
        assert_eq!(
            after.composition.sheets[0]
                .frames
                .iter()
                .find(|frame| frame.frame_id == expected.frame_id),
            Some(&expected),
        );
    }
    assert_eq!(after.state.revision, before.state.revision + 1);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, after.state.album);
}

fn export_jpeg(snapshot: RenderSnapshot, source: RenderSource, output: &Path) -> RgbImage {
    let units = snapshot
        .export_units(
            &[snapshot.composition.sheets[0].sheet_id.clone()],
            ExportMode::Sheet,
        )
        .unwrap();
    let mut paths = OperationPathContext::new();
    paths.capture(source.source_path()).unwrap();
    paths.capture(output).unwrap();
    let request = AlbumRenderRequest {
        protocol_version: IMAGING_PROTOCOL_VERSION,
        request_id: "frame-editing-journey".into(),
        snapshot,
        format: RenderFormat::Jpeg { quality: 100 },
        outputs: vec![AlbumRenderOutput {
            prepared_path: output.to_path_buf().into(),
            units,
        }],
        sources: vec![source],
        root_bindings: paths.freeze(),
    };
    let result = super::invoke_imaging_command(&ImagingCommand::RenderAlbum(request), None);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let (_, response) = decode_event_stream(&result.stdout).unwrap();
    let ImagingResponse::AlbumCompleted { completion, .. } = response else {
        panic!("unexpected export response: {response:?}");
    };
    assert_eq!(completion.outputs.len(), 1);
    assert_eq!(
        completion.outputs[0].source_count, 1,
        "occurrences share the Original"
    );
    image::open(output).unwrap().to_rgb8()
}

fn assert_color(image: &RgbImage, x: u32, y: u32, expected: [u8; 3]) {
    let actual = image.get_pixel(x, y).0;
    assert!(
        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 4),
        "pixel {x},{y}: {actual:?}, expected {expected:?}",
    );
}

#[test]
fn edited_frames_keep_crop_and_stack_through_history_save_reopen_and_jpeg() {
    let root = tempfile::tempdir().unwrap();
    let source_path = root.path().join("original.png");
    let red = [210, 30, 20];
    let blue = [20, 60, 210];
    RgbImage::from_fn(120, 80, |x, _| Rgb(if x < 60 { red } else { blue }))
        .save(&source_path)
        .unwrap();
    let original_bytes = std::fs::read(&source_path).unwrap();
    let project_path = root.path().join("Edicao.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&project_path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                101_600,
                50_800,
                100,
                0,
                0,
                2,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let metadata =
        PhotoSourceMetadata::new(120, 80, ["#D21E14", "#143CD2", "#FFFFFF"].map(String::from))
            .unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(source_path.clone(), metadata.clone()))
        .unwrap();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    let mut ids = Vec::new();
    // Physical rectangles correspond to (40,40,120,80) and (100,60,120,80) at 100 DPI.
    for (x, y) in [(10_160, 10_160), (25_400, 15_240)] {
        let added = project
            .apply_with_outcome(ProjectIntent::AddFrame {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
        let id = added.affected_frame_id.unwrap();
        let current = &added.projection.state.album.sheets[0]
            .frames
            .last()
            .unwrap()
            .rect;
        edit_geometry(
            &mut project,
            std::slice::from_ref(&id),
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: 30_480 - current.width,
                delta_y_um: 20_320 - current.height,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        );
        let current = project.projection().state.album.sheets[0]
            .frames
            .last()
            .unwrap()
            .rect
            .clone();
        edit_geometry(
            &mut project,
            std::slice::from_ref(&id),
            FrameGeometryGesture::Move {
                delta_x_um: x - current.x,
                delta_y_um: y - current.y,
            },
        );
        let filled = project
            .apply_with_outcome(ProjectIntent::DropPhoto {
                sheet_id: sheet_id.clone(),
                media_id: imported.media_id,
                x_um: x + 1_000,
                y_um: y + 1_000,
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
        assert_eq!(filled.affected_frame_id.as_ref(), Some(&id));
        ids.push(id);
    }
    // Pan and zoom crop the first occurrence to red; the other occurrence stays independent.
    let before = project.projection();
    let cropped = project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: ids[0].clone(),
            delta_pan_x: 1.0,
            delta_pan_y: 0.0,
            delta_zoom: 1.0,
        })
        .unwrap();
    assert_eq!(
        cropped.state.album.sheets[0].frames[1],
        before.state.album.sheets[0].frames[1]
    );
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, cropped.state.album);
    project
        .apply(ProjectIntent::OrientPhotos {
            frame_ids: vec![ids[1].clone()],
            action: PhotoOrientationAction::ToggleHorizontalMirror,
        })
        .unwrap();
    edit_geometry(
        &mut project,
        &ids,
        FrameGeometryGesture::Move {
            delta_x_um: 5_080,
            delta_y_um: 2_540,
        },
    );
    edit_geometry(
        &mut project,
        &ids,
        FrameGeometryGesture::Resize {
            handle: FrameResizeHandle::BottomRight,
            delta_x_um: 9_144,
            delta_y_um: 5_080,
            preserve_aspect_ratio: false,
            from_center: false,
        },
    );
    let source = RenderSource::new(imported.media_id, source_path.clone()).unwrap();
    let before_order = project.projection();
    let lower = export_jpeg(
        project.render_snapshot(),
        source.clone(),
        &root.path().join("before-order.jpg"),
    );
    assert_eq!(lower.dimensions(), (400, 200));
    assert_color(&lower, 180, 100, blue);
    project
        .apply(ProjectIntent::ArrangeFrames {
            frame_ids: vec![ids[0].clone()],
            action: FrameStackAction::BringToFront,
        })
        .unwrap();
    let after_order = project.projection();
    assert_eq!(after_order.state.revision, before_order.state.revision + 1);
    let upper = export_jpeg(
        project.render_snapshot(),
        source.clone(),
        &root.path().join("after-order.jpg"),
    );
    assert_color(&upper, 180, 100, red);
    assert_color(&upper, 190, 60, red); // Zoom/Pan survived group resize.
    assert_color(&upper, 250, 100, red); // The mirrored second occurrence remains visible.
    assert_color(&upper, 140, 155, blue);
    assert_color(&upper, 40, 60, [255, 255, 255]); // The Photo is clipped to its moved Frame.
    assert_color(&upper, 190, 180, [255, 255, 255]);
    assert_eq!(
        project.undo().unwrap().state.album,
        before_order.state.album
    );
    assert_eq!(
        export_jpeg(
            project.render_snapshot(),
            source.clone(),
            &root.path().join("undone.jpg")
        ),
        lower
    );
    assert_eq!(project.redo().unwrap().state.album, after_order.state.album);
    project.save(project.revision()).unwrap();
    let saved = project.render_snapshot();
    drop(project);
    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .unwrap();
    reopened
        .observe_photo_source(imported.media_id, metadata)
        .unwrap();
    assert_eq!(
        reopened.projection().state.album.sheets,
        after_order.state.album.sheets
    );
    assert_eq!(reopened.render_snapshot().composition, saved.composition);
    assert_eq!(
        export_jpeg(
            reopened.render_snapshot(),
            source,
            &root.path().join("reopened.jpg")
        ),
        upper
    );
    assert_eq!(std::fs::read(source_path).unwrap(), original_bytes);
}
