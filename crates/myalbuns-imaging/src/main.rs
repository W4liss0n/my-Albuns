use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Rgba,
    RgbaImage, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder,
};
use myalbuns_core::ComposedFrame;
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheCompletion, CacheRequest, CacheResetRequest, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingRequest, ImagingResponse, MediaSource, RenderCompletion,
    RenderSourcePolicy,
};
use myalbuns_logging::{
    ProcessRole, init_local_logging, safe_log_identifier, sidecar_log_directory,
};
use myalbuns_paths::{AppPaths, PreparedCacheStorage};
use sha2::{Digest, Sha256};

const MICROMETERS_PER_INCH: f64 = 25_400.0;
const CACHE_REPRESENTATION_VERSION: u32 = 1;

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
    let exit_code = if run(&app_paths).is_err() {
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

fn run(app_paths: &AppPaths) -> Result<(), String> {
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
    if let Ok(command) = serde_json::from_str(&source) {
        return match command {
            ImagingCommand::BuildCache(request) => run_cache(request, app_paths),
            ImagingCommand::ResetCache(request) => run_cache_reset(request, app_paths),
        };
    }
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

    request.validate().inspect_err(|_| {
        tracing::warn!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_request_rejected",
        );
    })?;

    let completion = render_request(&request).inspect_err(|_| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "imaging_render_failed",
        );
    })?;
    let response = ImagingResponse::completed(request.request_id.clone(), completion.clone());
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
        width_px = completion.width_px,
        height_px = completion.height_px,
        dpi = completion.dpi,
        source_count = completion.source_count,
        source_bytes = completion.source_bytes,
        output_bytes = completion.output_bytes,
        output_sha256 = completion.output_sha256.as_str(),
    );
    Ok(())
}

fn run_cache_reset(request: CacheResetRequest, app_paths: &AppPaths) -> Result<(), String> {
    let operation_id = safe_log_identifier(&request.request_id);
    request.validate()?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_count = request.project_ids.len(),
        event = "cache_reset_started",
    );
    let mut removed_count = 0;
    for project_id in &request.project_ids {
        let paths = app_paths
            .project_cache(project_id)
            .map_err(|error| error.to_string())?;
        removed_count += usize::from(
            app_paths
                .clear_project_cache(&paths)
                .map_err(|error| error.to_string())?,
        );
    }
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_count = request.project_ids.len(),
        removed_count,
        event = "cache_reset_completed",
    );
    let response = ImagingResponse::cache_reset(request.request_id, removed_count);
    serde_json::to_writer(std::io::stdout(), &response)
        .map_err(|error| format!("não foi possível responder: {error}"))
}

fn run_cache(request: CacheRequest, app_paths: &AppPaths) -> Result<(), String> {
    let operation_id = safe_log_identifier(&request.request_id);
    let project_id = safe_log_identifier(&request.project_id);
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        media_count = request.sources.len(),
        event = "cache_request_started",
    );
    request.validate()?;
    let completion = build_cache(&request, app_paths).inspect_err(|_| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            event = "cache_request_failed",
        );
    })?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        generated_count = completion.generated_count,
        reused_count = completion.reused_count,
        source_bytes = completion.source_bytes,
        preview_bytes = completion.preview_bytes,
        event = "cache_request_completed",
    );
    let response = ImagingResponse::cache_completed(request.request_id, completion);
    serde_json::to_writer(std::io::stdout(), &response)
        .map_err(|error| format!("não foi possível responder: {error}"))
}

fn build_cache(request: &CacheRequest, app_paths: &AppPaths) -> Result<CacheCompletion, String> {
    let storage = app_paths
        .prepare_cache_storage(&request.cache_paths)
        .map_err(|error| format!("não foi possível preparar o Cache: {error}"))?;

    let mut artifacts = Vec::with_capacity(request.sources.len());
    let mut generated_count = 0;
    let mut reused_count = 0;
    let mut source_bytes = 0_u64;
    let mut preview_bytes = 0_u64;
    for source in &request.sources {
        let verified_bytes = read_verified_source(source)?;
        source_bytes = source_bytes
            .checked_add(source.source_bytes())
            .ok_or_else(|| "o tamanho total das Fotos excedeu o limite".to_string())?;
        let generation_id = format!(
            "{}-v{}-{}",
            source.source_sha256()[..16].to_ascii_lowercase(),
            CACHE_REPRESENTATION_VERSION,
            request.max_edge_px
        );
        let preview_path = request
            .cache_paths
            .preview_file(source.media_id(), &generation_id)
            .map_err(|error| error.to_string())?;
        let temporary_path = request
            .cache_paths
            .preview_temporary_file(source.media_id(), &generation_id, std::process::id())
            .map_err(|error| error.to_string())?;
        let (width_px, height_px, generated) = prepare_preview(
            &storage,
            source,
            &verified_bytes,
            &preview_path,
            &temporary_path,
            request.max_edge_px,
        )?;
        if generated {
            generated_count += 1;
        } else {
            reused_count += 1;
        }
        let artifact_bytes = storage
            .open_existing_file(&preview_path)
            .map_err(|error| format!("representação reduzida indisponível: {error}"))?
            .ok_or_else(|| "representação reduzida indisponível".to_string())?
            .metadata()
            .map_err(|error| format!("representação reduzida indisponível: {error}"))?
            .len();
        preview_bytes = preview_bytes
            .checked_add(artifact_bytes)
            .ok_or_else(|| "o tamanho total do Cache excedeu o limite".to_string())?;
        artifacts.push(CacheArtifact {
            media_id: source.media_id().to_owned(),
            generation_id,
            width_px,
            height_px,
            preview_bytes: artifact_bytes,
        });
    }

    write_cache_metadata(&storage, request, &artifacts)?;
    Ok(CacheCompletion {
        artifacts,
        generated_count,
        reused_count,
        source_bytes,
        preview_bytes,
    })
}

