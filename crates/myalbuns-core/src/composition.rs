use std::collections::HashMap;

use crate::model::{
    AlbumSnapshot, ComposedFrame, ComposedPhoto, ComposedSheet, CompositionPlan, Matrix2,
    MediaCatalogItem, NormalizedPan, NumberRange, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX,
    PHOTO_ZOOM_MIN, PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot,
    RENDER_SNAPSHOT_SCHEMA_VERSION, RectUm, RenderSnapshot, SizeUm, VectorUm,
};

pub(crate) struct CompositionCore;

impl CompositionCore {
    pub(crate) fn compose(album: &AlbumSnapshot) -> CompositionPlan {
        let media_by_id = album
            .media
            .iter()
            .map(|media| (media.id.as_str(), media))
            .collect::<HashMap<_, _>>();
        CompositionPlan {
            sheets: album
                .sheets
                .iter()
                .map(|sheet| {
                    let mut frames = sheet
                        .frames
                        .iter()
                        .map(|frame| ComposedFrame {
                            frame_id: frame.id.clone(),
                            clip_rect: frame.rect.clone(),
                            z_index: frame.z_index,
                            photo: frame.photo.as_ref().map(|photo| {
                                let media = media_by_id
                                    .get(photo.media_id.as_str())
                                    .copied()
                                    .expect("validated Frame media reference");
                                compose_photo(&frame.rect, photo, media)
                            }),
                        })
                        .collect::<Vec<_>>();
                    frames.sort_by(|left, right| {
                        left.z_index
                            .cmp(&right.z_index)
                            .then_with(|| left.frame_id.cmp(&right.frame_id))
                    });

                    ComposedSheet {
                        sheet_id: sheet.id.clone(),
                        number: sheet.number,
                        width_um: sheet.width_um,
                        height_um: sheet.height_um,
                        has_overlay: sheet.has_overlay,
                        frames,
                    }
                })
                .collect(),
        }
    }
}

pub(crate) fn build_render_snapshot(
    project_id: &str,
    project_name: &str,
    revision: u64,
    album: &AlbumSnapshot,
) -> RenderSnapshot {
    RenderSnapshot {
        schema_version: RENDER_SNAPSHOT_SCHEMA_VERSION,
        project_id: project_id.into(),
        project_name: project_name.into(),
        revision,
        unit: "micrometers".into(),
        composition: CompositionCore::compose(album),
    }
}

fn compose_photo(frame: &RectUm, photo: &PhotoSnapshot, media: &MediaCatalogItem) -> ComposedPhoto {
    let rotation_degrees =
        photo.transform.quarter_turns as f32 * 90.0 + photo.transform.fine_rotation_degrees;
    let radians = (rotation_degrees as f64).to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    let frame_width = frame.width as f64;
    let frame_height = frame.height as f64;
    let source_width = media.source_width_px as f64;
    let source_height = media.source_height_px as f64;
    let required_width = cosine.abs() * frame_width + sine.abs() * frame_height;
    let required_height = sine.abs() * frame_width + cosine.abs() * frame_height;
    let fill_scale = (required_width / source_width).max(required_height / source_height);
    let draw_width_at_fill = source_width * fill_scale;
    let draw_height_at_fill = source_height * fill_scale;
    let current_pan = NormalizedPan {
        x: photo.transform.pan_x.clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX) as f64,
        y: photo.transform.pan_y.clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX) as f64,
    };
    let current_zoom = photo
        .transform
        .user_zoom
        .clamp(PHOTO_ZOOM_MIN, PHOTO_ZOOM_MAX) as f64;
    let pan_origin = VectorUm {
        x: frame_width / 2.0,
        y: frame_height / 2.0,
    };
    let horizontal_direction = VectorUm { x: cosine, y: sine };
    let vertical_direction = VectorUm {
        x: -sine,
        y: cosine,
    };
    let horizontal_span = (draw_width_at_fill * current_zoom - required_width).max(0.0);
    let vertical_span = (draw_height_at_fill * current_zoom - required_height).max(0.0);
    let horizontal_offset = scale_vector(&horizontal_direction, horizontal_span / 2.0);
    let vertical_offset = scale_vector(&vertical_direction, vertical_span / 2.0);
    let pan_to_center = matrix_from_columns(&horizontal_offset, &vertical_offset);
    let current_offset = apply_matrix(&pan_to_center, &current_pan);
    let current = PhotoPlacement {
        center: VectorUm {
            x: pan_origin.x + current_offset.x,
            y: pan_origin.y + current_offset.y,
        },
        size: SizeUm {
            width: draw_width_at_fill * current_zoom,
            height: draw_height_at_fill * current_zoom,
        },
    };
    let horizontal_zoom_delta = scale_vector(&horizontal_direction, draw_width_at_fill / 2.0);
    let vertical_zoom_delta = scale_vector(&vertical_direction, draw_height_at_fill / 2.0);
    let placement = PhotoPlacementPlan {
        current_pan,
        current_zoom,
        pan_range: NumberRange {
            minimum: PHOTO_PAN_MIN as f64,
            maximum: PHOTO_PAN_MAX as f64,
        },
        zoom_range: NumberRange {
            minimum: PHOTO_ZOOM_MIN as f64,
            maximum: PHOTO_ZOOM_MAX as f64,
        },
        current: current.clone(),
        pan_origin,
        pan_to_center,
        pan_to_center_per_zoom: matrix_from_columns(&horizontal_zoom_delta, &vertical_zoom_delta),
        size_per_zoom: SizeUm {
            width: draw_width_at_fill,
            height: draw_height_at_fill,
        },
    };

    ComposedPhoto {
        media_id: photo.media_id.clone(),
        name: media.name.clone(),
        draw_rect: RectUm {
            x: frame.x + (current.center.x - current.size.width / 2.0).round() as i64,
            y: frame.y + (current.center.y - current.size.height / 2.0).round() as i64,
            width: current.size.width.ceil() as i64,
            height: current.size.height.ceil() as i64,
        },
        placement,
        rotation_degrees,
        mirror_x: photo.transform.mirror_x,
        palette: media.palette.clone(),
    }
}

fn scale_vector(vector: &VectorUm, factor: f64) -> VectorUm {
    VectorUm {
        x: vector.x * factor,
        y: vector.y * factor,
    }
}

fn matrix_from_columns(horizontal: &VectorUm, vertical: &VectorUm) -> Matrix2 {
    Matrix2 {
        xx: horizontal.x,
        xy: vertical.x,
        yx: horizontal.y,
        yy: vertical.y,
    }
}

fn apply_matrix(matrix: &Matrix2, vector: &NormalizedPan) -> VectorUm {
    VectorUm {
        x: matrix.xx * vector.x + matrix.xy * vector.y,
        y: matrix.yx * vector.x + matrix.yy * vector.y,
    }
}
