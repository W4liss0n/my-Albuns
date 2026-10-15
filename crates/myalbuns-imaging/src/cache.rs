use std::{
    io::{BufReader, BufWriter, Write},
    time::{Duration, Instant},
};

use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as ResizeImage,
};
use image::{
    DynamicImage, ExtendedColorType, ImageEncoder, RgbImage, RgbaImage,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use myalbuns_imaging::preview::{CachePreviewSpec, SRGB_PROFILE, validate_cache_preview};
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheCompletion, CacheFingerprint, CacheJob,
    CacheJobFailure, CacheRequest, CacheReusableGeneration, ImagingProgressStage, ImagingResponse,
    root_binding_plan_sha256,
};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, ExpectedObject, PreparedCacheStorage};

use crate::{
    cache_error::{CacheError, CacheWriteMonitor},
    source::{
        MAX_DECODED_SOURCE_PIXELS_TOTAL, PreviewRaster, confirm_source_unchanged, open_cache_bytes,
        preview_dimensions, read_fingerprinted_source, transposes,
    },
    write_response,
};

pub(crate) fn run_cache(request: CacheRequest, app_paths: &AppPaths) -> Result<(), CacheError> {
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
    let started = Instant::now();
    let (completion, steps) = build_cache(&request, app_paths, |completed, total| {
        crate::write_progress(
            &request.request_id,
            ImagingProgressStage::PreparingPhotos,
            completed,
            total,
        )
    })
    .inspect_err(|error| {
        tracing::error!(
            target: "myalbuns.imaging",
            process_role = ProcessRole::Imaging.as_str(),
            protocol_version = request.protocol_version,
            operation_id,
            project_id,
            error = %error,
            event = "cache_request_failed",
        );
    })?;
    // Step times are summed over the request's media; `elapsed_us` also covers
    // preparing the Cache folder and the work between media.
    tracing::info!(
        target: "myalbuns.imaging",
        process_role = ProcessRole::Imaging.as_str(),
        protocol_version = request.protocol_version,
        operation_id,
        project_id,
        process_id = std::process::id(),
        media_count = request.jobs.len(),
        generated_count = completion.generated_count,
        reused_count = completion.reused_count,
        failed_count = completion.failures.len(),
        source_bytes = completion.source_bytes,
        preview_bytes = completion.preview_bytes,
        resolve_us = micros(steps.resolve),
        read_hash_us = micros(steps.read_hash),
        reuse_check_us = micros(steps.reuse_check),
        preflight_us = micros(steps.preflight),
        decode_us = micros(steps.decode),
        resize_us = micros(steps.resize),
        encode_us = micros(steps.encode),
        publish_us = micros(steps.publish),
        total_us = micros(steps.total),
        elapsed_us = micros(started.elapsed()),
        root_binding_plan_sha256,
        event = "cache_request_completed",
    );
    write_response(&ImagingResponse::cache_completed(
        request.request_id,
        completion,
    ))
    .map_err(Into::into)
}

/// Wall time of each step of one media, so a slow preparation can be
/// attributed without a profiler.
#[derive(Clone, Copy, Debug, Default)]
struct CacheSteps {
    /// Resolving the Original through the operation's root bindings.
    resolve: Duration,
    /// Reading the whole Original and its SHA-256.
    read_hash: Duration,
    /// Checking an indexed preview and the unchanged Original before reuse.
    reuse_check: Duration,
    /// Header, EXIF, colour and page checks before decoding.
    preflight: Duration,
    decode: Duration,
    /// Reduction, orientation and the transparency check.
    resize: Duration,
    encode: Duration,
    /// Flush to disk, source confirmation and the atomic rename.
    publish: Duration,
    total: Duration,
}

impl CacheSteps {
    fn add(&mut self, other: &Self) {
        self.resolve += other.resolve;
        self.read_hash += other.read_hash;
        self.reuse_check += other.reuse_check;
        self.preflight += other.preflight;
        self.decode += other.decode;
        self.resize += other.resize;
        self.encode += other.encode;
        self.publish += other.publish;
        self.total += other.total;
    }
}

fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

