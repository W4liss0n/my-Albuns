use crate::{
    format_output::{PdfOutput, write_png},
    jpeg_output::write_verified_quality,
    render::{RenderFailure, render_unit},
    source::{MAX_DECODED_SOURCE_PIXELS_TOTAL, capture_render_source},
};
use myalbuns_core::{ComposedBackground, ComposedOutputUnit, RectUm};
use myalbuns_imaging_protocol::{
    AlbumRenderCompletion, AlbumRenderRequest, ImagingFailureCode, ImagingPathCode,
    ImagingProgressStage, RenderCompletion, RenderFormat,
};
use myalbuns_paths::ExpectedObject;
use std::collections::{HashMap, HashSet};

pub(crate) fn render(
    request: &AlbumRenderRequest,
    progress: &mut dyn FnMut(ImagingProgressStage, u32, u32) -> Result<(), String>,
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
    let mut outputs = Vec::new();
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
            let mut sources = HashMap::new();
            let mut pixels = 0_u64;
            for id in required {
                let source = &captured[&id];
                pixels += source.pixel_count().map_err(|failure| failure.message)?;
                if pixels > MAX_DECODED_SOURCE_PIXELS_TOTAL {
                    return Err(RenderFailure::typed(
                        ImagingFailureCode::ResourceLimitExceeded,
                        Some(id.to_string()),
                        None,
                        "As fontes de uma unidade excedem o limite de memória.",
                    ));
                }
                sources.insert(
                    id,
                    source.decode_captured().map_err(|failure| {
                        RenderFailure::typed(
                            failure.code,
                            Some(id.to_string()),
                            failure.path_code,
                            failure.message,
                        )
                    })?,
                );
            }
            translate_viewport(&mut unit, &selected.viewport);
            progress(ImagingProgressStage::Composing, done, total)?;
            let image = render_unit(&unit, request.snapshot.dpi, &sources, &mut |_, _, _| {
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
    Ok(AlbumRenderCompletion { outputs })
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
