use serde::Deserialize;

use crate::composition::CompositionCore;
use crate::{
    AlbumSnapshot, ComposedPhoto, FrameSnapshot, Matrix2, MediaTransform, PhotoPlacement,
    PhotoPlacementPlan, PhotoSnapshot, ProjectCore, ProjectIntent, RectUm, SheetRole,
    SheetSnapshot, VectorUm,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhotoPlacementFixture {
    cases: Vec<PhotoPlacementCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhotoPlacementCase {
    name: String,
    frame: RectUm,
    photo: PhotoSnapshot,
    expected_plan: PhotoPlacementPlan,
}

#[test]
fn opens_a_representative_long_album() {
    let session = ProjectCore::open_sample_project(12);

    assert_eq!(session.state().album.sheets.len(), 12);
}

#[test]
fn commits_one_domain_revision_for_a_completed_pan_gesture() {
    let mut session = ProjectCore::open_sample_project(12);

    let state = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: -0.10,
            delta_zoom: 0.0,
        })
        .expect("the sample frame accepts a pan gesture");

    assert_eq!(state.revision, 1);
    assert_eq!(
        state.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the sample frame has a photo")
            .transform
            .pan_x,
        0.25
    );
    assert!(state.dirty);
    assert!(state.can_undo);
}

#[test]
fn commits_simultaneous_pan_and_zoom_as_one_domain_revision() {
    let mut session = ProjectCore::open_sample_project(12);

    let transformed = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.35,
            delta_pan_y: -0.2,
            delta_zoom: 0.12,
        })
        .expect("the sample photo accepts a combined transform");
    let transform = &transformed.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the sample frame has a photo")
        .transform;

    assert_eq!(transformed.revision, 1);
    assert_eq!(transform.pan_x, 0.35);
    assert_eq!(transform.pan_y, -0.2);
    assert_eq!(transform.user_zoom, 1.12);

    let undone = session
        .undo()
        .expect("the combined transform is one Undo action");
    let restored = &undone.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the sample frame retains its photo")
        .transform;
    assert_eq!(restored.pan_x, 0.0);
    assert_eq!(restored.pan_y, 0.0);
    assert_eq!(restored.user_zoom, 1.0);
}

#[test]
fn undo_and_redo_restore_the_document_without_storing_view_state() {
    let mut session = ProjectCore::open_sample_project(12);
    session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.40,
            delta_pan_y: 0.20,
            delta_zoom: 0.0,
        })
        .expect("pan is valid");

    let undone = session.undo().expect("the gesture can be undone");
    assert_eq!(
        undone.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("photo remains in the frame")
            .transform
            .pan_x,
        0.0
    );
    assert!(!undone.dirty);
    assert!(undone.can_redo);

    let redone = session.redo().expect("the gesture can be redone");
    assert_eq!(
        redone.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("photo remains in the frame")
            .transform
            .pan_x,
        0.40
    );
    assert!(redone.dirty);
}

#[test]
fn render_snapshot_uses_the_composition_plan_and_excludes_canvas_navigation() {
    let session = ProjectCore::open_sample_project(12);

    let snapshot = session.render_snapshot();
    let photo = snapshot.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the first composed frame has a photo");

    assert_eq!(
        photo.draw_rect,
        RectUm {
            x: -31_000,
            y: 28_000,
            width: 366_000,
            height: 244_000,
        }
    );

    let wire = serde_json::to_string(&snapshot).expect("snapshot is serializable");
    assert!(!wire.contains("viewport"));
    assert!(!wire.contains("selection"));
}

