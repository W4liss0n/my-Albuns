use std::collections::HashMap;

use image::{Rgba, RgbaImage};
use myalbuns_core::{ComposedBackground, ComposedFrame, MediaId, ProjectedFrameBorder, RectUm};
use myalbuns_imaging_protocol::{
    ImagingFailure, ImagingFailureCode, ImagingPathCode, ImagingProgressStage, ImagingRequest,
    RenderCompletion,
};
use myalbuns_paths::ExpectedObject;

use crate::{
    jpeg_output::{JpegFailure, RasterPlan, write_verified},
    source::{MAX_DECODED_SOURCE_PIXELS_TOTAL, open_render_source},
};

const MICROMETERS_PER_INCH: f64 = 25_400.0;

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

pub(crate) fn render_request(
    request: &ImagingRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<RenderCompletion, RenderFailure> {
    let sheet = &request.unit.sheet;
    let raster = RasterPlan::new(sheet.width_um, sheet.height_um, request.dpi)?;
    let pixels_per_micrometer = request.dpi as f64 / MICROMETERS_PER_INCH;
    let (sources, source_bytes) = load_render_sources(request, progress)?;
    let mut image = raster.allocate_rgba(opaque_rgb(&sheet.base.rgb))?;

    for background in &sheet.backgrounds {
        match background {
            ComposedBackground::Color { rgb, draw_rect } => {
                fill_composed_rect(&mut image, draw_rect, raster, opaque_rgb(rgb))?
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
                draw_stretched_media(&mut image, draw_rect, clip_rect.as_ref(), raster, source)?;
            }
        }
    }

    let frame_count = u32::try_from(sheet.frames.len())
        .map_err(|_| "a Lâmina contém Frames demais".to_string())?;
    let composition_units = frame_count.max(1);
    progress(ImagingProgressStage::Composing, 0, composition_units)?;
    for (index, frame) in sheet.frames.iter().enumerate() {
        draw_frame(&mut image, frame, pixels_per_micrometer, raster, &sources)?;
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
        )?;
    }

    progress(ImagingProgressStage::EncodingOutput, 0, 1)?;
    let operational_output = request
        .root_bindings
        .resolve(request.prepared_output_path())
        .map_err(|error| {
            RenderFailure::new(
                ImagingFailureCode::EncodeFailed,
                format!("não foi possível aplicar o plano de caminhos: {error}"),
            )
        })?;
    let verified = write_verified(&image, &operational_output, request.dpi)?;
    progress(ImagingProgressStage::EncodingOutput, 1, 1)?;
    Ok(RenderCompletion {
        width_px: raster.width_px,
        height_px: raster.height_px,
        dpi: request.dpi,
        source_count: sources.len(),
        source_bytes,
        output_bytes: verified.output_bytes,
        output_sha256: verified.output_sha256,
    })
}

