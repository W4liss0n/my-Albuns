use crate::{
    format_output::{PdfOutput, write_png},
    jpeg_output::write_verified_quality,
    render::{RenderFailure, composition_workers, render_unit},
    source::{MAX_DECODED_SOURCE_PIXELS_TOTAL, OpenRenderSource, capture_render_source},
};
use image::RgbaImage;
use myalbuns_core::{ComposedBackground, ComposedOutputUnit, MediaId, RectUm};
use myalbuns_imaging_protocol::{
    AlbumRenderCompletion, AlbumRenderRequest, ImagingFailureCode, ImagingPathCode,
    ImagingProgressStage, RenderCompletion, RenderFormat,
};
use myalbuns_paths::ExpectedObject;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

#[cfg(test)]
pub(crate) fn render(
    request: &AlbumRenderRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
) -> Result<AlbumRenderCompletion, RenderFailure> {
    render_retaining(request, progress, &mut Vec::new())
}

pub(crate) fn render_retaining(
    request: &AlbumRenderRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
    outputs: &mut Vec<RenderCompletion>,
) -> Result<AlbumRenderCompletion, RenderFailure> {
    request.validate().map_err(|message| {
        RenderFailure::typed(
            ImagingFailureCode::InvalidRenderRequest,
            None,
            None,
            message,
        )
    })?;
    let mut captured = HashMap::new();
    let mut source_bytes = 0;
    let mut ordered = request.sources.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|source| source.media_id().to_string());
    let total_sources = ordered.len().max(1) as u32;
    progress(ImagingProgressStage::LoadingSources, 0, total_sources)?;
    for (index, source) in ordered.iter().enumerate() {
        let object = request
            .root_bindings
            .resolve_existing(source.source_path(), ExpectedObject::RegularFile)
            .map_err(|error| {
                RenderFailure::typed(
                    ImagingFailureCode::SourceUnavailable,
                    Some(source.media_id().to_string()),
                    Some(ImagingPathCode::from_resolve_error(error)),
                    "O Original não está acessível.",
                )
            })?;
        let capture = capture_render_source(&object).map_err(|failure| {
            RenderFailure::typed(
                failure.code,
                Some(source.media_id().to_string()),
                failure.path_code,
                failure.message,
            )
        })?;
        source_bytes += capture.byte_count();
        captured.insert(source.media_id(), capture);
        progress(
            ImagingProgressStage::LoadingSources,
            (index + 1) as u32,
            total_sources,
        )?;
    }
    if ordered.is_empty() {
        progress(ImagingProgressStage::LoadingSources, 1, total_sources)?;
    }
    let total = request
        .outputs
        .iter()
        .map(|output| output.units.len())
        .sum::<usize>() as u32;
    let mut done = 0;
    let mut decoded = DecodedSources::new(composition_workers());
    for output in &request.outputs {
        let path = request
            .root_bindings
            .resolve(output.prepared_path.as_path())
            .map_err(|error| {
                RenderFailure::typed(
                    ImagingFailureCode::EncodeFailed,
                    None,
                    None,
                    error.to_string(),
                )
            })?;
        let mut pdf = if request.format == RenderFormat::Pdf {
            Some(PdfOutput::create(&path)?)
        } else {
            None
        };
        let mut receipt = None;
        let mut dimensions = (0, 0);
        for selected in &output.units {
            let mut unit = request
                .snapshot
                .output_unit(&selected.sheet_id)
                .map_err(|error| error.to_string())?;
            let required: HashSet<_> = unit.sheet.referenced_media_ids().collect();
            let sources = decoded.prepare(&captured, &required)?;
            translate_viewport(&mut unit, &selected.viewport);
            progress(ImagingProgressStage::Composing, done, total)?;
            let image = render_unit(&unit, request.snapshot.dpi, sources, &mut |_, _, _| {
                progress(ImagingProgressStage::Composing, done, total)
            })?;
            dimensions = (image.width(), image.height());
            progress(ImagingProgressStage::Composing, done, total)?;
            match request.format {
                RenderFormat::Jpeg { quality } => {
                    receipt = Some(write_verified_quality(
                        &image,
                        &path,
                        request.snapshot.dpi,
                        quality,
                    )?)
                }
                RenderFormat::Png => {
                    receipt = Some(write_png(&image, &path, request.snapshot.dpi)?)
                }
                RenderFormat::Pdf => pdf
                    .as_mut()
                    .expect("PDF writer owns the current output")
                    .add_page(&image, selected.viewport.width, selected.viewport.height)?,
            }
            done += 1;
            progress(ImagingProgressStage::Composing, done, total)?;
        }
        if let Some(pdf) = pdf {
            receipt = Some(pdf.finish(&path)?);
        }
        let receipt = receipt.expect("validated nonempty output has a receipt");
        outputs.push(RenderCompletion {
            width_px: dimensions.0,
            height_px: dimensions.1,
            dpi: request.snapshot.dpi,
            source_count: captured.len(),
            source_bytes,
            output_bytes: receipt.output_bytes,
            output_sha256: receipt.output_sha256,
        });
    }
    progress(ImagingProgressStage::EncodingOutput, total, total)?;
    Ok(AlbumRenderCompletion {
        outputs: outputs.clone(),
    })
}