/// Prepares the media one after another, releasing each Original before the
/// next is read. A media that fails does not stop the others; running out of
/// Cache storage does, since every later preview would fail the same way.
fn build_cache(
    request: &CacheRequest,
    app_paths: &AppPaths,
    mut progress: impl FnMut(u32, u32) -> Result<(), String>,
) -> Result<(CacheCompletion, CacheSteps), CacheError> {
    if request.policy.max_decoded_pixels != MAX_DECODED_SOURCE_PIXELS_TOTAL {
        return Err(
            "a política de decode diverge do limite comum do Processador"
                .to_string()
                .into(),
        );
    }
    let storage = app_paths
        .prepare_cache_storage(&request.cache_paths)
        .map_err(|error| CacheError::paths("não foi possível preparar o Cache", error))?;

    let total = request.jobs.len() as u32;
    progress(0, total)?;
    let mut artifacts = Vec::with_capacity(request.jobs.len());
    let mut failures = Vec::new();
    let mut generated_count = 0;
    let mut reused_count = 0;
    let mut source_bytes = 0_u64;
    let mut preview_bytes = 0_u64;
    let mut request_steps = CacheSteps::default();
    for (index, job) in request.jobs.iter().enumerate() {
        let started = Instant::now();
        let mut steps = CacheSteps::default();
        // A panic in one media's decoder fails that media, not the whole
        // batch: the Host would otherwise retry every media of the request.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            build_job(&storage, request, job, &mut steps)
        }))
        .unwrap_or_else(|_| {
            Err("o processamento da mídia foi interrompido"
                .to_string()
                .into())
        });
        steps.total = started.elapsed();
        request_steps.add(&steps);
        let operation_id = safe_log_identifier(&request.request_id);
        let media_id = safe_log_identifier(job.source.media_id());
        match outcome {
            Ok((artifact, generated)) => {
                tracing::info!(
                    target: "myalbuns.imaging",
                    process_role = ProcessRole::Imaging.as_str(),
                    operation_id,
                    process_id = std::process::id(),
                    media_id,
                    outcome = if generated { "generated" } else { "reused" },
                    source_bytes = artifact.fingerprint.source_bytes,
                    preview_bytes = artifact.preview_bytes,
                    resolve_us = micros(steps.resolve),
                    read_hash_us = micros(steps.read_hash),
                    reuse_check_us = micros(steps.reuse_check),
                    preflight_us = micros(steps.preflight),
                    decode_us = micros(steps.decode),
                    resize_us = micros(steps.resize),
                    encode_us = micros(steps.encode),
                    publish_us = micros(steps.publish),
                    total_us = micros(steps.total),
                    event = "cache_job_completed",
                );
                source_bytes = source_bytes
                    .checked_add(artifact.fingerprint.source_bytes)
                    .ok_or_else(|| "o tamanho total das mídias excedeu o limite".to_string())?;
                preview_bytes = preview_bytes
                    .checked_add(artifact.preview_bytes)
                    .ok_or_else(|| "o tamanho total do Cache excedeu o limite".to_string())?;
                if generated {
                    generated_count += 1;
                } else {
                    reused_count += 1;
                }
                artifacts.push(artifact);
            }
            Err(CacheError::StorageFull) => return Err(CacheError::StorageFull),
            Err(error) => {
                tracing::warn!(
                    target: "myalbuns.imaging",
                    process_role = ProcessRole::Imaging.as_str(),
                    operation_id,
                    process_id = std::process::id(),
                    media_id,
                    error = %error,
                    total_us = micros(steps.total),
                    event = "cache_job_failed",
                );
                failures.push(CacheJobFailure {
                    media_id: job.source.media_id().to_owned(),
                });
            }
        }
        progress(index as u32 + 1, total)?;
    }

    Ok((
        CacheCompletion {
            artifacts,
            failures,
            generated_count,
            reused_count,
            source_bytes,
            preview_bytes,
        },
        request_steps,
    ))
}

/// Returns the media's artifact and whether it is a new generation.
fn build_job(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    job: &CacheJob,
    steps: &mut CacheSteps,
) -> Result<(CacheArtifact, bool), CacheError> {
    let source = &job.source;
    let mut step = Instant::now();
    let mut lap = |slot: &mut Duration| {
        let now = Instant::now();
        *slot += now - step;
        step = now;
    };
    let resolved = request
        .root_bindings
        .resolve_existing(source.source_path(), ExpectedObject::RegularFile)
        .map_err(|error| {
            format!(
                "não foi possível abrir a mídia {} pelo plano da operação: {error}",
                source.media_id()
            )
        })?;
    lap(&mut steps.resolve);
    let (fingerprint, bytes) = read_fingerprinted_source(source.media_id(), &resolved)?;
    lap(&mut steps.read_hash);

    if let Some(reusable) = job
        .reusable
        .as_ref()
        .filter(|reusable| reusable.fingerprint == fingerprint)
        .filter(|reusable| {
            validate_existing_preview(storage, &request.cache_paths, source.media_id(), reusable)
                .unwrap_or(false)
        })
    {
        drop(bytes);
        confirm_source_unchanged(
            source.media_id(),
            &request.root_bindings,
            source.source_path(),
            &resolved,
            &fingerprint,
        )?;
        lap(&mut steps.reuse_check);
        return Ok((artifact_from_reusable(source.media_id(), reusable), false));
    }
    lap(&mut steps.reuse_check);
    let artifact = generate_preview(storage, request, job, &resolved, bytes, fingerprint, steps)?;
    Ok((artifact, true))
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
    validate_cache_preview(
        BufReader::new(file),
        CachePreviewSpec {
            format: reusable.format,
            width_px: reusable.width_px,
            height_px: reusable.height_px,
            bytes: reusable.preview_bytes,
        },
    )
    .map(|()| true)
}

