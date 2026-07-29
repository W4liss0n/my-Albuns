use std::io::BufRead;
use std::path::Path;
use std::process::ExitCode;

use image::{ImageFormat, Rgba, RgbaImage};
use myalbuns_core::{ComposedFrame, RenderSnapshot};
use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingRequest, ImagingResponse};
use myalbuns_logging::{
    ProcessRole, init_local_logging, safe_log_identifier, sidecar_log_directory,
};
use myalbuns_paths::AppPaths;

const PIXELS_PER_MICROMETER: f64 = 0.001;

fn main() -> ExitCode {
    let process_role = ProcessRole::Imaging;
    let app_paths = match AppPaths::discover() {
        Ok(app_paths) => app_paths,
        Err(error) => {
            eprintln!("pastas de dados do aplicativo indisponíveis: {error}");
            return ExitCode::FAILURE;
        }
    };
    let log_directory = sidecar_log_directory(&app_paths);
    let logging_guard = match init_local_logging(&log_directory, process_role) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("logging indisponível: {error}");
            None
        }
    };
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = process_role.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        event = "imaging_process_started",
    );
    let exit_code = if run().is_err() {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = process_role.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            event = "imaging_process_failed",
        );
        eprintln!("o Processador de Imagens não concluiu a solicitação.");
        ExitCode::FAILURE
    } else {
        tracing::info!(
            target: "myalbuns.imaging",
            process_role = process_role.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            event = "imaging_process_stopped",
            success = true,
        );
        ExitCode::SUCCESS
    };
    drop(logging_guard);
    exit_code
}

fn run() -> Result<(), String> {
    let mut source = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut source)
        .map_err(|error| {
            tracing::error!(
                target: "myalbuns.imaging",
                process_role = ProcessRole::Imaging.as_str(),
                protocol_version = IMAGING_PROTOCOL_VERSION,
                event = "imaging_request_read_failed",
            );
            format!("não foi possível ler a solicitação: {error}")
        })?;
    let request: ImagingRequest = serde_json::from_str(&source).map_err(|error| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = IMAGING_PROTOCOL_VERSION,
            event = "imaging_request_decode_failed",
        );
        format!("solicitação de renderização inválida: {error}")
    })?;
    let operation_id = safe_log_identifier(&request.request_id);
    let project_id = safe_log_identifier(&request.snapshot.project_id);
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        event = "imaging_request_started",
    );

    if request.protocol_version != IMAGING_PROTOCOL_VERSION {
        tracing::warn!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_protocol_rejected",
        );
        return Err(format!(
            "versão de protocolo não suportada: {}",
            request.protocol_version
        ));
    }
    request.snapshot.validate().map_err(|error| {
        tracing::warn!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_snapshot_rejected",
        );
        format!("snapshot inválido: {error}")
    })?;

    let (width_px, height_px) = render_first_sheet(&request.snapshot, &request.output_path)
        .inspect_err(|_| {
            tracing::error!(
                target: "myalbuns.imaging",
                process_role = ProcessRole::Imaging.as_str(),
                protocol_version = request.protocol_version,
                operation_id,
                project_id,
                event = "imaging_render_failed",
            );
        })?;
    let response = ImagingResponse::completed(request.request_id.clone(), width_px, height_px);
    serde_json::to_writer(std::io::stdout(), &response).map_err(|error| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_response_write_failed",
        );
        format!("não foi possível responder: {error}")
    })?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        event = "imaging_request_completed",
        width_px,
        height_px,
    );
    Ok(())
}

fn render_first_sheet(snapshot: &RenderSnapshot, output_path: &Path) -> Result<(u32, u32), String> {
    let sheet = snapshot
        .composition
        .sheets
        .first()
        .ok_or_else(|| "o snapshot não contém Lâminas".to_string())?;
    let width_px = to_pixels(sheet.width_um).max(1);
    let height_px = to_pixels(sheet.height_um).max(1);
    let mut image = RgbaImage::from_pixel(width_px, height_px, Rgba([239, 232, 218, 255]));

    for frame in &sheet.frames {
        draw_frame(&mut image, frame);
    }
    draw_vertical_line(&mut image, width_px / 2, Rgba([129, 112, 91, 90]));
    if sheet.has_overlay {
        draw_overlay(&mut image);
    }

    image
        .save_with_format(output_path, ImageFormat::Png)
        .map_err(|_| "não foi possível gravar a imagem exportada.".to_string())?;
    Ok((width_px, height_px))
}

fn draw_frame(image: &mut RgbaImage, frame: &ComposedFrame) {
    let left = to_pixels_signed(frame.clip_rect.x).max(0) as u32;
    let top = to_pixels_signed(frame.clip_rect.y).max(0) as u32;
    let right = to_pixels_signed(frame.clip_rect.x + frame.clip_rect.width).max(0) as u32;
    let bottom = to_pixels_signed(frame.clip_rect.y + frame.clip_rect.height).max(0) as u32;
    let right = right.min(image.width());
    let bottom = bottom.min(image.height());

    if let Some(photo) = &frame.photo {
        let colors = photo.palette.each_ref().map(|value| parse_hex_color(value));
        let draw_left = to_pixels_precise(photo.draw_rect.x);
        let draw_top = to_pixels_precise(photo.draw_rect.y);
        let draw_width = to_pixels_precise(photo.draw_rect.width).max(1.0);
        let draw_height = to_pixels_precise(photo.draw_rect.height).max(1.0);
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
                let horizon = blend(colors[0], colors[1], horizontal);
                let pixel = blend(horizon, colors[2], (vertical * 0.42).min(1.0));
                image.put_pixel(x, y, pixel);
            }
        }
    } else {
        fill_rect(image, left, top, right, bottom, Rgba([214, 207, 194, 255]));
    }

    stroke_rect(image, left, top, right, bottom, Rgba([255, 255, 255, 220]));
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

fn to_pixels(value_um: i64) -> u32 {
    (value_um as f64 * PIXELS_PER_MICROMETER).round().max(0.0) as u32
}

fn to_pixels_signed(value_um: i64) -> i64 {
    (value_um as f64 * PIXELS_PER_MICROMETER).round() as i64
}

fn to_pixels_precise(value_um: i64) -> f64 {
    value_um as f64 * PIXELS_PER_MICROMETER
}
