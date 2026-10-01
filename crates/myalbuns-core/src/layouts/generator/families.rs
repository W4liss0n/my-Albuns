use super::*;

#[derive(Clone, Copy)]
enum Axis {
    Rows,
    Columns,
}

#[derive(Clone, Copy)]
struct Band {
    orientation: FrameOrientation,
    count: usize,
}

pub(super) fn grouped(frames: &[Slot]) -> [Vec<Slot>; 3] {
    ORIENTATIONS.map(|o| {
        frames
            .iter()
            .filter(|s| s.orientation == o)
            .copied()
            .collect()
    })
}

fn band_patterns(frames: &[Slot]) -> Vec<Vec<Band>> {
    let counts = grouped(frames).map(|group| group.len());
    let choices = counts.map(|n| {
        if n == 0 {
            vec![0]
        } else {
            (1..=4.min(n)).filter(|v| n % v == 0).collect()
        }
    });
    fn arrange(
        counts: [usize; 3],
        bands: [usize; 3],
        remaining: [usize; 3],
        prefix: &mut Vec<Band>,
        output: &mut Vec<Vec<Band>>,
    ) {
        if remaining.iter().sum::<usize>() == 0 {
            output.push(prefix.clone());
            return;
        }
        for i in 0..3 {
            if remaining[i] == 0 {
                continue;
            }
            let mut rest = remaining;
            rest[i] -= 1;
            prefix.push(Band {
                orientation: ORIENTATIONS[i],
                count: counts[i] / bands[i],
            });
            arrange(counts, bands, rest, prefix, output);
            prefix.pop();
        }
    }
    let mut result = Vec::new();
    for &v in &choices[0] {
        for &h in &choices[1] {
            for &q in &choices[2] {
                if v + h + q > 4 {
                    continue;
                }
                arrange(counts, [v, h, q], [v, h, q], &mut Vec::new(), &mut result);
            }
        }
    }
    result
}

/// The frames of each band, taken in order from their orientation groups.
fn band_members(groups: &[Vec<Slot>; 3], pattern: &[Band]) -> Vec<Vec<Slot>> {
    let mut used = [0; 3];
    pattern
        .iter()
        .map(|band| {
            let kind = ORIENTATIONS
                .iter()
                .position(|o| *o == band.orientation)
                .unwrap();
            let members = groups[kind][used[kind]..used[kind] + band.count].to_vec();
            used[kind] += band.count;
            members
        })
        .collect()
}

/// How far a band's frames reach along it per unit of its thickness: their
/// widths per unit of height in a row, their heights per unit of width in a column.
fn reach(members: &[Slot], axis: Axis) -> f64 {
    members
        .iter()
        .map(|s| {
            if matches!(axis, Axis::Rows) {
                s.ratio
            } else {
                1.0 / s.ratio
            }
        })
        .sum()
}

/// Bands at their target proportions, with the gap fixed: rows are
/// `a * width + b` tall and columns are `a * height + b` wide.
fn band_line(bands: &[Vec<Slot>], axis: Axis, gap: f64) -> (f64, f64) {
    let weights: Vec<_> = bands
        .iter()
        .map(|members| 1.0 / reach(members, axis))
        .collect();
    let a: f64 = weights.iter().sum();
    let b = gap
        * ((bands.len() - 1) as f64
            - bands
                .iter()
                .zip(&weights)
                .map(|(members, w)| (members.len() - 1) as f64 * w)
                .sum::<f64>());
    (a, b)
}

fn bands(members: &[Vec<Slot>], bounds: Bounds, axis: Axis, gap: f64, fit: bool) -> Vec<Slot> {
    let rows = matches!(axis, Axis::Rows);
    let mut region = bounds;
    if fit {
        let (a, b) = band_line(members, axis, gap);
        if rows {
            let w = bounds.w.min((bounds.h - b) / a);
            region = bounds.centered(w, a * w + b);
        } else {
            let h = bounds.h.min((bounds.w - b) / a);
            region = bounds.centered(a * h + b, h);
        }
    }
    let along = if rows { region.w } else { region.h };
    let weights: Vec<_> = members
        .iter()
        .map(|band| (along - (band.len() - 1) as f64 * gap) / reach(band, axis))
        .collect();
    let room = (if rows { region.h } else { region.w }) - (members.len() - 1) as f64 * gap;
    let total: f64 = weights.iter().sum();
    let mut position = if rows { region.y } else { region.x };
    let mut slots = Vec::new();
    for (band, weight) in members.iter().zip(weights) {
        let thickness = room * weight / total;
        // Each frame takes the share of the band's length its proportion asks for.
        let length = along - (band.len() - 1) as f64 * gap;
        let spread = reach(band, axis);
        let mut across = if rows { region.x } else { region.y };
        for frame in band {
            let extent = length * if rows { frame.ratio } else { 1.0 / frame.ratio } / spread;
            slots.push(Slot {
                bounds: Bounds {
                    x: if rows { across } else { position },
                    y: if rows { position } else { across },
                    w: if rows { extent } else { thickness },
                    h: if rows { thickness } else { extent },
                },
                ..*frame
            });
            across += extent + gap;
        }
        position += thickness + gap;
    }
    slots
}

