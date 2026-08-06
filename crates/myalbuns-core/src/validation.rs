use std::collections::{HashMap, HashSet};

use crate::model::{
    AlbumSnapshot, ComposedBackground, ComposedOutputUnit, ComposedSheet, CoreError, MediaKind,
    PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN, ProjectedBackground,
    ProjectedBackgroundContent, ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent,
    RENDER_SNAPSHOT_SCHEMA_VERSION, RectUm, RenderSnapshot,
};
use crate::project_document::{Rgb, frame_border_width_is_valid};

pub(crate) fn validate_render_snapshot(snapshot: &RenderSnapshot) -> Result<(), CoreError> {
    if snapshot.schema_version != RENDER_SNAPSHOT_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema(snapshot.schema_version));
    }
    if snapshot.project_id.trim().is_empty() {
        return Err(CoreError::InvalidSnapshot(
            "a Identidade do Projeto está vazia".into(),
        ));
    }
    if snapshot.unit != "micrometers" {
        return Err(CoreError::InvalidSnapshot(format!(
            "unidade não suportada: {}",
            snapshot.unit
        )));
    }
    if !(1..=1_200).contains(&snapshot.dpi) {
        return Err(CoreError::InvalidSnapshot(
            "a resolução da Exportação é inválida".into(),
        ));
    }
    if snapshot.composition.sheets.is_empty() {
        return Err(CoreError::InvalidSnapshot(
            "a composição não contém Lâminas".into(),
        ));
    }

    validate_composed_content(
        &snapshot.composition.frame_border,
        &snapshot.composition.sheets,
    )
}

pub(crate) fn validate_composed_output_unit(unit: &ComposedOutputUnit) -> Result<(), CoreError> {
    validate_composed_content(&unit.frame_border, std::slice::from_ref(&unit.sheet))
}

fn validate_composed_content(
    frame_border: &ProjectedFrameBorder,
    sheets: &[ComposedSheet],
) -> Result<(), CoreError> {
    let mut sheet_ids = HashSet::new();
    let mut frame_ids = HashSet::new();
    for sheet in sheets {
        if sheet.sheet_id.trim().is_empty() || !sheet_ids.insert(sheet.sheet_id.as_str()) {
            return Err(CoreError::InvalidSnapshot(format!(
                "Identificador de Lâmina vazio ou duplicado: {}",
                sheet.sheet_id
            )));
        }
        validate_positive_dimensions(
            sheet.width_um,
            sheet.height_um,
            "Lâmina",
            &sheet.sheet_id,
            CoreError::InvalidSnapshot,
        )?;
        validate_rect_within(
            &sheet.base.draw_rect,
            sheet.width_um,
            sheet.height_um,
            "Base composta",
            &sheet.sheet_id,
            CoreError::InvalidSnapshot,
        )?;
        validate_canonical_color(&sheet.base.rgb, CoreError::InvalidSnapshot)?;
        for background in &sheet.backgrounds {
            let (id, draw_rect) = match background {
                ComposedBackground::Color { rgb, draw_rect } => {
                    validate_canonical_color(rgb, CoreError::InvalidSnapshot)?;
                    ("Background", draw_rect)
                }
                ComposedBackground::Media {
                    media_id,
                    draw_rect,
                    ..
                } => {
                    if media_id.trim().is_empty() {
                        return Err(CoreError::InvalidSnapshot(
                            "Identificador de Background vazio".into(),
                        ));
                    }
                    (media_id.as_str(), draw_rect)
                }
            };
            validate_rect_within(
                draw_rect,
                sheet.width_um,
                sheet.height_um,
                "Background composto",
                id,
                CoreError::InvalidSnapshot,
            )?;
        }
        for overlay in &sheet.overlays {
            if overlay.media_id.trim().is_empty() {
                return Err(CoreError::InvalidSnapshot(
                    "Identificador de Decorativo vazio".into(),
                ));
            }
            validate_rect_within(
                &overlay.draw_rect,
                sheet.width_um,
                sheet.height_um,
                "Decorativo composto",
                &overlay.media_id,
                CoreError::InvalidSnapshot,
            )?;
        }

        let mut previous_stack_key: Option<(u32, &str)> = None;
        for frame in &sheet.frames {
            if frame.frame_id.trim().is_empty() || !frame_ids.insert(frame.frame_id.as_str()) {
                return Err(CoreError::InvalidSnapshot(format!(
                    "Identificador de Frame vazio ou duplicado: {}",
                    frame.frame_id
                )));
            }
            validate_rect_within(
                &frame.clip_rect,
                sheet.width_um,
                sheet.height_um,
                "Frame",
                &frame.frame_id,
                CoreError::InvalidSnapshot,
            )?;
            let stack_key = (frame.z_index, frame.frame_id.as_str());
            if previous_stack_key.is_some_and(|previous| previous > stack_key) {
                return Err(CoreError::InvalidSnapshot(format!(
                    "Pilha visual de Frames inválida na Lâmina {}",
                    sheet.sheet_id
                )));
            }
            previous_stack_key = Some(stack_key);

            if let Some(photo) = &frame.photo {
                if photo.media_id.trim().is_empty() {
                    return Err(CoreError::InvalidSnapshot(
                        "Identificador de Foto vazio".into(),
                    ));
                }
                validate_positive_dimensions(
                    photo.draw_rect.width,
                    photo.draw_rect.height,
                    "Foto composta",
                    &photo.media_id,
                    CoreError::InvalidSnapshot,
                )?;
                if !photo.rotation_degrees.is_finite() {
                    return Err(CoreError::InvalidSnapshot(format!(
                        "rotação inválida para a Foto {}",
                        photo.media_id
                    )));
                }
                validate_palette(&photo.palette, &photo.media_id, CoreError::InvalidSnapshot)?;
            }
        }
    }

    validate_frame_border(frame_border, CoreError::InvalidSnapshot)?;
    Ok(())
}

