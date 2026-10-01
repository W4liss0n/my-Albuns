use std::collections::{BTreeSet, BinaryHeap};

use super::*;

mod families;
mod parallel;
mod quantization;
mod repetition;

const EPSILON: f64 = 1e-10;
const ORIENTATIONS: [FrameOrientation; 3] = [
    FrameOrientation::Vertical,
    FrameOrientation::Horizontal,
    FrameOrientation::Square,
];
/// Largest deduction for Page blocks that do not share a top and bottom line.
const HORIZON_PENALTY: f64 = 8.0;
/// Ceiling of one query; the list may end earlier when nothing distinct remains.
const MAXIMUM_SUGGESTIONS: usize = 20;
/// Work handed to each extra thread at least; below it, starting one costs more.
const DIVISIONS_PER_THREAD: usize = 4;
const CANDIDATES_PER_THREAD: usize = 64;

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
    /// Smallest distance to the suggestions already chosen in the same scope.
    novelty: f64,
    key: String,
    mirror_key: MirrorKey,
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
            novelty: 1.0,
            key: String::new(),
            mirror_key: MirrorKey::new(),
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
    /// Uniform grids are admitted only when nothing else fits.
    repetition_allowed: bool,
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

    /// Ranks a composition within its own region, such as one Page or group.
    fn score(&self, candidate: &Candidate) -> f64 {
        let usable = self.bounds().w * self.bounds().h;
        let occupation = (frame_area(&candidate.slots) / usable / 0.86).clamp(0.0, 1.0);
        self.score_with(candidate, occupation)
    }

    /// Ranks a complete suggestion against the fullest one of the same scope,
    /// so a Page layout or a lone Frame is not judged by the whole Sheet.
    fn final_score(&self, candidate: &Candidate, fullest: f64) -> f64 {
        let occupation = (frame_area(&candidate.slots) / fullest).clamp(0.0, 1.0);
        self.score_with(candidate, occupation)
    }

    fn score_with(&self, candidate: &Candidate, occupation: f64) -> f64 {
        let slots = &candidate.slots;
        if slots.is_empty() {
            return 0.0;
        }
        let area = frame_area(slots);
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
        let mut quality = 100.0
            * (0.45 * proportion
                + 0.4 * occupation
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
        if by_page && pages.iter().all(|page| !page.is_empty()) {
            let heights: Vec<f64> = pages
                .iter()
                .map(|page| {
                    let top = page
                        .iter()
                        .map(|s| s.bounds.y)
                        .fold(f64::INFINITY, f64::min);
                    let bottom = page
                        .iter()
                        .map(|s| s.bounds.y + s.bounds.h)
                        .fold(f64::NEG_INFINITY, f64::max);
                    bottom - top
                })
                .collect();
            let usable = self.height - 2.0 * self.margin;
            quality -= HORIZON_PENALTY.min(40.0 * (heights[0] - heights[1]).abs() / usable);
        }
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

fn frame_area(slots: &[Slot]) -> f64 {
    slots.iter().map(|s| s.bounds.w * s.bounds.h).sum()
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

/// Orientation and normalized rectangle of each Frame, in a stable order.
fn geometry_rows(slots: &[Slot], height: f64) -> Vec<(char, [i64; 4])> {
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
    rows
}

fn geometry_key(slots: &[Slot], height: f64) -> String {
    use std::fmt::Write;
    let mut key = String::with_capacity(slots.len() * 40);
    for (i, (o, r)) in geometry_rows(slots, height).iter().enumerate() {
        let separator = if i == 0 { "" } else { "|" };
        let _ = write!(key, "{separator}{o}:{}:{}:{}:{}", r[0], r[1], r[2], r[3]);
    }
    key
}

type MirrorKey = Vec<(u8, [i64; 4])>;

/// Identity shared by a composition and its mirrored forms: both axes and,
/// for Page layouts, each block flipped inside its Page or the Pages swapped.
/// Uses the resolved micrometres, doubled so that every reflection is exact.
fn mirror_key(candidate: &Candidate, surface: &LayoutSurface) -> MirrorKey {
    let (width, height) = (surface.width_um, surface.height_um);
    let by_page =
        surface.kind == LayoutSurfaceKind::DoubleSheet && candidate.scope == LayoutScope::Page;
    let mut key: Option<MirrorKey> = None;
    for vertical in [false, true] {
        for horizontal in [false, true] {
            for (left, right) in [(false, false), (true, false), (false, true), (true, true)] {
                if !by_page && (left || right) {
                    continue;
                }
                let mut rows: MirrorKey = candidate
                    .slots
                    .iter()
                    .zip(&candidate.positions)
                    .map(|(slot, r)| {
                        let (mut x, mut y, w, h) = (2 * r.x, 2 * r.y, 2 * r.width, 2 * r.height);
                        let on_left = x + w <= width;
                        if on_left && left {
                            x = width - x - w;
                        } else if !on_left && right {
                            x = 3 * width - x - w;
                        }
                        if horizontal {
                            x = 2 * width - x - w;
                        }
                        if vertical {
                            y = 2 * height - y - h;
                        }
                        (slot.orientation as u8, [x, y, w, h])
                    })
                    .collect();
                rows.sort_unstable();
                if key.as_ref().is_none_or(|key| rows < *key) {
                    key = Some(rows);
                }
            }
        }
    }
    key.unwrap_or_default()
}

fn distance(a: &Candidate, b: &Candidate) -> f64 {
    // A query has at most 30 Frames, so one bit per Frame of `b` marks the
    // ones already paired, in their original order.
    let mut paired = 0_u64;
    let mut overlap = 0.0;
    for slot in &a.slots {
        let mut best = None;
        let mut value = -1.0;
        for (i, other) in b.slots.iter().enumerate() {
            if paired & (1 << i) != 0 || slot.orientation != other.orientation {
                continue;
            }
            let a = slot.bounds;
            let b = other.bounds;
            let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            // Most pairs do not touch; their overlap is exactly zero.
            let iou = if w <= 0.0 || h <= 0.0 {
                0.0
            } else {
                let intersection = w * h;
                rounded(intersection / (a.w * a.h + b.w * b.h - intersection))
            };
            if iou > value {
                best = Some(i);
                value = iou;
            }
        }
        overlap += value.max(0.0);
        if let Some(index) = best {
            paired |= 1 << index;
        }
    }
    rounded(1.0 - overlap / a.slots.len() as f64)
}

/// A pool position ordered by its recorded utility, the earlier position first
/// on a tie, as the ranked pool would decide.
struct Ranked(f64, usize);

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Ranked {}

impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .total_cmp(&other.0)
            .then_with(|| other.1.cmp(&self.1))
    }
}

fn utility(candidate: &Candidate) -> f64 {
    rounded(0.85 * candidate.quality / 100.0 + 0.15 * candidate.novelty)
}

/// Chooses the suggestions from the ranked pool: each round takes the eligible
/// candidate of largest utility, the earlier one on a tie. Distinct structures
/// come first; mirrored forms only fill the remaining places.
///
/// Novelty only decreases as suggestions are chosen, so a recorded utility is
/// an upper bound. Candidates are examined from the largest recorded utility
/// and brought up to date only when they could still win, which gives the
/// same choices as updating every candidate after each round.
fn select(pool: Vec<Candidate>) -> Vec<Candidate> {
    let mut queue: BinaryHeap<_> = pool
        .iter()
        .enumerate()
        .map(|(position, c)| Ranked(utility(c), position))
        .collect();
    // Suggestions already accounted for in each candidate's novelty.
    let mut compared = vec![0; pool.len()];
    let mut pool: Vec<_> = pool.into_iter().map(Some).collect();
    let mut selected: Vec<Candidate> = Vec::new();
    for mirrors in [false, true] {
        let mut mirrored = Vec::new();
        while selected.len() < MAXIMUM_SUGGESTIONS {
            let Some(Ranked(recorded, position)) = queue.pop() else {
                break;
            };
            let c = pool[position].as_mut().unwrap();
            for chosen in &selected[compared[position]..] {
                if chosen.scope == c.scope {
                    c.novelty = c.novelty.min(distance(c, chosen));
                }
            }
            compared[position] = selected.len();
            let limit = if c.kind.starts_with("group-") { 4 } else { 2 };
            // Below the novelty floor or in a full family, it can never be chosen.
            if c.novelty < 0.25
                || (c.kind != "page"
                    && selected.iter().filter(|o| o.kind == c.kind).count() >= limit)
            {
                continue;
            }
            let current = utility(c);
            if current < recorded {
                queue.push(Ranked(current, position));
                continue;
            }
            if !mirrors
                && selected
                    .iter()
                    .any(|o| o.scope == c.scope && o.mirror_key == c.mirror_key)
            {
                mirrored.push(Ranked(current, position));
                continue;
            }
            selected.push(pool[position].take().unwrap());
        }
        queue.extend(mirrored);
    }
    selected
}

fn query_is_valid(query: &LayoutQuery) -> bool {
    query.surface.is_valid() && query.parameters.is_valid()
}

/// Every valid composition, scored and in ranking order.
fn candidates(frames: &[Slot], search: &Search<'_>) -> Vec<Candidate> {
    let query = search.query;
    let mut pool = if search.double() {
        families::pages(frames, search)
    } else {
        Vec::new()
    };
    if !search.double() || query.permission == LayoutPermission::PagesAndSheet {
        pool.extend(families::local(frames, search.bounds(), search));
        pool.extend(families::complementary_groups(
            frames,
            search.bounds(),
            search,
        ));
    }
    // Each composition is checked and converted on its own; duplicates are then
    // dropped in pool order, so the first of equal geometries stays.
    let accepted = parallel::map_in_order(&mut pool, CANDIDATES_PER_THREAD, |c| {
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
        true
    });
    let mut accepted = accepted.into_iter();
    let mut seen = BTreeSet::new();
    pool.retain(|c| accepted.next() == Some(true) && seen.insert(c.key.clone()));
    let fullest = |scope: LayoutScope| {
        pool.iter()
            .filter(|c| c.scope == scope)
            .map(|c| frame_area(&c.slots))
            .fold(0.0, f64::max)
    };
    let fullest = [fullest(LayoutScope::Page), fullest(LayoutScope::Sheet)];
    for c in &mut pool {
        c.quality = search.final_score(c, fullest[usize::from(c.scope == LayoutScope::Sheet)]);
    }
    pool.sort_by(|a, b| {
        b.quality
            .total_cmp(&a.quality)
            .then_with(|| a.key.cmp(&b.key))
    });
    pool
}

/// Pure, bounded generation. Positions always follow the caller's Frame order.
pub fn generate_layouts(query: &LayoutQuery) -> LayoutGeneration {
    let mut result = LayoutGeneration {
        algorithm_version: 2,
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
    let mut search = Search {
        query,
        height: rounded(query.surface.height_um as f64 / scale),
        margin: rounded(query.parameters.margin_um as f64 / scale),
        gap: rounded(query.parameters.gap_um as f64 / scale),
        minimum: rounded(query.parameters.minimum_side_um as f64 / scale),
        repetition_allowed: false,
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
    let mut pool = candidates(&frames, &search);
    if pool.is_empty() {
        // A uniform grid keeps orientations, margins and gaps, unlike the
        // arrangement of reserve that automations would apply otherwise.
        search.repetition_allowed = true;
        pool = candidates(&frames, &search);
        pool.truncate(1);
    }
    // Each scope keeps its own ten-point window: a Page layout and a panorama
    // across the fold are judged against their own kind, not against each other.
    let window = |scope: LayoutScope| {
        pool.iter()
            .filter(|c| c.scope == scope)
            .map(|c| c.quality - 10.0)
            .fold(72.0, f64::max)
    };
    let cutoffs = [window(LayoutScope::Page), window(LayoutScope::Sheet)];
    pool.retain(|c| c.quality >= cutoffs[usize::from(c.scope == LayoutScope::Sheet)]);
    parallel::map_in_order(&mut pool, CANDIDATES_PER_THREAD, |c| {
        c.mirror_key = mirror_key(c, &query.surface)
    });
    result.candidates = select(pool)
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