fn grids(frames: &[Slot], bounds: Bounds, gap: f64, fit: bool) -> Vec<Vec<Slot>> {
    if frames
        .iter()
        .any(|s| s.orientation != frames[0].orientation)
    {
        return Vec::new();
    }
    let mut result = Vec::new();
    for rows in 1..=frames.len() {
        let columns = frames.len().div_ceil(rows);
        if rows * columns != frames.len() {
            continue;
        }
        let mut w = (bounds.w - (columns - 1) as f64 * gap) / columns as f64;
        let mut h = (bounds.h - (rows - 1) as f64 * gap) / rows as f64;
        if fit {
            // Equal cells take the average of the frames' proportions.
            let ratio = frames.iter().map(|s| s.ratio).sum::<f64>() / frames.len() as f64;
            h = h.min(w / ratio);
            w = h * ratio;
        }
        let region = bounds.centered(
            columns as f64 * w + (columns - 1) as f64 * gap,
            rows as f64 * h + (rows - 1) as f64 * gap,
        );
        result.push(
            frames
                .iter()
                .enumerate()
                .map(|(i, s)| Slot {
                    bounds: Bounds {
                        w,
                        h,
                        x: region.x + (i % columns) as f64 * (w + gap),
                        y: region.y + (i / columns) as f64 * (h + gap),
                    },
                    ..*s
                })
                .collect(),
        );
    }
    result
}

