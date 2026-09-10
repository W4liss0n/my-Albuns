use myalbuns_core::{LayoutRules, LayoutScope, LayoutSurface, LayoutSurfaceKind};
use uuid::Uuid;

#[test]
fn custom_capture_infers_crossing_and_preserves_each_active_page() {
    use myalbuns_core::RectUm;
    let rect = |x, y, width, height| RectUm {
        x,
        y,
        width,
        height,
    };
    let double = LayoutSurface {
        kind: LayoutSurfaceKind::DoubleSheet,
        width_um: 600,
        height_um: 240,
    };
    for (input, expected) in [
        (vec![rect(320, 20, 60, 80)], vec![rect(320, 20, 60, 80)]),
        (
            vec![rect(320, 20, 60, 80), rect(10, 40, 100, 100)],
            vec![rect(320, 20, 60, 80), rect(10, 40, 100, 100)],
        ),
        (vec![rect(300, 0, 300, 240)], vec![rect(300, 0, 300, 240)]),
    ] {
        let captured = LayoutRules::capture_custom(double.clone(), input).unwrap();
        assert_eq!(captured.scope, LayoutScope::Page);
        assert_eq!(captured.positions, expected);
    }
    let crossing = vec![rect(280, 10, 50, 50), rect(10, 20, 50, 100)];
    let captured = LayoutRules::capture_custom(double.clone(), crossing.clone()).unwrap();
    assert_eq!(captured.scope, LayoutScope::Sheet);
    assert_eq!(captured.positions, crossing);
    assert!(LayoutRules::capture_custom(double.clone(), vec![]).is_err());
    assert!(LayoutRules::capture_custom(double, vec![rect(-1, 0, 20, 20)]).is_err());
    let single = LayoutSurface {
        kind: LayoutSurfaceKind::SinglePage,
        width_um: 300,
        height_um: 240,
    };
    let captured = LayoutRules::capture_custom(single, vec![rect(10, 20, 100, 80)]).unwrap();
    assert_eq!(captured.scope, LayoutScope::Page);
    assert_eq!(captured.positions, [rect(10, 20, 100, 80)]);
}

#[test]
fn custom_and_automatic_geometry_stay_in_their_own_sections_and_keep_priority() {
    use myalbuns_core::{
        CustomLayout, CustomLayoutId, LayoutOrigin, LayoutQuery, LayoutSources, StoredLayout,
    };
    let mut query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface":{"type":"singlePage","widthUm":210000,"heightUm":300000},
        "frameOrientations":["vertical"],"permission":"pagesOnly", "marginUm":15000,
        "gapUm":5000,"minimumSideUm":20000
    }))
    .unwrap();
    let generated = LayoutRules::list(&query, LayoutSources::default())
        .candidates
        .remove(0)
        .layout;
    let custom = [CustomLayout {
        id: CustomLayoutId::generate(),
        definition: generated.definition.clone(),
    }];
    let last = StoredLayout {
        origin: LayoutOrigin::Custom,
        definition: generated.definition.clone(),
    };
    let sources = LayoutSources {
        favorites: &[],
        last: Some(&last),
        custom: &custom,
    };
    let listing = LayoutRules::list(&query, sources);
    assert_eq!(
        listing
            .candidates
            .iter()
            .filter(|item| item.layout.origin == LayoutOrigin::Custom)
            .count(),
        1
    );
    assert_eq!(
        listing
            .candidates
            .iter()
            .filter(|item| item.layout.origin == LayoutOrigin::Automatic
                && LayoutRules::same_definition(&item.layout.definition, &last.definition))
            .count(),
        1
    );
    assert_eq!(listing.candidates[0].custom_id, Some(custom[0].id));
    let ids = [Uuid::new_v4()];
    assert_eq!(
        LayoutRules::automatic(
            &query,
            LayoutSources {
                favorites: &[],
                last: None,
                custom: &custom
            },
            &ids
        )
        .unwrap()
        .last_layout(),
        Some(&last)
    );
    query.surface.width_um *= 2;
    query.surface.height_um *= 2;
    assert_eq!(
        LayoutRules::list(&query, sources).candidates[0].custom_id,
        Some(custom[0].id)
    );
    query.surface.kind = LayoutSurfaceKind::DoubleSheet;
    assert!(
        LayoutRules::list(&query, sources)
            .candidates
            .iter()
            .all(|item| item.layout.origin != LayoutOrigin::Custom)
    );
}

#[test]
fn custom_page_layout_preserves_manual_placement_and_frame_order() {
    use myalbuns_core::RectUm;
    let surface = LayoutSurface {
        kind: LayoutSurfaceKind::DoubleSheet,
        width_um: 600_000,
        height_um: 240_000,
    };
    let positions = vec![
        RectUm {
            x: 10_000,
            y: 20_000,
            width: 60_000,
            height: 80_000,
        },
        RectUm {
            x: 90_000,
            y: 20_000,
            width: 90_000,
            height: 100_000,
        },
    ];
    let layout = LayoutRules::capture_custom(surface, positions.clone()).unwrap();
    assert_eq!(layout.scope, LayoutScope::Page);
    assert_eq!(layout.positions, positions);
}