fn read_verified_source(source: &MediaSource) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(source.source_path()).map_err(|error| {
        format!(
            "não foi possível abrir a mídia {}: {error}",
            source.media_id()
        )
    })?;
    if !metadata.is_file() || metadata.len() != source.source_bytes() {
        return Err(source_changed_error(source));
    }
    let bytes = fs::read(source.source_path())
        .map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    if bytes.len() as u64 != source.source_bytes()
        || !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(bytes)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let file =
        File::open(path).map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_source_current(source: &MediaSource) -> Result<(), String> {
    let metadata = fs::metadata(source.source_path()).map_err(|_| source_changed_error(source))?;
    if !metadata.is_file()
        || metadata.len() != source.source_bytes()
        || !sha256_file(source.source_path())?.eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(())
}

fn source_changed_error(source: &MediaSource) -> String {
    format!(
        "a mídia {} mudou desde o planejamento do Cache",
        source.media_id()
    )
}

fn prepare_preview(
    storage: &PreparedCacheStorage,
    source: &MediaSource,
    verified_bytes: &[u8],
    preview_path: &Path,
    temporary_path: &Path,
    max_edge_px: u32,
) -> Result<(u32, u32, bool), String> {
    if let Some(file) = storage
        .open_existing_file(preview_path)
        .map_err(|error| format!("representação reduzida inválida: {error}"))?
    {
        let reader = ImageReader::new(BufReader::new(file))
            .with_guessed_format()
            .map_err(|error| format!("representação reduzida inválida: {error}"))?;
        let (width, height) = reader
            .into_dimensions()
            .map_err(|error| format!("representação reduzida inválida: {error}"))?;
        return Ok((width, height, false));
    }

    let preview = DynamicImage::ImageRgba8(decode_jpeg(source.media_id(), verified_bytes)?)
        .thumbnail(max_edge_px, max_edge_px)
        .to_rgb8();
    let write_result = (|| -> Result<(), String> {
        let file = storage
            .create_temporary_file(temporary_path)
            .map_err(|error| format!("não foi possível criar o Cache temporário: {error}"))?;
        let mut writer = BufWriter::new(file);
        JpegEncoder::new_with_quality(&mut writer, 84)
            .encode_image(&preview)
            .map_err(|error| format!("não foi possível codificar a prévia JPEG: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("não foi possível finalizar a prévia: {error}"))?;
        let file = writer
            .into_inner()
            .map_err(|error| format!("não foi possível finalizar a prévia: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("não foi possível sincronizar a prévia: {error}"))?;
        verify_source_current(source)?;
        storage
            .replace_file(temporary_path, preview_path)
            .map_err(|error| format!("não foi possível publicar a prévia: {error}"))
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    write_result?;
    Ok((preview.width(), preview.height(), true))
}

fn write_cache_metadata(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    artifacts: &[CacheArtifact],
) -> Result<(), String> {
    let entries = artifacts
        .iter()
        .zip(&request.sources)
        .map(|(artifact, source)| -> Result<serde_json::Value, String> {
            let artifact_path = request
                .cache_paths
                .preview_file(&artifact.media_id, &artifact.generation_id)
                .map_err(|error| error.to_string())?;
            let artifact_name = artifact_path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| "o nome do artefato de Cache é inválido".to_string())?;
            Ok(serde_json::json!({
                "mediaId": artifact.media_id,
                "generationId": artifact.generation_id,
                "artifactName": artifact_name,
                "widthPx": artifact.width_px,
                "heightPx": artifact.height_px,
                "previewBytes": artifact.preview_bytes,
                "sourceBytes": source.source_bytes(),
                "sourceSha256": source.source_sha256(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = serde_json::json!({
        "schemaVersion": 1,
        "representationVersion": CACHE_REPRESENTATION_VERSION,
        "projectId": request.project_id,
        "maxEdgePx": request.max_edge_px,
        "format": "jpeg",
        "entries": entries,
    });
    let metadata_path = request.cache_paths.metadata_file();
    let temporary_path = request
        .cache_paths
        .metadata_temporary_file(std::process::id());
    let metadata_bytes = serde_json::to_vec_pretty(&metadata)
        .map_err(|error| format!("não foi possível serializar o índice: {error}"))?;
    let mut temporary = storage
        .create_temporary_file(&temporary_path)
        .map_err(|error| format!("não foi possível criar o índice temporário: {error}"))?;
    temporary
        .write_all(&metadata_bytes)
        .map_err(|error| format!("não foi possível gravar o índice temporário: {error}"))?;
    temporary
        .sync_all()
        .map_err(|error| format!("não foi possível sincronizar o índice: {error}"))?;
    drop(temporary);
    storage
        .replace_file(&temporary_path, &metadata_path)
        .map_err(|error| format!("não foi possível publicar o índice: {error}"))
}

fn render_request(request: &ImagingRequest) -> Result<RenderCompletion, String> {
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
    let (sources, source_bytes) = load_render_sources(request)?;
    let mut image = RgbaImage::from_pixel(width_px, height_px, Rgba([239, 232, 218, 255]));

    for frame in &sheet.frames {
        draw_frame(
            &mut image,
            frame,
            pixels_per_micrometer,
            request.source_policy,
            &sources,
        )?;
    }
    draw_vertical_line(&mut image, width_px / 2, Rgba([129, 112, 91, 90]));
    if sheet.has_overlay {
        draw_overlay(&mut image);
    }

    publish_png_atomically(&image, &request.output_path, &request.request_id)?;
    let output_bytes = fs::metadata(&request.output_path)
        .map_err(|error| format!("não foi possível verificar a imagem exportada: {error}"))?
        .len();
    let output_sha256 = sha256_file(&request.output_path)?;
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
) -> Result<(HashMap<String, RgbaImage>, u64), String> {
    if request.source_policy == RenderSourcePolicy::ProceduralFixture {
        return Ok((HashMap::new(), 0));
    }

    let mut decoded = HashMap::with_capacity(request.sources.len());
    let mut source_bytes = 0_u64;
    for source in &request.sources {
        let verified = read_verified_source(source)?;
        source_bytes = source_bytes
            .checked_add(source.source_bytes())
            .ok_or_else(|| "o tamanho total das fontes excedeu o limite".to_string())?;
        decoded.insert(
            source.media_id().to_owned(),
            decode_jpeg(source.media_id(), &verified)?,
        );
    }
    Ok((decoded, source_bytes))
}

fn decode_jpeg(media_id: &str, verified_bytes: &[u8]) -> Result<RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(verified_bytes))
        .with_guessed_format()
        .map_err(|error| format!("não foi possível inspecionar a Foto {media_id}: {error}"))?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Err(format!("a mídia {media_id} não é JPEG"));
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("não foi possível preparar o decoder JPEG: {error}"))?;
    let orientation = decoder
        .orientation()
        .map_err(|error| format!("não foi possível ler a orientação EXIF: {error}"))?;
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("não foi possível decodificar a Foto: {error}"))?;
    decoded.apply_orientation(orientation);
    Ok(decoded.to_rgba8())
}

fn publish_png_atomically(
    image: &RgbaImage,
    output_path: &Path,
    request_id: &str,
) -> Result<(), String> {
    let parent = output_path
        .parent()
        .ok_or_else(|| "o destino da Exportação não possui uma pasta".to_string())?;
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "o nome da Exportação é inválido".to_string())?;
    let temporary_path = parent.join(format!(".{file_name}.{request_id}.tmp"));
    let write_result = (|| -> Result<(), String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("não foi possível criar a Exportação temporária: {error}"))?;
        let mut writer = BufWriter::new(file);
        PngEncoder::new(&mut writer)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| format!("não foi possível codificar a imagem exportada: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("não foi possível finalizar a imagem exportada: {error}"))?;
        let file = writer
            .into_inner()
            .map_err(|error| format!("não foi possível finalizar a imagem exportada: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("não foi possível sincronizar a imagem exportada: {error}"))?;
        drop(file);
        fs::rename(&temporary_path, output_path)
            .map_err(|error| format!("não foi possível publicar a imagem exportada: {error}"))
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
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