pub(super) fn local(frames: &[Slot], bounds: Bounds, search: &Search<'_>) -> Vec<Candidate> {
    if frames.is_empty() {
        return vec![Candidate::new(Vec::new(), "Página livre", "empty")];
    }
    let mut candidates = Vec::new();
    let mut add = |slots: Vec<Slot>, family: &str, kind: &str| {
        if search.valid_local(&slots, bounds)
            && (search.repetition_allowed || !repetition::repetitive(&slots))
        {
            candidates.push(Candidate::new(slots, family, kind));
        }
    };
    let groups = grouped(frames);
    let patterns: Vec<_> = band_patterns(frames)
        .iter()
        .map(|pattern| band_members(&groups, pattern))
        .collect();
    for fit in [false, true] {
        for slots in grids(frames, bounds, search.gap, fit) {
            let label = match frames.len() {
                1 => "Frame único",
                2 => "Dupla alinhada",
                3 => "Trio alinhado",
                _ => "Grade regular",
            };
            add(slots, label, "grid");
        }
        for members in &patterns {
            for axis in [Axis::Rows, Axis::Columns] {
                let (label, kind) = if matches!(axis, Axis::Rows) {
                    ("Faixas horizontais alinhadas", "rows")
                } else {
                    ("Colunas alinhadas", "columns")
                };
                add(bands(members, bounds, axis, search.gap, fit), label, kind);
            }
        }
    }
    if frames.len() >= 4 {
        for pattern in varied_patterns(frames) {
            for axis in [Axis::Rows, Axis::Columns] {
                let (label, kind) = if matches!(axis, Axis::Rows) {
                    ("Faixas com tamanhos graduados", "varied-rows")
                } else {
                    ("Colunas com tamanhos graduados", "varied-columns")
                };
                add(
                    varied_bands(&groups, bounds, &pattern, axis, search.gap),
                    label,
                    kind,
                );
            }
        }
    }
    if frames.len() > 1 {
        for orientation in ORIENTATIONS {
            let Some(hero) = frames.iter().find(|s| s.orientation == orientation) else {
                continue;
            };
            let support: Vec<_> = frames
                .iter()
                .filter(|s| s.index != hero.index)
                .copied()
                .collect();
            let support_groups = grouped(&support);
            let support_patterns: Vec<_> = band_patterns(&support)
                .iter()
                .map(|pattern| band_members(&support_groups, pattern))
                .collect();
            for side in ["left", "right", "top", "bottom"] {
                for share in [1.0 / 3.0, 0.4, 0.5, 0.6, 2.0 / 3.0] {
                    let (large, small) = split(bounds, search.gap, side, share);
                    let hero = Slot {
                        bounds: large,
                        ..*hero
                    };
                    if !search.valid_local(&[hero], bounds) {
                        continue;
                    }
                    let label = hero_label(side);
                    let mut accept = |mut slots: Vec<Slot>| {
                        let largest = slots
                            .iter()
                            .map(|s| s.bounds.w * s.bounds.h)
                            .fold(0.0, f64::max);
                        if !slots.is_empty() && hero.bounds.w * hero.bounds.h >= 1.5 * largest {
                            slots.insert(0, hero);
                            add(slots, label, &format!("hero-{side}"));
                        }
                    };
                    for slots in grids(&support, small, search.gap, false) {
                        accept(slots);
                    }
                    for members in &support_patterns {
                        for axis in [Axis::Rows, Axis::Columns] {
                            accept(bands(members, small, axis, search.gap, false));
                        }
                    }
                }
                // Besides the fixed shares, solve the hero and its support
                // together at their target proportions and centre the block.
                for members in &support_patterns {
                    for axis in [Axis::Rows, Axis::Columns] {
                        let Some((large, small)) =
                            natural_hero(bounds, search.gap, side, *hero, members, axis)
                        else {
                            continue;
                        };
                        let slots = bands(members, small, axis, search.gap, false);
                        let largest = slots
                            .iter()
                            .map(|s| s.bounds.w * s.bounds.h)
                            .fold(0.0, f64::max);
                        if !slots.is_empty() && large.w * large.h >= 1.5 * largest {
                            let mut all = vec![Slot {
                                bounds: large,
                                ..*hero
                            }];
                            all.extend(slots);
                            add(all, hero_label(side), &format!("hero-{side}"));
                        }
                    }
                }
            }
        }
    }
    if trees::FRAMES.contains(&frames.len()) {
        for (slots, stacked) in trees::compositions(frames, bounds, search) {
            let kind = if stacked { "tree-rows" } else { "tree-columns" };
            add(slots, "Blocos encaixados", kind);
        }
    }
    let mut seen = BTreeSet::new();
    // The key is kept: Page and group rankings reuse it to break ties.
    candidates.retain_mut(|c| {
        c.key = geometry_key(&c.slots, search.height);
        seen.insert(c.key.clone())
    });
    candidates
}

fn varied_patterns(frames: &[Slot]) -> Vec<Vec<[usize; 3]>> {
    let counts = grouped(frames).map(|group| group.len());
    let mut result = Vec::new();
    if frames.len() > 12 && counts.iter().filter(|n| **n > 0).count() == 1 {
        let orientation = counts.iter().position(|n| *n > 0).unwrap();
        fn partition(
            left: usize,
            orientation: usize,
            maximum: usize,
            minimum: usize,
            prefix: &mut Vec<[usize; 3]>,
            result: &mut Vec<Vec<[usize; 3]>>,
        ) {
            if left == 0 {
                if prefix.len() >= 2 {
                    result.push(prefix.clone());
                }
                return;
            }
            if prefix.len() == 4 || left < minimum || left > maximum * (4 - prefix.len()) {
                return;
            }
            for n in minimum..=maximum.min(left) {
                let mut band = [0; 3];
                band[orientation] = n;
                prefix.push(band);
                partition(left - n, orientation, maximum, n, prefix, result);
                prefix.pop();
            }
        }
        partition(
            frames.len(),
            orientation,
            frames.len().div_ceil(2),
            1,
            &mut Vec::new(),
            &mut result,
        );
        return result;
    }
    fn arrange(
        remaining: [usize; 3],
        last_count: usize,
        prefix: &mut Vec<[usize; 3]>,
        result: &mut Vec<Vec<[usize; 3]>>,
    ) {
        let left = remaining.iter().sum::<usize>();
        if left == 0 {
            if prefix.len() >= 2 {
                result.push(prefix.clone());
            }
            return;
        }
        if prefix.len() == 4 {
            return;
        }
        for v in 0..=3.min(remaining[0]) {
            for h in 0..=3.min(remaining[1]) {
                for q in 0..=3.min(remaining[2]) {
                    let n = v + h + q;
                    if n < last_count || n > 4 || n == 0 || prefix.len() == 3 && n != left {
                        continue;
                    }
                    prefix.push([v, h, q]);
                    arrange(
                        [remaining[0] - v, remaining[1] - h, remaining[2] - q],
                        n,
                        prefix,
                        result,
                    );
                    prefix.pop();
                }
            }
        }
    }
    arrange(counts, 1, &mut Vec::new(), &mut result);
    result
}

