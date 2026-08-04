use serde::Deserialize;

use crate::composition::CompositionCore;
use crate::sample_project_fixture::SampleProject;
use crate::{
    AlbumSnapshot, ComposedPhoto, DemoEditableProject as EditableProject, FrameSnapshot, Matrix2,
    MediaCatalogItem, MediaKind, MediaTransform, NormalizedPan, PhotoPlacement, PhotoPlacementPlan,
    PhotoSnapshot, ProjectCore, ProjectIntent, ProjectedActiveSides, RectUm, SheetRole,
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
    photo: PlacementPhoto,
    expected_plan: PhotoPlacementPlan,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlacementPhoto {
    media_id: String,
    name: String,
    source_width_px: u32,
    source_height_px: u32,
    palette: [String; 3],
    transform: MediaTransform,
}

fn horizon_project(sheet_count: usize) -> EditableProject {
    sample_project(SampleProject::Horizon, sheet_count)
}

fn sample_project(sample: SampleProject, sheet_count: usize) -> EditableProject {
    let source = sample
        .persisted_source(sheet_count)
        .expect("the sample project serializes");
    ProjectCore::new()
        .open_demo_editable_session(&source)
        .expect("the sample project opens through ProjectCore")
}

#[test]
fn opens_a_representative_long_album() {
    let session = horizon_project(12);

    assert_eq!(session.state().album.sheets.len(), 12);
}

#[test]
fn projects_a_real_decorative_overlay_from_the_canonical_catalog() {
    let session = horizon_project(12);
    let projection = session.projection();
    let sheet = projection
        .composition
        .sheets
        .iter()
        .find(|sheet| !sheet.overlays.is_empty())
        .expect("the representative Album contains a composed Overlay");
    let overlay = sheet
        .overlays
        .first()
        .expect("the composed Overlay remains present");

    assert_eq!(overlay.media_id, "decorative-overlay");
    assert_eq!(overlay.name, "Overlay translúcido.png");
    assert_eq!(
        overlay.draw_rect,
        RectUm {
            x: 0,
            y: 0,
            width: sheet.width_um,
            height: sheet.height_um,
        }
    );
    assert_eq!(
        media_usage_count(&projection, "decorative-overlay"),
        Some(12)
    );
    assert_eq!(
        session.render_snapshot().composition,
        projection.composition,
        "Editor and Export must consume the same CompositionPlan"
    );
    assert_eq!(
        sheet
            .referenced_media_ids()
            .collect::<std::collections::BTreeSet<_>>(),
        ["decorative-overlay", "media-campo", "media-costa"]
            .into_iter()
            .collect(),
        "all consumers derive required originals from the ComposedSheet"
    );
}

#[test]
fn opens_an_editable_session_through_the_project_core_seam() {
    let mut fixture = horizon_project(12);
    fixture
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the fixture can create a persisted revision");
    let source = fixture
        .persisted_revision()
        .expect("the fixture revision serializes");

    let session = ProjectCore::new()
        .open_demo_editable_session(&source)
        .expect("the persisted project opens");
    let state = session.state();

    assert_eq!(state.album.sheets.len(), 12);
    assert_eq!(state.revision, 1);
    assert_eq!(state.saved_revision, 1);
    assert!(!state.dirty);
    assert!(!state.can_undo);
    assert!(!state.can_redo);
}

#[test]
fn project_core_admits_only_one_editable_session_per_project_until_drop() {
    let core = ProjectCore::new();
    let horizon = SampleProject::Horizon
        .persisted_source(12)
        .expect("the Horizon fixture serializes");
    let aurora = SampleProject::Aurora
        .persisted_source(12)
        .expect("the Aurora fixture serializes");

    let first = core
        .open_demo_editable_session(&horizon)
        .expect("the first editable session is admitted");
    let duplicate = core
        .open_demo_editable_session(&horizon)
        .err()
        .expect("the same Project cannot acquire a second mutable owner");
    let other = core
        .open_demo_editable_session(&aurora)
        .expect("a distinct Project remains independent");

    assert_eq!(
        duplicate,
        crate::CoreError::EditableSessionAlreadyOpen {
            project_id: SampleProject::Horizon.project_id().into(),
        }
    );
    assert_eq!(other.state().project_id, SampleProject::Aurora.project_id());

    drop(first);
    core.open_demo_editable_session(&horizon)
        .expect("dropping the owner releases the Project identity");
}

#[test]
fn rejects_the_previous_project_schema_after_media_categories_became_required() {
    let session = horizon_project(12);
    let persisted = session
        .persisted_revision()
        .expect("the current project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("the current project JSON is valid");
    document["schemaVersion"] = serde_json::json!(2);

    let error = ProjectCore::new()
        .open_demo_editable_session(
            &serde_json::to_string(&document).expect("the old project JSON serializes"),
        )
        .err()
        .expect("the old schema must be rejected explicitly");

    assert_eq!(error, crate::CoreError::UnsupportedSchema(2));
}

#[test]
fn rejects_media_categories_in_the_wrong_visual_role() {
    let persisted = horizon_project(12)
        .persisted_revision()
        .expect("the representative Project serializes");
    let mut photo_as_overlay: serde_json::Value =
        serde_json::from_str(&persisted).expect("the Project JSON is valid");
    photo_as_overlay["album"]["visualDefaults"]["overlay"]["both"]["mediaId"] =
        serde_json::json!("media-serra");

    let error = ProjectCore::new()
        .open_demo_editable_session(
            &serde_json::to_string(&photo_as_overlay).expect("the invalid Project serializes"),
        )
        .err()
        .expect("a Photo cannot be used as an Overlay");
    assert_eq!(
        error,
        crate::CoreError::InvalidProject("Overlay referencia uma Foto: media-serra".into())
    );

    let mut decorative_as_photo: serde_json::Value =
        serde_json::from_str(&persisted).expect("the Project JSON is valid");
    decorative_as_photo["album"]["sheets"][0]["frames"][0]["photo"]["mediaId"] =
        serde_json::json!("decorative-overlay");

    let error = ProjectCore::new()
        .open_demo_editable_session(
            &serde_json::to_string(&decorative_as_photo).expect("the invalid Project serializes"),
        )
        .err()
        .expect("a Decorative cannot fill a Frame");
    assert_eq!(
        error,
        crate::CoreError::InvalidProject(
            "Frame referencia um Decorativo: decorative-overlay".into()
        )
    );
}

#[test]
fn keeps_distinct_sample_projects_isolated() {
    let mut first = sample_project(SampleProject::Horizon, 12);
    let second = sample_project(SampleProject::Aurora, 12);

    first
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the first project accepts an isolated pan");

    assert_eq!(first.state().project_id, "project-spike-001");
    assert_eq!(first.state().revision, 1);
    assert_eq!(second.state().project_id, "project-spike-002");
    assert_eq!(second.state().revision, 0);
}

#[test]
fn keeps_project_and_media_identities_opaque_to_path_syntax() {
    let source = SampleProject::Horizon
        .persisted_source(2)
        .expect("the sample project serializes")
        .replace(
            SampleProject::Horizon.project_id(),
            "Projeto/\u{00c1}lbum CON",
        )
        .replace("media-costa", "Foto/\u{00c1}rvore CON");
    let project = ProjectCore::new()
        .open_demo_editable_session(&source)
        .expect("domain identities do not inherit filesystem restrictions");

    assert_eq!(project.state().project_id, "Projeto/\u{00c1}lbum CON");
    assert!(
        project
            .state()
            .album
            .media
            .iter()
            .any(|media| media.id == "Foto/\u{00c1}rvore CON")
    );
    assert!(
        project
            .render_snapshot()
            .composition
            .sheets
            .iter()
            .flat_map(|sheet| sheet.referenced_media_ids())
            .any(|media_id| media_id == "Foto/\u{00c1}rvore CON")
    );
}

#[test]
fn commits_one_domain_revision_for_a_completed_pan_gesture() {
    let mut session = horizon_project(12);

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
fn confirms_only_the_current_revision_as_saved() {
    let mut session = horizon_project(12);
    session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the sample frame accepts a persisted change");

    let error = session
        .confirm_saved_revision(0)
        .expect_err("an older persisted revision cannot clean the current session");
    assert_eq!(
        error,
        crate::CoreError::SavedRevisionMismatch {
            current: 1,
            confirmed: 0,
        }
    );
    assert!(session.state().dirty);
    assert_eq!(session.state().saved_revision, 0);

    let confirmed = session
        .confirm_saved_revision(1)
        .expect("the current persisted revision can be confirmed");
    assert_eq!(confirmed.saved_revision, 1);
    assert!(!confirmed.dirty);
    assert!(confirmed.can_undo);
}

#[test]
fn divergent_history_branch_never_reuses_the_saved_revision() {
    let mut session = horizon_project(12);
    let saved = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the first edit creates the revision that will be saved");
    session
        .confirm_saved_revision(saved.revision)
        .expect("the first edit is confirmed as the saved revision");

    session
        .undo()
        .expect("the session can return behind the saved revision");
    let branched = session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.0,
            delta_pan_y: 0.40,
            delta_zoom: 0.0,
        })
        .expect("a different edit creates a divergent history branch");

    assert_eq!(branched.revision, 2);
    assert_eq!(branched.saved_revision, saved.revision);
    assert!(branched.dirty);
    assert_eq!(
        session
            .confirm_saved_revision(saved.revision)
            .expect_err("an old save completion cannot confirm a divergent branch"),
        crate::CoreError::SavedRevisionMismatch {
            current: branched.revision,
            confirmed: saved.revision,
        }
    );
    assert!(session.state().dirty);
}

#[test]
fn commits_simultaneous_pan_and_zoom_as_one_domain_revision() {
    let mut session = horizon_project(12);

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
    let mut session = horizon_project(12);
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
    let session = horizon_project(12);

    let snapshot = session.render_snapshot();
    assert_eq!(snapshot.schema_version, 4);
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
    let photo = PlacementPhoto {
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

fn compose_through_public_contract(frame: &RectUm, photo: &PlacementPhoto) -> ComposedPhoto {
    let album = AlbumSnapshot {
        sheets: vec![SheetSnapshot {
            id: "sheet-under-test".into(),
            number: 1,
            role: SheetRole::Internal,
            active_sides: ProjectedActiveSides::Both,
            width_um: frame.x + frame.width,
            height_um: frame.y + frame.height,
            frames: vec![FrameSnapshot {
                id: "frame-under-test".into(),
                rect: frame.clone(),
                z_index: 1,
                photo: Some(PhotoSnapshot {
                    media_id: photo.media_id.clone(),
                    transform: photo.transform.clone(),
                }),
            }],
        }],
        media: vec![MediaCatalogItem {
            id: photo.media_id.clone(),
            kind: MediaKind::Photo,
            name: photo.name.clone(),
            source_width_px: Some(photo.source_width_px),
            source_height_px: Some(photo.source_height_px),
            palette: Some(photo.palette.clone()),
        }],
        visual_defaults: Default::default(),
    };

    CompositionCore::compose(&album).sheets[0].frames[0]
        .photo
        .clone()
        .expect("the public composition contract returns the photo")
}

fn assert_plan_close(actual: &PhotoPlacementPlan, expected: &PhotoPlacementPlan, case_name: &str) {
    assert_pan_close(&actual.current_pan, &expected.current_pan, case_name);
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

fn assert_pan_close(actual: &NormalizedPan, expected: &NormalizedPan, case_name: &str) {
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
    let mut session = horizon_project(12);
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

    let loaded = ProjectCore::new()
        .load_demo_persisted_revision(&persisted)
        .expect("persisted revision can be loaded read-only");

    assert_eq!(loaded.render_snapshot(), session.render_snapshot());
    assert_eq!(loaded.revision(), 1);
}

#[test]
fn persisted_revision_stays_saved_while_an_editable_session_has_unsaved_changes() {
    let core = ProjectCore::new();
    let source = SampleProject::Horizon
        .persisted_source(12)
        .expect("the persisted fixture serializes");
    let mut editable = core
        .open_demo_editable_session(&source)
        .expect("the persisted Project opens for editing");

    editable
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the editable session accepts an unsaved command");
    let loaded = core
        .load_demo_persisted_revision(&source)
        .expect("read-only loading coexists with the dirty editable session");
    let first_snapshot = loaded.render_snapshot();
    let mut detached_snapshot = first_snapshot.clone();
    detached_snapshot.revision = 99;

    assert_eq!(loaded.revision(), 0);
    assert_eq!(loaded.render_snapshot(), first_snapshot);
    assert_ne!(detached_snapshot, loaded.render_snapshot());
    assert_eq!(editable.state().revision, 1);
    assert!(editable.state().dirty);
    assert_ne!(editable.render_snapshot(), loaded.render_snapshot());
}

#[test]
fn the_core_fills_the_leftmost_placeholder_for_a_photo_intent() {
    let mut session = horizon_project(12);

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
fn editor_projection_derives_media_usage_across_fill_undo_and_redo() {
    let mut session = horizon_project(12);

    assert_eq!(
        media_usage_count(&session.projection(), "media-campo"),
        Some(7)
    );

    session
        .apply(ProjectIntent::FillLeftmostPlaceholder {
            sheet_id: "lamina-02".into(),
            media_id: "media-campo".into(),
        })
        .expect("the sample sheet accepts the Photo");
    assert_eq!(
        media_usage_count(&session.projection(), "media-campo"),
        Some(8)
    );

    session.undo().expect("the placement can be undone");
    assert_eq!(
        media_usage_count(&session.projection(), "media-campo"),
        Some(7)
    );

    session.redo().expect("the placement can be redone");
    assert_eq!(
        media_usage_count(&session.projection(), "media-campo"),
        Some(8)
    );
}

#[test]
fn persisted_revision_does_not_store_derived_media_usage() {
    let session = horizon_project(12);
    let persisted = session
        .persisted_revision()
        .expect("the sample project can be serialized");
    let document: serde_json::Value =
        serde_json::from_str(&persisted).expect("the sample JSON is valid");

    for media in document["album"]["media"]
        .as_array()
        .expect("the sample catalog is an array")
    {
        assert!(
            media.get("usageCount").is_none(),
            "usage is derived from Frame placements"
        );
    }
}

#[test]
fn filling_a_placeholder_uses_the_catalog_intrinsic_dimensions() {
    let session = horizon_project(12);
    let persisted = session
        .persisted_revision()
        .expect("the sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("the sample JSON is valid");
    let catalog = document["album"]["media"]
        .as_array_mut()
        .expect("the sample catalog is an array");
    let portrait = catalog
        .iter_mut()
        .find(|item| item["id"] == "media-campo")
        .expect("the sample catalog contains the Photo");
    portrait["sourceWidthPx"] = serde_json::json!(3_000);
    portrait["sourceHeightPx"] = serde_json::json!(5_000);

    let mut session = ProjectCore::new()
        .open_demo_editable_session(
            &serde_json::to_string(&document).expect("the modified project serializes"),
        )
        .expect("the project with a portrait Photo opens");
    session
        .apply(ProjectIntent::FillLeftmostPlaceholder {
            sheet_id: "lamina-02".into(),
            media_id: "media-campo".into(),
        })
        .expect("the portrait Photo fills the placeholder");
    let composition = session.projection().composition;
    let composed = composition.sheets[1].frames[0]
        .photo
        .as_ref()
        .expect("the placeholder contains the composed portrait Photo");

    assert_eq!(composed.placement.size_per_zoom.width, 252_000.0);
    assert_eq!(composed.placement.size_per_zoom.height, 420_000.0);
}

#[test]
fn composition_uses_the_catalog_as_the_single_source_of_media_metadata() {
    let session = horizon_project(12);
    let persisted = session
        .persisted_revision()
        .expect("the sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("the sample JSON is valid");
    let media_id = document["album"]["sheets"][0]["frames"][0]["photo"]["mediaId"]
        .as_str()
        .expect("the first Frame references a catalog item")
        .to_owned();
    let catalog = document["album"]["media"]
        .as_array_mut()
        .expect("the sample catalog is an array");
    let media = catalog
        .iter_mut()
        .find(|item| item["id"] == media_id)
        .expect("the referenced Photo exists in the catalog");
    media["sourceWidthPx"] = serde_json::json!(3_000);
    media["sourceHeightPx"] = serde_json::json!(5_000);

    let session = ProjectCore::new()
        .open_demo_editable_session(
            &serde_json::to_string(&document).expect("the modified project serializes"),
        )
        .expect("the project with updated catalog metadata opens");
    let composed = session.projection().composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the first Frame remains composed")
        .placement
        .size_per_zoom
        .clone();

    assert_eq!(composed.width, 252_000.0);
    assert_eq!(composed.height, 420_000.0);
}

fn media_usage_count(projection: &crate::EditorProjection, media_id: &str) -> Option<usize> {
    projection
        .media_usage
        .iter()
        .find(|usage| usage.media_id == media_id)
        .map(|usage| usage.count)
}

#[test]
fn photo_zoom_is_persistent_and_never_goes_below_fill() {
    let mut session = horizon_project(12);

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
    let session = horizon_project(12);
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
    let session = horizon_project(12);
    let persisted = session
        .persisted_revision()
        .expect("sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("sample JSON is valid");
    document["album"]["media"][0]["sourceWidthPx"] = serde_json::json!(0);

    let error = ProjectCore::new()
        .load_demo_persisted_revision(
            &serde_json::to_string(&document).expect("modified JSON is valid"),
        )
        .err()
        .expect("invalid dimensions must be rejected");

    assert!(error.to_string().contains("dimensões"));
}

#[test]
fn persisted_revision_rejects_an_album_with_fewer_than_two_sheets() {
    let session = horizon_project(2);
    let persisted = session
        .persisted_revision()
        .expect("sample project can be serialized");
    let mut document: serde_json::Value =
        serde_json::from_str(&persisted).expect("sample JSON is valid");
    document["album"]["sheets"]
        .as_array_mut()
        .expect("sample sheets are an array")
        .truncate(1);

    let error = ProjectCore::new()
        .load_demo_persisted_revision(
            &serde_json::to_string(&document).expect("modified JSON is valid"),
        )
        .err()
        .expect("an Álbum needs at least two Lâminas");

    assert!(error.to_string().contains("pelo menos duas Lâminas"));
}

#[test]
fn render_snapshot_rejects_empty_internal_identifiers() {
    let session = horizon_project(12);
    let mut snapshot = session.render_snapshot();
    snapshot.composition.sheets[0].frames[0].frame_id.clear();

    let error = snapshot
        .validate()
        .expect_err("an empty Frame identifier must be rejected");

    assert!(error.to_string().contains("Identificador de Frame"));
}
