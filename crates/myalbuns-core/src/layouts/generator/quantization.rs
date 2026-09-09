use super::*;

pub(super) fn resolve(
    slots: &[Slot],
    scope: LayoutScope,
    query: &LayoutQuery,
) -> Option<Vec<RectUm>> {
    let scale = query.surface.width_um as f64;
    let positions: Vec<_> = slots
        .iter()
        .map(|s| {
            let r = s.bounds;
            let x = (r.x * scale).round() as i64;
            let y = (r.y * scale).round() as i64;
            let mut width = ((r.x + r.w) * scale).round() as i64 - x;
            let mut height = ((r.y + r.h) * scale).round() as i64 - y;
            // Rounding independent origins can put the two square extents one unit
            // apart. Shrink the farther edge within the declared 1 µm tolerance.
            if s.orientation == FrameOrientation::Square {
                width = width.min(height);
                height = width;
            }
            RectUm {
                x,
                y,
                width,
                height,
            }
        })
        .collect();
    if valid(&positions, scope, query) {
        Some(positions)
    } else {
        None
    }
}

fn valid(positions: &[RectUm], scope: LayoutScope, query: &LayoutQuery) -> bool {
    let p = &query.parameters;
    let w = query.surface.width_um;
    let h = query.surface.height_um;
    for (r, o) in positions.iter().zip(&query.frame_orientations) {
        if r.width <= 0
            || r.height <= 0
            || r.width.min(r.height) < p.minimum_side_um - 1
            || r.x < 0
            || r.y < 0
            || r.x + r.width > w
            || r.y + r.height > h
            || r.x < p.margin_um - 1
            || r.y < p.margin_um - 1
            || r.x + r.width > w - p.margin_um + 1
            || r.y + r.height > h - p.margin_um + 1
        {
            return false;
        }
        let preserves_orientation = match o {
            FrameOrientation::Vertical => r.width < r.height,
            FrameOrientation::Horizontal => r.width > r.height,
            FrameOrientation::Square => r.width == r.height,
        };
        if !preserves_orientation {
            return false;
        }
    }
    for (i, a) in positions.iter().enumerate() {
        for b in &positions[i + 1..] {
            let separation = (b.x - a.x - a.width)
                .max(a.x - b.x - b.width)
                .max(b.y - a.y - a.height)
                .max(a.y - b.y - b.height);
            if separation < 0 || separation < p.gap_um - 1 {
                return false;
            }
        }
    }
    if query.surface.kind != LayoutSurfaceKind::DoubleSheet {
        return scope == LayoutScope::Page;
    }
    let crossing = positions
        .iter()
        .any(|r| 2 * r.x < w && 2 * (r.x + r.width) > w);
    if crossing {
        return scope == LayoutScope::Sheet && query.permission == LayoutPermission::PagesAndSheet;
    }
    if scope != LayoutScope::Page {
        return false;
    }
    for side in [0, w] {
        let page: Vec<_> = positions
            .iter()
            .filter(|r| 2 * r.x >= side && 2 * (r.x + r.width) <= side + w)
            .collect();
        if page.is_empty() {
            if positions.len() > 1 {
                return false;
            }
            continue;
        }
        let left = page.iter().map(|r| r.x).min().unwrap();
        let right = page.iter().map(|r| r.x + r.width).max().unwrap();
        let top = page.iter().map(|r| r.y).min().unwrap();
        let bottom = page.iter().map(|r| r.y + r.height).max().unwrap();
        let inset = (2 * p.margin_um).max(p.gap_um);
        if 2 * left < side + inset - 2
            || 2 * right > side + w - inset + 2
            || (2 * (left + right) - 2 * side - w).abs() > 4
            || (top + bottom - h).abs() > 2
        {
            return false;
        }
    }
    true
}
