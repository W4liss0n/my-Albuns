use std::collections::HashSet;

use crate::model::{
    AlbumSnapshot, CoreError, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN,
    PROJECT_SCHEMA_VERSION, RectUm, RenderSnapshot,
};

pub(crate) fn validate_render_snapshot(snapshot: &RenderSnapshot) -> Result<(), CoreError> {
    if snapshot.schema_version != PROJECT_SCHEMA_VERSION {
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
    if snapshot.composition.sheets.is_empty() {
        return Err(CoreError::InvalidSnapshot(
            "a composição não contém Lâminas".into(),
        ));
    }

    let mut sheet_ids = HashSet::new();
    let mut frame_ids = HashSet::new();
    for sheet in &snapshot.composition.sheets {
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

    Ok(())
}

pub(crate) fn validate_album(album: &AlbumSnapshot) -> Result<(), CoreError> {
    if album.sheets.len() < 2 {
        return Err(CoreError::InvalidProject(
            "o Álbum precisa conter pelo menos duas Lâminas".into(),
        ));
    }

    let mut media_ids = HashSet::new();
    for media in &album.media {
        if media.id.trim().is_empty() || !media_ids.insert(media.id.as_str()) {
            return Err(CoreError::InvalidProject(format!(
                "Identificador de Foto vazio ou duplicado: {}",
                media.id
            )));
        }
        validate_palette(&media.palette, &media.id, CoreError::InvalidProject)?;
    }

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
                if !media_ids.contains(photo.media_id.as_str()) {
                    return Err(CoreError::InvalidProject(format!(
                        "Foto não pertence ao catálogo: {}",
                        photo.media_id
                    )));
                }
                if photo.source_width_px == 0 || photo.source_height_px == 0 {
                    return Err(CoreError::InvalidProject(format!(
                        "dimensões de origem inválidas para a Foto {}",
                        photo.media_id
                    )));
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
                validate_palette(&photo.palette, &photo.media_id, CoreError::InvalidProject)?;
            }
        }
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
