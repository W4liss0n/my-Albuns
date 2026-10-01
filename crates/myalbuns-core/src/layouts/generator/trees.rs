//! Compositions cut out of a block by straight lines: the block is cut across
//! into parts, each part across the other way, up to three levels deep. Every
//! Frame keeps exactly the proportion it aims at and every gap is fixed, so
//! each part's width is linear in its height and the block is solved exactly.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::*;

/// Most levels of cuts, and most parts one cut makes.
const DEPTH: usize = 3;
const PARTS: usize = 4;
/// Frames a region may have; larger ones are left to bands, heroes and groups.
pub(super) const FRAMES: std::ops::RangeInclusive<usize> = 3..=6;
/// Compositions kept per region, the best scored first.
const KEPT: usize = 6;

/// A Frame of the orientation at this index of `ORIENTATIONS`, or parts cut
/// side by side or one above the other.
enum Block {
    Frame(usize),
    Cut {
        stacked: bool,
        parts: Vec<Rc<Block>>,
    },
}

type Shapes = HashMap<([usize; 3], bool, usize), Rc<Vec<Rc<Block>>>>;

/// Ordered ways to share `counts` among `parts` nonempty parts.
fn divisions(counts: [usize; 3], parts: usize) -> Vec<Vec<[usize; 3]>> {
    if parts == 1 {
        return vec![vec![counts]];
    }
    let mut result = Vec::new();
    for v in 0..=counts[0] {
        for h in 0..=counts[1] {
            for q in 0..=counts[2] {
                let rest = [counts[0] - v, counts[1] - h, counts[2] - q];
                if v + h + q == 0 || rest.iter().sum::<usize>() < parts - 1 {
                    continue;
                }
                for mut tail in divisions(rest, parts - 1) {
                    tail.insert(0, [v, h, q]);
                    result.push(tail);
                }
            }
        }
    }
    result
}

/// Every block of these Frames whose first cut is `stacked`, at most `depth`
/// levels deep. A part of one Frame is that Frame; a larger part is cut the
/// other way, since a cut in the same direction would only add parts.
fn shapes(
    counts: [usize; 3],
    stacked: bool,
    depth: usize,
    memo: &mut Shapes,
) -> Rc<Vec<Rc<Block>>> {
    if let Some(found) = memo.get(&(counts, stacked, depth)) {
        return found.clone();
    }
    let total: usize = counts.iter().sum();
    let mut result = Vec::new();
    if total == 1 {
        let kind = counts.iter().position(|&n| n == 1).unwrap();
        result.push(Rc::new(Block::Frame(kind)));
    } else if depth > 0 {
        for parts in 2..=PARTS.min(total) {
            for division in divisions(counts, parts) {
                let options: Vec<_> = division
                    .iter()
                    .map(|&part| shapes(part, !stacked, depth - 1, memo))
                    .collect();
                if options.iter().any(|o| o.is_empty()) {
                    continue;
                }
                let mut chosen = vec![0; options.len()];
                'product: loop {
                    result.push(Rc::new(Block::Cut {
                        stacked,
                        parts: options
                            .iter()
                            .zip(&chosen)
                            .map(|(o, &i)| o[i].clone())
                            .collect(),
                    }));
                    let mut k = options.len();
                    loop {
                        if k == 0 {
                            break 'product;
                        }
                        k -= 1;
                        chosen[k] += 1;
                        if chosen[k] < options[k].len() {
                            break;
                        }
                        chosen[k] = 0;
                    }
                }
            }
        }
    }
    let result = Rc::new(result);
    memo.insert((counts, stacked, depth), result.clone());
    result
}

/// Each block's width as `a * height + b`, in the order the block lists its
/// parts, with the Frames taken in that order from their orientation groups.
struct Solver<'a> {
    groups: &'a [Vec<Slot>; 3],
    gap: f64,
    used: [usize; 3],
    lines: Vec<(f64, f64)>,
    /// Orientation group and place in it of each Frame, in order.
    frames: Vec<(usize, usize)>,
}

impl Solver<'_> {
    fn measure(&mut self, block: &Block) -> (f64, f64) {
        let index = self.lines.len();
        self.lines.push((0.0, 0.0));
        let line = match block {
            Block::Frame(kind) => {
                let member = self.used[*kind];
                self.used[*kind] += 1;
                self.frames.push((*kind, member));
                (self.groups[*kind][member].ratio, 0.0)
            }
            // Side by side, the parts share a height and their widths add up.
            Block::Cut {
                stacked: false,
                parts,
            } => {
                let mut line = (0.0, self.gap * (parts.len() - 1) as f64);
                for part in parts {
                    let (a, b) = self.measure(part);
                    line = (line.0 + a, line.1 + b);
                }
                line
            }
            // Stacked, the parts share a width and their heights add up.
            Block::Cut {
                stacked: true,
                parts,
            } => {
                let (mut inverse, mut offset) = (0.0, -self.gap * (parts.len() - 1) as f64);
                for part in parts {
                    let (a, b) = self.measure(part);
                    inverse += 1.0 / a;
                    offset += b / a;
                }
                (1.0 / inverse, offset / inverse)
            }
        };
        self.lines[index] = line;
        line
    }

    fn place(&self, block: &Block, bounds: Bounds, node: &mut usize, slots: &mut Vec<Slot>) {
        *node += 1;
        match block {
            Block::Frame(_) => {
                let (kind, member) = self.frames[slots.len()];
                slots.push(Slot {
                    bounds,
                    ..self.groups[kind][member]
                });
            }
            Block::Cut { stacked, parts } => {
                let mut position = if *stacked { bounds.y } else { bounds.x };
                for part in parts {
                    let (a, b) = self.lines[*node];
                    let part_bounds = if *stacked {
                        Bounds {
                            y: position,
                            h: (bounds.w - b) / a,
                            ..bounds
                        }
                    } else {
                        Bounds {
                            x: position,
                            w: a * bounds.h + b,
                            ..bounds
                        }
                    };
                    position += self.gap
                        + if *stacked {
                            part_bounds.h
                        } else {
                            part_bounds.w
                        };
                    self.place(part, part_bounds, node, slots);
                }
            }
        }
    }
}