#[test]
fn reserve_organizes_every_frame_without_a_catalog_or_generator() {
    for kind in [
        LayoutSurfaceKind::SinglePage,
        LayoutSurfaceKind::DoubleSheet,
    ] {
        for count in (0..=128).chain([255, 1024]) {
            let surface = LayoutSurface {
                kind,
                width_um: 600_000,
                height_um: 240_000,
            };
            let ids: Vec<_> = (0..count).map(|_| Uuid::new_v4()).collect();
            let patch = LayoutRules::reserve(&surface, &ids).unwrap();
            assert_eq!(patch.frame_ids(), ids);
            assert_eq!(patch.definition().positions.len(), count);
            assert_eq!(patch.definition().scope, LayoutScope::Page);
            assert!(patch.last_layout().is_none());
            for r in &patch.definition().positions {
                assert!(r.width > 0 && r.height > 0 && r.x >= 0 && r.y >= 0);
                assert!(r.x + r.width <= 600_000 && r.y + r.height <= 240_000);
                if kind == LayoutSurfaceKind::DoubleSheet {
                    assert!(r.x + r.width <= 300_000 || r.x >= 300_000);
                }
            }
        }
    }
}

#[test]
fn last_layout_keeps_its_original_geometry_and_priority_after_manual_frame_edits() {
    use myalbuns_core::{FrameOrientation, LayoutOrigin, LayoutQuery, StoredLayout};
    let mut query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface":{"type":"singlePage","widthUm":210000,"heightUm":300000},
        "frameOrientations":["vertical"],"permission":"pagesOnly", "marginUm":15000,
        "gapUm":5000,"minimumSideUm":20000
    }))
    .unwrap();
    let original = LayoutRules::list(&query, Default::default());
    let last = StoredLayout {
        definition: original.candidates[0].layout.definition.clone(),
        origin: LayoutOrigin::Automatic,
    };
    query.frame_orientations[0] = FrameOrientation::Horizontal;
    query.surface.width_um *= 2;
    query.surface.height_um *= 2;
    let options = LayoutRules::list(
        &query,
        myalbuns_core::LayoutSources {
            favorites: &[],
            last: Some(&last),
            ..Default::default()
        },
    );
    assert!(options.candidates[0].is_last_applied);
    assert_eq!(options.candidates[0].layout, last);
    let id = Uuid::new_v4();
    let patch = LayoutRules::resolve(&last, &query.surface, &[id], query.permission).unwrap();
    assert_eq!(patch.frame_ids(), [id]);
    let rect = &patch.definition().positions[0];
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (30_000, 30_000, 360_000, 540_000)
    );
    assert_eq!(patch.last_layout(), Some(&last));
}

#[test]
fn favorite_order_is_persistent_with_identity_tiebreak_and_last_keeps_precedence() {
    use myalbuns_core::*;
    let query: LayoutQuery = serde_json::from_value(serde_json::json!({
        "surface":{"type":"singlePage","widthUm":210000,"heightUm":300000},
        "frameOrientations":["vertical"],"permission":"pagesOnly", "marginUm":15000,
        "gapUm":5000,"minimumSideUm":20000
    }))
    .unwrap();
    let layout = |x, origin| StoredLayout {
        origin,
        definition: LayoutDefinition {
            surface: query.surface.clone(),
            scope: LayoutScope::Page,
            positions: vec![RectUm {
                x,
                y: 10000,
                width: 50000,
                height: 100000,
            }],
        },
    };
    let a = FavoriteLayout {
        id: serde_json::from_str("\"00000000-0000-4000-8000-000000000802\"").unwrap(),
        order: 5,
        layout: layout(10000, LayoutOrigin::Automatic),
    };
    let b = FavoriteLayout {
        id: serde_json::from_str("\"00000000-0000-4000-8000-000000000801\"").unwrap(),
        order: 5,
        layout: layout(20000, LayoutOrigin::Custom),
    };
    let c = FavoriteLayout {
        id: LayoutFavoriteId::generate(),
        order: 2,
        layout: layout(30000, LayoutOrigin::Automatic),
    };
    let favorites = [a.clone(), b.clone(), c.clone()];
    let custom = [CustomLayout {
        id: CustomLayoutId::generate(),
        definition: layout(40000, LayoutOrigin::Custom).definition,
    }];
    let last = a.layout.clone();
    let sources = LayoutSources {
        last: Some(&last),
        favorites: &favorites,
        custom: &custom,
    };
    let candidates = LayoutRules::list(&query, sources).candidates;
    assert_eq!(candidates[0].favorite_id, Some(a.id));
    assert!(candidates[0].is_last_applied);
    assert_eq!(candidates[1].favorite_id, Some(c.id));
    assert_eq!(candidates[2].favorite_id, Some(b.id));
    assert_eq!(candidates[3].custom_id, Some(custom[0].id));
    let ids = [Uuid::new_v4()];
    assert_eq!(
        LayoutRules::automatic(&query, sources, &ids)
            .unwrap()
            .last_layout(),
        Some(&last)
    );
    let sources = LayoutSources {
        last: None,
        ..sources
    };
    let candidates = LayoutRules::list(&query, sources).candidates;
    assert_eq!(
        candidates
            .iter()
            .take(3)
            .map(|item| item.favorite_id.unwrap())
            .collect::<Vec<_>>(),
        [c.id, b.id, a.id]
    );
    assert_eq!(
        LayoutRules::automatic(&query, sources, &ids)
            .unwrap()
            .last_layout(),
        Some(&c.layout)
    );
    let reversed = [c, b, a];
    assert_eq!(
        LayoutRules::list(
            &query,
            LayoutSources {
                favorites: &reversed,
                ..sources
            }
        )
        .candidates,
        candidates
    );
}