#[test]
fn rotated_composition_keeps_every_frame_corner_covered() {
    let frame = RectUm {
        x: 20_000,
        y: 30_000,
        width: 300_000,
        height: 200_000,
    };
    let photo = PhotoSnapshot {
        media_id: "media-rotated".into(),
        name: "Foto rotacionada.jpg".into(),
        source_width_px: 6_000,
        source_height_px: 4_000,
        palette: ["#10202b".into(), "#648493".into(), "#dfa75e".into()],
        transform: MediaTransform {
            pan_x: 1.0,
            pan_y: -1.0,
            user_zoom: 1.0,
            quarter_turns: 0,
            fine_rotation_degrees: 30.0,
            mirror_x: false,
        },
    };

    let composed = compose_through_public_contract(&frame, &photo);
    let radians = (composed.rotation_degrees as f64).to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    let center_x = composed.draw_rect.x as f64 + composed.draw_rect.width as f64 / 2.0;
    let center_y = composed.draw_rect.y as f64 + composed.draw_rect.height as f64 / 2.0;
    let half_width = composed.draw_rect.width as f64 / 2.0;
    let half_height = composed.draw_rect.height as f64 / 2.0;

    for (corner_x, corner_y) in [
        (frame.x, frame.y),
        (frame.x + frame.width, frame.y),
        (frame.x, frame.y + frame.height),
        (frame.x + frame.width, frame.y + frame.height),
    ] {
        let delta_x = corner_x as f64 - center_x;
        let delta_y = corner_y as f64 - center_y;
        let local_x = cosine * delta_x + sine * delta_y;
        let local_y = -sine * delta_x + cosine * delta_y;

        assert!(local_x.abs() <= half_width + 1.0);
        assert!(local_y.abs() <= half_height + 1.0);
    }
}

#[test]
fn photo_placement_plan_matches_the_shared_renderer_contract() {
    let fixture: PhotoPlacementFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/photo-placement-cases.json"
    ))
    .expect("the shared Photo placement fixture is valid");

    for case in fixture.cases {
        let composed = compose_through_public_contract(&case.frame, &case.photo);
        assert_plan_close(&composed.placement, &case.expected_plan, &case.name);
    }
}

fn compose_through_public_contract(frame: &RectUm, photo: &PhotoSnapshot) -> ComposedPhoto {
    let album = AlbumSnapshot {
        sheets: vec![SheetSnapshot {
            id: "sheet-under-test".into(),
            number: 1,
            role: SheetRole::Internal,
            width_um: frame.x + frame.width,
            height_um: frame.y + frame.height,
            frames: vec![FrameSnapshot {
                id: "frame-under-test".into(),
                rect: frame.clone(),
                z_index: 1,
                photo: Some(photo.clone()),
            }],
            has_overlay: false,
        }],
        media: Vec::new(),
    };

    CompositionCore::compose(&album).sheets[0].frames[0]
        .photo
        .clone()
        .expect("the public composition contract returns the photo")
}

fn assert_plan_close(actual: &PhotoPlacementPlan, expected: &PhotoPlacementPlan, case_name: &str) {
    assert_vector_close(&actual.current_pan, &expected.current_pan, case_name);
    assert_close(actual.current_zoom, expected.current_zoom, case_name);
    assert_close(
        actual.pan_range.minimum,
        expected.pan_range.minimum,
        case_name,
    );
    assert_close(
        actual.pan_range.maximum,
        expected.pan_range.maximum,
        case_name,
    );
    assert_close(
        actual.zoom_range.minimum,
        expected.zoom_range.minimum,
        case_name,
    );
    assert_close(
        actual.zoom_range.maximum,
        expected.zoom_range.maximum,
        case_name,
    );
    assert_placement_close(&actual.current, &expected.current, case_name);
    assert_vector_close(&actual.pan_origin, &expected.pan_origin, case_name);
    assert_matrix_close(&actual.pan_to_center, &expected.pan_to_center, case_name);
    assert_matrix_close(
        &actual.pan_to_center_per_zoom,
        &expected.pan_to_center_per_zoom,
        case_name,
    );
    assert_close(
        actual.size_per_zoom.width,
        expected.size_per_zoom.width,
        case_name,
    );
    assert_close(
        actual.size_per_zoom.height,
        expected.size_per_zoom.height,
        case_name,
    );
}

fn assert_matrix_close(actual: &Matrix2, expected: &Matrix2, case_name: &str) {
    assert_close(actual.xx, expected.xx, case_name);
    assert_close(actual.xy, expected.xy, case_name);
    assert_close(actual.yx, expected.yx, case_name);
    assert_close(actual.yy, expected.yy, case_name);
}

fn assert_placement_close(actual: &PhotoPlacement, expected: &PhotoPlacement, case_name: &str) {
    assert_vector_close(&actual.center, &expected.center, case_name);
    assert_close(actual.size.width, expected.size.width, case_name);
    assert_close(actual.size.height, expected.size.height, case_name);
}

