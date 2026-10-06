use myalbuns_core::{
    FrameOrientation, LayoutGenerationStatus, LayoutParameters, LayoutPermission, LayoutQuery,
    LayoutScope, LayoutSurface, LayoutSurfaceKind, generate_layouts,
};
use proptest::{
    prelude::*,
    test_runner::{FileFailurePersistence, RngSeed},
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
    // A whole Page takes its Photo to the edges, past the Margin, whatever
    // the Frame's orientation; everything else keeps both.
    let whole = |r: &myalbuns_core::RectUm| {
        if query.surface.kind == LayoutSurfaceKind::DoubleSheet {
            fills_a_page(r, query.surface.width_um, query.surface.height_um)
        } else {
            *r == myalbuns_core::RectUm {
                x: 0,
                y: 0,
                width: query.surface.width_um,
                height: query.surface.height_um,
            }
        }
    };
    for (r, orientation) in positions.iter().zip(&query.frame_orientations) {
        assert!(
            r.width.min(r.height) >= query.parameters.minimum_side_um - 1,
            "{case}: minimum {r:?}"
        );
        if whole(r) {
            continue;
        }
        assert!(
            r.x >= query.parameters.margin_um - 1 && r.y >= query.parameters.margin_um - 1,
            "{case}: margin {r:?}"
        );
        assert!(
            r.x + r.width <= query.surface.width_um - query.parameters.margin_um + 1,
            "{case}: right margin"
        );
        assert!(
            r.y + r.height <= query.surface.height_um - query.parameters.margin_um + 1,
            "{case}: bottom margin"
        );
        // A free Frame becomes vertical or horizontal, never square.
        assert!(
            match orientation {
                Some(FrameOrientation::Vertical) => r.width < r.height,
                Some(FrameOrientation::Horizontal) => r.width > r.height,
                Some(FrameOrientation::Square) => r.width == r.height,
                None => r.width != r.height,
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
            if let [only] = page.as_slice()
                && whole(only)
            {
                groups.push(page);
                continue;
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

/// Every property runs the same generated queries on every machine: a fixed
/// seed, a fixed number of cases, and failures kept beside the tests so that a
/// counterexample found once is tried first from then on.
fn generated_queries() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        rng_seed: RngSeed::Fixed(0x6D79_416C_6275_6E73),
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/proptest-regressions/layout_generator.txt",
        ))),
        ..ProptestConfig::default()
    }
}

fn any_permission() -> impl Strategy<Value = LayoutPermission> {
    prop_oneof![
        Just(LayoutPermission::PagesOnly),
        Just(LayoutPermission::PagesAndSheet),
    ]
}

fn query(
    kind: LayoutSurfaceKind,
    (width_um, height_um): (i64, i64),
    frame_orientations: Vec<Option<FrameOrientation>>,
    permission: LayoutPermission,
    (margin_um, gap_um, minimum_side_um): (i64, i64, i64),
) -> LayoutQuery {
    LayoutQuery {
        surface: LayoutSurface {
            kind,
            width_um,
            height_um,
        },
        frame_orientations,
        frame_proportions: Vec::new(),
        permission,
        parameters: LayoutParameters {
            margin_um,
            gap_um,
            minimum_side_um,
        },
    }
}

/// Up to `maximum` Frames of the two orientations a Photo usually has.
fn vertical_and_horizontal_frames(
    maximum: usize,
) -> impl Strategy<Value = Vec<Option<FrameOrientation>>> {
    prop::collection::vec(
        prop_oneof![
            Just(Some(FrameOrientation::Vertical)),
            Just(Some(FrameOrientation::Horizontal)),
        ],
        1..=maximum,
    )
}

/// Any Album the product accepts at its usual sizes, with up to eight Frames
/// of any orientation, free ones included, and any valid Layout settings.
fn any_album_query() -> impl Strategy<Value = LayoutQuery> {
    let surface = prop_oneof![
        (100_000..=600_000_i64, 100_000..=600_000_i64)
            .prop_map(|size| (LayoutSurfaceKind::SinglePage, size)),
        (200_000..=1_200_000_i64, 100_000..=600_000_i64)
            .prop_map(|size| (LayoutSurfaceKind::DoubleSheet, size)),
    ];
    let orientation = prop_oneof![
        Just(None),
        Just(Some(FrameOrientation::Vertical)),
        Just(Some(FrameOrientation::Horizontal)),
        Just(Some(FrameOrientation::Square)),
    ];
    (
        surface,
        prop::collection::vec(orientation, 1..=8),
        any_permission(),
        (0..=30_000_i64, 0..=15_000_i64, 10_000..=40_000_i64),
    )
        .prop_map(|((kind, size), orientations, permission, parameters)| {
            query(kind, size, orientations, permission, parameters)
        })
}

