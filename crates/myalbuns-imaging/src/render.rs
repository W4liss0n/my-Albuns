use std::collections::HashMap;

use image::{Rgba, RgbaImage};
use myalbuns_core::{ComposedBackground, ComposedFrame, MediaId, ProjectedFrameBorder, RectUm};
use myalbuns_imaging_protocol::{
    ImagingFailure, ImagingFailureCode, ImagingPathCode, ImagingProgressStage,
};

use crate::jpeg_output::{JpegFailure, RasterPlan};

const MICROMETERS_PER_INCH: f64 = 25_400.0;
/// Smaller layers are painted inline; spawning workers would cost more.
const MIN_PARALLEL_PIXELS: usize = 16_384;
/// Export reserves the whole Processor capacity: logical CPUs minus one, up to eight.
const MAX_COMPOSITION_WORKERS: usize = 8;

pub(crate) struct RenderFailure {
    pub(crate) failure: ImagingFailure,
    pub(crate) message: String,
}

impl RenderFailure {
    fn new(code: ImagingFailureCode, message: impl Into<String>) -> Self {
        Self {
            failure: ImagingFailure {
                code,
                media_id: None,
                path_code: None,
            },
            message: message.into(),
        }
    }

    pub(crate) fn typed(
        code: ImagingFailureCode,
        media_id: Option<String>,
        path_code: Option<ImagingPathCode>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            failure: ImagingFailure {
                code,
                media_id,
                path_code,
            },
            message: message.into(),
        }
    }
}

impl From<String> for RenderFailure {
    fn from(message: String) -> Self {
        Self::new(ImagingFailureCode::CompositionFailed, message)
    }
}

impl From<JpegFailure> for RenderFailure {
    fn from(failure: JpegFailure) -> Self {
        Self::new(failure.code, failure.message)
    }
}

pub(crate) fn composition_workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .saturating_sub(1)
        .clamp(1, MAX_COMPOSITION_WORKERS)
}

pub(crate) fn render_unit(
    unit: &myalbuns_core::ComposedOutputUnit,
    dpi: u32,
    sources: &HashMap<MediaId, RgbaImage>,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<RgbaImage, RenderFailure> {
    render_unit_with_workers(unit, dpi, sources, progress, composition_workers())
}

/// Layers are painted in order; each layer splits its rows among `workers`.
/// Every pixel is written by exactly one worker with the same arithmetic, so
/// the raster does not depend on the worker count.
fn render_unit_with_workers(
    unit: &myalbuns_core::ComposedOutputUnit,
    dpi: u32,
    sources: &HashMap<MediaId, RgbaImage>,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
    workers: usize,
) -> Result<RgbaImage, RenderFailure> {
    let sheet = &unit.sheet;
    let raster = RasterPlan::new(sheet.width_um, sheet.height_um, dpi)?;
    let pixels_per_micrometer = dpi as f64 / MICROMETERS_PER_INCH;
    let mut image = raster.allocate_rgba(opaque_rgb(&sheet.base.rgb))?;

    for background in &sheet.backgrounds {
        match background {
            ComposedBackground::Color { rgb, draw_rect } => {
                fill_composed_rect(&mut image, draw_rect, raster, opaque_rgb(rgb), workers)?
            }
            ComposedBackground::Media {
                media_id,
                draw_rect,
                clip_rect,
                ..
            } => {
                let source = sources
                    .get(media_id)
                    .ok_or_else(|| format!("a fonte do Background {media_id} não foi carregada"))?;
                draw_stretched_media(
                    &mut image,
                    draw_rect,
                    clip_rect.as_ref(),
                    raster,
                    source,
                    workers,
                )?;
            }
        }
    }

    let frame_count = u32::try_from(sheet.frames.len())
        .map_err(|_| "a Lâmina contém Frames demais".to_string())?;
    let composition_units = frame_count.max(1);
    progress(ImagingProgressStage::Composing, 0, composition_units)?;
    for (index, frame) in sheet.frames.iter().enumerate() {
        draw_frame(
            &mut image,
            frame,
            pixels_per_micrometer,
            raster,
            sources,
            workers,
        )?;
        progress(
            ImagingProgressStage::Composing,
            u32::try_from(index + 1).map_err(|_| "a Lâmina contém Frames demais".to_string())?,
            composition_units,
        )?;
    }
    if frame_count == 0 {
        progress(ImagingProgressStage::Composing, 1, composition_units)?;
    }
    for overlay in &sheet.overlays {
        let source = sources.get(&overlay.media_id).ok_or_else(|| {
            format!(
                "a fonte do Decorativo {} não foi carregada",
                overlay.media_id
            )
        })?;
        draw_stretched_media(
            &mut image,
            &overlay.draw_rect,
            overlay.clip_rect.as_ref(),
            raster,
            source,
            workers,
        )?;
    }

    Ok(image)
}

/// A horizontal band of the Sheet raster, addressed in Sheet coordinates.
struct Rows<'a> {
    pixels: &'a mut [u8],
    width: u32,
    first: u32,
}