fn varied_bands(
    groups: &[Vec<Slot>; 3],
    bounds: Bounds,
    pattern: &[[usize; 3]],
    axis: Axis,
    gap: f64,
) -> Vec<Slot> {
    let mut used = [0; 3];
    let rows = matches!(axis, Axis::Rows);
    let bands: Vec<Vec<Slot>> = pattern
        .iter()
        .map(|counts| {
            let mut band = Vec::new();
            for i in 0..3 {
                for _ in 0..counts[i] {
                    band.push(groups[i][used[i]]);
                    used[i] += 1;
                }
            }
            band
        })
        .collect();
    let weights: Vec<f64> = bands
        .iter()
        .map(|band| {
            band.iter()
                .map(|s| if rows { s.ratio } else { 1.0 / s.ratio })
                .sum()
        })
        .collect();
    let natural: Vec<_> = bands
        .iter()
        .zip(&weights)
        .map(|(band, weight)| {
            ((if rows { bounds.w } else { bounds.h }) - (band.len() - 1) as f64 * gap) / weight
        })
        .collect();
    let factor = ((if rows { bounds.h } else { bounds.w }) - (bands.len() - 1) as f64 * gap)
        / natural.iter().sum::<f64>();
    let mut position = if rows { bounds.y } else { bounds.x };
    let mut slots = Vec::new();
    for (band, natural) in bands.iter().zip(natural) {
        let thickness = natural * factor;
        let mut across = if rows { bounds.x } else { bounds.y };
        for s in band {
            let extent = natural * if rows { s.ratio } else { 1.0 / s.ratio };
            slots.push(Slot {
                bounds: Bounds {
                    x: if rows { across } else { position },
                    y: if rows { position } else { across },
                    w: if rows { extent } else { thickness },
                    h: if rows { thickness } else { extent },
                },
                ..*s
            });
            across += extent + gap;
        }
        position += thickness + gap;
    }
    slots
}

pub(super) fn complementary_groups(
    frames: &[Slot],
    bounds: Bounds,
    search: &Search<'_>,
) -> Vec<Candidate> {
    if frames.len() < 8 {
        return Vec::new();
    }
    let groups = grouped(frames);
    let amounts = [3, frames.len() / 2, frames.len() - 3];
    let mut divisions = Vec::new();
    for axis in [Axis::Columns, Axis::Rows] {
        for share in [0.4, 0.5, 0.6] {
            for v in 0..=groups[0].len() {
                for h in 0..=groups[1].len() {
                    for q in 0..=groups[2].len() {
                        if amounts.contains(&(v + h + q)) {
                            divisions.push((axis, share, [v, h, q]));
                        }
                    }
                }
            }
        }
    }
    let select = |frames: &[Slot], region: Bounds| {
        let mut candidates = local(frames, region, search);
        candidates.retain(|c| {
            let block = bounding_box(&c.slots);
            (block.x - region.x).abs() < 1e-8
                && (block.y - region.y).abs() < 1e-8
                && (block.x + block.w - region.x - region.w).abs() < 1e-8
                && (block.y + block.h - region.y - region.h).abs() < 1e-8
        });
        for c in &mut candidates {
            c.scope = LayoutScope::Sheet;
            c.quality = search.score(&c.slots, c.scope);
        }
        candidates.sort_by(|a, b| {
            b.quality
                .total_cmp(&a.quality)
                .then_with(|| a.key.cmp(&b.key))
        });
        candidates.truncate(4);
        candidates
    };
    // Each division into two groups is searched independently.
    parallel::map_in_order(
        &mut divisions,
        DIVISIONS_PER_THREAD,
        |&mut (axis, share, counts)| {
            let columns = matches!(axis, Axis::Columns);
            let (first, second) = split(
                bounds,
                search.gap,
                if columns { "left" } else { "top" },
                share,
            );
            let a: Vec<_> = groups
                .iter()
                .zip(counts)
                .flat_map(|(g, n)| g[..n].iter().copied())
                .collect();
            let b: Vec<_> = groups
                .iter()
                .zip(counts)
                .flat_map(|(g, n)| g[n..].iter().copied())
                .collect();
            let first = select(&a, first);
            let second = select(&b, second);
            let mut pool = Vec::new();
            for ca in &first {
                for cb in &second {
                    let slots: Vec<_> = ca.slots.iter().chain(&cb.slots).copied().collect();
                    if search.repetition_allowed || !repetition::repetitive(&slots) {
                        pool.push(Candidate::new(
                            slots,
                            if columns {
                                "Grupos lado a lado com tamanhos variados"
                            } else {
                                "Grupos acima e abaixo com tamanhos variados"
                            },
                            if columns {
                                "group-columns"
                            } else {
                                "group-rows"
                            },
                        ));
                    }
                }
            }
            pool
        },
    )
    .into_iter()
    .flatten()
    .collect()
}

