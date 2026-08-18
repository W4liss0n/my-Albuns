use std::collections::HashMap;

use crate::model::{
    AlbumSnapshot, ComposedBackground, ComposedColor, ComposedDecorative, ComposedFrame,
    ComposedPhoto, ComposedSheet, CompositionPlan, Matrix2, MediaCatalogItem, MediaUsage,
    NormalizedPan, NumberRange, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN,
    PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot, ProjectedActiveSides, ProjectedBackground,
    ProjectedBackgroundContent, ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent,
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
            frame_border: album.visual_defaults.frame_border.clone(),
            sheets: album
                .sheets
                .iter()
                .map(|sheet| {
                    let surface =
                        active_surface_rect(sheet.active_sides, sheet.width_um, sheet.height_um);
                    let mut frames = sheet
                        .frames
                        .iter()
                        .map(|frame| ComposedFrame {
                            frame_id: frame.id.clone(),
                            clip_rect: frame.rect.clone(),
                            border_fill_rects: compose_frame_border_fill_rects(
                                &frame.rect,
                                &album.visual_defaults.frame_border,
                            ),
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
                        active_sides: sheet.active_sides,
                        width_um: surface.width,
                        height_um: sheet.height_um,
                        base: ComposedColor {
                            rgb: "#FFFFFF".into(),
                            draw_rect: surface.clone(),
                        },
                        backgrounds: compose_backgrounds(
                            &album.visual_defaults.background,
                            sheet.active_sides,
                            sheet.width_um,
                            sheet.height_um,
                            &media_by_id,
                        ),
                        frames,
                        overlays: compose_overlays(
                            &album.visual_defaults.overlay,
                            sheet.active_sides,
                            sheet.width_um,
                            sheet.height_um,
                            &media_by_id,
                        ),
                    }
                })
                .collect(),
        }
    }
}

pub(crate) fn compose_frame_border_fill_rects(
    frame: &RectUm,
    border: &ProjectedFrameBorder,
) -> Vec<RectUm> {
    let ProjectedFrameBorder::Solid { width_um, .. } = border else {
        return Vec::new();
    };
    let stroke = i64::try_from(*width_um)
        .unwrap_or(i64::MAX)
        .min(frame.width)
        .min(frame.height);
    if stroke <= 0 {
        return Vec::new();
    }

    vec![
        RectUm {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: stroke,
        },
        RectUm {
            x: frame.x,
            y: frame.y + frame.height - stroke,
            width: frame.width,
            height: stroke,
        },
        RectUm {
            x: frame.x,
            y: frame.y,
            width: stroke,
            height: frame.height,
        },
        RectUm {
            x: frame.x + frame.width - stroke,
            y: frame.y,
            width: stroke,
            height: frame.height,
        },
    ]
}

#[cfg(test)]
mod frame_border_tests {
    use super::*;

    #[test]
    fn composes_an_inward_border_and_saturates_without_degenerate_rects() {
        let frame = RectUm {
            x: 10,
            y: 20,
            width: 100,
            height: 60,
        };
        let border = ProjectedFrameBorder::Solid {
            rgb: "#000000".into(),
            width_um: 100,
        };

        assert_eq!(
            compose_frame_border_fill_rects(&frame, &border),
            vec![
                RectUm {
                    x: 10,
                    y: 20,
                    width: 100,
                    height: 60,
                },
                RectUm {
                    x: 10,
                    y: 20,
                    width: 100,
                    height: 60,
                },
                RectUm {
                    x: 10,
                    y: 20,
                    width: 60,
                    height: 60,
                },
                RectUm {
                    x: 50,
                    y: 20,
                    width: 60,
                    height: 60,
                },
            ],
        );
    }
}

pub(crate) fn derive_media_usage(
    album: &AlbumSnapshot,
    composition: &CompositionPlan,
) -> Vec<MediaUsage> {
    let mut counts = HashMap::<&str, usize>::new();
    for media_id in composition
        .sheets
        .iter()
        .flat_map(ComposedSheet::referenced_media_ids)
    {
        *counts.entry(media_id).or_default() += 1;
    }

    album
        .media
        .iter()
        .map(|media| MediaUsage {
            media_id: media.id.clone(),
            count: counts.get(media.id.as_str()).copied().unwrap_or_default(),
        })
        .collect()
}

fn active_surface_rect(
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
) -> RectUm {
    RectUm {
        x: 0,
        y: 0,
        width: match active_sides {
            ProjectedActiveSides::Both => full_width_um,
            ProjectedActiveSides::Left | ProjectedActiveSides::Right => full_width_um / 2,
        },
        height: height_um,
    }
}

