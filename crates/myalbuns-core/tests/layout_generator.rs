use myalbuns_core::{
    FrameOrientation, LayoutGenerationStatus, LayoutPermission, LayoutQuery, LayoutScope,
    generate_layouts,
};

#[test]
fn one_vertical_frame_uses_the_approved_centered_geometry() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/layouts/generator-v1.json")).unwrap();
    let example = &corpus["examples"][0];
    let query: LayoutQuery = serde_json::from_value(example["query"].clone()).unwrap();
    let result = generate_layouts(&query);
    assert_eq!(result.status, LayoutGenerationStatus::Candidates);
    assert_eq!(
        serde_json::to_value(&result.candidates[0].definition.positions).unwrap(),
        example["layouts"][0]["positions"]
    );
    assert_eq!(result, generate_layouts(&query));
}

#[test]
fn mixed_frames_keep_the_choice_between_pages_and_a_crossing_composition() {
    let query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface": {"type":"doubleSheet", "widthUm":600000, "heightUm":240000},
        "frameOrientations":["vertical", "vertical", "horizontal"],
        "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
        "minimumSideUm":20000
    }))
    .unwrap();
    let result = generate_layouts(&query);
    assert!(
        result
            .candidates
            .iter()
            .any(|c| c.definition.scope == LayoutScope::Page)
    );
    assert!(
        result
            .candidates
            .iter()
            .any(|c| c.definition.scope == LayoutScope::Sheet)
    );
    for candidate in &result.candidates {
        let positions = &candidate.definition.positions;
        assert_eq!(positions.len(), 3);
        assert!(positions[0].width < positions[0].height);
        assert!(positions[1].width < positions[1].height);
        assert!(positions[2].width > positions[2].height);
    }
}

#[test]
fn approved_counts_and_surfaces_produce_complete_varied_compositions() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/layouts/generator-v1.json")).unwrap();
    for example in corpus["examples"].as_array().unwrap() {
        let query: LayoutQuery = serde_json::from_value(example["query"].clone()).unwrap();
        let result = generate_layouts(&query);
        assert_eq!(
            result.status,
            LayoutGenerationStatus::Candidates,
            "{}",
            example["id"]
        );
        assert!((1..=20).contains(&result.candidates.len()));
        for candidate in &result.candidates {
            assert_generated_geometry(
                &query,
                &candidate.definition,
                example["id"].as_str().unwrap(),
            );
        }
    }
}

#[test]
fn physical_scaling_preserves_the_order_and_normalized_compositions() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/layouts/generator-v1.json")).unwrap();
    for example in corpus["examples"].as_array().unwrap() {
        let query: LayoutQuery = serde_json::from_value(example["query"].clone()).unwrap();
        let mut scaled = query.clone();
        scaled.surface.width_um *= 7;
        scaled.surface.height_um *= 7;
        scaled.parameters.margin_um *= 7;
        scaled.parameters.gap_um *= 7;
        scaled.parameters.minimum_side_um *= 7;
        let original = generate_layouts(&query);
        let enlarged = generate_layouts(&scaled);
        assert_eq!(
            original.candidates.len(),
            enlarged.candidates.len(),
            "{}",
            example["id"]
        );
        for (a, b) in original.candidates.iter().zip(&enlarged.candidates) {
            assert_eq!(a.family, b.family, "{}", example["id"]);
            assert_eq!(a.definition.scope, b.definition.scope);
            for (a, b) in a.definition.positions.iter().zip(&b.definition.positions) {
                for (a, b) in [
                    (a.x, b.x),
                    (a.y, b.y),
                    (a.x + a.width, b.x + b.width),
                    (a.y + a.height, b.y + b.height),
                ] {
                    assert!((a * 7 - b).abs() <= 7, "{}: quantization", example["id"]);
                }
            }
        }
    }
}

#[test]
fn quantization_keeps_squares_square_and_does_not_invent_central_crossings() {
    for permission in ["pagesOnly", "pagesAndSheet"] {
        for (margin, gap) in [(15001, 7501), (0, 0), (0, 1)] {
            let query: LayoutQuery = serde_json::from_value(serde_json::json!({
                "surface":{"type":"doubleSheet","widthUm":500501,"heightUm":250501},
                "frameOrientations":["square","horizontal","vertical","square","square"],
                "permission":permission,"marginUm":margin,"gapUm":gap,"minimumSideUm":20000
            }))
            .unwrap();
            let result = generate_layouts(&query);
            assert_eq!(result.status, LayoutGenerationStatus::Candidates);
            for c in result.candidates {
                assert_generated_geometry(&query, &c.definition, "quantization");
            }
        }
    }
}

