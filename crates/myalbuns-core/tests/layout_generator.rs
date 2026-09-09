use myalbuns_core::{LayoutGenerationStatus, LayoutQuery, LayoutScope, generate_layouts};

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
        assert!((1..=10).contains(&result.candidates.len()));
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
        if group.len() >= 4 {
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
