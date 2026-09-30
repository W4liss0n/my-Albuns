use crate::{
    format_output::{PdfOutput, write_png},
    jpeg_output::write_verified_quality,
    render::{LayerSources, RenderFailure, composition_workers, render_unit},
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
    collections::{HashMap, VecDeque},
    sync::{
        Condvar, Mutex, PoisonError,
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
    let mut units = Vec::with_capacity(request.outputs.len());
    for output in &request.outputs {
        let mut output_units = Vec::with_capacity(output.units.len());
        for selected in &output.units {
            let mut unit = request
                .snapshot
                .output_unit(&selected.sheet_id)
                .map_err(|error| error.to_string())?;
            translate_viewport(&mut unit, &selected.viewport);
            output_units.push(unit);
        }
        units.push(output_units);
    }
    let timeline = units
        .iter()
        .flatten()
        .flat_map(|unit| unit.sheet.referenced_media_ids())
        .collect::<Vec<_>>();
    let order = first_use_order(&timeline);
    // Reading ahead only pays where reading is slow: Originals on a share.
    let prefetch = request
        .sources
        .iter()
        .any(|source| request.root_bindings.is_remote(source.source_path()))
        .then(Prefetch::default);
    let captured = &captured;
    std::thread::scope(|scope| -> Result<AlbumRenderCompletion, RenderFailure> {
        let _stop = prefetch.as_ref().map(|prefetch| {
            let order = &order;
            scope.spawn(move || prefetch.run(captured, order, PREFETCH_BYTES));
            StopPrefetch(prefetch)
        });
        let mut decoded = DecodedSources::new(
            captured,
            timeline,
            composition_workers(),
            MAX_DECODED_SOURCE_PIXELS_TOTAL,
        )?
        .with_prefetch(prefetch.as_ref());
        for (output, output_units) in request.outputs.iter().zip(&units) {
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
            for (selected, unit) in output.units.iter().zip(output_units) {
                progress(ImagingProgressStage::Composing, done, total)?;
                let image =
                    render_unit(unit, request.snapshot.dpi, &mut decoded, &mut |_, _, _| {
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
                receipt = Some(pdf.finish()?);
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
    })
}

/// Compressed bytes read ahead and not yet decoded stay under this size.
const PREFETCH_BYTES: u64 = 128 * 1024 * 1024;

/// Reads the compressed bytes of upcoming Originals on its own thread, in the
/// order they are first needed, while earlier units are composed and encoded.
/// On a network share reading is the slowest step of an Export; this lets the
/// transfer overlap the CPU work instead of alternating with it. Decoding takes
/// the bytes when they are ready, waits while they are being read, and reads
/// the file itself for a source not read ahead. Only compressed bytes are kept,
/// within `PREFETCH_BYTES`, so the pixel budget of the unit is unchanged.
#[derive(Default)]
struct Prefetch {
    state: Mutex<PrefetchState>,
    changed: Condvar,
}

#[derive(Default)]
struct PrefetchState {
    sources: HashMap<MediaId, Prefetched>,
    held: u64,
    stopped: bool,
}

enum Prefetched {
    Reading,
    Ready(Vec<u8>),
    /// Decoding claimed the source; it is not read ahead again.
    Taken,
}

impl Prefetch {
    fn lock(&self) -> std::sync::MutexGuard<'_, PrefetchState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn run(&self, captured: &HashMap<MediaId, OpenRenderSource>, order: &[MediaId], budget: u64) {
        for id in order {
            let size = captured[id].byte_count();
            {
                let mut state = self.lock();
                while !state.stopped && state.held > 0 && state.held + size > budget {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                if state.stopped {
                    return;
                }
                if state.sources.contains_key(id) {
                    continue;
                }
                state.sources.insert(*id, Prefetched::Reading);
            }
            let read = captured[id].read_captured();
            let mut state = self.lock();
            match read {
                Ok(bytes) => {
                    state.held += bytes.len() as u64;
                    state.sources.insert(*id, Prefetched::Ready(bytes));
                }
                // Decoding reads the file itself and reports the failure.
                Err(_) => {
                    state.sources.remove(id);
                }
            }
            drop(state);
            self.changed.notify_all();
        }
    }

    /// The bytes read ahead for `id`, waiting while they are being read, or
    /// `None` when decoding must read the file itself.
    fn take(&self, id: MediaId) -> Option<Vec<u8>> {
        let mut state = self.lock();
        loop {
            match state.sources.get(&id) {
                None | Some(Prefetched::Taken) => {
                    state.sources.insert(id, Prefetched::Taken);
                    return None;
                }
                Some(Prefetched::Reading) => {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(Prefetched::Ready(_)) => {
                    let Some(Prefetched::Ready(bytes)) =
                        state.sources.insert(id, Prefetched::Taken)
                    else {
                        unreachable!("the entry was just seen ready");
                    };
                    state.held -= bytes.len() as u64;
                    drop(state);
                    self.changed.notify_all();
                    return Some(bytes);
                }
            }
        }
    }

    fn stop(&self) {
        self.lock().stopped = true;
        self.changed.notify_all();
    }
}

/// Stops the reader thread however the render ends, before its scope joins it.
struct StopPrefetch<'a>(&'a Prefetch);

impl Drop for StopPrefetch<'_> {
    fn drop(&mut self) {
        self.0.stop();
    }
}

/// Each source once, in the order the layers first need it.
fn first_use_order(timeline: &[MediaId]) -> Vec<MediaId> {
    let mut seen = std::collections::HashSet::new();
    timeline
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect()
}

/// Decoded Originals for every media layer of the request, in paint order.
/// Each raster is decoded before its first layer and dropped after its last,
/// so Page exports and Decoratives repeated on neighbouring Sheets decode the
/// Original once. The rasters held at any moment stay within `budget` pixels:
/// upcoming sources are decoded ahead, in parallel, only while they fit, and
/// under pressure the raster needed furthest ahead is dropped and decoded
/// again later. A Sheet therefore has no limit on the combined size of its
/// sources; only each single source must fit the budget.
struct DecodedSources<'a> {
    captured: &'a HashMap<MediaId, OpenRenderSource>,
    timeline: Vec<MediaId>,
    position: usize,
    uses: HashMap<MediaId, VecDeque<usize>>,
    pixels: HashMap<MediaId, u64>,
    rasters: HashMap<MediaId, RgbaImage>,
    resident: u64,
    budget: u64,
    workers: usize,
    prefetch: Option<&'a Prefetch>,
}

impl<'a> DecodedSources<'a> {
    fn new(
        captured: &'a HashMap<MediaId, OpenRenderSource>,
        timeline: Vec<MediaId>,
        workers: usize,
        budget: u64,
    ) -> Result<Self, RenderFailure> {
        let mut uses = HashMap::<MediaId, VecDeque<usize>>::new();
        for (position, id) in timeline.iter().enumerate() {
            uses.entry(*id).or_default().push_back(position);
        }
        let mut ordered = uses.keys().copied().collect::<Vec<_>>();
        ordered.sort_by_key(ToString::to_string);
        let mut pixels = HashMap::with_capacity(ordered.len());
        for id in ordered {
            let source = captured
                .get(&id)
                .ok_or_else(|| format!("a fonte da mídia {id} não foi capturada"))?;
            let count = source.pixel_count().map_err(|failure| failure.message)?;
            if count > budget {
                return Err(RenderFailure::typed(
                    ImagingFailureCode::ResourceLimitExceeded,
                    Some(id.to_string()),
                    None,
                    "Uma fonte excede o limite de memória.",
                ));
            }
            pixels.insert(id, count);
        }
        Ok(Self {
            captured,
            timeline,
            position: 0,
            uses,
            pixels,
            rasters: HashMap::new(),
            resident: 0,
            budget,
            workers: workers.max(1),
            prefetch: None,
        })
    }

    fn with_prefetch(mut self, prefetch: Option<&'a Prefetch>) -> Self {
        self.prefetch = prefetch;
        self
    }

    fn expect_current(&self, media_id: MediaId) -> Result<(), RenderFailure> {
        if self.timeline.get(self.position) != Some(&media_id) {
            return Err("a ordem das fontes divergiu da composição"
                .to_string()
                .into());
        }
        Ok(())
    }

    fn next_use(&self, media_id: &MediaId) -> usize {
        self.uses
            .get(media_id)
            .and_then(|uses| uses.front().copied())
            .unwrap_or(usize::MAX)
    }

    fn load(&mut self, media_id: MediaId) -> Result<(), RenderFailure> {
        let needed = self.pixels[&media_id];
        while self.resident + needed > self.budget {
            let victim = self
                .rasters
                .keys()
                .copied()
                .max_by_key(|id| (self.next_use(id), id.to_string()))
                .expect("a resident raster exists while the budget is exceeded");
            self.rasters.remove(&victim);
            self.resident -= self.pixels[&victim];
        }
        // Decode ahead, in paint order, the upcoming sources that still fit.
        let mut batch = vec![media_id];
        let mut planned = self.resident + needed;
        for id in &self.timeline[self.position + 1..] {
            if self.rasters.contains_key(id) || batch.contains(id) {
                continue;
            }
            let pixels = self.pixels[id];
            if planned + pixels > self.budget {
                break;
            }
            planned += pixels;
            batch.push(*id);
        }
        // Isolated progressive decoders own a separate worker budget and stay
        // sequential; in-process decoders run in parallel.
        let (isolated, in_process): (Vec<_>, Vec<_>) = batch
            .iter()
            .copied()
            .partition(|id| self.captured[id].decodes_in_isolated_worker());
        let mut decoded = decode_parallel(self.captured, self.prefetch, &in_process, self.workers);
        for id in isolated {
            decoded.push((id, decode(self.captured, self.prefetch, id)));
        }
        // Report the failure of the earliest layer.
        decoded.sort_by_key(|(id, _)| batch.iter().position(|queued| queued == id));
        for (id, raster) in decoded {
            self.rasters.insert(id, raster?);
            self.resident += self.pixels[&id];
        }
        Ok(())
    }
}

impl LayerSources for DecodedSources<'_> {
    fn acquire(&mut self, media_id: MediaId) -> Result<&RgbaImage, RenderFailure> {
        self.expect_current(media_id)?;
        if !self.rasters.contains_key(&media_id) {
            self.load(media_id)?;
        }
        Ok(&self.rasters[&media_id])
    }

    fn release(&mut self, media_id: MediaId) -> Result<(), RenderFailure> {
        self.expect_current(media_id)?;
        let uses = self
            .uses
            .get_mut(&media_id)
            .expect("every layer in the timeline has a use");
        uses.pop_front();
        if uses.is_empty() && self.rasters.remove(&media_id).is_some() {
            self.resident -= self.pixels[&media_id];
        }
        self.position += 1;
        Ok(())
    }
}

fn decode(
    captured: &HashMap<MediaId, OpenRenderSource>,
    prefetch: Option<&Prefetch>,
    id: MediaId,
) -> Result<RgbaImage, RenderFailure> {
    let source = &captured[&id];
    match prefetch.and_then(|prefetch| prefetch.take(id)) {
        Some(bytes) => source.decode_read(bytes),
        None => source.decode_captured(),
    }
    .map_err(|failure| {
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
    prefetch: Option<&Prefetch>,
    ids: &[MediaId],
    workers: usize,
) -> Vec<(MediaId, Result<RgbaImage, RenderFailure>)> {
    if ids.len() <= 1 || workers == 1 {
        return ids
            .iter()
            .map(|id| (*id, decode(captured, prefetch, *id)))
            .collect();
    }
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::with_capacity(ids.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers.min(ids.len()) {
            scope.spawn(|| {
                while let Some(id) = ids.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let decoded = decode(captured, prefetch, *id);
                    results
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push((*id, decoded));
                }
            });
        }
    });
    results.into_inner().unwrap_or_else(PoisonError::into_inner)
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
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat, ImportPhoto,
        InitialProject, InitialProjectConfiguration, PhotoPlacementMode, PhotoSourceMetadata,
        ProjectCore, ProjectIntent, ProjectLocation,
    };
    use myalbuns_paths::OperationPathContext;

    const IDS: [&str; 3] = [
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b01",
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b02",
        "8f6d3a53-5a6f-4b11-9d1e-6a0d2f5c7b03",
    ];

    fn photo(index: usize) -> RgbImage {
        RgbImage::from_fn(40 + index as u32, 30, |x, y| {
            Rgb([(x * 5) as u8, (y * 7) as u8, index as u8 * 60])
        })
    }

    fn capture(paths: &[(MediaId, std::path::PathBuf)]) -> HashMap<MediaId, OpenRenderSource> {
        let mut context = OperationPathContext::new();
        for (_, path) in paths {
            context.capture(path).unwrap();
        }
        let plan = context.freeze();
        paths
            .iter()
            .map(|(id, path)| {
                let resolved = plan
                    .resolve_existing(path, ExpectedObject::RegularFile)
                    .unwrap();
                (*id, capture_render_source(&resolved).unwrap())
            })
            .collect()
    }

    fn captured(root: &std::path::Path) -> HashMap<MediaId, OpenRenderSource> {
        let paths = IDS
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let path = root.join(format!("photo-{index}.jpg"));
                photo(index)
                    .save_with_format(&path, ImageFormat::Jpeg)
                    .unwrap();
                (id.parse().unwrap(), path)
            })
            .collect::<Vec<_>>();
        capture(&paths)
    }

    fn failed<T>(failure: RenderFailure) -> T {
        panic!("{}", failure.message)
    }

    fn timeline(indices: &[usize]) -> Vec<MediaId> {
        indices
            .iter()
            .map(|index| IDS[*index].parse().unwrap())
            .collect()
    }

    /// Paints every layer of `timeline` and returns the largest number of
    /// decoded pixels held at once.
    fn walk(sources: &mut DecodedSources<'_>, timeline: &[MediaId]) -> u64 {
        let mut peak = 0;
        for id in timeline {
            sources.acquire(*id).unwrap_or_else(failed);
            peak = peak.max(sources.resident);
            sources.release(*id).unwrap_or_else(failed);
        }
        assert!(sources.rasters.is_empty());
        assert_eq!(sources.resident, 0);
        peak
    }

    #[test]
    fn consecutive_units_decode_each_original_once_while_it_fits() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        // Both pages of a Page export use the same Sheet sources; the next
        // Sheet shares one Original.
        let layers = timeline(&[0, 1, 0, 1, 1, 2]);
        // One worker keeps decoding on this thread, where the counter lives.
        let mut sources =
            DecodedSources::new(&captured, layers.clone(), 1, u64::MAX).unwrap_or_else(failed);
        let before = crate::source::jpeg_decode_count();
        walk(&mut sources, &layers);
        assert_eq!(crate::source::jpeg_decode_count() - before, 3);
    }

    #[test]
    fn a_budget_below_the_unit_streams_sources_and_never_exceeds_it() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let largest = captured
            .values()
            .map(|source| source.pixel_count().unwrap())
            .max()
            .unwrap();
        // Only one source fits at a time; the first returns after the others.
        let layers = timeline(&[0, 1, 2, 0]);
        let mut sources =
            DecodedSources::new(&captured, layers.clone(), 1, largest).unwrap_or_else(failed);
        let before = crate::source::jpeg_decode_count();
        let peak = walk(&mut sources, &layers);
        assert!(peak <= largest);
        assert_eq!(crate::source::jpeg_decode_count() - before, 4);
    }

    #[test]
    fn a_single_source_above_the_budget_is_rejected_before_decoding() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let before = crate::source::jpeg_decode_count();
        let Err(failure) = DecodedSources::new(&captured, timeline(&[0, 2]), 1, 40 * 30) else {
            panic!("the 42x30 source exceeds a 40x30 budget");
        };
        assert_eq!(
            failure.failure.code,
            ImagingFailureCode::ResourceLimitExceeded
        );
        assert_eq!(failure.failure.media_id.as_deref(), Some(IDS[2]));
        assert_eq!(crate::source::jpeg_decode_count(), before);
    }

    #[test]
    fn sources_read_ahead_decode_to_the_same_rasters_from_memory() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let layers = timeline(&[0, 1, 0, 2]);
        let rasters = |prefetch: Option<&Prefetch>| {
            let mut sources = DecodedSources::new(&captured, layers.clone(), 1, u64::MAX)
                .unwrap_or_else(failed)
                .with_prefetch(prefetch);
            layers
                .iter()
                .map(|id| {
                    let raster = sources.acquire(*id).unwrap_or_else(failed).clone();
                    sources.release(*id).unwrap_or_else(failed);
                    raster
                })
                .collect::<Vec<_>>()
        };
        let prefetch = Prefetch::default();
        prefetch.run(&captured, &first_use_order(&layers), u64::MAX);
        assert!(
            prefetch
                .lock()
                .sources
                .values()
                .all(|source| matches!(source, Prefetched::Ready(_)))
        );
        assert!(rasters(Some(&prefetch)) == rasters(None));
        let state = prefetch.lock();
        assert_eq!(state.held, 0, "every source read ahead was decoded");
        assert!(
            state
                .sources
                .values()
                .all(|source| matches!(source, Prefetched::Taken))
        );
    }

    #[test]
    fn reading_ahead_waits_for_decoding_once_its_budget_is_full() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let order = first_use_order(&timeline(&[0, 1, 2]));
        let first = captured[&order[0]].byte_count();
        let prefetch = Prefetch::default();
        std::thread::scope(|scope| {
            let reader = scope.spawn(|| prefetch.run(&captured, &order, first));
            let ready = || {
                prefetch
                    .lock()
                    .sources
                    .values()
                    .filter(|source| matches!(source, Prefetched::Ready(_)))
                    .count()
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while ready() == 0 && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert_eq!(ready(), 1, "the budget holds one source");
            assert!(prefetch.take(order[0]).is_some());
            while !prefetch.lock().sources.contains_key(&order[1])
                && std::time::Instant::now() < deadline
            {
                std::thread::yield_now();
            }
            assert!(
                prefetch.take(order[1]).is_some(),
                "decoding waits for a source being read"
            );
            prefetch.stop();
            reader.join().unwrap();
        });
        assert!(
            prefetch.take(order[0]).is_none(),
            "a source decoded once is read from its file again"
        );
    }

    #[test]
    fn parallel_decoding_returns_the_same_rasters() {
        let root = tempfile::tempdir().unwrap();
        let captured = captured(root.path());
        let layers = timeline(&[0, 1, 2]);
        let rasters = |workers| {
            let mut sources = DecodedSources::new(&captured, layers.clone(), workers, u64::MAX)
                .unwrap_or_else(failed);
            layers
                .iter()
                .map(|id| {
                    let raster = sources.acquire(*id).unwrap_or_else(failed).clone();
                    sources.release(*id).unwrap_or_else(failed);
                    raster
                })
                .collect::<Vec<_>>()
        };
        assert!(rasters(1) == rasters(3));
    }

    #[test]
    fn a_sheet_larger_than_the_budget_composes_the_same_pixels() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Orcamento.myalbuns");
        let mut context = OperationPathContext::new();
        context.capture(&path).unwrap();
        let mut project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(path, context.freeze()),
                InitialProject::configured(InitialProjectConfiguration::new(
                    DisplayUnit::Mm,
                    60_000,
                    30_000,
                    150,
                    0,
                    0,
                    2,
                    EndSheetFormat::Double,
                    EndSheetFormat::Double,
                )),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let sheet_id = project.projection().state.album.sheets[0].id.clone();
        let mut paths = Vec::new();
        for index in 0..4 {
            let path = root.path().join(format!("frame-{index}.png"));
            let raster = photo(index);
            raster.save_with_format(&path, ImageFormat::Png).unwrap();
            let imported = project
                .import_photo(ImportPhoto::new(
                    path.clone(),
                    PhotoSourceMetadata::new(
                        raster.width(),
                        raster.height(),
                        ["#111111", "#888888", "#EEEEEE"].map(String::from),
                    )
                    .unwrap(),
                ))
                .unwrap();
            project
                .apply(ProjectIntent::AddPhoto {
                    sheet_id: sheet_id.clone(),
                    media_id: imported.media_id,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
            paths.push((imported.media_id, path));
        }
        let captured = capture(&paths);
        let unit = project.render_snapshot().output_unit(&sheet_id).unwrap();
        let layers = unit.sheet.referenced_media_ids().collect::<Vec<_>>();
        assert_eq!(layers.len(), 4);
        let largest = captured
            .values()
            .map(|source| source.pixel_count().unwrap())
            .max()
            .unwrap();
        let render = |budget| {
            let mut sources =
                DecodedSources::new(&captured, layers.clone(), 2, budget).unwrap_or_else(failed);
            render_unit(&unit, 150, &mut sources, &mut |_, _, _| Ok(())).unwrap_or_else(failed)
        };
        assert!(render(largest) == render(u64::MAX));
    }
}