impl Rows<'_> {
    fn range(&self) -> std::ops::Range<u32> {
        let rows = self.pixels.len() / (self.width as usize * 4);
        self.first..self.first + rows as u32
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        ((y - self.first) as usize * self.width as usize + x as usize) * 4
    }

    fn put(&mut self, x: u32, y: u32, pixel: Rgba<u8>) {
        let offset = self.offset(x, y);
        self.pixels[offset..offset + 4].copy_from_slice(&pixel.0);
    }

    fn get(&self, x: u32, y: u32) -> Rgba<u8> {
        let offset = self.offset(x, y);
        Rgba([
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ])
    }

    fn blend(&mut self, x: u32, y: u32, foreground: Rgba<u8>) {
        if foreground[3] == 0 {
            return;
        }
        if foreground[3] == 255 {
            self.put(x, y, foreground);
            return;
        }
        let background = self.get(x, y);
        // The Sheet starts opaque and every source-over operation preserves that invariant.
        let alpha = u32::from(foreground[3]);
        let channel = |index| {
            ((u32::from(foreground[index]) * alpha
                + u32::from(background[index]) * (255 - alpha)
                + 127)
                / 255) as u8
        };
        self.put(x, y, Rgba([channel(0), channel(1), channel(2), 255]));
    }

    fn blend_frame(&mut self, x: u32, y: u32, mut pixel: Rgba<u8>, opacity: u8) {
        // The ring replaces the Photo in the transparent Frame group; alpha is applied once.
        pixel[3] = ((u16::from(pixel[3]) * u16::from(opacity) + 127) / 255) as u8;
        self.blend(x, y, pixel);
    }
}