fn assert_vector_close(actual: &VectorUm, expected: &VectorUm, case_name: &str) {
    assert_close(actual.x, expected.x, case_name);
    assert_close(actual.y, expected.y, case_name);
}

fn assert_close(actual: f64, expected: f64, case_name: &str) {
    assert!(
        (actual - expected).abs() <= 0.01,
        "{case_name}: expected {expected}, received {actual}"
    );
}

#[test]
fn loads_a_persisted_revision_for_rendering_without_an_editable_session() {
    let mut session = ProjectCore::open_sample_project(12);
    session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: -0.35,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("pan is valid");
    let persisted = session
        .persisted_revision()
        .expect("sample project can be serialized");

    let loaded = ProjectCore::load_persisted_revision(&persisted)
        .expect("persisted revision can be loaded read-only");

    assert_eq!(loaded.render_snapshot(), session.render_snapshot());
    assert_eq!(loaded.revision(), 1);
}

#[test]
fn the_core_fills_the_leftmost_placeholder_for_a_photo_intent() {
    let mut session = ProjectCore::open_sample_project(12);

    let state = session
        .apply(ProjectIntent::FillLeftmostPlaceholder {
            sheet_id: "lamina-02".into(),
            media_id: "media-campo".into(),
        })
        .expect("the sample sheet has compatible placeholders");

    assert_eq!(
        state.album.sheets[1].frames[0]
            .photo
            .as_ref()
            .expect("leftmost placeholder was filled")
            .media_id,
        "media-campo"
    );
    assert!(state.album.sheets[1].frames[1].photo.is_none());
    assert_eq!(state.revision, 1);
}

#[test]
fn photo_zoom_is_persistent_and_never_goes_below_fill() {
    let mut session = ProjectCore::open_sample_project(12);

    let zoomed = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.0,
            delta_pan_y: 0.0,
            delta_zoom: 0.35,
        })
        .expect("the sample photo accepts zoom");
    assert_eq!(
        zoomed.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .transform
            .user_zoom,
        1.35
    );

    let reset = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.0,
            delta_pan_y: 0.0,
            delta_zoom: -2.0,
        })
        .expect("zoom is clamped to fill");
    assert_eq!(
        reset.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .transform
            .user_zoom,
        1.0
    );
}

#[test]
fn composition_plan_orders_frames_by_visual_stack() {
    let session = ProjectCore::open_sample_project(12);
    let mut album = session.state().album;
    album.sheets[0].frames.reverse();

    let composition = CompositionCore::compose(&album);
    let frame_ids = composition.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.frame_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(frame_ids, ["frame-01-a", "frame-01-b"]);
}

#[test]
fn persisted_revision_rejects_invalid_media_dimensions() {
    let session = ProjectCore::open_sample_project(12);
    let persisted = session
        .persisted_revision()
        .expect("sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("sample JSON is valid");
    document["album"]["sheets"][0]["frames"][0]["photo"]["sourceWidthPx"] = serde_json::json!(0);

    let error = ProjectCore::load_persisted_revision(
        &serde_json::to_string(&document).expect("modified JSON is valid"),
    )
    .err()
    .expect("invalid dimensions must be rejected");

    assert!(error.to_string().contains("dimensões"));
}

#[test]
fn persisted_revision_rejects_an_album_with_fewer_than_two_sheets() {
    let session = ProjectCore::open_sample_project(2);
    let persisted = session
        .persisted_revision()
        .expect("sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("sample JSON is valid");
    document["album"]["sheets"]
        .as_array_mut()
        .expect("sample sheets are an array")
        .truncate(1);

    let error = ProjectCore::load_persisted_revision(
        &serde_json::to_string(&document).expect("modified JSON is valid"),
    )
    .err()
    .expect("an Álbum needs at least two Lâminas");

    assert!(error.to_string().contains("pelo menos duas Lâminas"));
}

#[test]
fn render_snapshot_rejects_empty_internal_identifiers() {
    let session = ProjectCore::open_sample_project(12);
    let mut snapshot = session.render_snapshot();
    snapshot.composition.sheets[0].frames[0].frame_id.clear();

    let error = snapshot
        .validate()
        .expect_err("an empty Frame identifier must be rejected");

    assert!(error.to_string().contains("Identificador de Frame"));
}