fn hero_label(side: &str) -> &'static str {
    match side {
        "left" => "Destaque à esquerda",
        "right" => "Destaque à direita",
        "top" => "Destaque acima",
        _ => "Destaque abaixo",
    }
}

/// A hero beside its support block, both at their target proportions and sharing
/// the dimension across `side`, as large as `bounds` allows and centred in it.
fn natural_hero(
    bounds: Bounds,
    gap: f64,
    side: &str,
    hero: Slot,
    support: &[Vec<Slot>],
    axis: Axis,
) -> Option<(Bounds, Bounds)> {
    let ratio = hero.ratio;
    let (a, b) = band_line(support, axis, gap);
    let across = side == "left" || side == "right";
    // Support extent along the split as `k * shared + c`.
    let (k, c) = if matches!(axis, Axis::Rows) == across {
        (1.0 / a, -b / a)
    } else {
        (a, b)
    };
    let (large, small, block) = if across {
        let h = bounds.h.min((bounds.w - gap - c) / (ratio + k));
        let (hero_w, support_w) = (ratio * h, k * h + c);
        let block = bounds.centered(hero_w + gap + support_w, h);
        let (hero_x, support_x) = if side == "left" {
            (block.x, block.x + hero_w + gap)
        } else {
            (block.x + support_w + gap, block.x)
        };
        (
            Bounds {
                x: hero_x,
                y: block.y,
                w: hero_w,
                h,
            },
            Bounds {
                x: support_x,
                y: block.y,
                w: support_w,
                h,
            },
            block,
        )
    } else {
        let w = bounds.w.min((bounds.h - gap - c) / (1.0 / ratio + k));
        let (hero_h, support_h) = (w / ratio, k * w + c);
        let block = bounds.centered(w, hero_h + gap + support_h);
        let (hero_y, support_y) = if side == "top" {
            (block.y, block.y + hero_h + gap)
        } else {
            (block.y + support_h + gap, block.y)
        };
        (
            Bounds {
                x: block.x,
                y: hero_y,
                w,
                h: hero_h,
            },
            Bounds {
                x: block.x,
                y: support_y,
                w,
                h: support_h,
            },
            block,
        )
    };
    (block.w > 0.0 && block.h > 0.0 && small.w > 0.0 && small.h > 0.0).then_some((large, small))
}

fn split(bounds: Bounds, gap: f64, side: &str, share: f64) -> (Bounds, Bounds) {
    if side == "left" || side == "right" {
        let w = (bounds.w - gap) * share;
        (
            Bounds {
                x: if side == "left" {
                    bounds.x
                } else {
                    bounds.x + bounds.w - w
                },
                w,
                ..bounds
            },
            Bounds {
                x: if side == "left" {
                    bounds.x + w + gap
                } else {
                    bounds.x
                },
                w: bounds.w - w - gap,
                ..bounds
            },
        )
    } else {
        let h = (bounds.h - gap) * share;
        (
            Bounds {
                y: if side == "top" {
                    bounds.y
                } else {
                    bounds.y + bounds.h - h
                },
                h,
                ..bounds
            },
            Bounds {
                y: if side == "top" {
                    bounds.y + h + gap
                } else {
                    bounds.y
                },
                h: bounds.h - h - gap,
                ..bounds
            },
        )
    }
}

