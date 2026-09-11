use super::*;
use myalbuns_core::{FrameSnapKind, FrameSnapRequest, LayoutSelection};

fn snapped_edit(
    project: &myalbuns_core::EditableProject,
    gesture: FrameGeometryGesture,
) -> FrameGeometryEdit {
    let mut edit = selection(
        &project.projection().state.album.sheets[0].frames[..1],
        gesture,
    );
    edit.snap = Some(FrameSnapRequest {
        um_per_pixel_x: 500.0,
        um_per_pixel_y: 500.0,
        retained: vec![],
    });
    edit
}

#[test]
fn micrometer_dimension_variants_share_a_stable_exact_snap_reference() {
    for handle in [FrameResizeHandle::Right, FrameResizeHandle::Bottom] {
        for reverse in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let mut rectangles = vec![
                [213_000, 79_000, 60_000, 40_000],
                [55_000, 190_000, 86_666, 86_666],
                [420_000, 170_000, 86_667, 86_667],
            ];
            if reverse {
                rectangles.swap(1, 2);
            }
            let mut project = project_with_rectangles(root.path(), 0, false, &rectangles);
            let before = project.projection();
            let mut acquired = None;
            for free_size in [86_666, 86_667, 86_666, 86_667] {
                let horizontal = handle == FrameResizeHandle::Right;
                let edit = snapped_edit(
                    &project,
                    FrameGeometryGesture::Resize {
                        handle,
                        delta_x_um: if horizontal { free_size - 60_000 } else { 0 },
                        delta_y_um: if horizontal { 0 } else { free_size - 40_000 },
                        preserve_aspect_ratio: false,
                        from_center: false,
                    },
                );
                // Fresh acquisitions from either side choose the same reference,
                // even when frame stack order changes.
                let preview = project.preview_frame_geometry(&edit).unwrap();
                let rect = &preview.frames[0].clip_rect;
                assert_eq!(if horizontal { rect.width } else { rect.height }, 86_666);
                if let Some(retained) = &acquired {
                    assert_eq!(&preview.snap.retained, retained);
                }
                acquired = Some(preview.snap.retained);
                assert!(preview.snap.guides.iter().any(|guide| {
                    guide.kind == FrameSnapKind::Dimension
                        && guide.measurement_um == Some(86_666.0)
                        && if horizontal {
                            guide.x1 == 55_000.0
                        } else {
                            guide.y1 == 190_000.0
                        }
                }));
                assert_eq!(project.projection(), before);
            }
            let horizontal = handle == FrameResizeHandle::Right;
            let edit = snapped_edit(
                &project,
                FrameGeometryGesture::Resize {
                    handle,
                    delta_x_um: if horizontal { 26_667 } else { 0 },
                    delta_y_um: if horizontal { 0 } else { 46_667 },
                    preserve_aspect_ratio: false,
                    from_center: false,
                },
            );
            let after = project
                .apply(ProjectIntent::EditFrameGeometry { edit })
                .unwrap();
            let frames = &after.state.album.sheets[0].frames;
            assert_eq!(
                if horizontal {
                    frames[0].rect.width
                } else {
                    frames[0].rect.height
                },
                86_666
            );
            assert_eq!(&frames[1..], &before.state.album.sheets[0].frames[1..]);
        }
    }
}

#[test]
fn consecutive_micrometer_values_do_not_merge_distinct_dimension_groups() {
    let root = tempfile::tempdir().unwrap();
    let project = project_with_rectangles(
        root.path(),
        0,
        false,
        &[
            [213_000, 79_000, 60_000, 40_000],
            [55_000, 190_000, 86_666, 70_000],
            [420_000, 170_000, 86_667, 80_000],
            [180_000, 190_000, 86_668, 60_000],
        ],
    );
    let edit = snapped_edit(
        &project,
        FrameGeometryGesture::Resize {
            handle: FrameResizeHandle::Right,
            delta_x_um: 26_668,
            delta_y_um: 0,
            preserve_aspect_ratio: false,
            from_center: false,
        },
    );
    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(preview.frames[0].clip_rect.width, 86_668);
    assert!(preview.snap.guides.iter().any(|guide| {
        guide.kind == FrameSnapKind::Dimension && guide.measurement_um == Some(86_668.0)
    }));
}

