use image::GenericImageView;
use myalbuns_imaging_protocol::{
    CacheArtifactProperties, CacheReusableGeneration, ImagingResponse, ImportedPhotoDimensions,
    ImportedPhotoPreview, PhotoImportCandidate, PhotoImportCompletion, PhotoImportOutcome,
    PhotoImportRequest, PreparedPhotoImport,
};
use myalbuns_paths::{AppPaths, ExpectedObject};

use crate::{
    cache::write_preview,
    source::{fingerprint_source, open_cache_source, verify_source_fingerprint},
};

pub(crate) fn run(request: PhotoImportRequest, app_paths: &AppPaths) -> Result<(), String> {
    request.validate()?;
    let completion = prepare(&request, app_paths);
    crate::write_response(&ImagingResponse::PhotoImportCompleted {
        request_id: request.request_id,
        completion,
    })
}

fn prepare(request: &PhotoImportRequest, app_paths: &AppPaths) -> PhotoImportCompletion {
    // One contained writer handles this bounded batch, releasing each Original
    // raster before the next candidate is decoded. Failures are per source.
    let photos = request
        .candidates
        .iter()
        .map(|candidate| PreparedPhotoImport {
            source_id: candidate.source_id.clone(),
            outcome: prepare_photo(request, candidate, app_paths)
                .unwrap_or_else(|reason| PhotoImportOutcome::InspectionRequired { reason }),
        })
        .collect();
    PhotoImportCompletion { photos }
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
    if !opened.is_jpeg() {
        return Err("Escolha um Arquivo JPEG válido (.jpg ou .jpeg).".into());
    }
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
    fn a_batch_decodes_each_original_once_and_preserves_valid_neighbors() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let request = request(root.path(), &paths, 3);
        let before = std::fs::read(request.candidates[0].path()).unwrap();
        std::fs::write(request.candidates[1].path(), b"not a JPEG").unwrap();
        let decode_count = crate::source::jpeg_decode_count();
        let completion = prepare(&request, &paths);
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
        let completion = prepare(&request, &paths);
        completion.validate_for(&request).unwrap();
        assert_eq!(crate::source::jpeg_decode_count(), before);
        let PhotoImportOutcome::InspectionRequired { reason } = &completion.photos[0].outcome
        else {
            panic!("the oversized source needs isolated inspection")
        };
        assert!(reason.contains("limite de pixels"), "{reason}");
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
        let completion = prepare(&request, &paths);
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
