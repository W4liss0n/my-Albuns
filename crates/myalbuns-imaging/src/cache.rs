use std::{
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use image::{DynamicImage, ImageReader, codecs::jpeg::JpegEncoder};
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheCompletion, CacheRequest, CacheResetRequest,
    ImagingResponse,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, PreparedCacheStorage};

use crate::{
    source::{decode_jpeg, jpeg_orientation, read_verified_source, verify_source_current},
    write_response,
};

pub(crate) fn run_cache_reset(
    request: CacheResetRequest,
    app_paths: &AppPaths,
) -> Result<(), String> {
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
    write_response(&ImagingResponse::cache_reset(
        request.request_id,
        removed_count,
    ))
}

pub(crate) fn run_cache(request: CacheRequest, app_paths: &AppPaths) -> Result<(), String> {
    let operation_id = safe_log_identifier(&request.request_id);
    let project_id = safe_log_identifier(&request.project_id);
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        process_id = std::process::id(),
        media_count = request.jobs.len(),
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
    write_response(&ImagingResponse::cache_completed(
        request.request_id,
        completion,
    ))
}

fn build_cache(request: &CacheRequest, app_paths: &AppPaths) -> Result<CacheCompletion, String> {
    let storage = app_paths
        .prepare_cache_storage(&request.cache_paths)
        .map_err(|error| format!("não foi possível preparar o Cache: {error}"))?;

    let mut artifacts = Vec::with_capacity(request.jobs.len());
    let mut generated_count = 0;
    let mut reused_count = 0;
    let mut source_bytes = 0_u64;
    let mut preview_bytes = 0_u64;
    for job in &request.jobs {
        let source = &job.source;
        let operational_source = request
            .root_bindings
            .resolve(source.source_path())
            .map_err(|error| format!("não foi possível aplicar o plano de caminhos: {error}"))?;
        let verified_bytes = read_verified_source(source, &operational_source)?;
        source_bytes = source_bytes
            .checked_add(source.source_bytes())
            .ok_or_else(|| "o tamanho total das Fotos excedeu o limite".to_string())?;
        let preview_path = request
            .cache_paths
            .preview_file(source.media_id(), &job.generation_id)
            .map_err(|error| error.to_string())?;
        let temporary_path = request
            .cache_paths
            .preview_temporary_file(source.media_id(), &job.generation_id, std::process::id())
            .map_err(|error| error.to_string())?;
        let (width_px, height_px, generated, exif_orientation) = prepare_preview(
            &storage,
            source,
            &operational_source,
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
            generation_id: job.generation_id.clone(),
            width_px,
            height_px,
            preview_bytes: artifact_bytes,
            format: CacheArtifactFormat::Jpeg,
            exif_orientation: Some(exif_orientation),
        });
    }

    Ok(CacheCompletion {
        artifacts,
        generated_count,
        reused_count,
        source_bytes,
        preview_bytes,
    })
}

fn prepare_preview(
    storage: &PreparedCacheStorage,
    source: &myalbuns_imaging_protocol::MediaSource,
    operational_source: &Path,
    verified_bytes: &[u8],
    preview_path: &Path,
    temporary_path: &Path,
    max_edge_px: u32,
) -> Result<(u32, u32, bool, u8), String> {
    let exif_orientation = jpeg_orientation(source.media_id(), verified_bytes)?;
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
        return Ok((width, height, false, exif_orientation));
    }

    let decoded = decode_jpeg(source.media_id(), verified_bytes)?;
    let preview = DynamicImage::ImageRgba8(decoded.image)
        .thumbnail(max_edge_px, max_edge_px)
        .to_rgb8();
    let mut publication = storage
        .begin_file_publication(temporary_path, preview_path)
        .map_err(|error| format!("não foi possível criar o Cache temporário: {error}"))?;
    {
        let mut writer = BufWriter::new(&mut publication);
        JpegEncoder::new_with_quality(&mut writer, 84)
            .encode_image(&preview)
            .map_err(|error| format!("não foi possível codificar a prévia JPEG: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("não foi possível finalizar a prévia: {error}"))?;
    }
    let publication = publication
        .sync()
        .map_err(|error| format!("não foi possível sincronizar a prévia: {error}"))?;
    verify_source_current(source, operational_source)?;
    publication
        .publish()
        .map_err(|error| format!("não foi possível publicar a prévia: {error}"))?;
    Ok((
        preview.width(),
        preview.height(),
        true,
        decoded.exif_orientation,
    ))
}
