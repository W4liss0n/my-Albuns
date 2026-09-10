use std::collections::HashMap;

use crate::model::{
    AlbumSnapshot, ComposedBackground, ComposedColor, ComposedDecorative, ComposedFrame,
    ComposedPhoto, ComposedSheet, CompositionPlan, EditorProjection, EditorState, Matrix2,
    MediaCatalogItem, MediaId, MediaUsage, NormalizedPan, NumberRange, PHOTO_PAN_MAX,
    PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN, PhotoPlacement, PhotoPlacementPlan,
    PhotoSnapshot, ProjectedActiveSides, ProjectedBackground, ProjectedBackgroundContent,
    ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent, RectUm, SizeUm, VectorUm,
};

struct CompositionCore;

impl CompositionCore {
    fn compose(album: &AlbumSnapshot) -> CompositionPlan {
        let media_by_id = album
            .media
            .iter()
            .map(|media| (media.id, media))
            .collect::<HashMap<_, _>>();
        CompositionPlan {
            frame_border: album.visual_defaults.frame_border.clone(),
            sheets: album
                .sheets
                .iter()
                .map(|sheet| {
                    let visuals = sheet.visuals.clone().unwrap_or_default();
                    let surface =
                        active_surface_rect(sheet.active_sides, sheet.width_um, sheet.height_um);
                    let mut frames = sheet
                        .frames
                        .iter()
                        .map(|frame| {
                            let border = if frame.style.border_width_um == 0 {
                                ProjectedFrameBorder::None
                            } else {
                                ProjectedFrameBorder::Solid {
                                    rgb: frame.style.border_rgb.clone(),
                                    width_um: frame.style.border_width_um,
                                }
                            };
                            ComposedFrame {
                                frame_id: frame.id.clone(),
                                clip_rect: frame.rect.clone(),
                                opacity_byte: ((u16::from(frame.style.opacity_percent) * 255 + 50)
                                    / 100) as u8,
                                border_fill_rects: compose_frame_border_fill_rects(
                                    &frame.rect,
                                    &border,
                                ),
                                border,
                                z_index: frame.z_index,
                                photo: frame.photo.as_ref().map(|photo| {
                                    let media = media_by_id
                                        .get(&photo.media_id)
                                        .copied()
                                        .expect("validated Frame media reference");
                                    compose_photo(&frame.rect, photo, media)
                                }),
                            }
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
                            &visuals.background,
                            sheet.active_sides,
                            sheet.width_um,
                            sheet.height_um,
                            &media_by_id,
                        ),
                        frames,
                        overlays: compose_overlays(
                            &album.visual_defaults.overlay,
                            &visuals.overlay,
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
    let stroke = i64::try_from(*width_um).unwrap_or(i64::MAX);
    let inset_x = stroke.min(frame.width / 2);
    let inset_y = stroke.min(frame.height / 2);
    let inner_height = frame.height - inset_y * 2;

    vec![
        RectUm {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: inset_y,
        },
        RectUm {
            x: frame.x,
            y: frame.y + frame.height - inset_y,
            width: frame.width,
            height: inset_y,
        },
        RectUm {
            x: frame.x,
            y: frame.y + inset_y,
            width: inset_x,
            height: inner_height,
        },
        RectUm {
            x: frame.x + frame.width - inset_x,
            y: frame.y + inset_y,
            width: inset_x,
            height: inner_height,
        },
    ]
    .into_iter()
    .filter(|rect| rect.width > 0 && rect.height > 0)
    .collect()
}

fn derive_media_usage(album: &AlbumSnapshot, composition: &CompositionPlan) -> Vec<MediaUsage> {
    let mut counts = HashMap::<MediaId, crate::MediaUsageBreakdown>::new();
    for sheet in &composition.sheets {
        for frame in &sheet.frames {
            if let Some(photo) = &frame.photo {
                counts.entry(photo.media_id).or_default().frames += 1;
            }
        }
        for background in &sheet.backgrounds {
            if let ComposedBackground::Media { media_id, .. } = background {
                counts.entry(*media_id).or_default().backgrounds += 1;
            }
        }
        for overlay in &sheet.overlays {
            counts.entry(overlay.media_id).or_default().overlays += 1;
        }
    }
    let backgrounds = match &album.visual_defaults.background {
        ProjectedBackground::BothSides { both } => vec![both],
        ProjectedBackground::PerSide { left, right } => vec![left, right],
    };
    for content in backgrounds {
        if let ProjectedBackgroundContent::Media { media_id } = content {
            counts.entry(*media_id).or_default().album_backgrounds += 1;
        }
    }
    let overlays = match &album.visual_defaults.overlay {
        ProjectedOverlay::BothSides { both } => vec![both],
        ProjectedOverlay::PerSide { left, right } => vec![left, right],
    };
    for content in overlays.into_iter().flatten() {
        let ProjectedOverlayContent::Media { media_id } = content;
        counts.entry(*media_id).or_default().album_overlays += 1;
    }
    album
        .media
        .iter()
        .map(|media| {
            let breakdown = counts.remove(&media.id).unwrap_or_default();
            MediaUsage {
                media_id: media.id,
                count: breakdown.count(),
                breakdown: Some(breakdown),
            }
        })
        .collect()
}

/// The crate's only entry point that resolves an Album into a CompositionPlan.
pub(crate) fn resolve_editor_projection(state: EditorState) -> EditorProjection {
    let composition = CompositionCore::compose(&state.album);
    let media_usage = derive_media_usage(&state.album, &composition);
    EditorProjection {
        can_paste_frames: false,
        state,
        composition,
        media_usage,
    }
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

struct VisualRegion<'a, T> {
    content: &'a T,
    draw_rect: RectUm,
    clip_rect: Option<RectUm>,
}

fn visual_regions<'a, T>(
    local: &'a crate::SheetVisual<T>,
    defaults: Vec<VisualRegion<'a, T>>,
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
) -> Vec<VisualRegion<'a, T>> {
    use crate::{SheetVisual, SideVisual, VisualMapping};
    let surface = active_surface_rect(active_sides, full_width_um, height_um);
    match local {
        SheetVisual::Default => defaults,
        SheetVisual::BothSides { content } => vec![VisualRegion {
            content,
            draw_rect: surface,
            clip_rect: None,
        }],
        SheetVisual::PerSide { left, right } => {
            let sides = match active_sides {
                ProjectedActiveSides::Both => {
                    let [l, r] = side_rects(full_width_um, height_um);
                    vec![(left, l), (right, r)]
                }
                ProjectedActiveSides::Left => vec![(left, surface.clone())],
                ProjectedActiveSides::Right => vec![(right, surface.clone())],
            };
            sides
                .into_iter()
                .flat_map(|(side, rect)| match side {
                    SideVisual::Default => defaults
                        .iter()
                        .filter_map(|region| {
                            intersect_rects(&region.draw_rect, &rect).map(|clip| VisualRegion {
                                content: region.content,
                                draw_rect: region.draw_rect.clone(),
                                clip_rect: (clip != region.draw_rect).then_some(clip),
                            })
                        })
                        .collect(),
                    SideVisual::Custom { content, mapping } => {
                        let draw_rect = if *mapping == VisualMapping::BothSides {
                            RectUm {
                                x: if active_sides == ProjectedActiveSides::Right {
                                    -(full_width_um / 2)
                                } else {
                                    0
                                },
                                y: 0,
                                width: full_width_um,
                                height: height_um,
                            }
                        } else {
                            rect.clone()
                        };
                        vec![VisualRegion {
                            content,
                            clip_rect: (draw_rect != rect).then_some(rect),
                            draw_rect,
                        }]
                    }
                })
                .collect()
        }
    }
}

fn intersect_rects(a: &RectUm, b: &RectUm) -> Option<RectUm> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let width = (a.x + a.width).min(b.x + b.width) - x;
    let height = (a.y + a.height).min(b.y + b.height) - y;
    (width > 0 && height > 0).then_some(RectUm {
        x,
        y,
        width,
        height,
    })
}

fn default_visual_regions<'a, T>(
    both: Option<&'a T>,
    sides: Option<(&'a T, &'a T)>,
    active_sides: ProjectedActiveSides,
    width: i64,
    height: i64,
) -> Vec<VisualRegion<'a, T>> {
    let surface = active_surface_rect(active_sides, width, height);
    let entries = if let Some(content) = both {
        vec![(content, surface)]
    } else {
        let (left, right) = sides.expect("a visual default has either whole or per-side content");
        match active_sides {
            ProjectedActiveSides::Both => {
                let [l, r] = side_rects(width, height);
                vec![(left, l), (right, r)]
            }
            ProjectedActiveSides::Left => vec![(left, surface)],
            ProjectedActiveSides::Right => vec![(right, surface)],
        }
    };
    entries
        .into_iter()
        .map(|(content, draw_rect)| VisualRegion {
            content,
            draw_rect,
            clip_rect: None,
        })
        .collect()
}

fn compose_backgrounds(
    background: &ProjectedBackground,
    local: &crate::SheetVisual<ProjectedBackgroundContent>,
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
    media_by_id: &HashMap<MediaId, &MediaCatalogItem>,
) -> Vec<ComposedBackground> {
    let defaults = match background {
        ProjectedBackground::BothSides { both } => {
            default_visual_regions(Some(both), None, active_sides, full_width_um, height_um)
        }
        ProjectedBackground::PerSide { left, right } => default_visual_regions(
            None,
            Some((left, right)),
            active_sides,
            full_width_um,
            height_um,
        ),
    };
    visual_regions(local, defaults, active_sides, full_width_um, height_um)
        .into_iter()
        .map(|region| match region.content {
            ProjectedBackgroundContent::Color { rgb } => ComposedBackground::Color {
                rgb: rgb.clone(),
                draw_rect: region.clip_rect.unwrap_or(region.draw_rect),
            },
            ProjectedBackgroundContent::Media { media_id } => {
                let media = media_by_id
                    .get(media_id)
                    .expect("validated Background reference");
                ComposedBackground::Media {
                    media_id: *media_id,
                    name: media.name.clone(),
                    draw_rect: region.draw_rect,
                    clip_rect: region.clip_rect,
                }
            }
        })
        .collect()
}

fn compose_overlays(
    overlay: &ProjectedOverlay,
    local: &crate::SheetVisual<Option<ProjectedOverlayContent>>,
    active_sides: ProjectedActiveSides,
    full_width_um: i64,
    height_um: i64,
    media_by_id: &HashMap<MediaId, &MediaCatalogItem>,
) -> Vec<ComposedDecorative> {
    let defaults = match overlay {
        ProjectedOverlay::BothSides { both } => {
            default_visual_regions(Some(both), None, active_sides, full_width_um, height_um)
        }
        ProjectedOverlay::PerSide { left, right } => default_visual_regions(
            None,
            Some((left, right)),
            active_sides,
            full_width_um,
            height_um,
        ),
    };
    visual_regions(local, defaults, active_sides, full_width_um, height_um)
        .into_iter()
        .filter_map(|region| {
            let ProjectedOverlayContent::Media { media_id } = region.content.as_ref()?;
            let media = media_by_id
                .get(media_id)
                .expect("validated Overlay reference");
            Some(ComposedDecorative {
                media_id: *media_id,
                name: media.name.clone(),
                draw_rect: region.draw_rect,
                clip_rect: region.clip_rect,
            })
        })
        .collect()
}
fn compose_photo(frame: &RectUm, photo: &PhotoSnapshot, media: &MediaCatalogItem) -> ComposedPhoto {
    let rotation_degrees =
        photo.transform.quarter_turns as f32 * 90.0 - photo.transform.fine_rotation_degrees;
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
    // Horizontal mirroring reflects the fine angle's axes. Keep the quarter-turn
    // direction used by legacy projects, so mirroring never reverses Pan controls.
    let pan_radians = if photo.transform.mirror_x {
        f64::from(photo.transform.quarter_turns) * std::f64::consts::PI - radians
    } else {
        radians
    };
    let horizontal_direction = VectorUm {
        x: pan_radians.cos(),
        y: pan_radians.sin(),
    };
    let vertical_direction = VectorUm {
        x: -pan_radians.sin(),
        y: pan_radians.cos(),
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
        base_fill_zoom: fill_scale,
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
        media_id: photo.media_id,
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
        black_and_white: photo.transform.black_and_white,
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
                    height: 30,
                },
                RectUm {
                    x: 10,
                    y: 50,
                    width: 100,
                    height: 30,
                },
            ],
        );
    }
}
