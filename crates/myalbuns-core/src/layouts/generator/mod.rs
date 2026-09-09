use std::collections::BTreeSet;

use super::*;

mod families;
mod quantization;
mod repetition;

const EPSILON: f64 = 1e-10;
const ORIENTATIONS: [FrameOrientation; 3] = [
    FrameOrientation::Vertical,
    FrameOrientation::Horizontal,
    FrameOrientation::Square,
];

#[derive(Clone, Copy, Debug)]
struct Bounds {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Bounds {
    fn centered(self, w: f64, h: f64) -> Self {
        Self {
            x: self.x + (self.w - w) / 2.0,
            y: self.y + (self.h - h) / 2.0,
            w,
            h,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    index: usize,
    orientation: FrameOrientation,
    bounds: Bounds,
}

#[derive(Clone, Debug)]
struct Candidate {
    slots: Vec<Slot>,
    family: String,
    kind: String,
    scope: LayoutScope,
    quality: f64,
    key: String,
    positions: Vec<RectUm>,
}

impl Candidate {
    fn new(slots: Vec<Slot>, family: &str, kind: &str) -> Self {
        Self {
            slots,
            family: family.into(),
            kind: kind.into(),
            scope: LayoutScope::Page,
            quality: 0.0,
            key: String::new(),
            positions: Vec::new(),
        }
    }
}

struct Search<'a> {
    query: &'a LayoutQuery,
    height: f64,
    margin: f64,
    gap: f64,
    minimum: f64,
}

impl Search<'_> {
    fn bounds(&self) -> Bounds {
        Bounds {
            x: self.margin,
            y: self.margin,
            w: 1.0 - 2.0 * self.margin,
            h: self.height - 2.0 * self.margin,
        }
    }
    fn double(&self) -> bool {
        self.query.surface.kind == LayoutSurfaceKind::DoubleSheet
    }

    fn valid_local(&self, slots: &[Slot], bounds: Bounds) -> bool {
        slots.iter().all(|slot| {
            let r = slot.bounds;
            let ratio = r.w / r.h;
            [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite())
                && r.w.min(r.h) >= self.minimum - EPSILON
                && r.x >= bounds.x - EPSILON
                && r.y >= bounds.y - EPSILON
                && r.x + r.w <= bounds.x + bounds.w + EPSILON
                && r.y + r.h <= bounds.y + bounds.h + EPSILON
                && match slot.orientation {
                    FrameOrientation::Vertical => (0.45..=0.92).contains(&ratio),
                    FrameOrientation::Horizontal => (1.0 / 0.92..=1.0 / 0.45).contains(&ratio),
                    FrameOrientation::Square => (ratio - 1.0).abs() < 1e-8,
                }
        })
    }

    fn scope(&self, slots: &[Slot]) -> LayoutScope {
        if self.double()
            && slots
                .iter()
                .any(|s| s.bounds.x < 0.5 - EPSILON && s.bounds.x + s.bounds.w > 0.5 + EPSILON)
        {
            LayoutScope::Sheet
        } else {
            LayoutScope::Page
        }
    }

    fn valid_candidate(&self, candidate: &Candidate) -> bool {
        let slots = &candidate.slots;
        if slots.len() != self.query.frame_orientations.len()
            || !self.valid_local(slots, self.bounds())
        {
            return false;
        }
        for (i, a) in slots.iter().enumerate() {
            let a = a.bounds;
            for b in &slots[i + 1..] {
                let b = b.bounds;
                if a.x + a.w + self.gap > b.x + EPSILON
                    && b.x + b.w + self.gap > a.x + EPSILON
                    && a.y + a.h + self.gap > b.y + EPSILON
                    && b.y + b.h + self.gap > a.y + EPSILON
                {
                    return false;
                }
            }
        }
        if !self.double() {
            return true;
        }
        if candidate.scope == LayoutScope::Sheet {
            return self.query.permission == LayoutPermission::PagesAndSheet;
        }
        // A noncrossing whole-surface family must satisfy the Page contract too.
        let inset = self.margin.max(self.gap / 2.0);
        for side in [0.0, 0.5] {
            let page: Vec<_> = slots
                .iter()
                .filter(|s| {
                    s.bounds.x >= side - EPSILON && s.bounds.x + s.bounds.w <= side + 0.5 + EPSILON
                })
                .copied()
                .collect();
            if page.is_empty() {
                if slots.len() > 1 {
                    return false;
                }
                continue;
            }
            let bounds = Bounds {
                x: side + inset,
                y: self.margin,
                w: 0.5 - 2.0 * inset,
                h: self.height - 2.0 * self.margin,
            };
            let block = bounding_box(&page);
            if !self.valid_local(&page, bounds)
                || (block.x + block.w / 2.0 - side - 0.25).abs() > 1e-8
                || (block.y + block.h / 2.0 - self.height / 2.0).abs() > 1e-8
            {
                return false;
            }
        }
        true
    }