pub(crate) fn validate_album(album: &AlbumSnapshot) -> Result<(), CoreError> {
    if album.sheets.len() < 2 {
        return Err(CoreError::InvalidProject(
            "o Álbum precisa conter pelo menos duas Lâminas".into(),
        ));
    }

    let mut media_by_id = HashMap::new();
    for media in &album.media {
        if media.id.trim().is_empty() || media_by_id.insert(media.id.as_str(), media.kind).is_some()
        {
            return Err(CoreError::InvalidProject(format!(
                "Identificador de mídia vazio ou duplicado: {}",
                media.id
            )));
        }
        match media.kind {
            MediaKind::Photo => {
                let (Some(width), Some(height), Some(palette)) = (
                    media.source_width_px,
                    media.source_height_px,
                    media.palette.as_ref(),
                ) else {
                    return Err(CoreError::InvalidProject(format!(
                        "metadados de origem ausentes para a Foto {}",
                        media.id
                    )));
                };
                if width == 0 || height == 0 {
                    return Err(CoreError::InvalidProject(format!(
                        "dimensões de origem inválidas para a Foto {}",
                        media.id
                    )));
                }
                validate_palette(palette, &media.id, CoreError::InvalidProject)?;
            }
            MediaKind::Decorative => {
                let metadata_count = usize::from(media.source_width_px.is_some())
                    + usize::from(media.source_height_px.is_some())
                    + usize::from(media.palette.is_some());
                if !matches!(metadata_count, 0 | 3)
                    || media.source_width_px.is_some_and(|width| width == 0)
                    || media.source_height_px.is_some_and(|height| height == 0)
                {
                    return Err(CoreError::InvalidProject(format!(
                        "metadados derivados incompletos para o Decorativo {}",
                        media.id
                    )));
                }
                if let Some(palette) = &media.palette {
                    validate_palette(palette, &media.id, CoreError::InvalidProject)?;
                }
            }
        }
    }

    validate_visual_defaults(album, &media_by_id)?;

    let mut sheet_ids = HashSet::new();
    let mut frame_ids = HashSet::new();
    for sheet in &album.sheets {
        if sheet.id.trim().is_empty() || !sheet_ids.insert(sheet.id.as_str()) {
            return Err(CoreError::InvalidProject(format!(
                "Identificador de Lâmina vazio ou duplicado: {}",
                sheet.id
            )));
        }
        validate_positive_dimensions(
            sheet.width_um,
            sheet.height_um,
            "Lâmina",
            &sheet.id,
            CoreError::InvalidProject,
        )?;
        for frame in &sheet.frames {
            if frame.id.trim().is_empty() || !frame_ids.insert(frame.id.as_str()) {
                return Err(CoreError::InvalidProject(format!(
                    "Identificador de Frame vazio ou duplicado: {}",
                    frame.id
                )));
            }
            validate_rect_within(
                &frame.rect,
                sheet.width_um,
                sheet.height_um,
                "Frame",
                &frame.id,
                CoreError::InvalidProject,
            )?;

            if let Some(photo) = &frame.photo {
                match media_by_id.get(photo.media_id.as_str()) {
                    Some(MediaKind::Photo) => {}
                    Some(MediaKind::Decorative) => {
                        return Err(CoreError::InvalidProject(format!(
                            "Frame referencia um Decorativo: {}",
                            photo.media_id
                        )));
                    }
                    None => {
                        return Err(CoreError::InvalidProject(format!(
                            "Foto não pertence ao catálogo: {}",
                            photo.media_id
                        )));
                    }
                }
                let transform = &photo.transform;
                if !transform.pan_x.is_finite()
                    || !transform.pan_y.is_finite()
                    || !transform.user_zoom.is_finite()
                    || !transform.fine_rotation_degrees.is_finite()
                    || !(PHOTO_PAN_MIN..=PHOTO_PAN_MAX).contains(&transform.pan_x)
                    || !(PHOTO_PAN_MIN..=PHOTO_PAN_MAX).contains(&transform.pan_y)
                    || !(PHOTO_ZOOM_MIN..=PHOTO_ZOOM_MAX).contains(&transform.user_zoom)
                {
                    return Err(CoreError::InvalidProject(format!(
                        "transformação inválida para a Foto {}",
                        photo.media_id
                    )));
                }
            }
        }
    }

    Ok(())
}