#[test]
fn absence_and_invalid_constraints_are_explicit_without_truncating_the_query() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/layouts/generator-v1.json")).unwrap();
    for case in corpus["queryCases"].as_array().unwrap() {
        let query: LayoutQuery = serde_json::from_value(case["query"].clone()).unwrap();
        let result = generate_layouts(&query);
        assert_eq!(
            serde_json::to_value(result.status).unwrap(),
            case["expectedResult"]
        );
        assert!(result.candidates.is_empty());
        let mut invalid = query;
        invalid.surface.width_um = 0;
        assert_eq!(
            generate_layouts(&invalid).status,
            LayoutGenerationStatus::InvalidQuery
        );
        invalid.surface.kind = myalbuns_core::LayoutSurfaceKind::DoubleSheet;
        invalid.surface.width_um = 1;
        assert_eq!(
            generate_layouts(&invalid).status,
            LayoutGenerationStatus::InvalidQuery
        );
    }
}

fn assert_generated_geometry(
    query: &LayoutQuery,
    definition: &myalbuns_core::LayoutDefinition,
    case: &str,
) {
    assert_geometry(query, definition, case, false);
}

/// `uniform_allowed` admits the uniform grid the Generator returns only when
/// nothing else fits, instead of leaving the Frames to the reserve.
fn assert_geometry(
    query: &LayoutQuery,
    definition: &myalbuns_core::LayoutDefinition,
    case: &str,
    uniform_allowed: bool,
) {
    use myalbuns_core::{FrameOrientation, LayoutPermission, LayoutSurfaceKind};
    let positions = &definition.positions;
    assert_eq!(positions.len(), query.frame_orientations.len(), "{case}");
    for (r, orientation) in positions.iter().zip(&query.frame_orientations) {
        assert!(
            r.width.min(r.height) >= query.parameters.minimum_side_um - 1,
            "{case}: minimum {r:?}"
        );
        assert!(
            r.x >= query.parameters.margin_um - 1 && r.y >= query.parameters.margin_um - 1,
            "{case}: margin"
        );
        assert!(
            r.x + r.width <= query.surface.width_um - query.parameters.margin_um + 1,
            "{case}: right margin"
        );
        assert!(
            r.y + r.height <= query.surface.height_um - query.parameters.margin_um + 1,
            "{case}: bottom margin"
        );
        assert!(
            match orientation {
                FrameOrientation::Vertical => r.width < r.height,
                FrameOrientation::Horizontal => r.width > r.height,
                FrameOrientation::Square => r.width == r.height,
            },
            "{case}: orientation {r:?}"
        );
    }
    for (i, a) in positions.iter().enumerate() {
        for b in &positions[i + 1..] {
            let gap = query.parameters.gap_um - 1;
            assert!(
                a.x + a.width + gap <= b.x
                    || b.x + b.width + gap <= a.x
                    || a.y + a.height + gap <= b.y
                    || b.y + b.height + gap <= a.y,
                "{case}: gap"
            );
        }
    }
    let double = query.surface.kind == LayoutSurfaceKind::DoubleSheet;
    let width = query.surface.width_um;
    let crossing = positions
        .iter()
        .any(|r| 2 * r.x < width && 2 * (r.x + r.width) > width);
    assert_eq!(
        definition.scope == LayoutScope::Sheet,
        double && crossing,
        "{case}: scope"
    );
    if query.permission == LayoutPermission::PagesOnly {
        assert!(!double || !crossing, "{case}");
    }
    let groups = if double && definition.scope == LayoutScope::Page {
        let mut groups = Vec::new();
        for side in [0, width] {
            let page: Vec<_> = positions
                .iter()
                .filter(|r| 2 * r.x >= side && 2 * (r.x + r.width) <= side + width)
                .collect();
            if positions.len() > 1 {
                assert!(!page.is_empty(), "{case}: both pages");
            }
            if !page.is_empty() {
                let left = page.iter().map(|r| r.x).min().unwrap();
                let right = page.iter().map(|r| r.x + r.width).max().unwrap();
                let top = page.iter().map(|r| r.y).min().unwrap();
                let bottom = page.iter().map(|r| r.y + r.height).max().unwrap();
                assert!(
                    (2 * (left + right) - 2 * side - width).abs() <= 4,
                    "{case}: page centering"
                );
                assert!(
                    (top + bottom - query.surface.height_um).abs() <= 2,
                    "{case}: vertical centering"
                );
                assert!(
                    2 * left >= side + 2 * (query.parameters.margin_um - 1)
                        && 2 * right <= side + width - 2 * (query.parameters.margin_um - 1),
                    "{case}: inner margin"
                );
            }
            groups.push(page);
        }
        groups
    } else {
        vec![positions.iter().collect()]
    };
    for group in groups {
        if group.is_empty() {
            continue;
        }
        if group.len() >= 4 && !uniform_allowed {
            let first = group[0];
            assert!(
                group
                    .iter()
                    .any(|r| (r.width - first.width).abs() > 1
                        || (r.height - first.height).abs() > 1),
                "{case}: uniform grid"
            );
        }
        assert_no_internal_vacancy(&group, query.parameters.gap_um, case);
    }
}

