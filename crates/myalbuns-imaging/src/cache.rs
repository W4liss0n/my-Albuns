use std::io::{BufReader, BufWriter, Write};

use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheCompletion, CacheFingerprint, CacheJob, CacheRequest,
    CacheReusableGeneration, ImagingResponse, root_binding_plan_sha256,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, ExpectedObject, PreparedCacheStorage};

use crate::{
    source::{
        MAX_DECODED_SOURCE_PIXELS_TOTAL, fingerprint_source, open_cache_source,
        verify_source_fingerprint,
    },
    write_response,
};

const SRGB_PROFILE: &[u8] = include_bytes!("../assets/sRGB2014.icc");

pub(crate) fn run_cache(request: CacheRequest, app_paths: &AppPaths) -> Result<(), String> {
    let operation_id = safe_log_identifier(&request.request_id);
    let project_id = safe_log_identifier(&request.project_id);
    let root_binding_plan_sha256 = root_binding_plan_sha256(&request.root_bindings)?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        process_id = std::process::id(),
        media_count = request.jobs.len(),
        root_binding_plan_sha256,
        event = "cache_request_started",
    );
    request.validate()?;
    let completion = build_cache(&request, app_paths).inspect_err(|error| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            error,
            event = "cache_request_failed",
        );
    })?;
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        process_id = std::process::id(),
        generated_count = completion.generated_count,
        reused_count = completion.reused_count,
        source_bytes = completion.source_bytes,
        preview_bytes = completion.preview_bytes,
        root_binding_plan_sha256,
        event = "cache_request_completed",
    );
    write_response(&ImagingResponse::cache_completed(
        request.request_id,
        completion,
    ))
}

fn build_cache(request: &CacheRequest, app_paths: &AppPaths) -> Result<CacheCompletion, String> {
    if request.policy.max_decoded_pixels != MAX_DECODED_SOURCE_PIXELS_TOTAL {
        return Err("a política de decode diverge do limite comum do Processador".into());
    }
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
        let resolved = request
            .root_bindings
            .resolve_existing(source.source_path(), ExpectedObject::RegularFile)
            .map_err(|error| {
                format!(
                    "não foi possível abrir a mídia {} pelo plano da operação: {error}",
                    source.media_id()
                )
            })?;
        let fingerprint = fingerprint_source(source.media_id(), &resolved)?;
        source_bytes = source_bytes
            .checked_add(fingerprint.source_bytes)
            .ok_or_else(|| "o tamanho total das mídias excedeu o limite".to_string())?;

        let artifact = if let Some(reusable) = job
            .reusable
            .as_ref()
            .filter(|reusable| reusable.fingerprint == fingerprint)
            .filter(|reusable| {
                validate_existing_preview(
                    &storage,
                    &request.cache_paths,
                    source.media_id(),
                    reusable,
                )
                .unwrap_or(false)
            }) {
            verify_source_fingerprint(
                source.media_id(),
                &request.root_bindings,
                source.source_path(),
                &fingerprint,
            )?;
            reused_count += 1;
            artifact_from_reusable(source.media_id(), reusable)
        } else {
            let artifact = generate_preview(&storage, request, job, &resolved, fingerprint)?;
            generated_count += 1;
            artifact
        };
        preview_bytes = preview_bytes
            .checked_add(artifact.preview_bytes)
            .ok_or_else(|| "o tamanho total do Cache excedeu o limite".to_string())?;
        artifacts.push(artifact);
    }

    Ok(CacheCompletion {
        artifacts,
        generated_count,
        reused_count,
        source_bytes,
        preview_bytes,
    })
}

fn artifact_from_reusable(media_id: &str, reusable: &CacheReusableGeneration) -> CacheArtifact {
    CacheArtifact {
        media_id: media_id.to_owned(),
        generation_id: reusable.generation_id.clone(),
        width_px: reusable.width_px,
        height_px: reusable.height_px,
        preview_bytes: reusable.preview_bytes,
        format: reusable.format,
        exif_orientation: reusable.exif_orientation,
        source_page_count: reusable.source_page_count,
        basic_color_profile: reusable.basic_color_profile,
        fingerprint: reusable.fingerprint.clone(),
    }
}

fn validate_existing_preview(
    storage: &PreparedCacheStorage,
    cache_paths: &myalbuns_paths::CachePathPlan,
    media_id: &str,
    reusable: &CacheReusableGeneration,
) -> Result<bool, String> {
    let path = cache_paths
        .preview_file(media_id, &reusable.generation_id, reusable.format)
        .map_err(|error| error.to_string())?;
    let Some(file) = storage
        .open_existing_file(&path)
        .map_err(|error| format!("representação reduzida inválida: {error}"))?
    else {
        return Ok(false);
    };
    if file
        .metadata()
        .map_err(|error| format!("representação reduzida inválida: {error}"))?
        .len()
        != reusable.preview_bytes
    {
        return Ok(false);
    }
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| format!("representação reduzida inválida: {error}"))?;
    if reader.format() != Some(image_format(reusable.format)) {
        return Ok(false);
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("representação reduzida inválida: {error}"))?;
    if decoder.dimensions() != (reusable.width_px, reusable.height_px)
        || decoder
            .icc_profile()
            .map_err(|error| format!("perfil da representação reduzida inválido: {error}"))?
            .as_deref()
            != Some(SRGB_PROFILE)
    {
        return Ok(false);
    }
    DynamicImage::from_decoder(decoder)
        .map(|decoded| decoded.dimensions() == (reusable.width_px, reusable.height_px))
        .map_err(|error| format!("representação reduzida inválida: {error}"))
}

