use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ProjectRect, RectUm};

/// One gesture, expressed from the geometry that was visible at pointer-down.
/// Its baseline prevents a delayed gesture from overwriting another geometry edit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameGeometryEdit {
    pub frame_id: String,
    pub expected_rect: RectUm,
    pub gesture: FrameGeometryGesture,
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
    fn axes(self) -> (i8, i8) {
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

pub(crate) fn edited_rect(
    rect: ProjectRect,
    surface_width: u64,
    surface_height: u64,
    gesture: &FrameGeometryGesture,
) -> ProjectRect {
    match *gesture {
        FrameGeometryGesture::Move {
            delta_x_um,
            delta_y_um,
        } => ProjectRect::new(
            moved_coordinate(rect.x(), delta_x_um, surface_width - rect.width()),
            moved_coordinate(rect.y(), delta_y_um, surface_height - rect.height()),
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
                horizontal,
                from_center,
            );
            let y_axis = ResizeAxis::new(
                rect.y(),
                rect.height(),
                surface_height,
                vertical,
                from_center,
            );
            let mut width = x_axis.requested_size(delta_x_um);
            let mut height = y_axis.requested_size(delta_y_um);
            if preserve_aspect_ratio && horizontal != 0 && vertical != 0 {
                let x_scale = width / x_axis.original_size;
                let y_scale = height / y_axis.original_size;
                // The larger proportional pointer displacement determines the
                // corner's scale; both axes then stop together at the first limit.
                let scale = if (x_scale - 1.0).abs() >= (y_scale - 1.0).abs() {
                    x_scale
                } else {
                    y_scale
                };
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
    fn new(start: u64, size: u64, limit: u64, direction: i8, from_center: bool) -> Self {
        let anchor_ratio = if from_center {
            0.5
        } else if direction < 0 {
            1.0
        } else {
            0.0
        };
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
            delta_multiplier: f64::from(direction) * if from_center { 2.0 } else { 1.0 },
            minimum: if direction == 0 {
                size
            } else {
                MIN_FRAME_EDGE_UM.min(size)
            } as f64,
            maximum,
            limit,
        }
    }

    fn requested_size(&self, delta: i64) -> f64 {
        self.original_size + delta as f64 * self.delta_multiplier
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
