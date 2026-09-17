#![cfg(windows)]

use myalbuns_core::*;
use myalbuns_paths::OperationPathContext;
use std::{fs, path::Path};

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

fn information(width: i64, height: i64) -> AlbumInformation {
    AlbumInformation {
        display_unit: DisplayUnit::Mm,
        sheet_width_um: width,
        sheet_height_um: height,
        dpi: 300,
        bleed_um: 3_000,
        safety_um: 3_000,
        first_sheet: EndSheetFormat::SinglePage,
        last_sheet: EndSheetFormat::SinglePage,
    }
}

fn create(root: &Path, width: i64, height: i64) -> EditableProject {
    core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Dimensions.myalbuns")),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                width,
                height,
                300,
                3_000,
                3_000,
                3,
                EndSheetFormat::SinglePage,
                EndSheetFormat::SinglePage,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap()
}

fn metadata() -> PhotoSourceMetadata {
    PhotoSourceMetadata::new(
        1_200,
        800,
        ["#112233".into(), "#445566".into(), "#778899".into()],
    )
    .unwrap()
}

fn composed(root: &Path, rectangles: &[[i64; 4]], observe: bool) -> EditableProject {
    let mut project = create(root, 600_000, 300_000);
    let photo_path = root.join("Photo.jpg");
    fs::write(&photo_path, b"trusted original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(photo_path, metadata()))
        .unwrap();
    for sheet in &imported.projection.state.album.sheets {
        for _ in rectangles {
            project
                .apply(ProjectIntent::AddPhoto {
                    sheet_id: sheet.id.clone(),
                    media_id: imported.media_id,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
        }
    }
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.join("Dimensions.myalbuns");
    let mut payload: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for sheet in payload["project"]["sheets"].as_array_mut().unwrap() {
        for (frame, [x, y, width, height]) in sheet["frames"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(rectangles)
        {
            frame["rect"] = serde_json::json!({ "x": x, "y": y, "width": width, "height": height });
            frame["photo"]["transform"]["panX"] = 0.5.into();
            frame["photo"]["transform"]["panY"] = (-0.25).into();
            frame["photo"]["transform"]["userZoom"] = 2.into();
        }
    }
    fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
    let mut project = core(root)
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    if observe {
        project
            .observe_photo_source(imported.media_id, metadata())
            .unwrap();
    }
    project
}

fn resize(
    project: &mut EditableProject,
    information: AlbumInformation,
) -> Result<EditorProjection, CoreError> {
    let validation = project.validate_album_information(&information);
    let key = validation
        .impact
        .and_then(|impact| impact.dimensional_change)
        .map(|change| change.confirmation_key);
    project.apply(ProjectIntent::SetAlbumInformation {
        information,
        expected_dimension_key: key,
    })
}

#[test]
fn approved_formats_include_exactly_ten_percent_in_both_directions() {
    // Closed Album dimensions from the accepted reference table, expressed in um.
    let cases = [
        ((200_000, 200_000), (300_000, 300_000), true),
        ((200_000, 300_000), (300_000, 450_000), true),
        ((200_000, 300_000), (250_000, 350_000), true),
        ((250_000, 350_000), (300_000, 400_000), true),
        ((250_000, 300_000), (300_000, 350_000), true),
        ((300_000, 400_000), (300_000, 440_000), true),
        ((300_000, 400_000), (300_000, 450_000), false),
        ((200_000, 300_000), (300_000, 200_000), false),
        ((300_000, 300_000), (300_000, 330_000), true),
        ((300_000, 300_000), (300_000, 330_001), false),
        ((230_000, 240_000), (240_000, 230_000), true),
    ];
    for (from, to, allowed) in cases {
        for (from, to) in [(from, to), (to, from)] {
            let root = tempfile::tempdir().unwrap();
            let mut project = create(root.path(), from.0 * 2, from.1);
            let before = project.project().clone();
            let proposed = information(to.0 * 2, to.1);
            assert_eq!(
                project
                    .validate_album_information(&proposed)
                    .errors
                    .is_empty(),
                allowed,
                "{from:?} -> {to:?}"
            );
            assert_eq!(resize(&mut project, proposed).is_ok(), allowed);
            if !allowed {
                assert_eq!(project.project(), &before);
                assert_eq!(project.revision(), 0);
            }
        }
    }
}

#[test]
fn nonuniform_resize_preserves_focal_point_and_history_on_both_single_pages_and_double_sheet() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
    let before = project.project().clone();
    let file_before = fs::read(project.project_path()).unwrap();
    let revision = project.revision();
    let after = resize(&mut project, information(630_000, 300_000)).unwrap();
    for sheet in &after.state.album.sheets {
        let frame = &sheet.frames[0];
        assert_eq!(
            frame.rect,
            RectUm {
                x: 21_000,
                y: 20_000,
                width: 126_000,
                height: 80_000
            }
        );
        let transform = &frame.photo.as_ref().unwrap().transform;
        assert_eq!(transform.pan_x, 0.5);
        assert!((transform.pan_y + 0.238636).abs() < 0.000001);
        assert_eq!(transform.user_zoom, 2.0);
    }
    assert_eq!(project.revision(), revision + 1);
    assert_eq!(fs::read(project.project_path()).unwrap(), file_before);
    let changed = project.project().clone();
    project.undo().unwrap();
    assert_eq!(project.project(), &before);
    project.redo().unwrap();
    assert_eq!(project.project(), &changed);
    assert_eq!(
        project.freeze_rendering().projection().composition,
        after.composition
    );
    let sheet_id = &after.state.album.sheets[1].id;
    let frozen = project.freeze_rendering().into_sheet(sheet_id).unwrap();
    assert_eq!(frozen.output_unit().sheet, after.composition.sheets[1]);
    frozen.output_unit().validate().unwrap();
    project.save(project.revision()).unwrap();
    let path = project.project_path().to_owned();
    drop(project);
    let reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.project(), &changed);
}

#[test]
fn missing_metadata_blocks_only_nonuniform_changes_and_missing_original_with_metadata_works() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], false);
    let before = project.project().clone();
    let proposed = information(630_000, 300_000);
    assert_eq!(
        project.validate_album_information(&proposed).errors,
        [ProjectConfigurationValidationError::SheetDimensionsUnknownPhotoSize]
    );
    assert!(resize(&mut project, proposed).is_err());
    assert_eq!(project.project(), &before);
    resize(&mut project, information(1_200_000, 600_000)).unwrap();
    project.undo().unwrap();
    let media = project.projection().state.album.media[0].id;
    project.observe_photo_source(media, metadata()).unwrap();
    fs::remove_file(root.path().join("Photo.jpg")).unwrap();
    resize(&mut project, proposed).unwrap();
}

