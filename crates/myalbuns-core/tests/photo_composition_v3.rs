#![cfg(windows)]

use std::fs;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, ImportPhoto, InitialProject, MediaKind,
    OpenProjectRequest, PhotoDropTarget, PhotoPlacementMode, PhotoSourceMetadata, ProjectCore,
    ProjectIntent, ProjectLocation, RelinkMedia, SaveProjectOutcome,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the Project root is captured at the operation boundary");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn photo_metadata() -> PhotoSourceMetadata {
    PhotoSourceMetadata::new(
        1_200,
        800,
        ["#B83225".into(), "#477998".into(), "#F4EBD9".into()],
    )
    .expect("the observed JPEG metadata is valid")
}

fn create_project(core: &ProjectCore, path: &std::path::Path) -> myalbuns_core::EditableProject {
    core.create_editable(CreateProjectRequest::new(
        location(path),
        InitialProject::neutral(),
        CreateAuthorization::CreateOnly,
    ))
    .expect("the productive Project is created")
}

#[test]
fn imported_photo_adds_one_filled_frame_and_persists_only_the_external_link() {
    let root = tempfile::tempdir().expect("temporary first-photo Project");
    let project_path = root.path().join("Primeira composição.myalbuns");
    let original_path = root.path().join("Foto original.jpg");
    let original_bytes = b"external original remains byte-for-byte untouched";
    fs::write(&original_path, original_bytes).expect("the linked original exists");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project is created");

    let imported = project
        .import_photo(ImportPhoto::new(original_path.clone(), photo_metadata()))
        .expect("the trusted JPEG import is committed");
    assert_eq!(imported.projection.state.revision, 1);
    assert_eq!(imported.projection.state.album.media.len(), 1);
    assert_eq!(
        imported.projection.state.album.media[0].kind,
        MediaKind::Photo
    );
    assert_eq!(project.project().media()[0].path(), original_path);

    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    let added = project
        .apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id,
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .expect("double-click semantics add the first Photo");
    let affected_frame_id = added
        .affected_frame_id
        .as_deref()
        .expect("the mutation identifies exactly one affected Frame");
    assert_eq!(added.projection.state.revision, 2);
    assert_eq!(added.projection.state.album.sheets[0].frames.len(), 1);
    let frame = &added.projection.state.album.sheets[0].frames[0];
    assert_eq!(frame.id, affected_frame_id);
    assert_eq!(
        frame.photo.as_ref().expect("the Frame is filled").media_id,
        imported.media_id
    );
    assert!(frame.rect.width > 0 && frame.rect.height > 0);
    let composed = &added.projection.composition.sheets[0].frames[0];
    let photo = composed.photo.as_ref().expect("the Frame is composed");
    assert!(photo.draw_rect.width >= composed.clip_rect.width);
    assert!(photo.draw_rect.height >= composed.clip_rect.height);
    assert!(photo.placement.base_fill_zoom > 0.0);
    assert_eq!(photo.placement.current_zoom, 1.0);

    assert_eq!(
        project.save(2).expect("the composed Project is saved"),
        SaveProjectOutcome::Saved { revision: 2 }
    );
    let persisted_bytes = fs::read(&project_path).expect("the v3 Project is readable");
    let persisted: serde_json::Value =
        serde_json::from_slice(&persisted_bytes).expect("the v3 Project is JSON");
    assert_eq!(persisted["schemaVersion"], 3);
    assert_eq!(persisted["project"]["media"][0]["kind"], "photo");
    assert_eq!(
        persisted["project"]["sheets"][0]["frames"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let persisted_text = String::from_utf8(persisted_bytes).expect("the Project is UTF-8");
    for forbidden in [
        "sourceWidth",
        "sourceHeight",
        "palette",
        "availability",
        "fingerprint",
        "cache",
    ] {
        assert!(
            !persisted_text.contains(forbidden),
            "derived runtime state must not be persisted: {forbidden}"
        );
    }
    assert_eq!(
        fs::read(&original_path).expect("the linked original remains readable"),
        original_bytes,
        "creative edits never rewrite the linked Original"
    );
    drop(project);

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the saved composition reopens");
    reopened
        .observe_photo_source(imported.media_id, photo_metadata())
        .expect("the Host hydrates transient source metadata after reopening");
    let reopened_projection = reopened.projection();
    assert_eq!(reopened_projection.state.album.sheets[0].frames.len(), 1);
    assert_eq!(
        reopened_projection.state.album.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .media_id,
        imported.media_id
    );
    assert_eq!(
        reopened_projection.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .placement
            .current_zoom,
        1.0
    );
    assert!(!reopened_projection.state.dirty);
    assert!(!reopened_projection.state.can_undo);
}

#[test]
fn edit_drop_uses_topmost_rectangle_and_invalid_targets_leave_no_revision() {
    let root = tempfile::tempdir().expect("temporary drop-target Project");
    let project_path = root.path().join("Alvos sobrepostos.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path);
    let first = project
        .import_photo(ImportPhoto::new(
            root.path().join("A.jpg"),
            photo_metadata(),
        ))
        .unwrap();
    let second = project
        .import_photo(ImportPhoto::new(
            root.path().join("B.jpg"),
            photo_metadata(),
        ))
        .unwrap();
    let replacement = project
        .import_photo(ImportPhoto::new(
            root.path().join("C.jpg"),
            photo_metadata(),
        ))
        .unwrap();
    let sheet_id = replacement.projection.state.album.sheets[0].id.clone();

    let first_drop = project
        .apply_with_outcome(ProjectIntent::DropPhoto {
            sheet_id: sheet_id.clone(),
            media_id: first.media_id,
            x_um: 40_000,
            y_um: 20_000,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    let first_frame = first_drop.affected_frame_id.unwrap();
    let first_rect = first_drop.projection.state.album.sheets[0].frames[0]
        .rect
        .clone();
    assert_eq!((first_rect.x, first_rect.y), (0, 0));
    assert_eq!((first_rect.width, first_rect.height), (240_000, 160_000));

    let second_drop = project
        .apply_with_outcome(ProjectIntent::DropPhoto {
            sheet_id: sheet_id.clone(),
            media_id: second.media_id,
            x_um: 300_000,
            y_um: 80_000,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    let second_frame = second_drop.affected_frame_id.unwrap();
    assert_ne!(first_frame, second_frame);
    assert_eq!(second_drop.projection.state.album.sheets[0].frames.len(), 2);

    assert_eq!(
        project
            .photo_drop_target(&sheet_id, 200_000, 80_000, PhotoPlacementMode::Edit)
            .unwrap(),
        PhotoDropTarget::Frame {
            frame_id: second_frame.clone(),
        },
        "the later Frame wins by stack order wherever rectangles overlap"
    );
    let replaced = project
        .apply_with_outcome(ProjectIntent::DropPhoto {
            sheet_id: sheet_id.clone(),
            media_id: replacement.media_id,
            x_um: 200_000,
            y_um: 80_000,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    assert_eq!(
        replaced.affected_frame_id.as_deref(),
        Some(second_frame.as_str())
    );
    assert_eq!(
        replaced.projection.state.album.sheets[0].frames[0].id,
        first_frame
    );
    assert_eq!(
        replaced.projection.state.album.sheets[0].frames[1]
            .photo
            .as_ref()
            .unwrap()
            .media_id,
        replacement.media_id
    );

    let revision_before_invalid = project.revision();
    assert_eq!(
        project
            .photo_drop_target(&sheet_id, -1, 50, PhotoPlacementMode::Edit)
            .unwrap(),
        PhotoDropTarget::Invalid
    );
    assert!(
        project
            .apply_with_outcome(ProjectIntent::DropPhoto {
                sheet_id,
                media_id: replacement.media_id,
                x_um: 600_001,
                y_um: 10,
                mode: PhotoPlacementMode::Edit,
            })
            .is_err()
    );
    assert_eq!(project.revision(), revision_before_invalid);
}

#[test]
fn double_click_orders_placeholders_by_left_then_top_and_selection_is_not_history() {
    let root = tempfile::tempdir().expect("temporary placeholder Project");
    let project_path = root.path().join("Placeholders ordenados.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path);
    let imported = project
        .import_photo(ImportPhoto::new(
            root.path().join("Foto.jpg"),
            photo_metadata(),
        ))
        .unwrap();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    for (x_um, y_um) in [(40_000, 40_000), (300_000, 40_000), (560_000, 40_000)] {
        project
            .apply_with_outcome(ProjectIntent::DropPhoto {
                sheet_id: sheet_id.clone(),
                media_id: imported.media_id,
                x_um,
                y_um,
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
    }
    project.save(project.revision()).unwrap();
    drop(project);

    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&project_path).unwrap()).expect("the v3 fixture is JSON");
    let frames = value["project"]["sheets"][0]["frames"]
        .as_array_mut()
        .unwrap();
    for (frame, (x, y)) in
        frames
            .iter_mut()
            .zip([(200_000, 20_000), (100_000, 90_000), (100_000, 20_000)])
    {
        frame["rect"]["x"] = x.into();
        frame["rect"]["y"] = y.into();
        frame["rect"]["width"] = 80_000.into();
        frame["rect"]["height"] = 60_000.into();
        frame["photo"] = serde_json::Value::Null;
    }
    fs::write(&project_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the placeholder Project reopens");
    reopened
        .observe_photo_source(imported.media_id, photo_metadata())
        .unwrap();
    let expected_frame = reopened.projection().state.album.sheets[0].frames[2]
        .id
        .clone();
    let revision_before = reopened.revision();
    let filled = reopened
        .apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id,
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    assert_eq!(
        filled.affected_frame_id.as_deref(),
        Some(expected_frame.as_str())
    );
    assert_eq!(filled.projection.state.revision, revision_before + 1);
    assert_eq!(
        filled.projection.state.album.sheets[0]
            .frames
            .iter()
            .filter(|frame| frame.photo.is_some())
            .count(),
        1
    );
    let undone = reopened
        .undo()
        .expect("one Undo removes only the fill mutation");
    assert!(
        undone.state.album.sheets[0]
            .frames
            .iter()
            .all(|frame| frame.photo.is_none())
    );
}

#[test]
fn user_pan_and_zoom_are_distinct_from_fill_zoom_and_round_trip_with_history() {
    let root = tempfile::tempdir().expect("temporary transform Project");
    let project_path = root.path().join("Enquadramento.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path);
    let imported = project
        .import_photo(ImportPhoto::new(
            root.path().join("Pan.jpg"),
            photo_metadata(),
        ))
        .unwrap();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    let added = project
        .apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id,
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    let frame_id = added.affected_frame_id.unwrap();
    let base_fill = added.projection.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .placement
        .base_fill_zoom;

    let transformed = project
        .apply_with_outcome(ProjectIntent::TransformPhoto {
            frame_id: frame_id.clone(),
            delta_pan_x: 0.25,
            delta_pan_y: -0.2,
            delta_zoom: 0.75,
        })
        .unwrap();
    let placement = &transformed.projection.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .placement;
    assert_eq!(placement.base_fill_zoom, base_fill);
    assert_eq!(placement.current_zoom, 1.75);
    assert!((placement.current_pan.x - 0.25).abs() <= 1e-6);
    assert!((placement.current_pan.y + 0.2).abs() <= 1e-6);

    let undone = project.undo().unwrap();
    let placement = &undone.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .placement;
    assert_eq!(placement.current_zoom, 1.0);
    let redone = project.redo().unwrap();
    assert_eq!(
        redone.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .placement
            .current_zoom,
        1.75
    );
    project.save(project.revision()).unwrap();
    drop(project);

    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .unwrap();
    reopened
        .observe_photo_source(imported.media_id, photo_metadata())
        .unwrap();
    let reopened_projection = reopened.projection();
    let placement = &reopened_projection.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .placement;
    assert_eq!(placement.base_fill_zoom, base_fill);
    assert_eq!(placement.current_zoom, 1.75);
    assert!((placement.current_pan.x - 0.25).abs() <= 1e-6);
    assert!((placement.current_pan.y + 0.2).abs() <= 1e-6);
}

#[test]
fn relink_undo_and_redo_use_the_metadata_observed_for_each_logical_path() {
    let root = tempfile::tempdir().expect("temporary Photo relink Project");
    let project_path = root.path().join("Religação com histórico.myalbuns");
    let original_path = root.path().join("Original horizontal.jpg");
    let replacement_path = root.path().join("Substituta vertical.jpg");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path);
    let original_metadata = photo_metadata();
    let replacement_metadata = PhotoSourceMetadata::new(
        800,
        1_200,
        ["#234567".into(), "#456789".into(), "#6789AB".into()],
    )
    .expect("the replacement observation is valid");
    let imported = project
        .import_photo(ImportPhoto::new(
            original_path.clone(),
            original_metadata.clone(),
        ))
        .expect("the horizontal Original is linked");
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    let added = project
        .apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id,
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .expect("the linked Photo is composed");
    let original_fill_zoom = added.projection.composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the original Photo is composed")
        .placement
        .base_fill_zoom;

    project
        .relink_media(RelinkMedia::new(
            imported.media_id,
            replacement_path.clone(),
        ))
        .expect("the Photo link changes");
    project
        .observe_photo_source(imported.media_id, replacement_metadata)
        .expect("the replacement dimensions are observed");
    let replacement_fill_zoom = project.projection().composition.sheets[0].frames[0]
        .photo
        .as_ref()
        .expect("the replacement Photo is composed")
        .placement
        .base_fill_zoom;
    assert_ne!(replacement_fill_zoom, original_fill_zoom);

    let undone = project.undo().expect("Undo restores the previous link");
    assert_eq!(project.project().media()[0].path(), original_path);
    assert_eq!(
        undone.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the original Photo is composed immediately after Undo")
            .placement
            .base_fill_zoom,
        original_fill_zoom,
        "runtime observations follow the restored logical link"
    );

    let redone = project.redo().expect("Redo restores the replacement link");
    assert_eq!(project.project().media()[0].path(), replacement_path);
    assert_eq!(
        redone.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the replacement Photo is composed immediately after Redo")
            .placement
            .base_fill_zoom,
        replacement_fill_zoom,
        "runtime observations remain available for both history states"
    );
}
