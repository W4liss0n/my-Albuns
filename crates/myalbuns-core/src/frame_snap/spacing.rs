use super::{
    Candidate, FrameSnapAxis, FrameSnapGuide, FrameSnapKind, Motion, ProjectRect, SnapSurface,
    coordinate,
};

#[derive(Clone)]
pub(super) enum Spacing {
    Gap {
        neighbor: ProjectRect,
        direction: i8,
        gap: f64,
        matched: Option<(ProjectRect, ProjectRect)>,
    },
    Balanced {
        left: ProjectRect,
        right: ProjectRect,
    },
}

pub(super) fn candidates(
    surface: &SnapSurface<'_>,
    references: &[ProjectRect],
    motion: &Motion,
    free: ProjectRect,
    axis: usize,
    units: [f64; 2],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let left = neighbor(free, references, axis, -1, units[axis] * 10.0);
    let right = neighbor(free, references, axis, 1, units[axis] * 10.0);
    let mut gaps = vec![(surface.gap as f64, None)];
    for first in references {
        if let Some(second) = neighbor(*first, references, axis, 1, 0.0) {
            gaps.push((
                coordinate(second, axis, 0.0) - coordinate(*first, axis, 1.0),
                Some((*first, second)),
            ));
        }
    }
    for (direction, adjacent) in [(-1, left), (1, right)] {
        let Some(adjacent) = adjacent else { continue };
        let factor = if direction < 0 { 0.0 } else { 1.0 };
        let coefficient = motion.coefficient(axis, factor);
        if coefficient == 0.0 {
            continue;
        }
        for (gap, matched) in &gaps {
            let value = coordinate(adjacent, axis, 1.0 - factor) - f64::from(direction) * gap;
            let delta = (value - coordinate(motion.original, axis, factor)) / coefficient;
            let kind = if matched.is_some() {
                FrameSnapKind::EqualGap
            } else {
                FrameSnapKind::ProjectGap
            };
            candidates.push(Candidate {
                id: format!(
                    "g:{axis}:{direction}:{kind:?}:{}:{}:{gap}",
                    adjacent.x(),
                    adjacent.y()
                ),
                axis,
                kind,
                value,
                factor,
                reference: adjacent,
                delta,
                score: motion.distance(axis, delta, units),
                spacing: Some(Spacing::Gap {
                    neighbor: adjacent,
                    direction,
                    gap: *gap,
                    matched: *matched,
                }),
            });
        }
    }
    if let (Some(left), Some(right)) = (left, right) {
        let coefficient = motion.coefficient(axis, 0.5);
        if coefficient != 0.0 {
            let value = (coordinate(left, axis, 1.0) + coordinate(right, axis, 0.0)) / 2.0;
            let delta = (value - coordinate(motion.original, axis, 0.5)) / coefficient;
            candidates.push(Candidate {
                id: format!(
                    "b:{axis}:{}:{}:{}:{}",
                    left.x(),
                    left.y(),
                    right.x(),
                    right.y()
                ),
                axis,
                kind: FrameSnapKind::EqualGap,
                value,
                factor: 0.5,
                reference: left,
                delta,
                score: motion.distance(axis, delta, units),
                spacing: Some(Spacing::Balanced { left, right }),
            });
        }
    }
    candidates
}

fn overlaps(a: ProjectRect, b: ProjectRect, axis: usize) -> bool {
    coordinate(a, 1 - axis, 0.0).max(coordinate(b, 1 - axis, 0.0))
        < coordinate(a, 1 - axis, 1.0).min(coordinate(b, 1 - axis, 1.0))
}

fn neighbor(
    rect: ProjectRect,
    references: &[ProjectRect],
    axis: usize,
    direction: i8,
    slack: f64,
) -> Option<ProjectRect> {
    let factor = if direction < 0 { 0.0 } else { 1.0 };
    references
        .iter()
        .filter(|other| **other != rect && overlaps(rect, **other, axis))
        .filter_map(|other| {
            let gap = f64::from(direction)
                * (coordinate(*other, axis, 1.0 - factor) - coordinate(rect, axis, factor));
            (gap >= -slack).then_some((gap, *other))
        })
        .min_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| (a.1.x(), a.1.y()).cmp(&(b.1.x(), b.1.y())))
        })
        .map(|(_, rect)| rect)
}

pub(super) fn eligible(
    candidate: &Candidate,
    rect: ProjectRect,
    references: &[ProjectRect],
) -> bool {
    match &candidate.spacing {
        None => true,
        Some(Spacing::Gap {
            neighbor: target,
            direction,
            gap,
            ..
        }) => {
            let factor = if *direction < 0 { 0.0 } else { 1.0 };
            let achieved = f64::from(*direction)
                * (coordinate(*target, candidate.axis, 1.0 - factor)
                    - coordinate(rect, candidate.axis, factor));
            achieved >= 0.0
                && (achieved - gap).abs() <= 1.0
                && neighbor(rect, references, candidate.axis, *direction, 0.0) == Some(*target)
        }
        Some(Spacing::Balanced { left, right }) => {
            let left_gap =
                coordinate(rect, candidate.axis, 0.0) - coordinate(*left, candidate.axis, 1.0);
            let right_gap =
                coordinate(*right, candidate.axis, 0.0) - coordinate(rect, candidate.axis, 1.0);
            left_gap >= 0.0
                && right_gap >= 0.0
                && (left_gap - right_gap).abs() <= 1.0
                && neighbor(rect, references, candidate.axis, -1, 0.0) == Some(*left)
                && neighbor(rect, references, candidate.axis, 1, 0.0) == Some(*right)
        }
    }
}

pub(super) fn guides(
    spacing: &Spacing,
    kind: FrameSnapKind,
    rect: ProjectRect,
    axis: usize,
) -> Vec<FrameSnapGuide> {
    match spacing {
        Spacing::Gap {
            neighbor,
            direction,
            matched,
            ..
        } => {
            let mut guides = vec![if *direction < 0 {
                gap_guide(*neighbor, rect, axis, kind)
            } else {
                gap_guide(rect, *neighbor, axis, kind)
            }];
            if let Some((left, right)) = matched {
                guides.push(gap_guide(*left, *right, axis, kind));
            }
            guides
        }
        Spacing::Balanced { left, right } => vec![
            gap_guide(*left, rect, axis, kind),
            gap_guide(rect, *right, axis, kind),
        ],
    }
}

fn gap_guide(
    left: ProjectRect,
    right: ProjectRect,
    axis: usize,
    kind: FrameSnapKind,
) -> FrameSnapGuide {
    let start = coordinate(left, axis, 1.0);
    let end = coordinate(right, axis, 0.0);
    let cross = (coordinate(left, 1 - axis, 0.0).max(coordinate(right, 1 - axis, 0.0))
        + coordinate(left, 1 - axis, 1.0).min(coordinate(right, 1 - axis, 1.0)))
        / 2.0;
    if axis == 0 {
        FrameSnapGuide {
            kind,
            axis: FrameSnapAxis::X,
            x1: start,
            y1: cross,
            x2: end,
            y2: cross,
            measurement_um: Some(end - start),
        }
    } else {
        FrameSnapGuide {
            kind,
            axis: FrameSnapAxis::Y,
            x1: cross,
            y1: start,
            x2: cross,
            y2: end,
            measurement_um: Some(end - start),
        }
    }
}