#[test]
fn stale_source_observations_cannot_apply_a_previously_confirmed_crop() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
    let information = information(630_000, 300_000);
    let before = project.project().clone();
    let key = project
        .validate_album_information(&information)
        .impact
        .unwrap()
        .dimensional_change
        .unwrap()
        .confirmation_key;
    let media = project.projection().state.album.media[0].id;
    project
        .observe_photo_source(
            media,
            PhotoSourceMetadata::new(
                800,
                1_200,
                ["#112233".into(), "#445566".into(), "#778899".into()],
            )
            .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        project.apply(ProjectIntent::SetAlbumInformation {
            information,
            expected_dimension_key: Some(key),
        }),
        Err(CoreError::AlbumInformationReviewChanged)
    ));
    assert_eq!(project.project(), &before);
    resize(&mut project, information).unwrap();
}

#[test]
fn locked_layouts_scale_without_reorganization_and_remain_locked_after_save() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
    let ids: Vec<_> = project
        .projection()
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect();
    for id in &ids {
        let query = project.query_layouts(id).unwrap();
        project
            .apply(ProjectIntent::ToggleLayoutFavorite {
                selection: LayoutSelection {
                    query_id: query.query_id.clone(),
                    candidate_index: 0,
                },
            })
            .unwrap();
        let query = project.query_layouts(id).unwrap();
        project
            .apply(ProjectIntent::LockLayout {
                selection: LayoutSelection {
                    query_id: query.query_id,
                    candidate_index: 0,
                },
            })
            .unwrap();
    }
    let before = project.project().clone();
    resize(&mut project, information(630_000, 300_000)).unwrap();
    assert_eq!(
        project.project().favorite_layouts(),
        before.favorite_layouts()
    );
    for (old, new) in before.sheets().iter().zip(project.project().sheets()) {
        assert!(new.layout_locked());
        assert_eq!(old.frames()[0].id(), new.frames()[0].id());
        assert_eq!(
            new.frames()[0].rect().height(),
            old.frames()[0].rect().height()
        );
        assert_ne!(
            new.last_layout().unwrap().definition.surface.width_um,
            old.last_layout().unwrap().definition.surface.width_um
        );
    }
    assert!(matches!(
        project.apply(ProjectIntent::AddFrame {
            sheet_id: ids[1].clone()
        }),
        Err(CoreError::LayoutLocked)
    ));
    project.save(project.revision()).unwrap();
    let after = project.project().clone();
    let path = project.project_path().to_owned();
    drop(project);
    let reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.project(), &after);
}