/// Double Sheets whose measures never divide evenly: odd sizes, squares among
/// the Frames, and Margins and Gaps of a few micrometres or one past a round one.
fn odd_measure_query() -> impl Strategy<Value = LayoutQuery> {
    let odd = |range: std::ops::RangeInclusive<i64>| range.prop_map(|half| 2 * half + 1);
    let orientation = prop_oneof![
        Just(FrameOrientation::Square),
        Just(FrameOrientation::Vertical),
        Just(FrameOrientation::Horizontal),
    ];
    (
        (odd(200_000..=400_000), odd(100_000..=200_000)),
        prop::collection::vec(orientation, 0..=5),
        any_permission(),
        prop_oneof![0..=2_i64, 15_000..=15_002_i64],
        prop_oneof![0..=2_i64, 7_500..=7_502_i64],
    )
        .prop_map(|(size, others, permission, margin_um, gap_um)| {
            let mut orientations = vec![Some(FrameOrientation::Square)];
            orientations.extend(others.into_iter().map(Some));
            query(
                LayoutSurfaceKind::DoubleSheet,
                size,
                orientations,
                permission,
                (margin_um, gap_um, 20_000),
            )
        })
}

/// Double Sheets between two portrait and two wide Pages, with the Layout
/// settings a new Project starts from.
fn usual_double_sheet_query() -> impl Strategy<Value = LayoutQuery> {
    (
        400_000..=800_000_i64,
        vertical_and_horizontal_frames(8),
        any_permission(),
    )
        .prop_map(|(width_um, orientations, permission)| {
            query(
                LayoutSurfaceKind::DoubleSheet,
                (width_um, 300_000),
                orientations,
                permission,
                (15_000, 5_000, 20_000),
            )
        })
}

/// What version 1 left to the reserve: a Page of Frames that all share one
/// orientation, and a double Sheet with a single Frame.
fn repeated_orientation_query() -> impl Strategy<Value = LayoutQuery> {
    let orientation = prop_oneof![
        Just(FrameOrientation::Vertical),
        Just(FrameOrientation::Horizontal),
    ];
    prop_oneof![
        (200_000..=300_000_i64, orientation.clone(), 1..=15_usize).prop_map(
            |(width_um, orientation, count)| query(
                LayoutSurfaceKind::SinglePage,
                (width_um, 300_000),
                vec![Some(orientation); count],
                LayoutPermission::PagesOnly,
                (15_000, 5_000, 20_000),
            )
        ),
        (400_000..=800_000_i64, orientation, any_permission()).prop_map(
            |(width_um, orientation, permission)| query(
                LayoutSurfaceKind::DoubleSheet,
                (width_um, 300_000),
                vec![Some(orientation)],
                permission,
                (15_000, 5_000, 20_000),
            )
        ),
    ]
}

/// One to twenty suggestions, each one keeping every geometric invariant.
fn assert_suggestions(query: &LayoutQuery, result: &myalbuns_core::LayoutGeneration) {
    let case = format!("{query:?}");
    assert_eq!(result.status, LayoutGenerationStatus::Candidates, "{case}");
    assert!((1..=20).contains(&result.candidates.len()), "{case}");
    // The uniform grid comes back alone, when nothing else fits.
    let uniform_allowed = result.candidates.len() == 1;
    for candidate in &result.candidates {
        assert_geometry(query, &candidate.definition, &case, uniform_allowed);
    }
}