fn double_sheet_query(
    width_um: i64,
    height_um: i64,
    orientations: &[FrameOrientation],
    permission: LayoutPermission,
) -> LayoutQuery {
    serde_json::from_value(serde_json::json!({
        "surface": {"type":"doubleSheet", "widthUm":width_um, "heightUm":height_um},
        "frameOrientations": orientations,
        "permission": permission,
        "marginUm":15000, "gapUm":5000, "minimumSideUm":20000
    }))
    .unwrap()
}

fn mixes(count: usize) -> impl Iterator<Item = Vec<FrameOrientation>> {
    (0..=count).map(move |vertical| {
        let mut orientations = vec![FrameOrientation::Vertical; vertical];
        orientations.resize(count, FrameOrientation::Horizontal);
        orientations
    })
}

#[test]
fn double_sheets_offer_valid_suggestions_under_both_permissions() {
    for (width, height) in [(600000, 300000), (400000, 300000), (800000, 300000)] {
        for count in 1..=8 {
            for orientations in mixes(count) {
                for permission in [LayoutPermission::PagesAndSheet, LayoutPermission::PagesOnly] {
                    let query = double_sheet_query(width, height, &orientations, permission);
                    let case = format!("{width}x{height} {orientations:?} {permission:?}");
                    let result = generate_layouts(&query);
                    assert_eq!(result.status, LayoutGenerationStatus::Candidates, "{case}");
                    for candidate in &result.candidates {
                        assert_generated_geometry(&query, &candidate.definition, &case);
                    }
                }
            }
        }
    }
}

#[test]
fn common_frame_counts_are_not_left_to_the_reserve() {
    use FrameOrientation::{Horizontal, Vertical};
    let page = |width, height, orientations: Vec<FrameOrientation>| -> LayoutQuery {
        serde_json::from_value(serde_json::json!({
            "surface": {"type":"singlePage", "widthUm":width, "heightUm":height},
            "frameOrientations": orientations,
            "permission": "pagesOnly",
            "marginUm":15000, "gapUm":5000, "minimumSideUm":20000
        }))
        .unwrap()
    };
    // Version 1 left these to the reserve, which also changed orientations.
    let queries = [
        page(300000, 300000, vec![Vertical; 4]),
        page(300000, 300000, vec![Horizontal; 4]),
        page(200000, 300000, vec![Vertical; 4]),
        page(200000, 300000, vec![Vertical; 6]),
        page(200000, 300000, vec![Horizontal; 12]),
        page(200000, 300000, vec![Horizontal; 15]),
        double_sheet_query(800000, 300000, &[Vertical], LayoutPermission::PagesAndSheet),
        double_sheet_query(800000, 300000, &[Vertical], LayoutPermission::PagesOnly),
        double_sheet_query(400000, 300000, &[Horizontal], LayoutPermission::PagesOnly),
    ];
    for query in queries {
        let result = generate_layouts(&query);
        let case = format!("{query:?}");
        assert_eq!(result.status, LayoutGenerationStatus::Candidates, "{case}");
        for candidate in &result.candidates {
            assert_geometry(&query, &candidate.definition, &case, true);
        }
    }
}

#[test]
fn a_hero_may_be_supported_by_a_stack_of_equal_frames() {
    let query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface": {"type":"singlePage", "widthUm":300000, "heightUm":300000},
        "frameOrientations": ["vertical", "vertical", "vertical", "vertical"],
        "permission": "pagesOnly", "marginUm":15000, "gapUm":5000, "minimumSideUm":20000
    }))
    .unwrap();
    let result = generate_layouts(&query);
    assert!(result.candidates.iter().any(|candidate| {
        let mut areas: Vec<_> = candidate
            .definition
            .positions
            .iter()
            .map(|r| r.width * r.height)
            .collect();
        areas.sort();
        // Equal up to the 1 µm rounding of each edge.
        1000 * (areas[2] - areas[0]) <= areas[0] && areas[3] > 2 * areas[0]
    }));
}