fn generate_preview(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    job: &CacheJob,
    resolved: &myalbuns_paths::ResolvedObject,
    bytes: Vec<u8>,
    fingerprint: CacheFingerprint,
    steps: &mut CacheSteps,
) -> Result<CacheArtifact, CacheError> {
    let source = &job.source;
    let started = Instant::now();
    let opened = open_cache_bytes(bytes).map_err(|failure| failure.message)?;
    let pixel_count = opened.pixel_count().map_err(|failure| failure.message)?;
    if pixel_count > request.policy.max_decoded_pixels {
        return Err(format!(
            "a mídia {} excede o limite de pixels decodificados",
            source.media_id()
        )
        .into());
    }
    let exif_orientation = opened.exif_orientation();
    let source_page_count = opened.source_page_count();
    let basic_color_profile = opened.basic_color_profile();
    let decoding = Instant::now();
    steps.preflight += decoding - started;
    let decoded = opened
        .decode_preview(request.policy.max_edge_px)
        .map_err(|failure| failure.message)?;
    steps.decode += decoding.elapsed();
    let output = write_preview(
        storage,
        request.policy,
        decoded,
        |format| {
            Ok((
                request
                    .cache_paths
                    .preview_temporary_file(
                        source.media_id(),
                        &job.candidate_generation_id,
                        format,
                        std::process::id(),
                    )
                    .map_err(|error| error.to_string())?,
                request
                    .cache_paths
                    .preview_file(source.media_id(), &job.candidate_generation_id, format)
                    .map_err(|error| error.to_string())?,
            ))
        },
        || {
            confirm_source_unchanged(
                source.media_id(),
                &request.root_bindings,
                source.source_path(),
                resolved,
                &fingerprint,
            )
        },
    )?;
    steps.resize += output.steps.resize;
    steps.encode += output.steps.encode;
    steps.publish += output.steps.publish;
    Ok(CacheArtifact {
        media_id: source.media_id().to_owned(),
        generation_id: job.candidate_generation_id.clone(),
        width_px: output.width_px,
        height_px: output.height_px,
        preview_bytes: output.bytes,
        format: output.format,
        exif_orientation,
        source_page_count,
        basic_color_profile,
        fingerprint,
    })
}

pub(crate) struct PreviewOutput {
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) bytes: u64,
    pub(crate) format: CacheArtifactFormat,
    pub(crate) steps: PreviewSteps,
}

/// Wall time of the steps after decoding, for the Processor's log.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PreviewSteps {
    pub(crate) resize: Duration,
    pub(crate) encode: Duration,
    pub(crate) publish: Duration,
}