proptest! {
    #![proptest_config(generated_queries())]

    #[test]
    fn every_suggestion_for_a_valid_query_keeps_the_geometric_invariants(
        query in any_album_query()
    ) {
        let result = generate_layouts(&query);
        if result.status == LayoutGenerationStatus::NoCandidates {
            prop_assert!(result.candidates.is_empty());
        } else {
            assert_suggestions(&query, &result);
        }
        prop_assert_eq!(&result, &generate_layouts(&query));
    }

    #[test]
    fn quantization_keeps_squares_square_and_does_not_invent_central_crossings(
        query in odd_measure_query()
    ) {
        let result = generate_layouts(&query);
        prop_assume!(result.status != LayoutGenerationStatus::NoCandidates);
        assert_suggestions(&query, &result);
    }

    #[test]
    fn double_sheets_offer_valid_suggestions_under_both_permissions(
        query in usual_double_sheet_query()
    ) {
        assert_suggestions(&query, &generate_layouts(&query));
    }

    #[test]
    fn common_frame_counts_are_not_left_to_the_reserve(
        query in repeated_orientation_query()
    ) {
        assert_suggestions(&query, &generate_layouts(&query));
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
    // Six verticals now fill the list with distinct structures alone.
    let query = double_sheet_query(
        width,
        height,
        &[FrameOrientation::Vertical; 8],
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

fn ratio(rect: &myalbuns_core::RectUm) -> f64 {
    rect.width as f64 / rect.height as f64
}

fn page_query(orientations: serde_json::Value, proportions: serde_json::Value) -> LayoutQuery {
    serde_json::from_value(serde_json::json!({
        "surface": {"type":"singlePage", "widthUm":300000, "heightUm":300000},
        "frameOrientations": orientations,
        "frameProportions": proportions,
        "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
        "minimumSideUm":20000
    }))
    .unwrap()
}

#[test]
fn frames_take_the_proportions_of_their_photos() {
    let single = |proportions: serde_json::Value| {
        let result = generate_layouts(&page_query(serde_json::json!(["vertical"]), proportions));
        ratio(&result.candidates[0].definition.positions[0])
    };
    // A phone portrait is 3:4, wider than the reference 2:3.
    assert!((single(serde_json::json!([{"width":3, "height":4}])) - 0.75).abs() < 0.002);
    assert!((single(serde_json::json!([])) - 2.0 / 3.0).abs() < 0.002);
    // A proportion of the other orientation does not describe this Frame.
    assert!((single(serde_json::json!([{"width":4, "height":3}])) - 2.0 / 3.0).abs() < 0.002);
    // A very tall Photo stays within the shapes a vertical Frame may take.
    assert!((0.45..0.92).contains(&single(serde_json::json!([{"width":1, "height":3}]))));

    // Side by side, each Frame keeps its own Photo's proportion.
    let pair = generate_layouts(&page_query(
        serde_json::json!(["vertical", "vertical"]),
        serde_json::json!([{"width":3, "height":4}, {"width":9, "height":16}]),
    ));
    assert!(pair.candidates.iter().any(|candidate| {
        let positions = &candidate.definition.positions;
        (ratio(&positions[0]) - 0.75).abs() < 0.005 && (ratio(&positions[1]) - 0.5625).abs() < 0.005
    }));
}

fn fills_a_page(rect: &myalbuns_core::RectUm, width: i64, height: i64) -> bool {
    rect.y == 0
        && rect.height == height
        && (rect.x == 0 || rect.x + rect.width == width)
        && (2 * rect.width - width).abs() <= 1
}

#[test]
fn a_whole_page_may_take_one_photo_to_the_edges_beside_the_others() {
    let query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface": {"type":"doubleSheet", "widthUm":600000, "heightUm":300000},
        "frameOrientations":["vertical", "vertical", "horizontal"],
        "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
        "minimumSideUm":20000
    }))
    .unwrap();
    let result = generate_layouts(&query);
    let whole: Vec<_> = result
        .candidates
        .iter()
        .filter(|c| {
            c.definition
                .positions
                .iter()
                .any(|r| fills_a_page(r, 600_000, 300_000))
        })
        .collect();
    assert!(!whole.is_empty());
    for candidate in whole {
        assert_eq!(candidate.definition.scope, LayoutScope::Page);
        let positions = &candidate.definition.positions;
        let page = positions
            .iter()
            .find(|r| fills_a_page(r, 600_000, 300_000))
            .unwrap();
        // The Page is square, whatever the orientation of the Frame it holds.
        assert_eq!(page.width, page.height);
        // The other Frames keep the Margin and stay on the facing Page.
        for other in positions.iter().filter(|r| *r != page) {
            assert!(other.y >= 15_000 && other.y + other.height <= 285_000);
            if page.x == 0 {
                assert!(other.x >= 300_000 + 15_000 && other.x + other.width <= 585_000);
            } else {
                assert!(other.x >= 15_000 && other.x + other.width <= 300_000 - 15_000);
            }
        }
    }

    let single: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface": {"type":"singlePage", "widthUm":300000, "heightUm":300000},
        "frameOrientations":["horizontal"],
        "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
        "minimumSideUm":20000
    }))
    .unwrap();
    assert!(generate_layouts(&single).candidates.iter().any(|c| {
        c.definition.positions[0]
            == myalbuns_core::RectUm {
                x: 0,
                y: 0,
                width: 300_000,
                height: 300_000,
            }
    }));
}