/// A composition of a region, from its corner: each Frame's orientation group,
/// place in that group and rectangle, and whether the first cut is stacked.
pub(super) struct Solution {
    frames: Vec<(usize, usize, Bounds)>,
    stacked: bool,
}

/// Frame counts and proportions in group order, region size and whether
/// uniform grids are admitted: all that a region's compositions depend on.
type Key = ([usize; 3], Vec<u64>, [u64; 2], bool);

/// Compositions already solved during one search: Pages and groups of the
/// same size and Frames recur across divisions and orientation profiles.
#[derive(Default)]
pub(super) struct Solved(Mutex<HashMap<Key, Arc<Vec<Solution>>>>);

/// The best scored compositions of these Frames cut out of a block as large
/// as `bounds` allows and centred in it, each with whether its first cut is
/// stacked.
pub(super) fn compositions(
    frames: &[Slot],
    bounds: Bounds,
    search: &Search<'_>,
) -> Vec<(Vec<Slot>, bool)> {
    let groups = families::grouped(frames);
    let key = (
        groups.each_ref().map(Vec::len),
        groups.iter().flatten().map(|s| s.ratio.to_bits()).collect(),
        [bounds.w.to_bits(), bounds.h.to_bits()],
        search.repetition_allowed,
    );
    let found = search.trees.0.lock().unwrap().get(&key).cloned();
    let solutions = found.unwrap_or_else(|| {
        let solutions = Arc::new(solve(&groups, bounds, search));
        search
            .trees
            .0
            .lock()
            .unwrap()
            .insert(key, solutions.clone());
        solutions
    });
    solutions
        .iter()
        .map(|solution| {
            let slots = solution
                .frames
                .iter()
                .map(|&(kind, member, b)| Slot {
                    bounds: Bounds {
                        x: bounds.x + b.x,
                        y: bounds.y + b.y,
                        ..b
                    },
                    ..groups[kind][member]
                })
                .collect();
            (slots, solution.stacked)
        })
        .collect()
}

/// Solves every block in a region of the size of `bounds` placed at the
/// origin, so that the result holds wherever a region of that size lies.
fn solve(groups: &[Vec<Slot>; 3], bounds: Bounds, search: &Search<'_>) -> Vec<Solution> {
    let region = Bounds {
        x: 0.0,
        y: 0.0,
        ..bounds
    };
    let total = groups.iter().map(Vec::len).sum();
    let mut memo = Shapes::new();
    let mut solver = Solver {
        groups,
        gap: search.gap,
        used: [0; 3],
        lines: Vec::with_capacity(2 * total),
        frames: Vec::with_capacity(total),
    };
    let mut slots = Vec::with_capacity(total);
    let mut scored = Vec::new();
    for stacked in [true, false] {
        let counts = groups.each_ref().map(Vec::len);
        for block in shapes(counts, stacked, DEPTH, &mut memo).iter() {
            solver.used = [0; 3];
            solver.lines.clear();
            solver.frames.clear();
            let (a, b) = solver.measure(block);
            let h = region.h.min((region.w - b) / a);
            if !(h > 0.0 && a * h + b > 0.0) {
                continue;
            }
            slots.clear();
            solver.place(block, region.centered(a * h + b, h), &mut 0, &mut slots);
            if !search.valid_local(&slots, region) {
                continue;
            }
            // The region is judged as one group, wherever it lies.
            let quality = search.score(&slots, LayoutScope::Sheet);
            scored.push((quality, slots.clone(), stacked));
        }
    }
    // Ties keep the order of enumeration.
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .filter(|(_, slots, _)| search.repetition_allowed || !repetition::repetitive(slots))
        .take(KEPT)
        .map(|(_, slots, stacked)| Solution {
            frames: slots
                .iter()
                .map(|slot| {
                    let kind = ORIENTATIONS
                        .iter()
                        .position(|o| *o == slot.orientation)
                        .unwrap();
                    let member = groups[kind]
                        .iter()
                        .position(|s| s.index == slot.index)
                        .unwrap();
                    (kind, member, slot.bounds)
                })
                .collect(),
            stacked,
        })
        .collect()
}
