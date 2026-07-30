use std::{
    collections::HashMap,
    fs,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
};

use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder};
use myalbuns_core::ComposedFrame;
use myalbuns_imaging_protocol::{
    ImagingFailureStage, ImagingProgressStage, ImagingRequest, RenderCompletion, RenderSourcePolicy,
};

use crate::source::{decode_jpeg, read_verified_source, sha256_file};

const MICROMETERS_PER_INCH: f64 = 25_400.0;

pub(crate) struct RenderFailure {
    pub(crate) stage: ImagingFailureStage,
    _message: String,
}

impl RenderFailure {
    fn new(stage: ImagingFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            _message: message.into(),
        }
    }
}

impl From<String> for RenderFailure {
    fn from(message: String) -> Self {
        Self::new(ImagingFailureStage::Composition, message)
    }
}

pub(crate) fn render_request(
    request: &ImagingRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<RenderCompletion, RenderFailure> {
    let sheet = request
        .snapshot
        .composition
        .sheets
        .iter()
        .find(|sheet| sheet.sheet_id == request.sheet_id)
        .ok_or_else(|| "a Lâmina solicitada não existe no snapshot".to_string())?;
    let pixels_per_micrometer = request.dpi as f64 / MICROMETERS_PER_INCH;
    let width_px = to_pixels(sheet.width_um, pixels_per_micrometer).max(1);
    let height_px = to_pixels(sheet.height_um, pixels_per_micrometer).max(1);
    let (sources, source_bytes) = load_render_sources(request, progress)?;
    let mut image = RgbaImage::from_pixel(width_px, height_px, Rgba([239, 232, 218, 255]));

    let frame_count = u32::try_from(sheet.frames.len())
        .map_err(|_| "a Lâmina contém Frames demais".to_string())?;
    progress(ImagingProgressStage::Composing, 0, frame_count)?;
    for (index, frame) in sheet.frames.iter().enumerate() {
        draw_frame(
            &mut image,
            frame,
            pixels_per_micrometer,
            request.source_policy,
            &sources,
        )?;
        progress(
            ImagingProgressStage::Composing,
            u32::try_from(index + 1).map_err(|_| "a Lâmina contém Frames demais".to_string())?,
            frame_count,
        )?;
    }
    draw_vertical_line(&mut image, width_px / 2, Rgba([129, 112, 91, 90]));
    if sheet.has_overlay {
        draw_overlay(&mut image);
    }

    progress(ImagingProgressStage::EncodingOutput, 0, 1)?;
    let operational_output = request
        .root_bindings
        .resolve(&request.prepared_output_path)
        .map_err(|error| {
            RenderFailure::new(
                ImagingFailureStage::OutputPrepare,
                format!("não foi possível aplicar o plano de caminhos: {error}"),
            )
        })?;
    let (output_bytes, output_sha256) = write_verified_png(&image, &operational_output)?;
    progress(ImagingProgressStage::EncodingOutput, 1, 1)?;
    Ok(RenderCompletion {
        width_px,
        height_px,
        dpi: request.dpi,
        source_count: sources.len(),
        source_bytes,
        output_bytes,
        output_sha256,
    })
}

fn load_render_sources(
    request: &ImagingRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<(HashMap<String, RgbaImage>, u64), RenderFailure> {
    if request.source_policy == RenderSourcePolicy::ProceduralFixture {
        progress(ImagingProgressStage::LoadingSources, 1, 1)?;
        return Ok((HashMap::new(), 0));
    }

    let mut decoded = HashMap::with_capacity(request.sources.len());
    let mut source_bytes = 0_u64;
    let source_count = u32::try_from(request.sources.len())
        .map_err(|_| "a Exportação contém fontes demais".to_string())?;
    progress(ImagingProgressStage::LoadingSources, 0, source_count)?;
    for (index, source) in request.sources.iter().enumerate() {
        let operational_source = request
            .root_bindings
            .resolve(source.source_path())
            .map_err(|error| {
                RenderFailure::new(
                    ImagingFailureStage::SourceVerification,
                    format!("não foi possível aplicar o plano de caminhos: {error}"),
                )
            })?;
        let verified = read_verified_source(source, &operational_source)
            .map_err(|error| RenderFailure::new(ImagingFailureStage::SourceVerification, error))?;
        source_bytes = source_bytes
            .checked_add(source.source_bytes())
            .ok_or_else(|| {
                RenderFailure::new(
                    ImagingFailureStage::SourceVerification,
                    "o tamanho total das fontes excedeu o limite",
                )
            })?;
        decoded.insert(
            source.media_id().to_owned(),
            decode_jpeg(source.media_id(), &verified)
                .map_err(|error| RenderFailure::new(ImagingFailureStage::SourceDecode, error))?
                .image,
        );
        progress(
            ImagingProgressStage::LoadingSources,
            u32::try_from(index + 1)
                .map_err(|_| "a Exportação contém fontes demais".to_string())?,
            source_count,
        )?;
    }
    Ok((decoded, source_bytes))
}

fn write_verified_png(
    image: &RgbaImage,
    prepared_output_path: &Path,
) -> Result<(u64, String), RenderFailure> {
    let mut created = false;
    let write_result = (|| -> Result<(u64, String), RenderFailure> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(prepared_output_path)
            .map_err(|error| {
                RenderFailure::new(
                    ImagingFailureStage::OutputPrepare,
                    format!("não foi possível criar a preparação da Exportação: {error}"),
                )
            })?;
        created = true;
        let mut writer = BufWriter::new(file);
        PngEncoder::new(&mut writer)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| {
                RenderFailure::new(
                    ImagingFailureStage::OutputEncode,
                    format!("não foi possível codificar a imagem exportada: {error}"),
                )
            })?;
        writer.flush().map_err(|error| {
            RenderFailure::new(
                ImagingFailureStage::OutputEncode,
                format!("não foi possível finalizar a imagem exportada: {error}"),
            )
        })?;
        let file = writer.into_inner().map_err(|error| {
            RenderFailure::new(
                ImagingFailureStage::OutputEncode,
                format!("não foi possível finalizar a imagem exportada: {error}"),
            )
        })?;
        file.sync_all().map_err(|error| {
            RenderFailure::new(
                ImagingFailureStage::OutputEncode,
                format!("não foi possível sincronizar a imagem exportada: {error}"),
            )
        })?;
        drop(file);
        let output_bytes = fs::metadata(prepared_output_path)
            .map_err(|error| {
                RenderFailure::new(
                    ImagingFailureStage::OutputVerify,
                    format!("não foi possível verificar a imagem preparada: {error}"),
                )
            })?
            .len();
        let output_sha256 = sha256_file(prepared_output_path)
            .map_err(|error| RenderFailure::new(ImagingFailureStage::OutputVerify, error))?;
        Ok((output_bytes, output_sha256))
    })();
    if write_result.is_err() && created {
        let _ = fs::remove_file(prepared_output_path);
    }
    write_result
}