#[test]
fn page_blocks_share_a_top_and_bottom_line_in_the_first_suggestion() {
    for orientations in [
        vec![FrameOrientation::Vertical; 3],
        vec![FrameOrientation::Vertical; 4],
        vec![FrameOrientation::Vertical; 8],
    ] {
        let query = double_sheet_query(600000, 300000, &orientations, LayoutPermission::PagesOnly);
        let first = &generate_layouts(&query).candidates[0].definition.positions;
        let lines = |left: bool| {
            let page: Vec<_> = first
                .iter()
                .filter(|r| (2 * (r.x + r.width) <= 600000) == left)
                .collect();
            (
                page.iter().map(|r| r.y).min().unwrap(),
                page.iter().map(|r| r.y + r.height).max().unwrap(),
            )
        };
        let (left, right) = (lines(true), lines(false));
        assert!(
            (left.0 - right.0).abs() <= 1 && (left.1 - right.1).abs() <= 1,
            "{orientations:?}: {left:?} x {right:?}"
        );
    }
}

#[test]
fn mirrored_forms_follow_the_distinct_structures() {
    let (width, height) = (600000, 300000);
    let query = double_sheet_query(
        width,
        height,
        &[FrameOrientation::Vertical; 6],
        LayoutPermission::PagesOnly,
    );
    // Smallest sorted rectangle list among the flips of both axes, of each
    // Page block and of the Page swap, in doubled micrometres.
    let structure = |definition: &myalbuns_core::LayoutDefinition| {
        let mut smallest: Option<Vec<[i64; 4]>> = None;
        for vertical in [false, true] {
            for horizontal in [false, true] {
                for (left, right) in [(false, false), (true, false), (false, true), (true, true)] {
                    let mut rects: Vec<_> = definition
                        .positions
                        .iter()
                        .map(|r| {
                            let (mut x, mut y) = (2 * r.x, 2 * r.y);
                            let (w, h) = (2 * r.width, 2 * r.height);
                            if x + w <= width {
                                if left {
                                    x = width - x - w;
                                }
                            } else if right {
                                x = 3 * width - x - w;
                            }
                            if horizontal {
                                x = 2 * width - x - w;
                            }
                            if vertical {
                                y = 2 * height - y - h;
                            }
                            [x, y, w, h]
                        })
                        .collect();
                    rects.sort();
                    if smallest.as_ref().is_none_or(|s| rects < *s) {
                        smallest = Some(rects);
                    }
                }
            }
        }
        smallest.unwrap()
    };
    let structures: Vec<_> = generate_layouts(&query)
        .candidates
        .iter()
        .map(|c| structure(&c.definition))
        .collect();
    let repeated: Vec<_> = structures
        .iter()
        .enumerate()
        .map(|(i, s)| structures[..i].contains(s))
        .collect();
    assert!(repeated.contains(&true), "the case has mirrored forms");
    assert!(
        repeated.windows(2).all(|pair| pair[1] || !pair[0]),
        "a new structure after a mirrored form: {repeated:?}"
    );
}

fn assert_no_internal_vacancy(rects: &[&myalbuns_core::RectUm], gap: i64, case: &str) {
    // Independently sweep the union after expanding each rectangle by half a gap.
    // Doubled coordinates preserve half-micrometer edges without float arithmetic.
    let top = rects.iter().map(|r| 2 * r.y).min().unwrap();
    let bottom = rects.iter().map(|r| 2 * (r.y + r.height)).max().unwrap();
    let left = rects.iter().map(|r| 2 * r.x).min().unwrap();
    let right = rects.iter().map(|r| 2 * (r.x + r.width)).max().unwrap();
    let expanded: Vec<_> = rects
        .iter()
        .map(|r| {
            (
                (2 * r.x - gap - 2).max(left),
                (2 * (r.x + r.width) + gap + 2).min(right),
                (2 * r.y - gap - 2).max(top),
                (2 * (r.y + r.height) + gap + 2).min(bottom),
            )
        })
        .collect();
    let mut xs: Vec<_> = expanded.iter().flat_map(|r| [r.0, r.1]).collect();
    xs.sort();
    xs.dedup();
    for pair in xs.windows(2) {
        let mut intervals: Vec<_> = expanded
            .iter()
            .filter(|r| r.0 <= pair[0] && r.1 >= pair[1])
            .map(|r| (r.2, r.3))
            .collect();
        intervals.sort();
        let mut edge = top;
        for (start, end) in intervals {
            assert!(start <= edge, "{case}: internal vacancy");
            edge = edge.max(end);
        }
        assert_eq!(edge, bottom, "{case}: incomplete column");
    }
}