    fn score(&self, candidate: &Candidate) -> f64 {
        let slots = &candidate.slots;
        if slots.is_empty() {
            return 0.0;
        }
        let area: f64 = slots.iter().map(|s| s.bounds.w * s.bounds.h).sum();
        let proportion = slots
            .iter()
            .map(|s| {
                let actual = s.bounds.w / s.bounds.h;
                let ideal = base_ratio(s.orientation);
                (actual / ideal).min(ideal / actual)
            })
            .sum::<f64>()
            / slots.len() as f64;
        let shortest = slots
            .iter()
            .map(|s| s.bounds.w.min(s.bounds.h))
            .fold(f64::INFINITY, f64::min);
        let coverage = area / (self.bounds().w * self.bounds().h);
        let mut quality = 100.0
            * (0.45 * proportion
                + 0.4 * (coverage / 0.86).clamp(0.0, 1.0)
                + 0.15 * (shortest / (2.0 * self.minimum)).clamp(0.0, 1.0));
        let by_page = self.double() && candidate.scope == LayoutScope::Page;
        if by_page && slots.len() > 1 {
            let left: f64 = slots
                .iter()
                .filter(|s| s.bounds.x + s.bounds.w <= 0.5 + EPSILON)
                .map(|s| s.bounds.w * s.bounds.h)
                .sum();
            quality -= 6.0 * (2.0 * left / area - 1.0).abs();
        }
        let pages = if by_page {
            vec![
                slots
                    .iter()
                    .filter(|s| s.bounds.x + s.bounds.w <= 0.5 + EPSILON)
                    .collect::<Vec<_>>(),
                slots
                    .iter()
                    .filter(|s| s.bounds.x >= 0.5 - EPSILON)
                    .collect(),
            ]
        } else {
            vec![slots.iter().collect()]
        };
        for page in pages {
            if page.len() < 5 {
                continue;
            }
            let areas: Vec<_> = page.iter().map(|s| s.bounds.w * s.bounds.h).collect();
            let largest = areas.iter().copied().fold(0.0, f64::max);
            let smallest = areas.iter().copied().fold(f64::INFINITY, f64::min);
            quality -= (12.0_f64).min((largest / areas.iter().sum::<f64>() - 0.4).max(0.0) * 35.0)
                + (10.0_f64).min((largest / smallest - 6.0).max(0.0) * 1.2);
        }
        rounded(quality)
    }
}

fn rounded(value: f64) -> f64 {
    (value * 1e10).round() / 1e10
}

fn base_ratio(orientation: FrameOrientation) -> f64 {
    match orientation {
        FrameOrientation::Vertical => 2.0 / 3.0,
        FrameOrientation::Horizontal => 1.5,
        FrameOrientation::Square => 1.0,
    }
}

fn bounding_box(slots: &[Slot]) -> Bounds {
    let x = slots
        .iter()
        .map(|s| s.bounds.x)
        .fold(f64::INFINITY, f64::min);
    let y = slots
        .iter()
        .map(|s| s.bounds.y)
        .fold(f64::INFINITY, f64::min);
    let right = slots
        .iter()
        .map(|s| s.bounds.x + s.bounds.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = slots
        .iter()
        .map(|s| s.bounds.y + s.bounds.h)
        .fold(f64::NEG_INFINITY, f64::max);
    Bounds {
        x,
        y,
        w: right - x,
        h: bottom - y,
    }
}

fn geometry_key(slots: &[Slot], height: f64) -> String {
    let mut rows: Vec<_> = slots
        .iter()
        .map(|s| {
            let r = s.bounds;
            let orientation = match s.orientation {
                FrameOrientation::Vertical => 'V',
                FrameOrientation::Horizontal => 'H',
                FrameOrientation::Square => 'Q',
            };
            (
                orientation,
                [r.x, r.y / height, r.w, r.h / height].map(|v| (v * 1e6).round() as i64),
            )
        })
        .collect();
    rows.sort();
    rows.iter()
        .map(|(o, r)| format!("{o}:{}:{}:{}:{}", r[0], r[1], r[2], r[3]))
        .collect::<Vec<_>>()
        .join("|")
}

fn distance(a: &Candidate, b: &Candidate) -> f64 {
    let mut remaining = b.slots.clone();
    let mut overlap = 0.0;
    for slot in &a.slots {
        let mut best = None;
        let mut value = -1.0;
        for (i, other) in remaining.iter().enumerate() {
            if slot.orientation != other.orientation {
                continue;
            }
            let a = slot.bounds;
            let b = other.bounds;
            let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            let intersection = w.max(0.0) * h.max(0.0);
            let iou = rounded(intersection / (a.w * a.h + b.w * b.h - intersection));
            if iou > value {
                best = Some(i);
                value = iou;
            }
        }
        overlap += value.max(0.0);
        if let Some(index) = best {
            remaining.remove(index);
        }
    }
    rounded(1.0 - overlap / a.slots.len() as f64)
}

fn query_is_valid(query: &LayoutQuery) -> bool {
    query.surface.is_valid() && query.parameters.is_valid()
}

/// Pure, bounded generation. Positions always follow the caller's Frame order.
pub fn generate_layouts(query: &LayoutQuery) -> LayoutGeneration {
    let mut result = LayoutGeneration {
        algorithm_version: 1,
        status: LayoutGenerationStatus::NoCandidates,
        candidates: Vec::new(),
    };
    if !query_is_valid(query) {
        result.status = LayoutGenerationStatus::InvalidQuery;
        return result;
    }
    if query.frame_orientations.is_empty() {
        result.status = LayoutGenerationStatus::Empty;
        return result;
    }
    if query.frame_orientations.len() > 30 {
        result.status = LayoutGenerationStatus::OutsideCoverage;
        return result;
    }
    let scale = query.surface.width_um as f64;
    let search = Search {
        query,
        height: rounded(query.surface.height_um as f64 / scale),
        margin: rounded(query.parameters.margin_um as f64 / scale),
        gap: rounded(query.parameters.gap_um as f64 / scale),
        minimum: rounded(query.parameters.minimum_side_um as f64 / scale),
    };
    if search.bounds().w <= 0.0 || search.bounds().h <= 0.0 {
        return result;
    }
    let frames: Vec<_> = query
        .frame_orientations
        .iter()
        .enumerate()
        .map(|(index, orientation)| Slot {
            index,
            orientation: *orientation,
            bounds: search.bounds(),
        })
        .collect();
    let mut pool = if search.double() {
        families::pages(&frames, &search)
    } else {
        Vec::new()
    };
    if !search.double() || query.permission == LayoutPermission::PagesAndSheet {
        pool.extend(families::local(&frames, search.bounds(), &search));
        pool.extend(families::complementary_groups(
            &frames,
            search.bounds(),
            &search,
        ));
    }
    let mut seen = BTreeSet::new();
    pool.retain_mut(|c| {
        c.scope = search.scope(&c.slots);
        if !search.valid_candidate(c) {
            return false;
        }
        c.slots.sort_by_key(|s| s.index);
        let Some(positions) = quantization::resolve(&c.slots, c.scope, query) else {
            return false;
        };
        c.positions = positions;
        c.key = geometry_key(&c.slots, search.height);
        if !seen.insert(c.key.clone()) {
            return false;
        }
        c.quality = search.score(c);
        true
    });
    pool.sort_by(|a, b| {
        b.quality
            .total_cmp(&a.quality)
            .then_with(|| a.key.cmp(&b.key))
    });
    let Some(first) = pool.first() else {
        return result;
    };
    let cutoff = 72.0_f64.max(first.quality - 10.0);
    pool.retain(|c| c.quality >= cutoff);
    let mut selected: Vec<Candidate> = Vec::new();
    while !pool.is_empty() && selected.len() < 10 {
        let mut best = None;
        let mut utility = f64::NEG_INFINITY;
        for (i, c) in pool.iter().enumerate() {
            let novelty = selected
                .iter()
                .filter(|o| o.scope == c.scope)
                .map(|o| distance(c, o))
                .fold(1.0, f64::min);
            if novelty < 0.25 {
                continue;
            }
            let limit = if c.kind.starts_with("group-") { 4 } else { 2 };
            if c.kind != "page" && selected.iter().filter(|o| o.kind == c.kind).count() >= limit {
                continue;
            }
            let value = rounded(0.85 * c.quality / 100.0 + 0.15 * novelty);
            if value > utility {
                best = Some(i);
                utility = value;
            }
        }
        let Some(best) = best else {
            break;
        };
        selected.push(pool.remove(best));
    }
    result.candidates = selected
        .into_iter()
        .map(|c| GeneratedLayout {
            definition: LayoutDefinition {
                surface: query.surface.clone(),
                scope: c.scope,
                positions: c.positions,
            },
            family: c.family,
            quality: c.quality,
        })
        .collect();
    if !result.candidates.is_empty() {
        result.status = LayoutGenerationStatus::Candidates;
    }
    result
}