/// Paints rows `[top, bottom)` in contiguous bands. The layer never reads a
/// pixel outside the one it writes, so bands are independent.
fn paint_rows(
    image: &mut RgbaImage,
    top: u32,
    bottom: u32,
    columns: u32,
    workers: usize,
    paint: impl Fn(&mut Rows<'_>) + Sync,
) {
    if top >= bottom || columns == 0 {
        return;
    }
    let width = image.width();
    let stride = width as usize * 4;
    let rows = (bottom - top) as usize;
    let pixels: &mut [u8] = image;
    let pixels = &mut pixels[top as usize * stride..bottom as usize * stride];
    let workers = if rows * columns as usize >= MIN_PARALLEL_PIXELS {
        workers.clamp(1, rows)
    } else {
        1
    };
    if workers == 1 {
        paint(&mut Rows {
            pixels,
            width,
            first: top,
        });
        return;
    }
    let band_rows = rows.div_ceil(workers);
    let paint = &paint;
    std::thread::scope(|scope| {
        for (index, band) in pixels.chunks_mut(band_rows * stride).enumerate() {
            scope.spawn(move || {
                paint(&mut Rows {
                    pixels: band,
                    width,
                    first: top + (index * band_rows) as u32,
                })
            });
        }
    });
}

fn draw_frame(
    image: &mut RgbaImage,
    frame: &ComposedFrame,
    pixels_per_micrometer: f64,
    raster: RasterPlan,
    sources: &HashMap<MediaId, RgbaImage>,
    workers: usize,
) -> Result<(), RenderFailure> {
    if frame.opacity_byte == 0 {
        return Ok(());
    }
    let (left, top, right, bottom) = raster_rect(image, &frame.clip_rect, raster)?;
    let border = match &frame.border {
        ProjectedFrameBorder::None => None,
        ProjectedFrameBorder::Solid { rgb, .. } => Some(opaque_rgb(rgb)),
    };
    let border_bounds = frame
        .border_fill_rects
        .iter()
        .map(|rect| raster_rect(image, rect, raster))
        .collect::<Result<Vec<_>, _>>()?;
    let opacity = frame.opacity_byte;
    // Horizontal border spans crossing one row; a pixel is in the ring when any
    // span contains it, exactly as testing every border rectangle.
    let row_borders = |y: u32, spans: &mut Vec<(u32, u32)>| {
        spans.clear();
        spans.extend(
            border_bounds
                .iter()
                .filter(|&&(_, t, _, b)| y >= t && y < b)
                .map(|&(l, _, r, _)| (l, r)),
        );
    };
    let in_border = |x: u32, spans: &[(u32, u32)]| spans.iter().any(|&(l, r)| x >= l && x < r);

    if let Some(photo) = &frame.photo {
        let source = sources
            .get(&photo.media_id)
            .ok_or_else(|| format!("a fonte da mídia {} não foi carregada", photo.media_id))?;
        let draw_left = to_pixels_precise(photo.draw_rect.x, pixels_per_micrometer);
        let draw_top = to_pixels_precise(photo.draw_rect.y, pixels_per_micrometer);
        let draw_width = to_pixels_precise(photo.draw_rect.width, pixels_per_micrometer).max(1.0);
        let draw_height = to_pixels_precise(photo.draw_rect.height, pixels_per_micrometer).max(1.0);
        let draw_center_x = draw_left + draw_width / 2.0;
        let draw_center_y = draw_top + draw_height / 2.0;
        let radians = (photo.rotation_degrees as f64).to_radians();
        let cosine = radians.cos();
        let sine = radians.sin();
        let mirror_x = photo.mirror_x;
        let black_and_white = photo.black_and_white;

        paint_rows(
            image,
            top,
            bottom,
            right.saturating_sub(left),
            workers,
            |rows| {
                let mut spans = Vec::new();
                for y in rows.range() {
                    row_borders(y, &mut spans);
                    for x in left..right {
                        if in_border(x, &spans)
                            && let Some(color) = border
                        {
                            rows.blend_frame(x, y, color, opacity);
                            continue;
                        }
                        let mut delta_x = x as f64 + 0.5 - draw_center_x;
                        let delta_y = y as f64 + 0.5 - draw_center_y;
                        // Invert horizontal mirroring before inverting the Photo's rotation.
                        if mirror_x {
                            delta_x = -delta_x;
                        }
                        let source_x = (cosine * delta_x + sine * delta_y) / draw_width;
                        let source_y = (-sine * delta_x + cosine * delta_y) / draw_height;
                        let horizontal = (source_x + 0.5).clamp(0.0, 1.0) as f32;
                        let vertical = (source_y + 0.5).clamp(0.0, 1.0) as f32;
                        let mut pixel = sample_bilinear(source, horizontal, vertical);
                        if black_and_white {
                            let [r, g, b, alpha] = pixel.0;
                            let luminance =
                                ((54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b) + 128)
                                    / 256) as u8;
                            pixel = Rgba([luminance, luminance, luminance, alpha]);
                        }
                        rows.blend_frame(x, y, pixel, opacity);
                    }
                }
            },
        );
    } else {
        paint_rows(
            image,
            top,
            bottom,
            right.saturating_sub(left),
            workers,
            |rows| {
                let mut spans = Vec::new();
                for y in rows.range() {
                    row_borders(y, &mut spans);
                    for x in left..right {
                        let pixel = if in_border(x, &spans) { border } else { None }
                            .unwrap_or(Rgba([214, 207, 194, 255]));
                        rows.blend_frame(x, y, pixel, opacity);
                    }
                }
            },
        );
    }

    Ok(())
}

fn sample_bilinear(image: &RgbaImage, horizontal: f32, vertical: f32) -> Rgba<u8> {
    let width = image.width();
    let x = horizontal.clamp(0.0, 1.0) * width.saturating_sub(1) as f32;
    let y = vertical.clamp(0.0, 1.0) * image.height().saturating_sub(1) as f32;
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(image.height() - 1);
    let x_amount = x - x0 as f32;
    let y_amount = y - y0 as f32;
    let raw = image.as_raw();
    let pixel = |x: u32, y: u32| {
        let offset = (y as usize * width as usize + x as usize) * 4;
        Rgba([
            raw[offset],
            raw[offset + 1],
            raw[offset + 2],
            raw[offset + 3],
        ])
    };
    let top = blend(pixel(x0, y0), pixel(x1, y0), x_amount);
    let bottom = blend(pixel(x0, y1), pixel(x1, y1), x_amount);
    blend(top, bottom, y_amount)
}