#[test]
fn quantization_failure_never_publishes_a_partial_resize() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 1, 1]], true);
    let before = project.project().clone();
    let mut proposed = information(60_000, 30_000);
    proposed.bleed_um = 0;
    proposed.safety_um = 0;
    assert_eq!(
        project.validate_album_information(&proposed).errors,
        [ProjectConfigurationValidationError::SheetDimensionsInvalidContent]
    );
    assert!(resize(&mut project, proposed).is_err());
    assert_eq!(project.project(), &before);
}

#[test]
fn photo_orientation_fine_angle_and_mirroring_keep_the_same_focal_point() {
    for turns in 0..4 {
        for angle in [-45.0, -12.3, 0.0, 45.0] {
            for mirror in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
                project.save(project.revision()).unwrap();
                drop(project);
                let path = root.path().join("Dimensions.myalbuns");
                let mut payload: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                for sheet in payload["project"]["sheets"].as_array_mut().unwrap() {
                    let transform = &mut sheet["frames"][0]["photo"]["transform"];
                    transform["quarterTurns"] = turns.into();
                    transform["angleTenths"] = ((angle * 10.0) as i32).into();
                    transform["mirrorX"] = mirror.into();
                    transform["blackAndWhite"] = true.into();
                }
                fs::write(&path, serde_json::to_vec(&payload).unwrap()).unwrap();
                let mut project = core(root.path())
                    .open_editable(OpenProjectRequest::new(location(&path)))
                    .unwrap();
                let media = project.projection().state.album.media[0].id;
                project.observe_photo_source(media, metadata()).unwrap();
                let before = project.projection();
                let after = resize(&mut project, information(630_000, 300_000)).unwrap();
                for (old, new) in before
                    .composition
                    .sheets
                    .iter()
                    .zip(&after.composition.sheets)
                {
                    let center_in_photo = |frame: &ComposedFrame| {
                        let p = &frame.photo.as_ref().unwrap().placement;
                        (
                            (p.current.center.x - frame.clip_rect.width as f64 / 2.0)
                                / p.current.size.width,
                            (p.current.center.y - frame.clip_rect.height as f64 / 2.0)
                                / p.current.size.width,
                        )
                    };
                    let old = center_in_photo(&old.frames[0]);
                    let point = center_in_photo(&new.frames[0]);
                    assert!(
                        (old.0 - point.0).abs() < 0.000001 && (old.1 - point.1).abs() < 0.000001
                    );
                }
                let t = &after.state.album.sheets[0].frames[0]
                    .photo
                    .as_ref()
                    .unwrap()
                    .transform;
                assert_eq!(t.quarter_turns, turns);
                assert_eq!(t.fine_rotation_degrees, angle as f32);
                assert_eq!(t.mirror_x, mirror);
                assert!(t.black_and_white);
            }
        }
    }
}