#[test]
fn a_dimension_group_uses_an_exactly_reachable_reference_at_the_resize_limit() {
    for has_valid_reference in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut rectangles = vec![
            [213_000, 79_000, 60_000, 40_000],
            [55_000, 190_000, 11_999, 70_000],
        ];
        if has_valid_reference {
            rectangles.push([420_000, 170_000, 12_000, 80_000]);
        }
        let project = project_with_rectangles(root.path(), 0, false, &rectangles);
        let edit = snapped_edit(
            &project,
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::Right,
                delta_x_um: -48_000,
                delta_y_um: 0,
                preserve_aspect_ratio: false,
                from_center: false,
            },
        );
        let preview = project.preview_frame_geometry(&edit).unwrap();
        assert_eq!(preview.frames[0].clip_rect.width, 12_000);
        let measurements: Vec<_> = preview
            .snap
            .guides
            .iter()
            .filter(|guide| guide.kind == FrameSnapKind::Dimension)
            .map(|guide| guide.measurement_um)
            .collect();
        assert_eq!(
            measurements,
            if has_valid_reference {
                vec![Some(12_000.0); 2]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn confirmed_album_gap_drives_generation_and_snap_without_reflowing_existing_frames() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_rectangles(
        root.path(),
        0,
        false,
        &[
            [213_000, 47_000, 60_000, 40_000],
            [100_000, 40_000, 50_000, 50_000],
        ],
    );
    let before = project.projection();
    let sheet = before.state.album.sheets[0].id.clone();
    let old_query = project.query_layouts(&sheet).unwrap();
    let captured = project.capture_custom_layout(&sheet).unwrap();
    let intent = |gap| ProjectIntent::SetAlbumDesign {
        visual_defaults: before.state.album.visual_defaults.clone(),
        frame_gap_um: gap,
    };
    assert!(project.apply(intent(-1)).is_err());
    assert_eq!(project.projection(), before);
    let after = project.apply(intent(9_000)).unwrap();
    assert_eq!(after.state.album.sheets, before.state.album.sheets);
    assert_eq!(project.capture_custom_layout(&sheet).unwrap(), captured);
    assert!(
        project
            .apply(ProjectIntent::ApplyLayout {
                selection: LayoutSelection {
                    query_id: old_query.query_id,
                    candidate_index: 0
                }
            })
            .is_err()
    );
    let query = project.query_layouts(&sheet).unwrap();
    assert_eq!(query.settings.parameters.gap_um, 9_000);
    assert_ne!(query.listing.candidates, old_query.listing.candidates);
    let edit = snapped_edit(
        &project,
        FrameGeometryGesture::Move {
            delta_x_um: -52_000,
            delta_y_um: 0,
        },
    );
    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(preview.frames[0].clip_rect.x, 159_000);
    assert!(
        preview
            .snap
            .guides
            .iter()
            .any(|guide| guide.kind == FrameSnapKind::ProjectGap
                && guide.measurement_um == Some(9_000.0))
    );
    project.undo().unwrap();
    assert_eq!(
        project
            .query_layouts(&sheet)
            .unwrap()
            .settings
            .parameters
            .gap_um,
        5_000
    );
    project.redo().unwrap();
    assert_eq!(
        project
            .query_layouts(&sheet)
            .unwrap()
            .settings
            .parameters
            .gap_um,
        9_000
    );
}

#[test]
fn chosen_initial_gap_is_persisted_and_invalid_creation_has_no_file() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    for gap in [0, 9_000, -1] {
        let path = root.path().join(format!("Gap{gap}.myalbuns"));
        let result = core.create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral().with_frame_gap_um(gap),
            CreateAuthorization::CreateOnly,
        ));
        if gap < 0 {
            assert!(result.is_err());
            assert!(!path.exists());
            continue;
        }
        let project = result.unwrap();
        assert_eq!(
            project.projection().state.layout_settings.parameters.gap_um,
            gap
        );
        drop(project);
        let project = core
            .open_editable(OpenProjectRequest::new(location(&path)))
            .unwrap();
        assert_eq!(
            project.projection().state.layout_settings.parameters.gap_um,
            gap
        );
    }
}