fn validate_visual_defaults(
    album: &AlbumSnapshot,
    media_by_id: &HashMap<&str, MediaKind>,
) -> Result<(), CoreError> {
    let validate_background = |content: &ProjectedBackgroundContent| match content {
        ProjectedBackgroundContent::Color { rgb } => {
            validate_canonical_color(rgb, CoreError::InvalidProject)
        }
        ProjectedBackgroundContent::Media { media_id } => {
            validate_decorative_reference(media_id, "Background", media_by_id)
        }
    };
    match &album.visual_defaults.background {
        ProjectedBackground::BothSides { both } => validate_background(both)?,
        ProjectedBackground::PerSide { left, right } => {
            validate_background(left)?;
            validate_background(right)?;
        }
    }

    let validate_overlay = |content: &ProjectedOverlayContent| match content {
        ProjectedOverlayContent::Media { media_id } => {
            validate_decorative_reference(media_id, "Overlay", media_by_id)
        }
    };
    match &album.visual_defaults.overlay {
        ProjectedOverlay::BothSides { both } => {
            if let Some(content) = both {
                validate_overlay(content)?;
            }
        }
        ProjectedOverlay::PerSide { left, right } => {
            if let Some(content) = left {
                validate_overlay(content)?;
            }
            if let Some(content) = right {
                validate_overlay(content)?;
            }
        }
    }

    validate_frame_border(
        &album.visual_defaults.frame_border,
        CoreError::InvalidProject,
    )
}

fn validate_decorative_reference(
    media_id: &str,
    role: &str,
    media_by_id: &HashMap<&str, MediaKind>,
) -> Result<(), CoreError> {
    match media_by_id.get(media_id) {
        Some(MediaKind::Decorative) => Ok(()),
        Some(MediaKind::Photo) => Err(CoreError::InvalidProject(format!(
            "{role} referencia uma Foto: {media_id}"
        ))),
        None => Err(CoreError::InvalidProject(format!(
            "Decorativo não pertence ao catálogo: {media_id}"
        ))),
    }
}

fn validate_frame_border(
    border: &ProjectedFrameBorder,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    match border {
        ProjectedFrameBorder::None => Ok(()),
        ProjectedFrameBorder::Solid { rgb, width_um } => {
            validate_canonical_color(rgb, error)?;
            if !frame_border_width_is_valid(*width_um) {
                return Err(error("espessura de Borda inválida".into()));
            }
            Ok(())
        }
    }
}

fn validate_canonical_color(rgb: &str, error: fn(String) -> CoreError) -> Result<(), CoreError> {
    if Rgb::parse_canonical(rgb).is_none() {
        return Err(error(format!("cor canônica inválida: {rgb}")));
    }
    Ok(())
}

fn validate_positive_dimensions(
    width: i64,
    height: i64,
    kind: &str,
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    if width <= 0 || height <= 0 {
        return Err(error(format!("dimensões inválidas para {kind} {id}")));
    }
    Ok(())
}

fn validate_rect_within(
    rect: &RectUm,
    surface_width: i64,
    surface_height: i64,
    kind: &str,
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    validate_positive_dimensions(rect.width, rect.height, kind, id, error)?;
    let right = rect.x.checked_add(rect.width);
    let bottom = rect.y.checked_add(rect.height);
    if rect.x < 0
        || rect.y < 0
        || right.is_none_or(|value| value > surface_width)
        || bottom.is_none_or(|value| value > surface_height)
    {
        return Err(error(format!(
            "{kind} {id} ultrapassa a superfície da Lâmina"
        )));
    }
    Ok(())
}

fn validate_palette(
    palette: &[String; 3],
    id: &str,
    error: fn(String) -> CoreError,
) -> Result<(), CoreError> {
    if palette.iter().any(|color| {
        color.len() != 7
            || !color.starts_with('#')
            || !color[1..].bytes().all(|value| value.is_ascii_hexdigit())
    }) {
        return Err(error(format!("paleta inválida para {id}")));
    }
    Ok(())
}