fn draw_stretched_media(
    image: &mut RgbaImage,
    draw_rect: &RectUm,
    clip_rect: Option<&RectUm>,
    raster: RasterPlan,
    source: &RgbaImage,
    workers: usize,
) -> Result<(), RenderFailure> {
    let edges @ (mapped_left, mapped_top, mapped_right, mapped_bottom) =
        raster_edges(draw_rect, raster)?;
    let (left, top, right, bottom) = clamp_raster_edges(image, edges);
    let width = mapped_right.saturating_sub(mapped_left).max(1);
    let height = mapped_bottom.saturating_sub(mapped_top).max(1);
    let (clip_left, clip_top, clip_right, clip_bottom) = clip_rect
        .map(|clip| raster_rect(image, clip, raster))
        .transpose()?
        .unwrap_or((left, top, right, bottom));
    let (first_x, last_x) = (left.max(clip_left), right.min(clip_right));
    paint_rows(
        image,
        top.max(clip_top),
        bottom.min(clip_bottom),
        last_x.saturating_sub(first_x),
        workers,
        |rows| {
            for y in rows.range() {
                for x in first_x..last_x {
                    let horizontal = i64::from(x).saturating_sub(mapped_left) as f32
                        / width.saturating_sub(1).max(1) as f32;
                    let vertical = i64::from(y).saturating_sub(mapped_top) as f32
                        / height.saturating_sub(1).max(1) as f32;
                    rows.blend(x, y, sample_bilinear(source, horizontal, vertical));
                }
            }
        },
    );
    Ok(())
}

fn fill_composed_rect(
    image: &mut RgbaImage,
    draw_rect: &RectUm,
    raster: RasterPlan,
    color: Rgba<u8>,
    workers: usize,
) -> Result<(), RenderFailure> {
    let (left, top, right, bottom) = raster_rect(image, draw_rect, raster)?;
    paint_rows(
        image,
        top,
        bottom,
        right.saturating_sub(left),
        workers,
        |rows| {
            for y in rows.range() {
                for x in left..right {
                    rows.put(x, y, color);
                }
            }
        },
    );
    Ok(())
}

fn raster_rect(
    image: &RgbaImage,
    draw_rect: &RectUm,
    raster: RasterPlan,
) -> Result<(u32, u32, u32, u32), RenderFailure> {
    Ok(clamp_raster_edges(image, raster_edges(draw_rect, raster)?))
}

fn clamp_raster_edges(
    image: &RgbaImage,
    (left, top, right, bottom): (i64, i64, i64, i64),
) -> (u32, u32, u32, u32) {
    let edge = |value: i64, limit: u32| value.clamp(0, i64::from(limit)) as u32;
    (
        edge(left, image.width()),
        edge(top, image.height()),
        edge(right, image.width()),
        edge(bottom, image.height()),
    )
}

fn raster_edges(
    draw_rect: &RectUm,
    raster: RasterPlan,
) -> Result<(i64, i64, i64, i64), RenderFailure> {
    let far_x = draw_rect.x.checked_add(draw_rect.width).ok_or_else(|| {
        RenderFailure::typed(
            ImagingFailureCode::ResourceLimitExceeded,
            None,
            None,
            "a borda horizontal da composição excedeu o intervalo seguro",
        )
    })?;
    let far_y = draw_rect.y.checked_add(draw_rect.height).ok_or_else(|| {
        RenderFailure::typed(
            ImagingFailureCode::ResourceLimitExceeded,
            None,
            None,
            "a borda vertical da composição excedeu o intervalo seguro",
        )
    })?;
    Ok((
        raster.edge_px(draw_rect.x)?,
        raster.edge_px(draw_rect.y)?,
        raster.edge_px(far_x)?,
        raster.edge_px(far_y)?,
    ))
}

fn opaque_rgb(value: &str) -> Rgba<u8> {
    let channel = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .expect("CompositionCore provides canonical RGB")
    };
    Rgba([channel(1), channel(3), channel(5), 255])
}

fn blend(from: Rgba<u8>, to: Rgba<u8>, amount: f32) -> Rgba<u8> {
    let amount = amount.clamp(0.0, 1.0);
    Rgba([
        (from[0] as f32 + (to[0] as f32 - from[0] as f32) * amount) as u8,
        (from[1] as f32 + (to[1] as f32 - from[1] as f32) * amount) as u8,
        (from[2] as f32 + (to[2] as f32 - from[2] as f32) * amount) as u8,
        (from[3] as f32 + (to[3] as f32 - from[3] as f32) * amount) as u8,
    ])
}