/// Decoded Originals retained between consecutive units. Page exports and
/// Decoratives repeated on neighbouring Sheets reuse the same raster instead
/// of decoding the Original again. Only the current unit's sources are kept,
/// so the retained set honours the same pixel ceiling as a single unit.
struct DecodedSources {
    rasters: HashMap<MediaId, RgbaImage>,
    workers: usize,
}

impl DecodedSources {
    fn new(workers: usize) -> Self {
        Self {
            rasters: HashMap::new(),
            workers: workers.max(1),
        }
    }

    fn prepare(
        &mut self,
        captured: &HashMap<MediaId, OpenRenderSource>,
        required: &HashSet<MediaId>,
    ) -> Result<&HashMap<MediaId, RgbaImage>, RenderFailure> {
        let mut ordered = required.iter().copied().collect::<Vec<_>>();
        ordered.sort_by_key(ToString::to_string);
        let mut pixels = 0_u64;
        for id in &ordered {
            pixels += captured[id]
                .pixel_count()
                .map_err(|failure| failure.message)?;
            if pixels > MAX_DECODED_SOURCE_PIXELS_TOTAL {
                return Err(RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(id.to_string()),
                    None,
                    "As fontes de uma unidade excedem o limite de memória.",
                ));
            }
        }
        self.rasters.retain(|id, _| required.contains(id));
        let missing = ordered
            .into_iter()
            .filter(|id| !self.rasters.contains_key(id))
            .collect::<Vec<_>>();
        // Isolated progressive decoders own a separate worker budget and stay
        // sequential. In-process decoders run in parallel: with the retained
        // rasters they stay within the unit's pixel ceiling, below the peak
        // later reached while composing the Sheet.
        let (isolated, in_process): (Vec<_>, Vec<_>) = missing
            .into_iter()
            .partition(|id| captured[id].decodes_in_isolated_worker());
        for id in isolated {
            let raster = decode(captured, id)?;
            self.rasters.insert(id, raster);
        }
        for (id, raster) in decode_parallel(captured, &in_process, self.workers) {
            self.rasters.insert(id, raster?);
        }
        Ok(&self.rasters)
    }
}

fn decode(
    captured: &HashMap<MediaId, OpenRenderSource>,
    id: MediaId,
) -> Result<RgbaImage, RenderFailure> {
    captured[&id].decode_captured().map_err(|failure| {
        RenderFailure::typed(
            failure.code,
            Some(id.to_string()),
            failure.path_code,
            failure.message,
        )
    })
}

