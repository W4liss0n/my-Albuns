use image::GenericImageView;
use myalbuns_imaging_protocol::{
    CacheArtifactProperties, CacheReusableGeneration, ImagingProgressStage, ImagingResponse,
    ImportedPhotoDimensions, ImportedPhotoPreview, PhotoImportCandidate, PhotoImportCompletion,
    PhotoImportOutcome, PhotoImportRequest, PreparedPhotoImport,
};
use myalbuns_paths::{AppPaths, ExpectedObject};

use crate::{
    cache::write_preview,
    source::{fingerprint_source, open_cache_source, verify_source_fingerprint},
};

pub(crate) fn run(request: PhotoImportRequest, app_paths: &AppPaths) -> Result<(), String> {
    request.validate()?;
    let completion = prepare(&request, app_paths, |completed, total| {
        crate::write_progress(
            &request.request_id,
            ImagingProgressStage::PreparingPhotos,
            completed,
            total,
        )
    })?;
    crate::write_response(&ImagingResponse::PhotoImportCompleted {
        request_id: request.request_id,
        completion,
    })
}

fn prepare(
    request: &PhotoImportRequest,
    app_paths: &AppPaths,
    mut progress: impl FnMut(u32, u32) -> Result<(), String>,
) -> Result<PhotoImportCompletion, String> {
    // One contained writer handles this bounded batch, releasing each Original
    // raster before the next candidate is decoded. Failures are per source.
    let total = request.candidates.len() as u32;
    let mut completed = 0;
    progress(completed, total)?;
    let mut photos = Vec::with_capacity(request.candidates.len());
    for candidate in &request.candidates {
        let photo = PreparedPhotoImport {
            source_id: candidate.source_id.clone(),
            outcome: prepare_photo(request, candidate, app_paths)
                .unwrap_or_else(|reason| PhotoImportOutcome::InspectionRequired { reason }),
        };
        if matches!(photo.outcome, PhotoImportOutcome::Validated { .. }) {
            completed += 1;
            progress(completed, total)?;
        }
        photos.push(photo);
    }
    Ok(PhotoImportCompletion { photos })
}