fn draw_frame(
    image: &mut RgbaImage,
    frame: &ComposedFrame,
    pixels_per_micrometer: f64,
    source_policy: RenderSourcePolicy,
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
        let colors = photo.palette.each_ref().map(|value| parse_hex_color(value));
        let linked_source = match source_policy {
            RenderSourcePolicy::LinkedOriginals => {
                Some(sources.get(&photo.media_id).ok_or_else(|| {
                    format!("a fonte da mídia {} não foi carregada", photo.media_id)
                })?)
            }
            RenderSourcePolicy::ProceduralFixture => None,
        };
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
                let pixel = if let Some(source) = linked_source {
                    sample_bilinear(source, horizontal, vertical)
                } else {
                    let horizon = blend(colors[0], colors[1], horizontal);
                    blend(horizon, colors[2], (vertical * 0.42).min(1.0))
                };
                image.put_pixel(x, y, pixel);
            }
        }
    } else {
        fill_rect(image, left, top, right, bottom, Rgba([214, 207, 194, 255]));
    }

    stroke_rect(image, left, top, right, bottom, Rgba([255, 255, 255, 220]));
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

fn draw_overlay(image: &mut RgbaImage) {
    let band = (image.height() / 18).max(2);
    for y in 0..band {
        let alpha = ((1.0 - y as f32 / band as f32) * 90.0) as u8;
        for x in 0..image.width() {
            blend_pixel(image, x, y, Rgba([23, 36, 45, alpha]));
            let bottom_y = image.height() - 1 - y;
            blend_pixel(image, x, bottom_y, Rgba([23, 36, 45, alpha]));
        }
    }
}

fn fill_rect(image: &mut RgbaImage, left: u32, top: u32, right: u32, bottom: u32, color: Rgba<u8>) {
    for y in top..bottom {
        for x in left..right {
            image.put_pixel(x, y, color);
        }
    }
}

fn stroke_rect(
    image: &mut RgbaImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    color: Rgba<u8>,
) {
    if right <= left || bottom <= top {
        return;
    }
    for x in left..right {
        image.put_pixel(x, top, color);
        image.put_pixel(x, bottom - 1, color);
    }
    for y in top..bottom {
        image.put_pixel(left, y, color);
        image.put_pixel(right - 1, y, color);
    }
}

fn draw_vertical_line(image: &mut RgbaImage, x: u32, color: Rgba<u8>) {
    if x >= image.width() {
        return;
    }
    for y in 0..image.height() {
        blend_pixel(image, x, y, color);
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
        255,
    ])
}

fn parse_hex_color(value: &str) -> Rgba<u8> {
    let value = value.trim_start_matches('#');
    if value.len() != 6 {
        return Rgba([127, 127, 127, 255]);
    }
    let red = u8::from_str_radix(&value[0..2], 16).unwrap_or(127);
    let green = u8::from_str_radix(&value[2..4], 16).unwrap_or(127);
    let blue = u8::from_str_radix(&value[4..6], 16).unwrap_or(127);
    Rgba([red, green, blue, 255])
}

fn to_pixels(value_um: i64, pixels_per_micrometer: f64) -> u32 {
    (value_um as f64 * pixels_per_micrometer).round().max(0.0) as u32
}

fn to_pixels_signed(value_um: i64, pixels_per_micrometer: f64) -> i64 {
    (value_um as f64 * pixels_per_micrometer).round() as i64
}

fn to_pixels_precise(value_um: i64, pixels_per_micrometer: f64) -> f64 {
    value_um as f64 * pixels_per_micrometer
}