#[test]
fn focal_point_clamps_to_fill_without_increasing_user_zoom() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 100_000, 100_000]], true);
    let frame_id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id,
            delta_pan_x: 0.5,
            delta_pan_y: 0.25,
            delta_zoom: -1.0,
        })
        .unwrap();
    let after = resize(&mut project, information(660_000, 300_000)).unwrap();
    let transform = &after.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .transform;
    assert_eq!(transform.pan_x, 1.0);
    assert_eq!(transform.pan_y, 0.0);
    assert_eq!(transform.user_zoom, 1.0);
}

#[test]
fn dpi_and_unit_only_changes_leave_composition_and_physical_styles_intact() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], false);
    let before = project.project().clone();
    let composition = project.projection().composition;
    let mut proposed = information(600_000, 300_000);
    proposed.display_unit = DisplayUnit::In;
    proposed.dpi = 600;
    assert!(
        project
            .validate_album_information(&proposed)
            .impact
            .unwrap()
            .dimensional_change
            .is_none()
    );
    resize(&mut project, proposed).unwrap();
    assert_eq!(project.project().sheets(), before.sheets());
    assert_eq!(
        project.project().visual_defaults(),
        before.visual_defaults()
    );
    assert_eq!(project.projection().composition, composition);
}

#[test]
fn borders_and_layout_parameters_scale_by_the_smaller_axis_and_technical_margins_stay_physical() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
    let projection = project.projection();
    let mut defaults = projection.state.album.visual_defaults;
    defaults.frame_border = ProjectedFrameBorder::Solid {
        rgb: "#123456".into(),
        width_um: 2_000,
    };
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults,
        })
        .unwrap();
    project
        .apply(ProjectIntent::SetFrameStyle {
            edit: FrameStyleEdit {
                frame_ids: vec![projection.state.album.sheets[0].frames[0].id.clone()],
                change: FrameStyleChange::BorderWidth { width_um: 3_000 },
            },
        })
        .unwrap();
    let before = project.project().clone();
    let changed = resize(&mut project, information(1_260_000, 600_000)).unwrap();
    assert_eq!(
        changed.state.album.sheets[0].frames[0]
            .style
            .border_width_um,
        6_000
    );
    assert_eq!(
        changed.state.album.sheets[1].frames[0]
            .style
            .border_width_um,
        4_000
    );
    assert_eq!(changed.state.layout_settings.parameters.margin_um, 30_000);
    assert_eq!(changed.state.layout_settings.parameters.gap_um, 10_000);
    assert_eq!(
        changed.state.layout_settings.parameters.minimum_side_um,
        40_000
    );
    assert_eq!(changed.state.document.bleed_um, 3_000);
    assert_eq!(changed.state.document.safety_um, 3_000);
    assert_eq!(
        project.project().visual_defaults().background(),
        before.visual_defaults().background()
    );
    assert_eq!(
        project.project().visual_defaults().overlay(),
        before.visual_defaults().overlay()
    );
}

#[test]
fn resize_and_edge_conversion_are_one_action_and_do_not_unlock_the_middle_sheet() {
    let root = tempfile::tempdir().unwrap();
    let mut project = composed(root.path(), &[[20_000, 20_000, 120_000, 80_000]], true);
    for sheet in project.projection().state.album.sheets {
        let query = project.query_layouts(&sheet.id).unwrap();
        project
            .apply(ProjectIntent::LockLayout {
                selection: LayoutSelection {
                    query_id: query.query_id,
                    candidate_index: 0,
                },
            })
            .unwrap();
    }
    let before = project.project().clone();
    let revision = project.revision();
    let mut proposed = information(630_000, 300_000);
    proposed.first_sheet = EndSheetFormat::Double;
    proposed.last_sheet = EndSheetFormat::Double;
    resize(&mut project, proposed).unwrap();
    assert_eq!(project.revision(), revision + 1);
    let sheets = project.project().sheets();
    assert_eq!(sheets[0].active_sides(), ActiveSides::Both);
    assert!(!sheets[0].layout_locked());
    assert!(sheets[1].layout_locked());
    assert!(!sheets[2].layout_locked());
    project.undo().unwrap();
    assert_eq!(project.project(), &before);
}