fn side_rects(full_width_um: i64, height_um: i64) -> [RectUm; 2] {
    let left_width = full_width_um / 2;
    [
        RectUm {
            x: 0,
            y: 0,
            width: left_width,
            height: height_um,
        },
        RectUm {
            x: left_width,
            y: 0,
            width: full_width_um - left_width,
            height: height_um,
        },
    ]
}

fn compose_backgrounds(
    background: &ProjectedBackground,
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
    media_by_id: &HashMap<&str, &MediaCatalogItem>,
) -> Vec<ComposedBackground> {
    let surface = active_surface_rect(active_sides, full_width_um, height_um);
    match background {
        ProjectedBackground::BothSides { both } => {
            vec![compose_background(both, surface, media_by_id)]
        }
        ProjectedBackground::PerSide { left, right } => match active_sides {
            ProjectedActiveSides::Both => {
                let [left_rect, right_rect] = side_rects(full_width_um, height_um);
                vec![
                    compose_background(left, left_rect, media_by_id),
                    compose_background(right, right_rect, media_by_id),
                ]
            }
            ProjectedActiveSides::Left => {
                vec![compose_background(left, surface, media_by_id)]
            }
            ProjectedActiveSides::Right => {
                vec![compose_background(right, surface, media_by_id)]
            }
        },
    }
}

fn compose_background(
    content: &ProjectedBackgroundContent,
    draw_rect: RectUm,
    media_by_id: &HashMap<&str, &MediaCatalogItem>,
) -> ComposedBackground {
    match content {
        ProjectedBackgroundContent::Color { rgb } => ComposedBackground::Color {
            rgb: rgb.clone(),
            draw_rect,
        },
        ProjectedBackgroundContent::Media { media_id } => {
            let media = media_by_id
                .get(media_id.as_str())
                .copied()
                .expect("validated Background media reference");
            ComposedBackground::Media {
                media_id: media.id.clone(),
                name: media.name.clone(),
                draw_rect,
            }
        }
    }
}

fn compose_overlays(
    overlay: &ProjectedOverlay,
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
    media_by_id: &HashMap<&str, &MediaCatalogItem>,
) -> Vec<ComposedDecorative> {
    let surface = active_surface_rect(active_sides, full_width_um, height_um);
    match overlay {
        ProjectedOverlay::BothSides { both } => both
            .as_ref()
            .map(|content| compose_overlay(content, surface, media_by_id))
            .into_iter()
            .collect(),
        ProjectedOverlay::PerSide { left, right } => match active_sides {
            ProjectedActiveSides::Both => {
                let [left_rect, right_rect] = side_rects(full_width_um, height_um);
                [
                    left.as_ref()
                        .map(|content| compose_overlay(content, left_rect, media_by_id)),
                    right
                        .as_ref()
                        .map(|content| compose_overlay(content, right_rect, media_by_id)),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            ProjectedActiveSides::Left => left
                .as_ref()
                .map(|content| compose_overlay(content, surface, media_by_id))
                .into_iter()
                .collect(),
            ProjectedActiveSides::Right => right
                .as_ref()
                .map(|content| compose_overlay(content, surface, media_by_id))
                .into_iter()
                .collect(),
        },
    }
}

fn compose_overlay(
    content: &ProjectedOverlayContent,
    draw_rect: RectUm,
    media_by_id: &HashMap<&str, &MediaCatalogItem>,
) -> ComposedDecorative {
    let ProjectedOverlayContent::Media { media_id } = content;
    let media = media_by_id
        .get(media_id.as_str())
        .copied()
        .expect("validated Overlay media reference");
    ComposedDecorative {
        media_id: media.id.clone(),
        name: media.name.clone(),
        draw_rect,
    }
}

pub(crate) fn build_render_snapshot(
    project_id: &str,
    project_name: &str,
    revision: u64,
    dpi: u32,
    album: &AlbumSnapshot,
) -> RenderSnapshot {
    RenderSnapshot {
        schema_version: RENDER_SNAPSHOT_SCHEMA_VERSION,
        project_id: project_id.into(),
        project_name: project_name.into(),
        revision,
        dpi,
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
    let source_width = media.source_width_px.expect("validated Photo width") as f64;
    let source_height = media.source_height_px.expect("validated Photo height") as f64;
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
        palette: media.palette.clone().expect("validated Photo palette"),
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