/// Both bound media and not-yet-imported sources use this exact representation
/// policy. The owner supplies guarded candidate names and source verification.
pub(crate) fn write_preview(
    storage: &PreparedCacheStorage,
    policy: myalbuns_imaging_protocol::CacheRepresentationPolicy,
    decoded: PreviewRaster,
    paths: impl FnOnce(CacheArtifactFormat) -> Result<(std::path::PathBuf, std::path::PathBuf), String>,
    verify_source: impl FnOnce() -> Result<(), String>,
) -> Result<PreviewOutput, CacheError> {
    let started = Instant::now();
    let (width, height) = decoded.oriented_dimensions();
    let preview = if width > policy.max_edge_px || height > policy.max_edge_px {
        let (width, height) = preview_dimensions(width, height, policy.max_edge_px);
        // Reduce the raster as decoded, then orient only the reduced pixels.
        let (width, height) = if transposes(decoded.pending) {
            (height, width)
        } else {
            (width, height)
        };
        reduce(decoded.image, width, height)?
    } else {
        decoded.image
    };
    let preview =
        PreviewRaster::orient(preview, decoded.pending).map_err(|failure| failure.message)?;
    let format = if preview
        .as_rgba8()
        .is_some_and(|rgba| rgba.pixels().any(|pixel| pixel[3] != u8::MAX))
    {
        CacheArtifactFormat::Png
    } else {
        CacheArtifactFormat::Jpeg
    };
    let encoding = Instant::now();
    let (temporary_path, preview_path) = paths(format)?;
    let mut publication = storage
        .begin_file_publication(&temporary_path, &preview_path)
        .map_err(|error| CacheError::paths("não foi possível criar o Cache temporário", error))?;
    {
        let mut writer = CacheWriteMonitor::new(BufWriter::new(&mut publication));
        let encoded = (|| -> Result<(), CacheError> {
            match format {
                CacheArtifactFormat::Jpeg => {
                    let mut encoder =
                        JpegEncoder::new_with_quality(&mut writer, policy.jpeg_quality);
                    encoder
                        .set_icc_profile(SRGB_PROFILE.to_vec())
                        .map_err(|error| {
                            format!("não foi possível incluir o perfil sRGB: {error}")
                        })?;
                    let result = match &preview {
                        DynamicImage::ImageRgb8(rgb) => encoder.encode_image(rgb),
                        _ => encoder.encode_image(&preview),
                    };
                    result.map_err(|error| {
                        CacheError::image("não foi possível codificar a prévia JPEG", error)
                    })?;
                }
                CacheArtifactFormat::Png => {
                    let mut encoder = PngEncoder::new(&mut writer);
                    encoder
                        .set_icc_profile(SRGB_PROFILE.to_vec())
                        .map_err(|error| {
                            format!("não foi possível incluir o perfil sRGB: {error}")
                        })?;
                    encoder
                        .write_image(
                            preview.as_bytes(),
                            preview.width(),
                            preview.height(),
                            ExtendedColorType::Rgba8,
                        )
                        .map_err(|error| {
                            CacheError::image("não foi possível codificar a prévia PNG", error)
                        })?;
                }
            }
            writer
                .flush()
                .map_err(|error| CacheError::io("não foi possível finalizar a prévia", error))?;
            Ok(())
        })();
        if writer.storage_full {
            return Err(CacheError::StorageFull);
        }
        encoded?;
    }
    let publishing = Instant::now();
    let publication = publication
        .sync()
        .map_err(|error| CacheError::paths("não foi possível sincronizar a prévia", error))?;
    verify_source()?;
    publication
        .publish()
        .map_err(|error| CacheError::paths("não foi possível publicar a prévia", error))?;
    let preview_bytes = storage
        .open_existing_file(&preview_path)
        .map_err(|error| format!("representação reduzida indisponível: {error}"))?
        .ok_or_else(|| "representação reduzida indisponível".to_string())?
        .metadata()
        .map_err(|error| format!("representação reduzida indisponível: {error}"))?
        .len();

    Ok(PreviewOutput {
        width_px: preview.width(),
        height_px: preview.height(),
        bytes: preview_bytes,
        format,
        steps: PreviewSteps {
            resize: encoding - started,
            encode: publishing - encoding,
            publish: publishing.elapsed(),
        },
    })
}

/// Area-averaging reduction. Transparent pixels are weighted by alpha, so
/// their hidden colour does not bleed into visible neighbours. A raster
/// decoded at a reduced scale may already have the preview's size.
fn reduce(image: DynamicImage, width: u32, height: u32) -> Result<DynamicImage, CacheError> {
    if (image.width(), image.height()) == (width, height) {
        return Ok(image);
    }
    let image = match image {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => image,
        other => DynamicImage::ImageRgba8(other.into_rgba8()),
    };
    let pixel_type = if image.as_rgb8().is_some() {
        PixelType::U8x3
    } else {
        PixelType::U8x4
    };
    let failed = |error: &dyn std::fmt::Display| {
        CacheError::from(format!("não foi possível reduzir a prévia: {error}"))
    };
    let source = ResizeImage::from_vec_u8(
        image.width(),
        image.height(),
        image.into_bytes(),
        pixel_type,
    )
    .map_err(|error| failed(&error))?;
    let mut target = ResizeImage::new(width, height, pixel_type);
    Resizer::new()
        .resize(
            &source,
            &mut target,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Box)),
        )
        .map_err(|error| failed(&error))?;
    let pixels = target.into_vec();
    let reduced = match pixel_type {
        PixelType::U8x3 => RgbImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgb8),
        _ => RgbaImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgba8),
    };
    reduced.ok_or_else(|| {
        "a prévia reduzida não corresponde às dimensões esperadas"
            .to_string()
            .into()
    })
}