fn load_render_sources(
    request: &ImagingRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<(HashMap<MediaId, RgbaImage>, u64), RenderFailure> {
    let mut opened = Vec::new();
    opened
        .try_reserve_exact(request.sources.len())
        .map_err(|_| {
            RenderFailure::typed(
                ImagingFailureCode::ResourceLimitExceeded,
                None,
                None,
                "não há memória suficiente para planejar as fontes da Exportação",
            )
        })?;
    let mut source_bytes = 0_u64;
    let mut source_pixels = 0_u64;
    let source_count = u32::try_from(request.sources.len())
        .map_err(|_| "a Exportação contém fontes demais".to_string())?;
    let loading_units = source_count.max(1);
    progress(ImagingProgressStage::LoadingSources, 0, loading_units)?;
    for source in &request.sources {
        let resolved = request
            .root_bindings
            .resolve_existing(source.source_path(), ExpectedObject::RegularFile)
            .map_err(|error| {
                RenderFailure::typed(
                    ImagingFailureCode::SourceUnavailable,
                    Some(source.media_id().to_string()),
                    Some(ImagingPathCode::from_resolve_error(error)),
                    format!("não foi possível aplicar o plano de caminhos: {error}"),
                )
            })?;
        let opened_source = open_render_source(&resolved).map_err(|failure| {
            RenderFailure::typed(
                failure.code,
                Some(source.media_id().to_string()),
                failure.path_code,
                failure.message,
            )
        })?;
        source_bytes = source_bytes
            .checked_add(opened_source.byte_count())
            .ok_or_else(|| {
                RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(source.media_id().to_string()),
                    None,
                    "o tamanho total das fontes excedeu o limite",
                )
            })?;
        source_pixels = source_pixels
            .checked_add(opened_source.pixel_count().map_err(|failure| {
                RenderFailure::typed(
                    failure.code,
                    Some(source.media_id().to_string()),
                    failure.path_code,
                    failure.message,
                )
            })?)
            .ok_or_else(|| {
                RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(source.media_id().to_string()),
                    None,
                    "a soma dos pixels das fontes excedeu o intervalo seguro",
                )
            })?;
        if source_pixels > MAX_DECODED_SOURCE_PIXELS_TOTAL {
            return Err(RenderFailure::typed(
                ImagingFailureCode::ResourceLimitExceeded,
                Some(source.media_id().to_string()),
                None,
                format!(
                    "as fontes teriam {source_pixels} pixels e excedem o limite de {MAX_DECODED_SOURCE_PIXELS_TOTAL}"
                ),
            ));
        }
        opened.push((source.media_id(), opened_source));
    }

    let mut decoded = HashMap::new();
    decoded.try_reserve(request.sources.len()).map_err(|_| {
        RenderFailure::typed(
            ImagingFailureCode::ResourceLimitExceeded,
            None,
            None,
            "não há memória suficiente para indexar as fontes decodificadas",
        )
    })?;
    for (index, (media_id, opened_source)) in opened.into_iter().enumerate() {
        let image = opened_source.decode().map_err(|failure| {
            RenderFailure::typed(
                failure.code,
                Some(media_id.to_string()),
                failure.path_code,
                failure.message,
            )
        })?;
        decoded.insert(media_id, image);
        progress(
            ImagingProgressStage::LoadingSources,
            u32::try_from(index + 1)
                .map_err(|_| "a Exportação contém fontes demais".to_string())?,
            loading_units,
        )?;
    }
    if source_count == 0 {
        progress(ImagingProgressStage::LoadingSources, 1, loading_units)?;
    }
    Ok((decoded, source_bytes))
}

fn draw_frame(
    image: &mut RgbaImage,
    frame: &ComposedFrame,
    pixels_per_micrometer: f64,
    raster: RasterPlan,
    sources: &HashMap<MediaId, RgbaImage>,
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
    let in_border = |x, y| {
        border_bounds
            .iter()
            .any(|&(l, t, r, b)| x >= l && x < r && y >= t && y < b)
    };

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

        for y in top..bottom {
            for x in left..right {
                if in_border(x, y)
                    && let Some(color) = border
                {
                    blend_frame_pixel(image, x, y, color, frame.opacity_byte);
                    continue;
                }
                let mut delta_x = x as f64 + 0.5 - draw_center_x;
                let delta_y = y as f64 + 0.5 - draw_center_y;
                // Invert horizontal mirroring before inverting the Photo's rotation.
                if photo.mirror_x {
                    delta_x = -delta_x;
                }
                let source_x = (cosine * delta_x + sine * delta_y) / draw_width;
                let source_y = (-sine * delta_x + cosine * delta_y) / draw_height;
                let horizontal = (source_x + 0.5).clamp(0.0, 1.0) as f32;
                let vertical = (source_y + 0.5).clamp(0.0, 1.0) as f32;
                let mut pixel = sample_bilinear(source, horizontal, vertical);
                if photo.black_and_white {
                    let [r, g, b, alpha] = pixel.0;
                    let luminance =
                        ((54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b) + 128) / 256)
                            as u8;
                    pixel = Rgba([luminance, luminance, luminance, alpha]);
                }
                blend_frame_pixel(image, x, y, pixel, frame.opacity_byte);
            }
        }
    } else {
        for y in top..bottom {
            for x in left..right {
                let pixel = if in_border(x, y) { border } else { None }
                    .unwrap_or(Rgba([214, 207, 194, 255]));
                blend_frame_pixel(image, x, y, pixel, frame.opacity_byte);
            }
        }
    }

    Ok(())
}

fn blend_frame_pixel(image: &mut RgbaImage, x: u32, y: u32, mut pixel: Rgba<u8>, opacity: u8) {
    // The ring replaces the Photo in the transparent Frame group; alpha is applied once.
    pixel[3] = ((u16::from(pixel[3]) * u16::from(opacity) + 127) / 255) as u8;
    blend_pixel(image, x, y, pixel);
}

