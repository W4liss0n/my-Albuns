use std::collections::{BTreeSet, BinaryHeap};

use super::*;

mod families;
mod parallel;
mod quantization;
mod repetition;
mod trees;

const EPSILON: f64 = 1e-10;
const ORIENTATIONS: [FrameOrientation; 3] = [
    FrameOrientation::Vertical,
    FrameOrientation::Horizontal,
    FrameOrientation::Square,
];
/// Largest deduction for Page blocks that do not share a top and bottom line.
const HORIZON_PENALTY: f64 = 8.0;
/// Share of the surface height below which a Frame's short side reads poorly.
const READABLE_HEIGHT: f64 = 0.15;
/// Largest group in which more than three Frame sizes cost points.
const HARMONY_GROUP: usize = 6;
/// Ceiling of one query; the list may end earlier when nothing distinct remains.
const MAXIMUM_SUGGESTIONS: usize = 20;
/// Width over height a vertical Frame may take; horizontal ones take the inverse.
const VERTICAL_RATIOS: std::ops::RangeInclusive<f64> = 0.45..=0.92;
/// Work handed to each extra thread at least; below it, starting one costs more.
const DIVISIONS_PER_THREAD: usize = 4;
const CANDIDATES_PER_THREAD: usize = 64;
/// Most shares of verticals searched among the Frames left free.
const FREE_PROFILES: usize = 7;

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
    /// Width over height this Frame aims at: its Photo's, or the reference one.
    ratio: f64,
    /// Covers a whole Page, or a single page's whole surface: the Photo reaches
    /// the edges, past the Margin, whatever the Frame's orientation.
    full_page: bool,
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
    trees: trees::Solved,
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
            if slot.full_page {
                return [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite())
                    && r.w.min(r.h) >= self.minimum - EPSILON;
            }
            [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite())
                && r.w.min(r.h) >= self.minimum - EPSILON
                && r.x >= bounds.x - EPSILON
                && r.y >= bounds.y - EPSILON
                && r.x + r.w <= bounds.x + bounds.w + EPSILON
                && r.y + r.h <= bounds.y + bounds.h + EPSILON
                && match slot.orientation {
                    FrameOrientation::Vertical => VERTICAL_RATIOS.contains(&ratio),
                    FrameOrientation::Horizontal => (1.0 / VERTICAL_RATIOS.end()
                        ..=1.0 / VERTICAL_RATIOS.start())
                        .contains(&ratio),
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
            if let [only] = page.as_slice()
                && only.full_page
            {
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
    fn score(&self, slots: &[Slot], scope: LayoutScope) -> f64 {
        let usable = self.bounds().w * self.bounds().h;
        let occupation = (frame_area(slots) / usable / 0.86).clamp(0.0, 1.0);
        self.score_with(slots, scope, occupation)
    }

    /// Ranks a complete suggestion against the fullest one of the same scope,
    /// so a Page layout or a lone Frame is not judged by the whole Sheet.
    fn final_score(&self, candidate: &Candidate, fullest: f64) -> f64 {
        let occupation = (frame_area(&candidate.slots) / fullest).clamp(0.0, 1.0);
        self.score_with(&candidate.slots, candidate.scope, occupation)
    }

    fn score_with(&self, slots: &[Slot], scope: LayoutScope, occupation: f64) -> f64 {
        if slots.is_empty() {
            return 0.0;
        }
        let area = frame_area(slots);
        let fit = |s: &Slot| {
            let actual = s.bounds.w / s.bounds.h;
            (actual / s.ratio).min(s.ratio / actual)
        };
        // The crop of a whole Page's Photo shows as much as the Page is large.
        let proportion = if slots.iter().any(|s| s.full_page) {
            slots
                .iter()
                .map(|s| fit(s) * s.bounds.w * s.bounds.h)
                .sum::<f64>()
                / area
        } else {
            slots.iter().map(fit).sum::<f64>() / slots.len() as f64
        };
        let shortest = slots
            .iter()
            .map(|s| s.bounds.w.min(s.bounds.h))
            .fold(f64::INFINITY, f64::min);
        // A Frame reads well from twice the minimum side and 15% of the height.
        let readable = (2.0 * self.minimum).max(READABLE_HEIGHT * self.height);
        let mut quality = 100.0
            * (0.45 * proportion + 0.4 * occupation + 0.15 * (shortest / readable).clamp(0.0, 1.0));
        let by_page = self.double() && scope == LayoutScope::Page;
        // A whole Page beside a composition is meant to be uneven and has no
        // block of its own to align with the facing Page.
        let full_page = slots.iter().any(|s| s.full_page);
        if by_page && !full_page && slots.len() > 1 {
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
        if by_page && !full_page && pages.iter().all(|page| !page.is_empty()) {
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
            if page.len() < 3 {
                continue;
            }
            let areas: Vec<_> = page.iter().map(|s| s.bounds.w * s.bounds.h).collect();
            let largest = areas.iter().copied().fold(0.0, f64::max);
            let smallest = areas.iter().copied().fold(f64::INFINITY, f64::min);
            // In a small group, more than three sizes read as disorder; large
            // groups are graduated on purpose.
            if page.len() <= HARMONY_GROUP {
                let mut sides: Vec<_> = areas.iter().map(|a| a.sqrt()).collect();
                sides.sort_by(f64::total_cmp);
                let mut sizes = 0_usize;
                let mut last = f64::NEG_INFINITY;
                for side in sides {
                    if side > last * 1.04 {
                        sizes += 1;
                        last = side;
                    }
                }
                quality -= 4.0 * sizes.saturating_sub(3) as f64;
            }
            quality -= (10.0_f64).min((largest / smallest - 6.0).max(0.0) * 1.2);
            if page.len() >= 5 {
                quality -=
                    (12.0_f64).min((largest / areas.iter().sum::<f64>() - 0.4).max(0.0) * 35.0);
            }
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

/// The proportion Frame `index` aims at: its Photo's when the query gives one
/// of the same orientation, kept a little inside the shapes that orientation
/// allows, or the reference proportion.
fn target_ratio(query: &LayoutQuery, index: usize, orientation: FrameOrientation) -> f64 {
    let proportion = query.frame_proportions.get(index).copied().flatten();
    match proportion.filter(|p| p.orientation() == orientation) {
        Some(p) if orientation != FrameOrientation::Square => {
            let ratio = f64::from(p.width) / f64::from(p.height);
            let (low, high) = (VERTICAL_RATIOS.start() * 1.01, VERTICAL_RATIOS.end() / 1.01);
            if orientation == FrameOrientation::Vertical {
                ratio.clamp(low, high)
            } else {
                ratio.clamp(1.0 / high, 1.0 / low)
            }
        }
        _ => base_ratio(orientation),
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
            let orientation = match (s.full_page, s.orientation) {
                (true, _) => 'P',
                (_, FrameOrientation::Vertical) => 'V',
                (_, FrameOrientation::Horizontal) => 'H',
                (_, FrameOrientation::Square) => 'Q',
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

/// What a position is compared by: its Frame's orientation, or a whole Page,
/// whose shape is the Page's whatever the Frame's orientation.
fn shape(slot: &Slot) -> u8 {
    if slot.full_page {
        3
    } else {
        slot.orientation as u8
    }
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
                        (shape(slot), [x, y, w, h])
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
            if paired & (1 << i) != 0 || shape(slot) != shape(other) {
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

/// Compositions judged against each other for occupation and the window:
/// Page layouts, layouts across the fold, and a whole Page beside the others.
const CATEGORIES: usize = 3;

fn category(candidate: &Candidate) -> usize {
    if candidate.slots.iter().any(|s| s.full_page) {
        2
    } else {
        usize::from(candidate.scope == LayoutScope::Sheet)
    }
}

/// Every valid composition of these Frames, checked and placed in micrometres.
fn compositions(frames: &[Slot], search: &Search<'_>) -> Vec<Candidate> {
    let query = search.query;
    let mut pool = if search.double() {
        let mut pool = families::pages(frames, search);
        pool.extend(families::full_pages(frames, search));
        pool
    } else {
        families::full_surface(frames, search)
    };
    if !search.double() || query.permission == LayoutPermission::PagesAndSheet {
        pool.extend(families::local(frames, search.bounds(), search));
        pool.extend(families::complementary_groups(
            frames,
            search.bounds(),
            search,
        ));
    }
    // Each composition is checked and converted on its own.
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
    pool.retain(|_| accepted.next() == Some(true));
    pool
}

/// The compositions scored against the fullest of their category, in ranking
/// order. Of equal geometries, the first ranked stays.
fn ranked(mut pool: Vec<Candidate>, search: &Search<'_>) -> Vec<Candidate> {
    let mut fullest = [0.0_f64; CATEGORIES];
    for c in &pool {
        fullest[category(c)] = fullest[category(c)].max(frame_area(&c.slots));
    }
    for c in &mut pool {
        c.quality = search.final_score(c, fullest[category(c)]);
    }
    pool.sort_by(|a, b| {
        b.quality
            .total_cmp(&a.quality)
            .then_with(|| a.key.cmp(&b.key))
    });
    let mut seen = BTreeSet::new();
    pool.retain(|c| seen.insert(c.key.clone()));
    pool
}

/// The orientations searched: the query's own when every Frame has one;
/// otherwise up to `FREE_PROFILES` shares of verticals among the free Frames,
/// evenly spaced from none to all, the others horizontal.
fn profiles(query: &LayoutQuery) -> Vec<Vec<FrameOrientation>> {
    let free: Vec<_> = query
        .frame_orientations
        .iter()
        .enumerate()
        .filter_map(|(index, orientation)| orientation.is_none().then_some(index))
        .collect();
    let steps = free.len().min(FREE_PROFILES - 1);
    let shares: BTreeSet<_> = (0..=steps)
        .map(|k| (k * free.len() + steps / 2).checked_div(steps).unwrap_or(0))
        .collect();
    shares
        .into_iter()
        .map(|vertical| {
            let mut orientations: Vec<_> = query
                .frame_orientations
                .iter()
                .map(|orientation| orientation.unwrap_or(FrameOrientation::Horizontal))
                .collect();
            for &index in &free[..vertical] {
                orientations[index] = FrameOrientation::Vertical;
            }
            orientations
        })
        .collect()
}

/// Pure, bounded generation. Positions always follow the caller's Frame order.
pub fn generate_layouts(query: &LayoutQuery) -> LayoutGeneration {
    let mut result = LayoutGeneration {
        algorithm_version: 3,
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
        trees: Default::default(),
    };
    if search.bounds().w <= 0.0 || search.bounds().h <= 0.0 {
        return result;
    }
    let profiles = profiles(query);
    // Frames left free are searched in each profile; all compete in one ranking.
    let search_all = |search: &Search<'_>| {
        let mut pool = Vec::new();
        for orientations in &profiles {
            let frames: Vec<_> = orientations
                .iter()
                .enumerate()
                .map(|(index, &orientation)| Slot {
                    index,
                    orientation,
                    ratio: target_ratio(query, index, orientation),
                    full_page: false,
                    bounds: search.bounds(),
                })
                .collect();
            pool.extend(compositions(&frames, search));
        }
        ranked(pool, search)
    };
    let mut pool = search_all(&search);
    if pool.is_empty() {
        // A uniform grid keeps orientations, margins and gaps, unlike the
        // arrangement of reserve that automations would apply otherwise.
        search.repetition_allowed = true;
        pool = search_all(&search);
        pool.truncate(1);
    }
    // Each category keeps its own ten-point window: a Page layout, a panorama
    // across the fold and a whole Page are judged against their own kind.
    let mut cutoffs = [72.0_f64; CATEGORIES];
    for c in &pool {
        cutoffs[category(c)] = cutoffs[category(c)].max(c.quality - 10.0);
    }
    pool.retain(|c| c.quality >= cutoffs[category(c)]);
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