fn decode_parallel(
    captured: &HashMap<MediaId, OpenRenderSource>,
    ids: &[MediaId],
    workers: usize,
) -> Vec<(MediaId, Result<RgbaImage, RenderFailure>)> {
    if ids.len() <= 1 || workers == 1 {
        return ids.iter().map(|id| (*id, decode(captured, *id))).collect();
    }
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(ids.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers.min(ids.len()) {
            scope.spawn(|| {
                while let Some(id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let decoded = decode(captured, *id);
                    results
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((*id, decoded));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_else(PoisonError::into_inner);
    // Report the first failure in media order, as sequential decoding did.
    results.sort_by_key(|(id, _)| id.to_string());
    results
}

/// Translate the already-composed physical geometry. Clipping occurs while rasterizing
/// the page, preserving the original photo/background mapping across the center.
fn translate_viewport(unit: &mut ComposedOutputUnit, view: &RectUm) {
    let shift = |rect: &mut RectUm| {
        rect.x -= view.x;
        rect.y -= view.y;
    };
    let sheet = &mut unit.sheet;
    sheet.width_um = view.width;
    sheet.height_um = view.height;
    sheet.base.draw_rect = RectUm {
        x: 0,
        y: 0,
        width: view.width,
        height: view.height,
    };
    for background in &mut sheet.backgrounds {
        match background {
            ComposedBackground::Color { draw_rect, .. } => shift(draw_rect),
            ComposedBackground::Media {
                draw_rect,
                clip_rect,
                ..
            } => {
                shift(draw_rect);
                if let Some(clip) = clip_rect {
                    shift(clip);
                }
            }
        }
    }
    for frame in &mut sheet.frames {
        shift(&mut frame.clip_rect);
        for rect in &mut frame.border_fill_rects {
            shift(rect);
        }
        if let Some(photo) = &mut frame.photo {
            shift(&mut photo.draw_rect);
        }
    }
    for overlay in &mut sheet.overlays {
        shift(&mut overlay.draw_rect);
        if let Some(clip) = &mut overlay.clip_rect {
            shift(clip);
        }
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, ExportMode, InitialProject, ProjectCore,
        ProjectLocation,
    };
    use myalbuns_imaging_protocol::{AlbumRenderOutput, IMAGING_PROTOCOL_VERSION};
    use myalbuns_paths::{NativePathDto, OperationPathContext, test_support::DiskFull};

    #[test]
    fn encoder_disk_full_returns_only_finished_receipts_and_retries_the_incomplete_file() {
        for format in [
            RenderFormat::Jpeg { quality: 100 },
            RenderFormat::Png,
            RenderFormat::Pdf,
        ] {
            let root = tempfile::tempdir().unwrap();
            let project_path = root.path().join("test.myalbuns");
            let mut paths = OperationPathContext::new();
            paths.capture(&project_path).unwrap();
            let project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(project_path, paths.freeze()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            let mut snapshot = project.render_snapshot();
            snapshot.dpi = 4;
            let ids = snapshot
                .composition
                .sheets
                .iter()
                .map(|sheet| sheet.sheet_id.clone())
                .collect::<Vec<_>>();
            let units = snapshot.export_units(&ids, ExportMode::Sheet).unwrap();
            let groups = if format == RenderFormat::Pdf {
                vec![units]
            } else {
                units.into_iter().map(|unit| vec![unit]).collect()
            };
            let mut paths = OperationPathContext::new();
            let outputs = groups
                .into_iter()
                .enumerate()
                .map(|(index, units)| {
                    let path = root
                        .path()
                        .join(format!("prepared-{index}.{}", format.extension()));
                    paths.capture(&path).unwrap();
                    AlbumRenderOutput {
                        prepared_path: NativePathDto::from(path),
                        units,
                    }
                })
                .collect::<Vec<_>>();
            let failed_index = usize::from(outputs.len() > 1);
            let failed_path = outputs[failed_index].prepared_path.as_path().to_owned();
            let mut request = AlbumRenderRequest {
                protocol_version: IMAGING_PROTOCOL_VERSION,
                request_id: "recovery-test".into(),
                snapshot,
                format,
                outputs,
                sources: vec![],
                root_bindings: paths.freeze(),
            };
            let fault = DiskFull::after_bytes(&failed_path, 32);
            let mut completed = Vec::new();
            let failure =
                render_retaining(&request, &mut |_, _, _| Ok(()), &mut completed).unwrap_err();
            assert_eq!(failure.failure.code, ImagingFailureCode::OutputStorageFull);
            assert!(fault.failure_count() > 0);
            assert_eq!(completed.len(), failed_index);
            drop(fault);
            let earlier = (failed_index > 0)
                .then(|| std::fs::read(request.outputs[0].prepared_path.as_path()).unwrap());
            if failed_path.exists() {
                std::fs::remove_file(&failed_path).unwrap();
            }
            request.outputs.drain(..failed_index);
            let result = render(&request, &mut |_, _, _| Ok(()))
                .unwrap_or_else(|error| panic!("{}", error.message));
            assert_eq!(result.outputs.len(), request.outputs.len());
            if let Some(earlier) = earlier {
                assert_eq!(
                    std::fs::read(
                        root.path()
                            .join(format!("prepared-0.{}", request.format.extension()))
                    )
                    .unwrap(),
                    earlier
                );
            }
        }
    }
}

#[cfg(test)]
mod source_retention_tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_paths::OperationPathContext;

    const IDS: [&str; 3] = [
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b01",
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b02",
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b03",
    ];

    fn captured(root: &std::path::Path) -> HashMap<MediaId, OpenRenderSource> {
        let mut context = OperationPathContext::new();
        let paths = IDS
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let path = root.join(format!("photo-{index}.jpg"));
                RgbImage::from_fn(40 + index as u32, 30, |x, y| {
                    Rgb([x as u8, y as u8, index as u8 * 60])
                })
                .save_with_format(&path, ImageFormat::Jpeg)
                .unwrap();
                context.capture(&path).unwrap();
                path
            })
            .collect::<Vec<_>>();
        let plan = context.freeze();
        IDS.iter()
            .zip(paths)
            .map(|(id, path)| {
                let resolved = plan
                    .resolve_existing(&path, ExpectedObject::RegularFile)
                    .unwrap();
                (
                    id.parse().unwrap(),
                    capture_render_source(&resolved).unwrap(),
                )
            })
            .collect()
    }

    fn failed<T>(failure: RenderFailure) -> T {
        panic!("{}", failure.message)
    }

    fn ids(indices: &[usize]) -> HashSet<MediaId> {
        indices
            .iter()
            .map(|index| IDS[*index].parse().unwrap())
            .collect()
    }

    #[test]
    fn consecutive_units_decode_each_retained_original_once() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        // One worker keeps decoding on this thread, where the counter lives.
        let mut decoded = DecodedSources::new(1);
        let before = crate::source::jpeg_decode_count();
        // Both pages of a Page export require the same Sheet sources.
        for _ in 0..2 {
            let sources = decoded
                .prepare(&captured, &ids(&[0, 1]))
                .unwrap_or_else(failed);
            assert_eq!(sources.len(), 2);
        }
        assert_eq!(crate::source::jpeg_decode_count() - before, 2);
        // A neighbour sharing one Original decodes only the new one and
        // releases the source it no longer references.
        let sources = decoded
            .prepare(&captured, &ids(&[1, 2]))
            .unwrap_or_else(failed);
        assert_eq!(
            sources.keys().copied().collect::<HashSet<_>>(),
            ids(&[1, 2])
        );
        assert_eq!(crate::source::jpeg_decode_count() - before, 3);
    }

    #[test]
    fn parallel_decoding_returns_the_same_rasters() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let serial = DecodedSources::new(1)
            .prepare(&captured, &ids(&[0, 1, 2]))
            .unwrap_or_else(failed)
            .clone();
        let mut parallel = DecodedSources::new(3);
        let parallel = parallel
            .prepare(&captured, &ids(&[0, 1, 2]))
            .unwrap_or_else(failed);
        assert_eq!(*parallel, serial);
    }
}
