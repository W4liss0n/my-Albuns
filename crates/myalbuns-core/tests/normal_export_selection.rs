use myalbuns_core::{
    ComposedColor, ComposedSheet, CompositionPlan, ExportMode, ProjectedActiveSides,
    ProjectedFrameBorder, RectUm, RenderSnapshot,
};

fn snapshot() -> RenderSnapshot {
    let sheets = [
        ProjectedActiveSides::Right,
        ProjectedActiveSides::Both,
        ProjectedActiveSides::Left,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, active_sides)| {
        let width = if active_sides == ProjectedActiveSides::Both {
            600_000
        } else {
            300_000
        };
        ComposedSheet {
            sheet_id: format!("sheet-{index}"),
            number: index + 1,
            active_sides,
            width_um: width,
            height_um: 300_000,
            base: ComposedColor {
                rgb: "#FFFFFF".into(),
                draw_rect: RectUm {
                    x: 0,
                    y: 0,
                    width,
                    height: 300_000,
                },
            },
            backgrounds: vec![],
            frames: vec![],
            overlays: vec![],
        }
    })
    .collect();
    RenderSnapshot {
        schema_version: 6,
        project_id: "project".into(),
        project_name: "Album".into(),
        revision: 7,
        dpi: 300,
        unit: "micrometers".into(),
        composition: CompositionPlan {
            frame_border: ProjectedFrameBorder::None,
            sheets,
        },
    }
}

#[test]
fn page_intervals_preserve_original_indexes_and_independent_physical_dimensions() {
    let snapshot = snapshot();
    let units = snapshot
        .export_units(&["sheet-1".into()], ExportMode::Page)
        .unwrap();
    assert_eq!(
        units.iter().map(|unit| unit.index).collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        units[0].viewport,
        RectUm {
            x: 0,
            y: 0,
            width: 300_000,
            height: 300_000
        }
    );
    assert_eq!(
        units[1].viewport,
        RectUm {
            x: 300_000,
            y: 0,
            width: 300_000,
            height: 300_000
        }
    );
    let all = snapshot
        .export_units(
            &["sheet-0".into(), "sheet-1".into(), "sheet-2".into()],
            ExportMode::Page,
        )
        .unwrap();
    assert_eq!(
        all.iter().map(|unit| unit.index).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    let sheets = snapshot
        .export_units(&["sheet-1".into()], ExportMode::Sheet)
        .unwrap();
    assert_eq!(sheets[0].index, 2);
    assert_eq!(sheets[0].viewport.width, 600_000);
}

#[test]
fn export_rejects_noncontinuous_duplicate_empty_and_unknown_selections() {
    for selection in [
        vec![],
        vec!["sheet-1", "sheet-1"],
        vec!["sheet-0", "sheet-2"],
        vec!["unknown"],
    ] {
        assert!(
            snapshot()
                .export_units(
                    &selection.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                    ExportMode::Sheet
                )
                .is_err()
        );
    }
}
