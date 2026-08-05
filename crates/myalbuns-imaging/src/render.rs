use std::collections::HashMap;

use image::{Rgba, RgbaImage};
use myalbuns_core::{ComposedBackground, ComposedFrame, ProjectedFrameBorder, RectUm};
use myalbuns_imaging_protocol::{
    ImagingFailure, ImagingFailureCode, ImagingFailureStage, ImagingPathCode, ImagingProgressStage,
    ImagingRequest, RenderCompletion,
};
use myalbuns_paths::ExpectedObject;

use crate::{
    jpeg_output::{JpegFailure, RasterPlan, write_verified},
    source::{MAX_DECODED_SOURCE_PIXELS_TOTAL, open_render_source},
};

const MICROMETERS_PER_INCH: f64 = 25_400.0;

pub(crate) struct RenderFailure {
    pub(crate) stage: ImagingFailureStage,
    pub(crate) failure: ImagingFailure,
    pub(crate) message: String,
}

impl RenderFailure {
    fn new(stage: ImagingFailureStage, message: impl Into<String>) -> Self {
        let code = match stage {
            ImagingFailureStage::InvalidRenderRequest => ImagingFailureCode::InvalidRenderRequest,
            ImagingFailureStage::SourceVerification => ImagingFailureCode::SourceUnavailable,
            ImagingFailureStage::SourceDecode => ImagingFailureCode::DecodeFailed,
            ImagingFailureStage::Composition => ImagingFailureCode::CompositionFailed,
            ImagingFailureStage::ResourceLimitExceeded => ImagingFailureCode::ResourceLimitExceeded,
            ImagingFailureStage::OutputPrepare | ImagingFailureStage::OutputEncode => {
                ImagingFailureCode::EncodeFailed
            }
            ImagingFailureStage::OutputVerify => ImagingFailureCode::VerificationFailed,
            ImagingFailureStage::CacheProcessing => ImagingFailureCode::DecodeFailed,
        };
        Self {
            stage,
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
            stage: code.stage(),
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
        Self::new(ImagingFailureStage::Composition, message)
    }
}

impl From<JpegFailure> for RenderFailure {
    fn from(failure: JpegFailure) -> Self {
        Self::new(failure.stage, failure.message)
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
                ..
            } => {
                let source = sources
                    .get(media_id)
                    .ok_or_else(|| format!("a fonte do Background {media_id} não foi carregada"))?;
                draw_stretched_media(&mut image, draw_rect, raster, source)?;
            }
        }
    }

    let frame_count = u32::try_from(sheet.frames.len())
        .map_err(|_| "a Lâmina contém Frames demais".to_string())?;
    let composition_units = frame_count.max(1);
    progress(ImagingProgressStage::Composing, 0, composition_units)?;
    for (index, frame) in sheet.frames.iter().enumerate() {
        draw_frame(&mut image, frame, pixels_per_micrometer, &sources)?;
        draw_frame_border(
            &mut image,
            &frame.clip_rect,
            &request.unit.frame_border,
            pixels_per_micrometer,
        );
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
        draw_stretched_media(&mut image, &overlay.draw_rect, raster, source)?;
    }

    progress(ImagingProgressStage::EncodingOutput, 0, 1)?;
    let operational_output = request
        .root_bindings
        .resolve(request.prepared_output_path())
        .map_err(|error| {
            RenderFailure::new(
                ImagingFailureStage::OutputPrepare,
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
) -> Result<(HashMap<String, RgbaImage>, u64), RenderFailure> {
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
                    Some(source.media_id().to_owned()),
                    Some(ImagingPathCode::from_resolve_error(error)),
                    format!("não foi possível aplicar o plano de caminhos: {error}"),
                )
            })?;
        let opened_source = open_render_source(source, &resolved).map_err(|failure| {
            RenderFailure::typed(
                failure.code,
                Some(source.media_id().to_owned()),
                failure.path_code,
                failure.message,
            )
        })?;
        source_bytes = source_bytes
            .checked_add(source.source_bytes())
            .ok_or_else(|| {
                RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(source.media_id().to_owned()),
                    None,
                    "o tamanho total das fontes excedeu o limite",
                )
            })?;
        source_pixels = source_pixels
            .checked_add(opened_source.pixel_count().map_err(|failure| {
                RenderFailure::typed(
                    failure.code,
                    Some(source.media_id().to_owned()),
                    failure.path_code,
                    failure.message,
                )
            })?)
            .ok_or_else(|| {
                RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(source.media_id().to_owned()),
                    None,
                    "a soma dos pixels das fontes excedeu o intervalo seguro",
                )
            })?;
        if source_pixels > MAX_DECODED_SOURCE_PIXELS_TOTAL {
            return Err(RenderFailure::typed(
                ImagingFailureCode::ResourceLimitExceeded,
                Some(source.media_id().to_owned()),
                None,
                format!(
                    "as fontes teriam {source_pixels} pixels e excedem o limite de {MAX_DECODED_SOURCE_PIXELS_TOTAL}"
                ),
            ));
        }
        opened.push((source.media_id().to_owned(), opened_source));
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
                Some(media_id.clone()),
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
    sources: &HashMap<String, RgbaImage>,
) -> Result<(), String> {
    let left = to_pixels_signed(frame.clip_rect.x, pixels_per_micrometer).max(0) as u32;
    let top = to_pixels_signed(frame.clip_rect.y, pixels_per_micrometer).max(0) as u32;
    let right = to_pixels_signed(
        frame.clip_rect.x + frame.clip_rect.width,
        pixels_per_micrometer,
    )
    .max(0) as u32;
    let bottom = to_pixels_signed(
        frame.clip_rect.y + frame.clip_rect.height,
        pixels_per_micrometer,
    )
    .max(0) as u32;
    let right = right.min(image.width());
    let bottom = bottom.min(image.height());

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
                let delta_x = x as f64 + 0.5 - draw_center_x;
                let delta_y = y as f64 + 0.5 - draw_center_y;
                let mut source_x = (cosine * delta_x + sine * delta_y) / draw_width;
                let source_y = (-sine * delta_x + cosine * delta_y) / draw_height;
                if photo.mirror_x {
                    source_x = -source_x;
                }
                let horizontal = (source_x + 0.5).clamp(0.0, 1.0) as f32;
                let vertical = (source_y + 0.5).clamp(0.0, 1.0) as f32;
                let pixel = sample_bilinear(source, horizontal, vertical);
                image.put_pixel(x, y, pixel);
            }
        }
    } else {
        fill_rect(image, left, top, right, bottom, Rgba([214, 207, 194, 255]));
    }

    Ok(())
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
    raster: RasterPlan,
    source: &RgbaImage,
) -> Result<(), RenderFailure> {
    let (left, top, right, bottom) = raster_rect(image, draw_rect, raster)?;
    let width = right.saturating_sub(left).max(1);
    let height = bottom.saturating_sub(top).max(1);

    for y in top..bottom {
        for x in left..right {
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

fn draw_frame_border(
    image: &mut RgbaImage,
    frame: &RectUm,
    border: &ProjectedFrameBorder,
    pixels_per_micrometer: f64,
) {
    let ProjectedFrameBorder::Solid { rgb, width_um } = border else {
        return;
    };
    let left = (to_pixels_signed(frame.x, pixels_per_micrometer).max(0) as u32).min(image.width());
    let top = (to_pixels_signed(frame.y, pixels_per_micrometer).max(0) as u32).min(image.height());
    let right = (to_pixels_signed(frame.x + frame.width, pixels_per_micrometer).max(0) as u32)
        .min(image.width());
    let bottom = (to_pixels_signed(frame.y + frame.height, pixels_per_micrometer).max(0) as u32)
        .min(image.height());
    if right <= left || bottom <= top {
        return;
    }
    let stroke = ((*width_um as f64 * pixels_per_micrometer).round().max(1.0) as u32)
        .min(right - left)
        .min(bottom - top);
    let color = opaque_rgb(rgb);
    fill_rect(image, left, top, right, (top + stroke).min(bottom), color);
    fill_rect(
        image,
        left,
        bottom.saturating_sub(stroke),
        right,
        bottom,
        color,
    );
    fill_rect(image, left, top, (left + stroke).min(right), bottom, color);
    fill_rect(
        image,
        right.saturating_sub(stroke),
        top,
        right,
        bottom,
        color,
    );
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
    let background = *image.get_pixel(x, y);
    let alpha = foreground[3] as f32 / 255.0;
    let blended = Rgba([
        (foreground[0] as f32 * alpha + background[0] as f32 * (1.0 - alpha)) as u8,
        (foreground[1] as f32 * alpha + background[1] as f32 * (1.0 - alpha)) as u8,
        (foreground[2] as f32 * alpha + background[2] as f32 * (1.0 - alpha)) as u8,
        255,
    ]);
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

fn to_pixels_signed(value_um: i64, pixels_per_micrometer: f64) -> i64 {
    (value_um as f64 * pixels_per_micrometer).round() as i64
}

fn to_pixels_precise(value_um: i64, pixels_per_micrometer: f64) -> f64 {
    value_um as f64 * pixels_per_micrometer
}