fn sample_bilinear(image: &RgbaImage, horizontal: f32, vertical: f32) -> Rgba<u8> {
    let x = horizontal.clamp(0.0, 1.0) * image.width().saturating_sub(1) as f32;
    let y = vertical.clamp(0.0, 1.0) * image.height().saturating_sub(1) as f32;
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width() - 1);
    let y1 = (y0 + 1).min(image.height() - 1);
    let x_amount = x - x0 as f32;
    let y_amount = y - y0 as f32;
    let top = blend(*image.get_pixel(x0, y0), *image.get_pixel(x1, y0), x_amount);
    let bottom = blend(*image.get_pixel(x0, y1), *image.get_pixel(x1, y1), x_amount);
    blend(top, bottom, y_amount)
}

fn draw_stretched_media(
    image: &mut RgbaImage,
    draw_rect: &RectUm,
    clip_rect: Option<&RectUm>,
    raster: RasterPlan,
    source: &RgbaImage,
) -> Result<(), RenderFailure> {
    let (left, top, right, bottom) = raster_rect(image, draw_rect, raster)?;
    let width = right.saturating_sub(left).max(1);
    let height = bottom.saturating_sub(top).max(1);
    let (clip_left, clip_top, clip_right, clip_bottom) = clip_rect
        .map(|clip| raster_rect(image, clip, raster))
        .transpose()?
        .unwrap_or((left, top, right, bottom));
    for y in top.max(clip_top)..bottom.min(clip_bottom) {
        for x in left.max(clip_left)..right.min(clip_right) {
            let horizontal = (x - left) as f32 / width.saturating_sub(1).max(1) as f32;
            let vertical = (y - top) as f32 / height.saturating_sub(1).max(1) as f32;
            blend_pixel(image, x, y, sample_bilinear(source, horizontal, vertical));
        }
    }
    Ok(())
}

fn fill_composed_rect(
    image: &mut RgbaImage,
    draw_rect: &RectUm,
    raster: RasterPlan,
    color: Rgba<u8>,
) -> Result<(), RenderFailure> {
    let (left, top, right, bottom) = raster_rect(image, draw_rect, raster)?;
    fill_rect(image, left, top, right, bottom, color);
    Ok(())
}

fn raster_rect(
    image: &RgbaImage,
    draw_rect: &RectUm,
    raster: RasterPlan,
) -> Result<(u32, u32, u32, u32), RenderFailure> {
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
    let left = u32::try_from(raster.edge_px(draw_rect.x)?.max(0)).unwrap_or(u32::MAX);
    let top = u32::try_from(raster.edge_px(draw_rect.y)?.max(0)).unwrap_or(u32::MAX);
    let right = u32::try_from(raster.edge_px(far_x)?.max(0)).unwrap_or(u32::MAX);
    let bottom = u32::try_from(raster.edge_px(far_y)?.max(0)).unwrap_or(u32::MAX);
    Ok((
        left.min(image.width()),
        top.min(image.height()),
        right.min(image.width()),
        bottom.min(image.height()),
    ))
}

fn opaque_rgb(value: &str) -> Rgba<u8> {
    let channel = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .expect("CompositionCore provides canonical RGB")
    };
    Rgba([channel(1), channel(3), channel(5), 255])
}

fn fill_rect(image: &mut RgbaImage, left: u32, top: u32, right: u32, bottom: u32, color: Rgba<u8>) {
    for y in top..bottom {
        for x in left..right {
            image.put_pixel(x, y, color);
        }
    }
}

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, foreground: Rgba<u8>) {
    if foreground[3] == 0 {
        return;
    }
    if foreground[3] == 255 {
        image.put_pixel(x, y, foreground);
        return;
    }
    let background = *image.get_pixel(x, y);
    // The Sheet starts opaque and every source-over operation preserves that invariant.
    let alpha = u32::from(foreground[3]);
    let channel = |index| {
        ((u32::from(foreground[index]) * alpha
            + u32::from(background[index]) * (255 - alpha)
            + 127)
            / 255) as u8
    };
    let blended = Rgba([channel(0), channel(1), channel(2), 255]);
    image.put_pixel(x, y, blended);
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