#[test]
fn spacing_resize_preserves_anchors_in_both_axes_including_centered_resize() {
    for vertical in [false, true] {
        for centered in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let rectangles = if vertical {
                [
                    [47_000, 40_000, 40_000, 60_000],
                    [40_000, 150_000, 50_000, 50_000],
                ]
            } else {
                [
                    [40_000, 47_000, 60_000, 40_000],
                    [150_000, 40_000, 50_000, 50_000],
                ]
            };
            let project = project_with_rectangles(root.path(), 0, false, &rectangles);
            let delta = 43_000;
            let edit = snapped_edit(
                &project,
                FrameGeometryGesture::Resize {
                    handle: if vertical {
                        FrameResizeHandle::Bottom
                    } else {
                        FrameResizeHandle::Right
                    },
                    delta_x_um: if vertical { 0 } else { delta },
                    delta_y_um: if vertical { delta } else { 0 },
                    preserve_aspect_ratio: false,
                    from_center: centered,
                },
            );
            let preview = project.preview_frame_geometry(&edit).unwrap();
            let rect = &preview.frames[0].clip_rect;
            let (start, size) = if vertical {
                (rect.y, rect.height)
            } else {
                (rect.x, rect.width)
            };
            // Centered growth cannot reach the neighbor without crossing the surface.
            if centered {
                assert_eq!(start, 0);
                assert_eq!(size, 140_000);
                assert!(
                    !preview
                        .snap
                        .guides
                        .iter()
                        .any(|guide| guide.kind == FrameSnapKind::ProjectGap)
                );
            } else {
                assert_eq!((start, size), (40_000, 105_000));
                assert!(
                    preview
                        .snap
                        .guides
                        .iter()
                        .any(|guide| guide.kind == FrameSnapKind::ProjectGap)
                );
            }
        }
    }
}

#[test]
fn snap_commit_round_trips_without_transient_feedback_and_noop_has_no_history() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frame(root.path());
    let before = project.projection();
    let noop = snapped_edit(
        &project,
        FrameGeometryGesture::Move {
            delta_x_um: 0,
            delta_y_um: 0,
        },
    );
    assert_eq!(
        project
            .apply(ProjectIntent::EditFrameGeometry { edit: noop })
            .unwrap(),
        before
    );
    let edit = snapped_edit(
        &project,
        FrameGeometryGesture::Move {
            delta_x_um: -28_000,
            delta_y_um: 0,
        },
    );
    let preview = project.preview_frame_geometry(&edit).unwrap();
    project
        .apply(ProjectIntent::EditFrameGeometry { edit })
        .unwrap();
    let composition = project.render_snapshot().composition;
    assert_eq!(composition.sheets[0].frames, preview.frames);
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.path().join("Frame.myalbuns");
    let serialized = fs::read_to_string(&path).unwrap();
    assert!(!serialized.contains("retained"));
    assert!(!serialized.contains("measurementUm"));
    let mut reopened = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let media_id = reopened.projection().state.album.media[0].id;
    reopened
        .observe_photo_source(media_id, photo_metadata())
        .unwrap();
    assert_eq!(reopened.render_snapshot().composition, composition);
}

#[test]
fn technical_targets_match_active_edges_and_zero_disables_only_the_technical_reference() {
    for single in [false, true] {
        for enabled in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let mut project = project_with_rectangles(
                root.path(),
                0,
                single,
                &[[80_000, 47_000, 60_000, 40_000]],
            );
            project.apply(ProjectIntent::SetAlbumInformation { information: serde_json::from_value(serde_json::json!({
                "displayUnit": "mm", "sheetWidthUm": 600_000, "sheetHeightUm": 300_000, "dpi": 300,
                "bleedUm": if enabled { 10_000 } else { 0 }, "safetyUm": if enabled { 12_000 } else { 0 },
                "firstSheet": if single { "singlePage" } else { "double" }, "lastSheet": "double"
            })).unwrap() }).unwrap();
            for (target, expected) in [
                (10_000, if enabled { 10_000 } else { 12_000 }),
                (22_000, if enabled { 22_000 } else { 24_000 }),
            ] {
                let edit = snapped_edit(
                    &project,
                    FrameGeometryGesture::Move {
                        delta_x_um: target + 2_000 - 80_000,
                        delta_y_um: 0,
                    },
                );
                let preview = project.preview_frame_geometry(&edit).unwrap();
                // The first single Page is right-active: its binding edge has no
                // vertical cut/safety reference, unlike the outer edge of a double.
                assert_eq!(
                    preview.frames[0].clip_rect.x,
                    if single { target + 2_000 } else { expected }
                );
            }
            let edit = snapped_edit(
                &project,
                FrameGeometryGesture::Move {
                    delta_x_um: 0,
                    delta_y_um: 12_000 - 47_000,
                },
            );
            assert_eq!(
                project.preview_frame_geometry(&edit).unwrap().frames[0]
                    .clip_rect
                    .y,
                if enabled { 10_000 } else { 12_000 }
            );
        }
    }
}