#[test]
fn automatic_arrangement_never_takes_a_whole_page_on_its_own() {
    // On 20 × 30 cm Pages a vertical Photo fills a whole Page without a crop,
    // so such a suggestion leads the list; automations still keep the Margin.
    let query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface": {"type":"doubleSheet", "widthUm":400000, "heightUm":300000},
        "frameOrientations":["vertical", "vertical", "vertical"],
        "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
        "minimumSideUm":20000
    }))
    .unwrap();
    let first = &generate_layouts(&query).candidates[0];
    assert!(
        first
            .definition
            .positions
            .iter()
            .any(|r| fills_a_page(r, 400_000, 300_000))
    );
    let ids: Vec<_> = (0..3).map(|_| uuid::Uuid::new_v4()).collect();
    let patch = myalbuns_core::LayoutRules::automatic(&query, Default::default(), &ids).unwrap();
    assert!(
        !patch
            .definition()
            .positions
            .iter()
            .any(|r| fills_a_page(r, 400_000, 300_000))
    );
}

#[test]
fn free_frames_are_vertical_in_some_suggestions_and_horizontal_in_others() {
    for (width, height) in [(600000, 300000), (400000, 300000), (300000, 300000)] {
        let kind = if width == height {
            "singlePage"
        } else {
            "doubleSheet"
        };
        let query: LayoutQuery = serde_json::from_value(serde_json::json!({
            "surface": {"type":kind, "widthUm":width, "heightUm":height},
            "frameOrientations":[null, null, "square", null, null],
            "permission":"pagesAndSheet", "marginUm":15000, "gapUm":5000,
            "minimumSideUm":20000
        }))
        .unwrap();
        let case = format!("{kind} {width}x{height}");
        let result = generate_layouts(&query);
        assert_eq!(result.status, LayoutGenerationStatus::Candidates, "{case}");
        let mut verticals = std::collections::BTreeSet::new();
        for candidate in &result.candidates {
            assert_generated_geometry(&query, &candidate.definition, &case);
            let positions = &candidate.definition.positions;
            verticals.insert(
                [0, 1, 3, 4]
                    .iter()
                    .filter(|&&i| positions[i].width < positions[i].height)
                    .count(),
            );
        }
        // The Frame with an orientation keeps it; the free ones are mixed.
        assert!(verticals.len() >= 3, "{case}: {verticals:?}");
    }
}

#[test]
fn blocks_cut_across_keep_every_frame_at_its_photo_proportion() {
    let proportions = serde_json::json!([
        {"width":3, "height":4}, {"width":2, "height":3}, {"width":3, "height":2},
        {"width":9, "height":16}, {"width":4, "height":3}
    ]);
    let query = page_query(
        serde_json::json!([
            "vertical",
            "vertical",
            "horizontal",
            "vertical",
            "horizontal"
        ]),
        proportions,
    );
    let targets = [0.75, 2.0 / 3.0, 1.5, 9.0 / 16.0, 4.0 / 3.0];
    let result = generate_layouts(&query);
    let blocks: Vec<_> = result
        .candidates
        .iter()
        .filter(|c| c.family == "Blocos encaixados")
        .collect();
    assert!(!blocks.is_empty());
    for candidate in blocks {
        assert_generated_geometry(&query, &candidate.definition, "blocks");
        for (rect, target) in candidate.definition.positions.iter().zip(targets) {
            assert!(
                (ratio(rect) / target - 1.0).abs() < 0.002,
                "{rect:?} {target}"
            );
        }
    }
}

#[test]
fn the_first_suggestion_keeps_a_small_group_in_harmony() {
    // Version 2 led these with a hero beside Frames under 5 cm and over 14
    // times smaller; a small group now keeps its sizes within six times and
    // every short side at 15% of the height.
    for orientations in [
        ["vertical", "vertical", "vertical", "horizontal"],
        ["vertical", "horizontal", "horizontal", "horizontal"],
    ] {
        let query = page_query(serde_json::json!(orientations), serde_json::json!([]));
        let first = &generate_layouts(&query).candidates[0].definition.positions;
        let areas: Vec<_> = first.iter().map(|r| r.width * r.height).collect();
        let (largest, smallest) = (areas.iter().max().unwrap(), areas.iter().min().unwrap());
        assert!(*largest <= 6 * smallest, "{orientations:?}: {first:?}");
        assert!(
            first.iter().all(|r| r.width.min(r.height) >= 45_000),
            "{orientations:?}: {first:?}"
        );
    }
}
