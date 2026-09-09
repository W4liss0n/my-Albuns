use myalbuns_core::{LayoutRules, LayoutScope, LayoutSurface, LayoutSurfaceKind};
use uuid::Uuid;

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
    let original = LayoutRules::list(&query, None);
    let last = StoredLayout {
        definition: original.candidates[0].layout.definition.clone(),
        origin: LayoutOrigin::Automatic,
    };
    query.frame_orientations[0] = FrameOrientation::Horizontal;
    query.surface.width_um *= 2;
    query.surface.height_um *= 2;
    let options = LayoutRules::list(&query, Some(&last));
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