#[test]
fn equal_corrections_prefer_alignment_to_dimension_and_do_not_change_the_reference() {
    let root = tempfile::tempdir().unwrap();
    let project = project_with_rectangles(
        root.path(),
        0,
        false,
        &[
            [213_000, 79_000, 60_000, 40_000],
            [315_000, 190_000, 100_000, 80_000],
        ],
    );
    let before = project.projection();
    let edit = snapped_edit(
        &project,
        FrameGeometryGesture::Resize {
            handle: FrameResizeHandle::Right,
            delta_x_um: 41_000,
            delta_y_um: 0,
            preserve_aspect_ratio: false,
            from_center: false,
        },
    );
    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(preview.frames[0].clip_rect.width, 102_000);
    assert!(
        preview
            .snap
            .guides
            .iter()
            .all(|guide| guide.kind == FrameSnapKind::Alignment)
    );
    assert_eq!(project.projection(), before);
}

#[test]
fn overlapping_neighbors_cannot_be_skipped_to_invent_a_free_gap() {
    for vertical in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut rectangles = [
            [250_000, 47_000, 40_000, 40_000],
            [20_000, 40_000, 50_000, 50_000],
            [50_000, 40_000, 50_000, 50_000],
            [110_000, 40_000, 50_000, 50_000],
        ];
        if vertical {
            for rect in &mut rectangles {
                rect.swap(0, 1);
                rect.swap(2, 3);
            }
        }
        let mut project = project_with_rectangles(root.path(), 0, false, &rectangles);
        let edit = snapped_edit(
            &project,
            FrameGeometryGesture::Move {
                delta_x_um: if vertical { 0 } else { -52_000 },
                delta_y_um: if vertical { -52_000 } else { 0 },
            },
        );
        let preview = project.preview_frame_geometry(&edit).unwrap();
        let rect = &preview.frames[0].clip_rect;
        assert_eq!(if vertical { rect.y } else { rect.x }, 198_000);
        assert!(
            !preview
                .snap
                .guides
                .iter()
                .any(|guide| guide.kind == FrameSnapKind::EqualGap
                    && guide.measurement_um == Some(40_000.0))
        );
        let after = project
            .apply(ProjectIntent::EditFrameGeometry { edit })
            .unwrap();
        assert_eq!(
            after.state.album.sheets[0].frames[0].rect,
            preview.frames[0].clip_rect
        );
    }
}

#[test]
fn equidistant_technical_references_follow_numeric_geometric_order() {
    let root = tempfile::tempdir().unwrap();
    let mut project =
        project_with_rectangles(root.path(), 0, false, &[[80_000, 47_000, 60_000, 40_000]]);
    project
        .apply(ProjectIntent::SetAlbumInformation {
            information: serde_json::from_value(serde_json::json!({
                "displayUnit": "mm", "sheetWidthUm": 600_000, "sheetHeightUm": 300_000, "dpi": 300,
                "bleedUm": 6_000, "safetyUm": 6_000, "firstSheet": "double", "lastSheet": "double"
            }))
            .unwrap(),
        })
        .unwrap();
    let mut edit = snapped_edit(
        &project,
        FrameGeometryGesture::Move {
            delta_x_um: -71_000,
            delta_y_um: 0,
        },
    );
    edit.snap = Some(FrameSnapRequest {
        um_per_pixel_x: 1000.0,
        um_per_pixel_y: 1000.0,
        retained: vec![],
    });
    let preview = project.preview_frame_geometry(&edit).unwrap();
    assert_eq!(
        preview.frames[0].clip_rect.x, 6_000,
        "cut at 6 mm precedes safety at 12 mm; IDs must not be sorted as text"
    );
}