fn generate_preview(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    job: &CacheJob,
    resolved: &myalbuns_paths::ResolvedObject,
    fingerprint: CacheFingerprint,
) -> Result<CacheArtifact, String> {
    let source = &job.source;
    let opened = open_cache_source(resolved).map_err(|failure| failure.message)?;
    let pixel_count = opened.pixel_count().map_err(|failure| failure.message)?;
    if pixel_count > request.policy.max_decoded_pixels {
        return Err(format!(
            "a mídia {} excede o limite de pixels decodificados",
            source.media_id()
        ));
    }
    let exif_orientation = opened.exif_orientation();
    let source_page_count = opened.source_page_count();
    let basic_color_profile = opened.basic_color_profile();
    let decoded = opened.decode().map_err(|failure| failure.message)?;
    let (width, height) = decoded.dimensions();
    let preview = if width > request.policy.max_edge_px || height > request.policy.max_edge_px {
        DynamicImage::ImageRgba8(decoded)
            .thumbnail(request.policy.max_edge_px, request.policy.max_edge_px)
            .to_rgba8()
    } else {
        decoded
    };
    let format = if preview.pixels().any(|pixel| pixel[3] != u8::MAX) {
        CacheArtifactFormat::Png
    } else {
        CacheArtifactFormat::Jpeg
    };
    let preview_path = request
        .cache_paths
        .preview_file(source.media_id(), &job.candidate_generation_id, format)
        .map_err(|error| error.to_string())?;
    let temporary_path = request
        .cache_paths
        .preview_temporary_file(
            source.media_id(),
            &job.candidate_generation_id,
            format,
            std::process::id(),
        )
        .map_err(|error| error.to_string())?;
    let mut publication = storage
        .begin_file_publication(&temporary_path, &preview_path)
        .map_err(|error| format!("não foi possível criar o Cache temporário: {error}"))?;
    {
        let mut writer = BufWriter::new(&mut publication);
        match format {
            CacheArtifactFormat::Jpeg => {
                let mut encoder =
                    JpegEncoder::new_with_quality(&mut writer, request.policy.jpeg_quality);
                encoder
                    .set_icc_profile(SRGB_PROFILE.to_vec())
                    .map_err(|error| format!("não foi possível incluir o perfil sRGB: {error}"))?;
                encoder
                    .encode_image(&DynamicImage::ImageRgba8(preview.clone()).to_rgb8())
                    .map_err(|error| {
                        format!("não foi possível codificar a prévia JPEG: {error}")
                    })?;
            }
            CacheArtifactFormat::Png => {
                let mut encoder = PngEncoder::new(&mut writer);
                encoder
                    .set_icc_profile(SRGB_PROFILE.to_vec())
                    .map_err(|error| format!("não foi possível incluir o perfil sRGB: {error}"))?;
                encoder
                    .write_image(
                        preview.as_raw(),
                        preview.width(),
                        preview.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(|error| format!("não foi possível codificar a prévia PNG: {error}"))?;
            }
        }
        writer
            .flush()
            .map_err(|error| format!("não foi possível finalizar a prévia: {error}"))?;
    }
    let publication = publication
        .sync()
        .map_err(|error| format!("não foi possível sincronizar a prévia: {error}"))?;
    verify_source_fingerprint(
        source.media_id(),
        &request.root_bindings,
        source.source_path(),
        &fingerprint,
    )?;
    publication
        .publish()
        .map_err(|error| format!("não foi possível publicar a prévia: {error}"))?;
    let preview_bytes = storage
        .open_existing_file(&preview_path)
        .map_err(|error| format!("representação reduzida indisponível: {error}"))?
        .ok_or_else(|| "representação reduzida indisponível".to_string())?
        .metadata()
        .map_err(|error| format!("representação reduzida indisponível: {error}"))?
        .len();

    Ok(CacheArtifact {
        media_id: source.media_id().to_owned(),
        generation_id: job.candidate_generation_id.clone(),
        width_px: preview.width(),
        height_px: preview.height(),
        preview_bytes,
        format,
        exif_orientation,
        source_page_count,
        basic_color_profile,
        fingerprint,
    })
}

const fn image_format(format: CacheArtifactFormat) -> ImageFormat {
    match format {
        CacheArtifactFormat::Jpeg => ImageFormat::Jpeg,
        CacheArtifactFormat::Png => ImageFormat::Png,
    }
}