pub(super) fn pages(frames: &[Slot], search: &Search<'_>) -> Vec<Candidate> {
    let inset = search.margin.max(search.gap / 2.0);
    let left = Bounds {
        x: inset,
        y: search.margin,
        w: 0.5 - 2.0 * inset,
        h: search.height - 2.0 * search.margin,
    };
    let right = Bounds {
        x: 0.5 + inset,
        ..left
    };
    let groups = grouped(frames);
    let mut divisions = Vec::new();
    for v in 0..=groups[0].len() {
        for h in 0..=groups[1].len() {
            for q in 0..=groups[2].len() {
                divisions.push([v, h, q]);
            }
        }
    }
    let select = |frames: &[Slot], region| {
        let mut candidates = local(frames, region, search);
        for c in &mut candidates {
            c.quality = search.score(&c.slots, c.scope);
        }
        candidates.sort_by(|a, b| {
            b.quality
                .total_cmp(&a.quality)
                .then_with(|| a.key.cmp(&b.key))
        });
        candidates.truncate(6);
        candidates
    };
    // Each division of the Frames between the Pages is searched independently.
    parallel::map_in_order(&mut divisions, DIVISIONS_PER_THREAD, |&mut counts| {
        let a: Vec<_> = groups
            .iter()
            .zip(counts)
            .flat_map(|(g, n)| g[..n].iter().copied())
            .collect();
        let b: Vec<_> = groups
            .iter()
            .zip(counts)
            .flat_map(|(g, n)| g[n..].iter().copied())
            .collect();
        let mut pool = Vec::new();
        if frames.len() > 1 && (a.is_empty() || b.is_empty()) {
            return pool;
        }
        let first = select(&a, left);
        let second = select(&b, right);
        for ca in &first {
            for cb in &second {
                let slots = ca.slots.iter().chain(&cb.slots).copied().collect();
                pool.push(Candidate::new(
                    slots,
                    &format!("Por Página · {} + {}", a.len(), b.len()),
                    "page",
                ));
            }
        }
        pool
    })
    .into_iter()
    .flatten()
    .collect()
}

/// How much a Frame's Photo is cropped by a `ratio` shape: 1 means no crop.
fn crop(frame: &Slot, ratio: f64) -> f64 {
    (frame.ratio / ratio).max(ratio / frame.ratio)
}

/// One Frame printed over a whole Page, edge to edge, beside a composition of
/// the other Frames on the facing Page. In each orientation, the Frame whose
/// Photo the Page crops least takes the Page.
pub(super) fn full_pages(frames: &[Slot], search: &Search<'_>) -> Vec<Candidate> {
    let page_ratio = 0.5 / search.height;
    let inset = search.margin.max(search.gap / 2.0);
    let mut pool = Vec::new();
    for orientation in ORIENTATIONS {
        let Some(chosen) = frames
            .iter()
            .filter(|s| s.orientation == orientation)
            .min_by(|a, b| crop(a, page_ratio).total_cmp(&crop(b, page_ratio)))
        else {
            continue;
        };
        let others: Vec<_> = frames
            .iter()
            .filter(|s| s.index != chosen.index)
            .copied()
            .collect();
        for side in [0.0, 0.5] {
            let page = Slot {
                full_page: true,
                bounds: Bounds {
                    x: side,
                    y: 0.0,
                    w: 0.5,
                    h: search.height,
                },
                ..*chosen
            };
            let facing = Bounds {
                x: 0.5 - side + inset,
                y: search.margin,
                w: 0.5 - 2.0 * inset,
                h: search.height - 2.0 * search.margin,
            };
            let mut compositions = local(&others, facing, search);
            for c in &mut compositions {
                c.quality = search.score(&c.slots, c.scope);
            }
            compositions.sort_by(|a, b| {
                b.quality
                    .total_cmp(&a.quality)
                    .then_with(|| a.key.cmp(&b.key))
            });
            compositions.truncate(6);
            for composition in compositions {
                let mut slots = vec![page];
                slots.extend(composition.slots);
                pool.push(Candidate::new(slots, "Página inteira", "full-page"));
            }
        }
    }
    pool
}

/// A single Frame over a single page's whole surface, edge to edge.
pub(super) fn full_surface(frames: &[Slot], search: &Search<'_>) -> Vec<Candidate> {
    let [only] = frames else {
        return Vec::new();
    };
    let slot = Slot {
        full_page: true,
        bounds: Bounds {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: search.height,
        },
        ..*only
    };
    vec![Candidate::new(vec![slot], "Página inteira", "full-page")]
}