#[test]
fn snap_visual_corpus_uses_public_core_previews() {
    let resize = |handle, dx, dy, shift, alt| FrameGeometryGesture::Resize {
        handle,
        delta_x_um: dx,
        delta_y_um: dy,
        preserve_aspect_ratio: shift,
        from_center: alt,
    };
    let mov = |dx, dy| FrameGeometryGesture::Move {
        delta_x_um: dx,
        delta_y_um: dy,
    };
    let dimensions = vec![
        [213_000, 79_000, 60_000, 40_000],
        [420_000, 190_000, 100_000, 80_000],
    ];
    let horizontal = vec![
        [210_000, 47_000, 60_000, 40_000],
        [20_000, 40_000, 50_000, 50_000],
        [90_000, 40_000, 50_000, 50_000],
    ];
    let mut cases = Vec::new();
    for (name, rectangles, count, single, gesture) in [
        ("alignment", dimensions.clone(), 1, false, mov(-61_000, 0)),
        (
            "width",
            dimensions.clone(),
            1,
            false,
            resize(FrameResizeHandle::Right, 38_000, 0, false, false),
        ),
        (
            "height",
            dimensions.clone(),
            1,
            false,
            resize(FrameResizeHandle::Bottom, 0, 38_000, false, false),
        ),
        (
            "micrometer-width",
            vec![
                [213_000, 79_000, 60_000, 40_000],
                [55_000, 190_000, 86_666, 86_666],
                [420_000, 170_000, 86_667, 86_667],
            ],
            1,
            false,
            resize(FrameResizeHandle::Right, 26_667, 0, false, false),
        ),
        (
            "micrometer-height",
            vec![
                [213_000, 79_000, 60_000, 40_000],
                [55_000, 190_000, 86_666, 86_666],
                [420_000, 170_000, 86_667, 86_667],
            ],
            1,
            false,
            resize(FrameResizeHandle::Bottom, 0, 46_667, false, false),
        ),
        (
            "corner",
            dimensions.clone(),
            1,
            false,
            resize(FrameResizeHandle::BottomRight, 38_000, 38_000, false, false),
        ),
        (
            "shift",
            dimensions.clone(),
            1,
            false,
            resize(FrameResizeHandle::BottomRight, 38_000, 20_000, true, false),
        ),
        (
            "alt",
            dimensions.clone(),
            1,
            false,
            resize(FrameResizeHandle::BottomRight, 19_000, 19_000, false, true),
        ),
        (
            "shift-alt",
            dimensions,
            1,
            false,
            resize(FrameResizeHandle::BottomRight, 19_000, 10_000, true, true),
        ),
        ("project-gap", horizontal.clone(), 1, false, mov(-63_000, 0)),
        ("equal-gap", horizontal, 1, false, mov(-48_000, 0)),
        (
            "vertical-gap",
            vec![
                [47_000, 210_000, 40_000, 60_000],
                [40_000, 20_000, 50_000, 50_000],
                [40_000, 90_000, 50_000, 50_000],
            ],
            1,
            false,
            mov(0, -48_000),
        ),
        (
            "balanced",
            vec![
                [115_000, 47_000, 40_000, 40_000],
                [20_000, 40_000, 50_000, 50_000],
                [210_000, 40_000, 50_000, 50_000],
            ],
            1,
            false,
            mov(7_000, 0),
        ),
        (
            "group",
            vec![
                [20_000, 47_000, 50_000, 40_000],
                [100_000, 57_000, 50_000, 30_000],
                [260_000, 190_000, 150_000, 60_000],
            ],
            2,
            false,
            resize(FrameResizeHandle::Right, 18_000, 0, false, false),
        ),
        (
            "single",
            vec![[80_000, 47_000, 60_000, 40_000]],
            1,
            true,
            mov(72_000, 0),
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let project = project_with_rectangles(root.path(), 0, single, &rectangles);
        let before = project.projection();
        let mut edit = selection(&before.state.album.sheets[0].frames[..count], gesture);
        edit.snap = Some(FrameSnapRequest {
            um_per_pixel_x: 500.0,
            um_per_pixel_y: 500.0,
            retained: vec![],
        });
        let preview = project.preview_frame_geometry(&edit).unwrap();
        assert!(!preview.snap.guides.is_empty(), "{name}");
        let mut value = serde_json::json!({ "name": name, "sheet": before.composition.sheets[0], "edit": edit, "preview": preview,
            "technicalGuides": { "bleedUm": before.state.document.bleed_um, "safetyUm": before.state.document.safety_um } });
        // Stable fixture identities only; all geometry and feedback come from Core.
        value["sheet"]["sheetId"] = "snap-sheet".into();
        for (index, frame) in before.state.album.sheets[0].frames.iter().enumerate() {
            let mut encoded = serde_json::to_string(&value).unwrap();
            encoded = encoded.replace(&frame.id, &format!("snap-frame-{index}"));
            value = serde_json::from_str(&encoded).unwrap();
        }
        let encoded = serde_json::to_string(&value).unwrap().replace(
            &before.state.album.media[0].id.to_string(),
            "00000000-0000-4000-8000-000000000001",
        );
        cases.push(serde_json::from_str::<serde_json::Value>(&encoded).unwrap());
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({ "cases": cases })).unwrap()
    );
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/frame-snap-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_SNAP_FIXTURE").is_some() {
        fs::write(&path, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    );
}
