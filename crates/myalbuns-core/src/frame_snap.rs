use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::frame_geometry::{dominant_resize_axis, resize_anchor_ratio, resize_delta_multiplier};
use crate::{ActiveSides, ComposedFrame, FrameGeometryGesture, ProjectRect};

mod spacing;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameSnapRequest {
    pub um_per_pixel_x: f64,
    pub um_per_pixel_y: f64,
    pub retained: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameSnapFeedback {
    pub retained: Vec<String>,
    pub guides: Vec<FrameSnapGuide>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameGeometryPreview {
    pub frames: Vec<ComposedFrame>,
    pub snap: FrameSnapFeedback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FrameSnapKind {
    Alignment,
    Dimension,
    ProjectGap,
    EqualGap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FrameSnapAxis {
    X,
    Y,
}

/// A line in active-surface coordinates. A measurement adds end ticks and a label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameSnapGuide {
    pub kind: FrameSnapKind,
    pub axis: FrameSnapAxis,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub measurement_um: Option<f64>,
}

pub(crate) struct SnapSurface<'a> {
    pub width: u64,
    pub height: u64,
    pub sides: ActiveSides,
    pub bleed: u64,
    pub safety: u64,
    pub gap: u64,
    pub others: &'a [ProjectRect],
}

#[derive(Clone)]
struct Candidate {
    id: String,
    axis: usize,
    kind: FrameSnapKind,
    // Position of the selected point in this axis, or a size for a dimension.
    value: f64,
    factor: f64,
    reference: ProjectRect,
    delta: f64,
    score: f64,
    spacing: Option<spacing::Spacing>,
}

impl Candidate {
    fn reference_position(&self) -> [f64; 2] {
        let mut position = [self.reference.x() as f64, self.reference.y() as f64];
        if self.kind == FrameSnapKind::Alignment {
            position[self.axis] = self.value;
        }
        position
    }
}

pub(crate) fn resolve(
    rects: &[ProjectRect],
    surface: &SnapSurface<'_>,
    gesture: &FrameGeometryGesture,
    request: &FrameSnapRequest,
) -> Result<(Vec<ProjectRect>, FrameSnapFeedback), crate::CoreError> {
    let units = [request.um_per_pixel_x, request.um_per_pixel_y];
    if units
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(crate::CoreError::InvalidFrameGeometrySelection);
    }
    let original = bounds(rects);
    let motion = Motion::new(original, gesture);
    let free = bounds(&crate::frame_geometry::edited_rects(
        rects,
        surface.width,
        surface.height,
        gesture,
    ));
    let mut candidates = Vec::new();
    let mut references = surface.others.to_vec();
    references.sort_by_key(|rect| (rect.x(), rect.y(), rect.width(), rect.height()));
    for axis in 0..2 {
        if !motion.controls(axis) {
            continue;
        }
        for (value, reference) in alignment_references(surface, &references, axis) {
            for factor in [0.0, 0.5, 1.0] {
                let coefficient = motion.coefficient(axis, factor);
                if coefficient == 0.0 {
                    continue;
                }
                let delta = (value - coordinate(original, axis, factor)) / coefficient;
                candidates.push(Candidate {
                    id: format!("a:{axis}:{factor}:{value}"),
                    axis,
                    kind: FrameSnapKind::Alignment,
                    value,
                    factor,
                    reference,
                    delta,
                    score: motion.distance(axis, delta, units),
                    spacing: None,
                });
            }
        }
        if matches!(gesture, FrameGeometryGesture::Resize { .. }) {
            let mut dimensions = Vec::new();
            for reference in &references {
                let value = if axis == 0 {
                    reference.width()
                } else {
                    reference.height()
                } as f64;
                let delta = (value - motion.size(axis)) / motion.multipliers[axis];
                dimensions.push(Candidate {
                    id: format!("d:{axis}:{value}"),
                    axis,
                    kind: FrameSnapKind::Dimension,
                    value,
                    factor: 1.0,
                    reference: *reference,
                    delta,
                    score: motion.distance(axis, delta, units),
                    spacing: None,
                });
            }
            dimensions.retain(|candidate| {
                let proposal = crate::frame_geometry::snapped_rects(
                    rects,
                    surface.width,
                    surface.height,
                    gesture,
                    motion.corrected(motion.delta, candidate.axis, candidate.delta),
                );
                reached(candidate, bounds(&proposal))
            });
            candidates.extend(canonical_dimensions(dimensions));
        }
        candidates.extend(spacing::candidates(
            surface,
            &references,
            &motion,
            free,
            axis,
            units,
        ));
    }
    candidates.retain(|candidate| {
        let tolerance = if request.retained.contains(&candidate.id) {
            10.0
        } else {
            6.0
        };
        candidate.score <= tolerance && candidate.delta.is_finite()
    });
    candidates.sort_by(|a, b| {
        let held_a = request.retained.contains(&a.id);
        let held_b = request.retained.contains(&b.id);
        let reference_a = a.reference_position();
        let reference_b = b.reference_position();
        held_b
            .cmp(&held_a)
            .then_with(|| a.score.total_cmp(&b.score))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| reference_a[0].total_cmp(&reference_b[0]))
            .then_with(|| reference_a[1].total_cmp(&reference_b[1]))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut delta = motion.delta;
    let mut selected = Vec::new();
    for candidate in &candidates {
        if selected
            .iter()
            .any(|other: &&Candidate| motion.proportional || other.axis == candidate.axis)
        {
            continue;
        }
        let next_delta = motion.corrected(delta, candidate.axis, candidate.delta);
        let proposal = crate::frame_geometry::snapped_rects(
            rects,
            surface.width,
            surface.height,
            gesture,
            next_delta,
        );
        if !reached(candidate, bounds(&proposal))
            || !spacing::eligible(candidate, bounds(&proposal), &references)
        {
            continue;
        }
        delta = next_delta;
        selected.push(candidate);
    }
    let mut result =
        crate::frame_geometry::snapped_rects(rects, surface.width, surface.height, gesture, delta);
    // An independent correction on the other axis can change the neighbor corridor.
    // Drop that spacing correction instead of painting a relation no longer reached.
    for _ in 0..2 {
        let final_bounds = bounds(&result);
        let previous_count = selected.len();
        selected.retain(|candidate| {
            reached(candidate, final_bounds)
                && spacing::eligible(candidate, final_bounds, &references)
        });
        if selected.len() == previous_count {
            break;
        }
        delta = motion.delta;
        for candidate in &selected {
            delta = motion.corrected(delta, candidate.axis, candidate.delta);
        }
        result = crate::frame_geometry::snapped_rects(
            rects,
            surface.width,
            surface.height,
            gesture,
            delta,
        );
    }
    let result_bounds = bounds(&result);
    let mut feedback = FrameSnapFeedback::default();
    // One reference per axis keeps repeated dimensions and coincident technical
    // lines from accumulating indicators. A proportional corner can show both
    // dimensions only if the same scale actually reached them.
    if motion.proportional
        && let Some(winner) = selected.first()
        && let Some(compatible) = candidates.iter().find(|candidate| {
            candidate.axis != winner.axis
                && reached(candidate, result_bounds)
                && spacing::eligible(candidate, result_bounds, &references)
        })
    {
        selected.push(compatible);
    }
    for candidate in selected {
        feedback.retained.push(candidate.id.clone());
        if let Some(spacing) = &candidate.spacing {
            for guide in spacing::guides(spacing, candidate.kind, result_bounds, candidate.axis) {
                if !feedback.guides.contains(&guide) {
                    feedback.guides.push(guide);
                }
            }
            continue;
        }
        if candidate.kind == FrameSnapKind::Dimension {
            for rect in [result_bounds, candidate.reference] {
                let guide = dimension_guide(rect, candidate.axis, candidate.value, units);
                if !feedback.guides.contains(&guide) {
                    feedback.guides.push(guide);
                }
            }
            continue;
        }
        let start = coordinate(candidate.reference, 1 - candidate.axis, 0.0).min(coordinate(
            result_bounds,
            1 - candidate.axis,
            0.0,
        ));
        let end = coordinate(candidate.reference, 1 - candidate.axis, 1.0).max(coordinate(
            result_bounds,
            1 - candidate.axis,
            1.0,
        ));
        let guide = if candidate.axis == 0 {
            FrameSnapGuide {
                kind: candidate.kind,
                axis: FrameSnapAxis::X,
                x1: candidate.value,
                y1: start,
                x2: candidate.value,
                y2: end,
                measurement_um: None,
            }
        } else {
            FrameSnapGuide {
                kind: candidate.kind,
                axis: FrameSnapAxis::Y,
                x1: start,
                y1: candidate.value,
                x2: end,
                y2: candidate.value,
                measurement_um: None,
            }
        };
        if !feedback.guides.contains(&guide) {
            feedback.guides.push(guide);
        }
    }
    Ok((result, feedback))
}

fn canonical_dimensions(mut dimensions: Vec<Candidate>) -> Vec<Candidate> {
    dimensions.sort_by(|a, b| a.value.total_cmp(&b.value));
    let mut remaining = dimensions.into_iter().peekable();
    let mut result = Vec::new();
    while let Some(mut reference) = remaining.next() {
        // Bound the whole group to one micrometer. Adjacent values must not
        // chain together and absorb a genuinely different dimension.
        let smallest = reference.value;
        while remaining
            .peek()
            .is_some_and(|candidate| candidate.value - smallest <= 1.0)
        {
            let candidate = remaining.next().unwrap();
            if candidate.reference_position() < reference.reference_position() {
                reference = candidate;
            }
        }
        result.push(reference);
    }
    result
}

fn reached(candidate: &Candidate, rect: ProjectRect) -> bool {
    if candidate.kind == FrameSnapKind::Dimension {
        let size = if candidate.axis == 0 {
            rect.width()
        } else {
            rect.height()
        } as f64;
        return size == candidate.value;
    }
    (coordinate(rect, candidate.axis, candidate.factor) - candidate.value).abs() <= 1.0
}

fn dimension_guide(rect: ProjectRect, axis: usize, value: f64, units: [f64; 2]) -> FrameSnapGuide {
    let inset = units[1 - axis] * 10.0;
    let offset = (coordinate(rect, 1 - axis, 0.0) - inset).max(inset);
    if axis == 0 {
        FrameSnapGuide {
            kind: FrameSnapKind::Dimension,
            axis: FrameSnapAxis::X,
            x1: rect.x() as f64,
            y1: offset,
            x2: (rect.x() + rect.width()) as f64,
            y2: offset,
            measurement_um: Some(value),
        }
    } else {
        FrameSnapGuide {
            kind: FrameSnapKind::Dimension,
            axis: FrameSnapAxis::Y,
            x1: offset,
            y1: rect.y() as f64,
            x2: offset,
            y2: (rect.y() + rect.height()) as f64,
            measurement_um: Some(value),
        }
    }
}

fn alignment_references(
    surface: &SnapSurface<'_>,
    others: &[ProjectRect],
    axis: usize,
) -> Vec<(f64, ProjectRect)> {
    let mut references = Vec::new();
    for rect in others {
        for factor in [0.0, 0.5, 1.0] {
            references.push((coordinate(*rect, axis, factor), *rect));
        }
    }
    let whole = ProjectRect::new(0, 0, surface.width, surface.height);
    let fractions: &[f64] = if axis == 0 && surface.sides == ActiveSides::Both {
        &[0.0, 0.25, 0.5, 0.75, 1.0]
    } else {
        &[0.0, 0.5, 1.0]
    };
    references.extend(
        fractions
            .iter()
            .map(|factor| (coordinate(whole, axis, *factor), whole)),
    );
    for (enabled, inset) in [
        (surface.bleed > 0, surface.bleed),
        (surface.safety > 0, surface.bleed + surface.safety),
    ] {
        if !enabled {
            continue;
        }
        let size = if axis == 0 {
            surface.width
        } else {
            surface.height
        };
        if axis == 1 || surface.sides != ActiveSides::Right {
            references.push((inset as f64, whole));
        }
        if axis == 1 || surface.sides != ActiveSides::Left {
            references.push((size.saturating_sub(inset).max(inset) as f64, whole));
        }
    }
    references
}

pub(crate) fn bounds(rects: &[ProjectRect]) -> ProjectRect {
    let x = rects
        .iter()
        .map(|rect| rect.x())
        .min()
        .expect("nonempty selection");
    let y = rects.iter().map(|rect| rect.y()).min().unwrap();
    let right = rects
        .iter()
        .map(|rect| rect.x() + rect.width())
        .max()
        .unwrap();
    let bottom = rects
        .iter()
        .map(|rect| rect.y() + rect.height())
        .max()
        .unwrap();
    ProjectRect::new(x, y, right - x, bottom - y)
}

fn coordinate(rect: ProjectRect, axis: usize, factor: f64) -> f64 {
    if axis == 0 {
        rect.x() as f64 + factor * rect.width() as f64
    } else {
        rect.y() as f64 + factor * rect.height() as f64
    }
}

struct Motion {
    original: ProjectRect,
    source: FrameGeometryGesture,
    delta: [f64; 2],
    multipliers: [f64; 2],
    anchors: [f64; 2],
    proportional: bool,
}

impl Motion {
    fn new(original: ProjectRect, source: &FrameGeometryGesture) -> Self {
        let (delta, multipliers, anchors, proportional) = match *source {
            FrameGeometryGesture::Move {
                delta_x_um,
                delta_y_um,
            } => (
                [delta_x_um as f64, delta_y_um as f64],
                [1.0; 2],
                [0.0; 2],
                false,
            ),
            FrameGeometryGesture::Resize {
                handle,
                delta_x_um,
                delta_y_um,
                preserve_aspect_ratio,
                from_center,
            } => {
                let (x, y) = handle.axes();
                (
                    [delta_x_um as f64, delta_y_um as f64],
                    [
                        resize_delta_multiplier(x, from_center),
                        resize_delta_multiplier(y, from_center),
                    ],
                    [
                        resize_anchor_ratio(x, from_center),
                        resize_anchor_ratio(y, from_center),
                    ],
                    preserve_aspect_ratio && x != 0 && y != 0,
                )
            }
        };
        Self {
            original,
            source: source.clone(),
            delta,
            multipliers,
            anchors,
            proportional,
        }
    }
    fn controls(&self, axis: usize) -> bool {
        self.multipliers[axis] != 0.0
    }
    fn coefficient(&self, axis: usize, factor: f64) -> f64 {
        if matches!(self.source, FrameGeometryGesture::Move { .. }) {
            1.0
        } else {
            (factor - self.anchors[axis]) * self.multipliers[axis]
        }
    }
    fn size(&self, axis: usize) -> f64 {
        if axis == 0 {
            self.original.width() as f64
        } else {
            self.original.height() as f64
        }
    }
    fn corrected(&self, mut delta: [f64; 2], axis: usize, value: f64) -> [f64; 2] {
        delta[axis] = value;
        if self.proportional {
            let other = 1 - axis;
            let growth = value * self.multipliers[axis] / self.size(axis);
            delta[other] = growth * self.size(other) / self.multipliers[other];
        }
        delta
    }
    fn distance(&self, axis: usize, value: f64, units: [f64; 2]) -> f64 {
        if !self.proportional {
            return (value - self.delta[axis]).abs() / units[axis];
        }
        let scales = [0, 1].map(|index| {
            (self.size(index) + self.delta[index] * self.multipliers[index]) / self.size(index)
        });
        let dominant = dominant_resize_axis(scales);
        let free = self.corrected(self.delta, dominant, self.delta[dominant]);
        let corrected = self.corrected(self.delta, axis, value);
        ((corrected[0] - free[0]) / units[0]).hypot((corrected[1] - free[1]) / units[1])
    }
}
