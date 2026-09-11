use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ProjectRect, RectUm};

/// One gesture, expressed from the geometry that was visible at pointer-down.
/// Its baseline prevents a delayed gesture from overwriting another geometry edit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameGeometryEdit {
    pub frames: Vec<FrameGeometryTarget>,
    pub gesture: FrameGeometryGesture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub snap: Option<crate::FrameSnapRequest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameGeometryTarget {
    pub frame_id: String,
    pub expected_rect: RectUm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(tag = "kind")]
pub enum FrameGeometryGesture {
    Move {
        delta_x_um: i64,
        delta_y_um: i64,
    },
    Resize {
        handle: FrameResizeHandle,
        delta_x_um: i64,
        delta_y_um: i64,
        preserve_aspect_ratio: bool,
        from_center: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FrameResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl FrameResizeHandle {
    pub(crate) fn axes(self) -> (i8, i8) {
        match self {
            Self::TopLeft => (-1, -1),
            Self::Top => (0, -1),
            Self::TopRight => (1, -1),
            Self::Right => (1, 0),
            Self::BottomRight => (1, 1),
            Self::Bottom => (0, 1),
            Self::BottomLeft => (-1, 1),
            Self::Left => (-1, 0),
        }
    }
}

// Interactive resize lower bound, calibrated with the eight screen-sized handles.
// Legacy smaller Frames can retain their size and grow, without shrinking further.
const MIN_FRAME_EDGE_UM: u64 = 12_000;

pub(crate) fn edited_rects(
    rects: &[ProjectRect],
    surface_width: u64,
    surface_height: u64,
    gesture: &FrameGeometryGesture,
) -> Vec<ProjectRect> {
    transform_rects(rects, surface_width, surface_height, gesture, None)
}

/// A snap can require a fractional pointer correction (centers and proportional
/// corners). Quantize the resulting physical rectangles once, at the same owner
/// as ordinary resize, instead of rounding each pointer axis before scaling.
pub(crate) fn snapped_rects(
    rects: &[ProjectRect],
    surface_width: u64,
    surface_height: u64,
    gesture: &FrameGeometryGesture,
    delta: [f64; 2],
) -> Vec<ProjectRect> {
    transform_rects(rects, surface_width, surface_height, gesture, Some(delta))
}

fn transform_rects(
    rects: &[ProjectRect],
    surface_width: u64,
    surface_height: u64,
    gesture: &FrameGeometryGesture,
    delta: Option<[f64; 2]>,
) -> Vec<ProjectRect> {
    let x = rects
        .iter()
        .map(|rect| rect.x())
        .min()
        .expect("nonempty selection");
    let y = rects
        .iter()
        .map(|rect| rect.y())
        .min()
        .expect("nonempty selection");
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
    let bounds = ProjectRect::new(x, y, right - x, bottom - y);
    let minimum_width = minimum_group_axis(bounds.width(), rects.iter().map(|rect| rect.width()));
    let minimum_height =
        minimum_group_axis(bounds.height(), rects.iter().map(|rect| rect.height()));
    let resized = edited_rect(
        bounds,
        surface_width,
        surface_height,
        minimum_width,
        minimum_height,
        gesture,
        delta,
    );
    rects
        .iter()
        .map(|rect| {
            let left = scaled_offset(rect.x() - x, bounds.width(), resized.width());
            let top = scaled_offset(rect.y() - y, bounds.height(), resized.height());
            let right = scaled_offset(rect.x() + rect.width() - x, bounds.width(), resized.width());
            let bottom = scaled_offset(
                rect.y() + rect.height() - y,
                bounds.height(),
                resized.height(),
            );
            ProjectRect::new(
                resized.x() + left,
                resized.y() + top,
                right - left,
                bottom - top,
            )
        })
        .collect()
}

fn scaled_offset(offset: u64, original: u64, resized: u64) -> u64 {
    ((u128::from(offset) * u128::from(resized) + u128::from(original / 2)) / u128::from(original))
        as u64
}

fn minimum_group_axis(bounds_size: u64, member_sizes: impl Iterator<Item = u64>) -> u64 {
    member_sizes
        .map(|size| {
            let numerator = u128::from(bounds_size) * u128::from(MIN_FRAME_EDGE_UM.min(size));
            let denominator = u128::from(size);
            (numerator / denominator + u128::from(numerator % denominator != 0)) as u64
        })
        .max()
        .expect("nonempty selection")
}

fn edited_rect(
    rect: ProjectRect,
    surface_width: u64,
    surface_height: u64,
    minimum_width: u64,
    minimum_height: u64,
    gesture: &FrameGeometryGesture,
    delta: Option<[f64; 2]>,
) -> ProjectRect {
    match *gesture {
        FrameGeometryGesture::Move {
            delta_x_um,
            delta_y_um,
        } => ProjectRect::new(
            delta.map_or_else(
                || moved_coordinate(rect.x(), delta_x_um, surface_width - rect.width()),
                |delta| {
                    (rect.x() as f64 + delta[0])
                        .round()
                        .clamp(0.0, (surface_width - rect.width()) as f64)
                        as u64
                },
            ),
            delta.map_or_else(
                || moved_coordinate(rect.y(), delta_y_um, surface_height - rect.height()),
                |delta| {
                    (rect.y() as f64 + delta[1])
                        .round()
                        .clamp(0.0, (surface_height - rect.height()) as f64)
                        as u64
                },
            ),
            rect.width(),
            rect.height(),
        ),
        FrameGeometryGesture::Resize {
            handle,
            delta_x_um,
            delta_y_um,
            preserve_aspect_ratio,
            from_center,
        } => {
            let (horizontal, vertical) = handle.axes();
            let x_axis = ResizeAxis::new(
                rect.x(),
                rect.width(),
                surface_width,
                minimum_width,
                horizontal,
                from_center,
            );
            let y_axis = ResizeAxis::new(
                rect.y(),
                rect.height(),
                surface_height,
                minimum_height,
                vertical,
                from_center,
            );
            let delta = delta.unwrap_or([delta_x_um as f64, delta_y_um as f64]);
            let mut width = x_axis.requested_size(delta[0]);
            let mut height = y_axis.requested_size(delta[1]);
            if preserve_aspect_ratio && horizontal != 0 && vertical != 0 {
                let x_scale = width / x_axis.original_size;
                let y_scale = height / y_axis.original_size;
                // The larger proportional pointer displacement determines the
                // corner's scale; both axes then stop together at the first limit.
                let scales = [x_scale, y_scale];
                let scale = scales[dominant_resize_axis(scales)];
                let minimum = (x_axis.minimum / x_axis.original_size)
                    .max(y_axis.minimum / y_axis.original_size);
                let maximum = (x_axis.maximum / x_axis.original_size)
                    .min(y_axis.maximum / y_axis.original_size);
                let scale = scale.clamp(minimum, maximum);
                width = x_axis.original_size * scale;
                height = y_axis.original_size * scale;
            }
            let (x, width) = x_axis.resolve(width);
            let (y, height) = y_axis.resolve(height);
            ProjectRect::new(x, y, width, height)
        }
    }
}

struct ResizeAxis {
    original_size: f64,
    anchor: f64,
    anchor_ratio: f64,
    delta_multiplier: f64,
    minimum: f64,
    maximum: f64,
    limit: u64,
}

impl ResizeAxis {
    fn new(
        start: u64,
        size: u64,
        limit: u64,
        minimum_size: u64,
        direction: i8,
        from_center: bool,
    ) -> Self {
        let anchor_ratio = resize_anchor_ratio(direction, from_center);
        let anchor = start as f64 + size as f64 * anchor_ratio;
        let maximum = if direction == 0 {
            size as f64
        } else if from_center {
            2.0 * anchor.min(limit as f64 - anchor)
        } else if direction < 0 {
            anchor
        } else {
            limit as f64 - anchor
        };
        Self {
            original_size: size as f64,
            anchor,
            anchor_ratio,
            delta_multiplier: resize_delta_multiplier(direction, from_center),
            minimum: if direction == 0 { size } else { minimum_size } as f64,
            maximum,
            limit,
        }
    }

    fn requested_size(&self, delta: f64) -> f64 {
        self.original_size + delta * self.delta_multiplier
    }

    fn resolve(&self, requested: f64) -> (u64, u64) {
        let size = requested.round().clamp(self.minimum, self.maximum) as u64;
        let start = (self.anchor - size as f64 * self.anchor_ratio)
            .round()
            .clamp(0.0, (self.limit - size) as f64) as u64;
        (start, size)
    }
}

fn moved_coordinate(start: u64, delta: i64, maximum: u64) -> u64 {
    (i128::from(start) + i128::from(delta)).clamp(0, i128::from(maximum)) as u64
}

// Geometry owns modifier interpretation for both free transforms and the snap
// solver's analytical pointer corrections.
pub(crate) fn resize_anchor_ratio(direction: i8, from_center: bool) -> f64 {
    if from_center {
        0.5
    } else if direction < 0 {
        1.0
    } else {
        0.0
    }
}

pub(crate) fn resize_delta_multiplier(direction: i8, from_center: bool) -> f64 {
    f64::from(direction) * if from_center { 2.0 } else { 1.0 }
}

pub(crate) fn dominant_resize_axis(scales: [f64; 2]) -> usize {
    usize::from((scales[1] - 1.0).abs() > (scales[0] - 1.0).abs())
}

impl From<ProjectRect> for RectUm {
    fn from(rect: ProjectRect) -> Self {
        Self {
            x: rect.x() as i64,
            y: rect.y() as i64,
            width: rect.width() as i64,
            height: rect.height() as i64,
        }
    }
}