fn prepare_photo(
    request: &PhotoImportRequest,
    candidate: &PhotoImportCandidate,
    app_paths: &AppPaths,
) -> Result<PhotoImportOutcome, String> {
    let resolved = request
        .root_bindings
        .resolve_existing(candidate.path(), ExpectedObject::RegularFile)
        .map_err(|error| error.to_string())?;
    let fingerprint = fingerprint_source(candidate.source_id.as_str(), &resolved)?;
    let opened = open_cache_source(&resolved).map_err(|failure| failure.message)?;
    if opened.pixel_count().map_err(|failure| failure.message)? > request.policy.max_decoded_pixels
    {
        return Err("O Original excede o limite de pixels do Cache.".into());
    }
    let exif_orientation = opened.exif_orientation();
    let source_page_count = opened.source_page_count();
    let basic_color_profile = opened.basic_color_profile();
    let decoded = opened.decode_preview().map_err(|failure| failure.message)?;
    let (width_px, height_px) = decoded.dimensions();
    let dimensions = ImportedPhotoDimensions {
        width_px,
        height_px,
    };
    let verify = || {
        verify_source_fingerprint(
            candidate.source_id.as_str(),
            &request.root_bindings,
            candidate.path(),
            &fingerprint,
        )
    };
    // A failed Cache directory, write, or encoder still yields the validated
    // Original metadata. Its fingerprint is checked even on the error path.
    let preview = (|| {
        let storage = app_paths
            .prepare_cache_storage(&request.cache_paths)
            .map_err(|error| error.to_string())?;
        let output = write_preview(
            &storage,
            request.policy,
            decoded,
            |format| {
                Ok((
                    request
                        .cache_paths
                        .import_preview_temporary_file(
                            &request.attempt_id,
                            candidate.source_id.as_str(),
                            &candidate.generation_id,
                            format,
                            std::process::id(),
                        )
                        .map_err(|error| error.to_string())?,
                    request
                        .cache_paths
                        .import_preview_file(
                            &request.attempt_id,
                            candidate.source_id.as_str(),
                            &candidate.generation_id,
                            format,
                        )
                        .map_err(|error| error.to_string())?,
                ))
            },
            verify,
        )?;
        CacheReusableGeneration::new(
            candidate.generation_id.clone(),
            CacheArtifactProperties::new(
                output.format,
                output.width_px,
                output.height_px,
                output.bytes,
                exif_orientation,
                source_page_count,
                basic_color_profile,
            ),
            fingerprint.clone(),
        )
    })();
    let preview = match preview {
        Ok(generation) => ImportedPhotoPreview::Prepared {
            generation: Box::new(generation),
        },
        Err(reason) => {
            verify()?;
            ImportedPhotoPreview::Unavailable { reason }
        }
    };
    Ok(PhotoImportOutcome::Validated {
        dimensions,
        fingerprint,
        preview,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_imaging_protocol::{
        CacheRepresentationPolicy, IMAGING_PROTOCOL_VERSION, PhotoImportSourceId,
    };
    use myalbuns_paths::{NativePathDto, OperationPathContext, project_data_namespace};

    fn request(root: &std::path::Path, app_paths: &AppPaths, count: usize) -> PhotoImportRequest {
        std::fs::create_dir_all(root.join("roaming")).unwrap();
        std::fs::create_dir_all(root.join("local")).unwrap();
        let paths = app_paths
            .project_cache(&project_data_namespace("import-fixture"))
            .unwrap();
        let mut context = OperationPathContext::new();
        context.capture(paths.root()).unwrap();
        let candidates = (0..count)
            .map(|index| {
                let path = root.join(format!("photo-{index}.jpg"));
                RgbImage::from_pixel(31, 17, Rgb([21, 70, 150]))
                    .save_with_format(&path, ImageFormat::Jpeg)
                    .unwrap();
                context.capture(&path).unwrap();
                PhotoImportCandidate {
                    source_id: PhotoImportSourceId::new(format!("source-{index}")).unwrap(),
                    source_path: NativePathDto::from(path),
                    generation_id: format!("generation-{index}"),
                }
            })
            .collect();
        PhotoImportRequest {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: "import-fixture".into(),
            attempt_id: "attempt-fixture".into(),
            project_id: "import-fixture".into(),
            cache_paths: paths,
            candidates,
            policy: CacheRepresentationPolicy::measured_v1(),
            root_bindings: context.freeze(),
        }
    }

    #[test]
    fn png_and_tiff_imports_preserve_transparency_in_the_prepared_preview() {
        for format in [ImageFormat::Png, ImageFormat::Tiff] {
            let root = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            let request = request(root.path(), &paths, 1);
            image::RgbaImage::from_pixel(31, 17, image::Rgba([21, 70, 150, 100]))
                .save_with_format(request.candidates[0].path(), format)
                .unwrap();
            let completion = prepare(&request, &paths, |_, _| Ok(())).unwrap();
            let PhotoImportOutcome::Validated {
                dimensions,
                preview: ImportedPhotoPreview::Prepared { generation },
                ..
            } = &completion.photos[0].outcome
            else {
                panic!(
                    "PNG/TIFF must prepare successfully: {:?}",
                    completion.photos[0].outcome
                )
            };
            assert_eq!((dimensions.width_px, dimensions.height_px), (31, 17));
            let output = request
                .cache_paths
                .import_preview_file(
                    &request.attempt_id,
                    request.candidates[0].source_id.as_str(),
                    &generation.generation_id,
                    generation.format,
                )
                .unwrap();
            let image = image::open(output).unwrap().to_rgba8();
            assert_eq!(image.get_pixel(0, 0).0, [21, 70, 150, 100]);
        }
    }

    #[test]
    fn a_batch_decodes_each_original_once_and_preserves_valid_neighbors() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let request = request(root.path(), &paths, 3);
        let before = std::fs::read(request.candidates[0].path()).unwrap();
        std::fs::write(request.candidates[1].path(), b"not a JPEG").unwrap();
        let decode_count = crate::source::jpeg_decode_count();
        let completion = prepare(&request, &paths, |_, _| Ok(())).unwrap();
        completion.validate_for(&request).unwrap();
        assert_eq!(crate::source::jpeg_decode_count() - decode_count, 2);
        assert!(matches!(
            completion.photos[1].outcome,
            PhotoImportOutcome::InspectionRequired { .. }
        ));
        for index in [0, 2] {
            let PhotoImportOutcome::Validated {
                dimensions,
                preview: ImportedPhotoPreview::Prepared { generation },
                ..
            } = &completion.photos[index].outcome
            else {
                panic!("valid neighbor keeps its preparation")
            };
            assert_eq!((dimensions.width_px, dimensions.height_px), (31, 17));
            let preview_path = request
                .cache_paths
                .import_preview_file(
                    &request.attempt_id,
                    request.candidates[index].source_id.as_str(),
                    &generation.generation_id,
                    generation.format,
                )
                .unwrap();
            assert!(preview_path.is_file());
        }
        assert!(!request.cache_paths.metadata_file().exists());
        assert_eq!(std::fs::read(request.candidates[0].path()).unwrap(), before);
    }

    #[test]
    fn an_original_above_the_pixel_limit_is_rejected_before_decode() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let request = request(root.path(), &paths, 1);
        let path = request.candidates[0].path();
        let mut jpeg = std::fs::read(path).unwrap();
        let frame = jpeg
            .windows(2)
            .position(|bytes| bytes == [0xff, 0xc0])
            .unwrap();
        jpeg[frame + 5..frame + 7].copy_from_slice(&10_000_u16.to_be_bytes());
        jpeg[frame + 7..frame + 9].copy_from_slice(&15_000_u16.to_be_bytes());
        std::fs::write(path, jpeg).unwrap();
        let before = crate::source::jpeg_decode_count();
        let completion = prepare(&request, &paths, |_, _| Ok(())).unwrap();
        completion.validate_for(&request).unwrap();
        assert_eq!(crate::source::jpeg_decode_count(), before);
        let PhotoImportOutcome::InspectionRequired { reason } = &completion.photos[0].outcome
        else {
            panic!("the oversized source needs isolated inspection")
        };
        assert!(reason.contains("limite de pixels"), "{reason}");
    }

    #[test]
    fn disk_full_during_preview_keeps_valid_originals_and_continues_the_batch() {
        for (format, size) in [
            (ImageFormat::Jpeg, 31),
            (ImageFormat::Jpeg, 256),
            (ImageFormat::Png, 256),
        ] {
            let height = if size == 31 { 17 } else { 256 };
            let root = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            let request = request(root.path(), &paths, 2);
            if format == ImageFormat::Png {
                image::RgbaImage::from_fn(size, height, |x, y| {
                    image::Rgba([x as u8, (x * y) as u8, (x ^ y) as u8, 120])
                })
                .save_with_format(request.candidates[0].path(), format)
                .unwrap();
            } else {
                RgbImage::from_fn(size, height, |x, y| {
                    Rgb([x as u8, (x * y) as u8, (x ^ y) as u8])
                })
                .save_with_format(request.candidates[0].path(), format)
                .unwrap();
            }
            let cache_format = if format == ImageFormat::Png {
                myalbuns_paths::CacheArtifactFormat::Png
            } else {
                myalbuns_paths::CacheArtifactFormat::Jpeg
            };
            let originals = request
                .candidates
                .iter()
                .map(|candidate| std::fs::read(candidate.path()).unwrap())
                .collect::<Vec<_>>();
            let candidate = &request.candidates[0];
            let preview_path = request
                .cache_paths
                .import_preview_file(
                    &request.attempt_id,
                    candidate.source_id.as_str(),
                    &candidate.generation_id,
                    cache_format,
                )
                .unwrap();
            let temporary_path = request
                .cache_paths
                .import_preview_temporary_file(
                    &request.attempt_id,
                    candidate.source_id.as_str(),
                    &candidate.generation_id,
                    cache_format,
                    std::process::id(),
                )
                .unwrap();
            let fault = myalbuns_paths::test_support::DiskFull::after_bytes(&preview_path, 32);
            let mut progress = Vec::new();
            let completion = prepare(&request, &paths, |completed, total| {
                progress.push((completed, total));
                Ok(())
            })
            .unwrap();
            completion.validate_for(&request).unwrap();
            assert_eq!(fault.written_bytes(), 32);
            assert!(fault.failure_count() > 0);
            let PhotoImportOutcome::Validated {
                dimensions,
                preview: ImportedPhotoPreview::Unavailable { reason },
                ..
            } = &completion.photos[0].outcome
            else {
                panic!("a full Cache disk must preserve the validated Original");
            };
            assert_eq!((dimensions.width_px, dimensions.height_px), (size, height));
            assert!(reason.contains("Libere espaço"), "{reason}");
            assert!(matches!(
                completion.photos[1].outcome,
                PhotoImportOutcome::Validated {
                    preview: ImportedPhotoPreview::Prepared { .. },
                    ..
                }
            ));
            assert_eq!(progress, [(0, 2), (1, 2), (2, 2)]);
            assert!(!preview_path.exists());
            assert!(!temporary_path.exists());
            for (candidate, before) in request.candidates.iter().zip(&originals) {
                assert_eq!(std::fs::read(candidate.path()).unwrap(), *before);
            }
            drop(fault);

            let recovered = prepare(&request, &paths, |_, _| Ok(())).unwrap();
            assert!(matches!(
                recovered.photos[0].outcome,
                PhotoImportOutcome::Validated {
                    preview: ImportedPhotoPreview::Prepared { .. },
                    ..
                }
            ));
            assert_eq!(
                image::open(&preview_path).unwrap().dimensions(),
                (size, height)
            );
        }
    }

    #[test]
    fn disk_full_during_cache_command_reports_a_deterministic_storage_failure() {
        use myalbuns_imaging_protocol::{
            CacheJob, CacheMediaSource, CacheRequest, ImagingFailureStage,
        };
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let import = request(root.path(), &paths, 1);
        let source = CacheMediaSource::new(
            "photo",
            myalbuns_core::MediaKind::Photo,
            import.candidates[0].path().to_owned(),
        )
        .unwrap();
        let original = std::fs::read(source.source_path()).unwrap();
        let preview = import
            .cache_paths
            .preview_file(
                "photo",
                "new-generation",
                myalbuns_paths::CacheArtifactFormat::Jpeg,
            )
            .unwrap();
        let request = CacheRequest::new(
            "cache-test",
            import.project_id,
            import.cache_paths,
            vec![CacheJob::new(source, "new-generation", None).unwrap()],
            import.policy,
            import.root_bindings,
        )
        .unwrap();
        let fault = myalbuns_paths::test_support::DiskFull::after_bytes(&preview, 32);
        let error = crate::cache::run_cache(request, &paths).unwrap_err();
        assert!(fault.failure_count() > 0);
        assert!(matches!(error, crate::cache_error::CacheError::StorageFull));
        assert_eq!(
            crate::cache_failure(error).stage,
            Some(ImagingFailureStage::CacheStorageFull)
        );
        assert_eq!(ImagingFailureStage::CacheStorageFull.exit_code(), 30);
        assert_eq!(
            std::fs::read(import.candidates[0].path()).unwrap(),
            original
        );
        assert!(!preview.exists());
    }

    #[test]
    fn a_cache_failure_still_returns_fingerprinted_original_dimensions() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let request = request(root.path(), &paths, 1);
        let storage = paths.prepare_cache_storage(&request.cache_paths).unwrap();
        let candidate = &request.candidates[0];
        let preview_path = request
            .cache_paths
            .import_preview_file(
                &request.attempt_id,
                candidate.source_id.as_str(),
                &candidate.generation_id,
                myalbuns_paths::CacheArtifactFormat::Jpeg,
            )
            .unwrap();
        // An occupied immutable destination forces publication failure after
        // Original decode, without simulating or suppressing a codec error.
        std::fs::create_dir(&preview_path).unwrap();
        let decode_count = crate::source::jpeg_decode_count();
        let completion = prepare(&request, &paths, |_, _| Ok(())).unwrap();
        completion.validate_for(&request).unwrap();
        assert_eq!(crate::source::jpeg_decode_count() - decode_count, 1);
        assert!(matches!(
            completion.photos[0].outcome,
            PhotoImportOutcome::Validated {
                dimensions: ImportedPhotoDimensions {
                    width_px: 31,
                    height_px: 17
                },
                preview: ImportedPhotoPreview::Unavailable { .. },
                ..
            }
        ));
        drop(storage);
    }
}