fn to_pixels_precise(value_um: i64, pixels_per_micrometer: f64) -> f64 {
    value_um as f64 * pixels_per_micrometer
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, DecorativeScope, DisplayUnit, EndSheetFormat,
        FrameStyleChange, FrameStyleEdit, ImportPhoto, InitialProject, InitialProjectConfiguration,
        PhotoAngleEdit, PhotoOrientationAction, PhotoPlacementMode, PhotoSourceMetadata,
        ProjectCore, ProjectIntent, ProjectLocation, SheetVisualChange,
    };
    use myalbuns_paths::OperationPathContext;

    #[test]
    fn composition_is_identical_for_any_worker_count() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Faixas.myalbuns");
        let mut context = OperationPathContext::new();
        context.capture(&path).unwrap();
        let mut project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(path, context.freeze()),
                InitialProject::configured(InitialProjectConfiguration::new(
                    DisplayUnit::Mm,
                    60_000,
                    30_000,
                    300,
                    0,
                    0,
                    2,
                    EndSheetFormat::Double,
                    EndSheetFormat::Double,
                )),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let photos = [(97, 61), (53, 88)].map(|(width, height)| {
            let raster = RgbImage::from_fn(width, height, |x, y| {
                Rgb([(x * 7) as u8, (y * 5) as u8, (x * y) as u8])
            });
            let imported = project
                .import_photo(ImportPhoto::new(
                    root.path().join(format!("{width}x{height}.png")),
                    PhotoSourceMetadata::new(
                        width,
                        height,
                        ["#111111", "#888888", "#EEEEEE"].map(String::from),
                    )
                    .unwrap(),
                ))
                .unwrap();
            (imported.media_id, raster)
        });
        let sheet_id = project.projection().state.album.sheets[0].id.clone();
        for (media_id, _) in &photos {
            project
                .apply(ProjectIntent::AddPhoto {
                    sheet_id: sheet_id.clone(),
                    media_id: *media_id,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
        }
        let frames = project.projection().state.album.sheets[0]
            .frames
            .iter()
            .map(|frame| frame.id.clone())
            .collect::<Vec<_>>();
        for intent in [
            ProjectIntent::OrientPhotos {
                frame_ids: vec![frames[0].clone()],
                action: PhotoOrientationAction::ToggleHorizontalMirror,
            },
            ProjectIntent::SetPhotoAngle {
                edit: PhotoAngleEdit {
                    frame_ids: vec![frames[0].clone()],
                    angle_tenths: 125,
                },
            },
            ProjectIntent::TogglePhotoBlackAndWhite {
                frame_ids: vec![frames[1].clone()],
            },
            ProjectIntent::SetFrameStyle {
                edit: FrameStyleEdit {
                    frame_ids: frames.clone(),
                    change: FrameStyleChange::BorderWidth { width_um: 1_000 },
                },
            },
            ProjectIntent::SetFrameStyle {
                edit: FrameStyleEdit {
                    frame_ids: vec![frames[0].clone()],
                    change: FrameStyleChange::Opacity {
                        opacity_percent: 65,
                    },
                },
            },
            ProjectIntent::EditSheetVisual {
                sheet_id: sheet_id.clone(),
                scope: DecorativeScope::Left,
                change: SheetVisualChange::BackgroundColor {
                    rgb: "#BBAA88".into(),
                },
            },
        ] {
            project.apply(intent).unwrap();
        }
        let unit = project.render_snapshot().output_unit(&sheet_id).unwrap();
        let sources = photos
            .into_iter()
            .map(|(media_id, raster)| {
                (
                    media_id,
                    image::DynamicImage::ImageRgb8(raster).into_rgba8(),
                )
            })
            .collect::<HashMap<_, _>>();
        let render = |workers| {
            render_unit_with_workers(&unit, 300, &sources, &mut |_, _, _| Ok(()), workers)
                .unwrap_or_else(|failure| panic!("{}", failure.message))
        };
        let serial = render(1);
        assert!(serial.width() as usize * serial.height() as usize > MIN_PARALLEL_PIXELS * 8);
        for workers in [2, 3, 8] {
            assert!(
                render(workers) == serial,
                "{workers} workers changed pixels"
            );
        }
    }
}
