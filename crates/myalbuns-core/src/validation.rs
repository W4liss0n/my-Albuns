use std::collections::HashSet;

use crate::composition::compose_frame_border_fill_rects;
use crate::model::{
    ComposedBackground, ComposedOutputUnit, ComposedSheet, CoreError, ProjectedFrameBorder,
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
                    ("Background".to_owned(), draw_rect)
                }
                ComposedBackground::Media {
                    media_id,
                    draw_rect,
                    ..
                } => (media_id.to_string(), draw_rect),
            };
            validate_rect_within(
                draw_rect,
                sheet.width_um,
                sheet.height_um,
                "Background composto",
                &id,
                CoreError::InvalidSnapshot,
            )?;
        }
        for overlay in &sheet.overlays {
            let media_id = overlay.media_id.to_string();
            validate_rect_within(
                &overlay.draw_rect,
                sheet.width_um,
                sheet.height_um,
                "Decorativo composto",
                &media_id,
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
            if frame.border_fill_rects
                != compose_frame_border_fill_rects(&frame.clip_rect, frame_border)
            {
                return Err(CoreError::InvalidSnapshot(format!(
                    "plano de Borda inválido para o Frame {}",
                    frame.frame_id
                )));
            }
            let stack_key = (frame.z_index, frame.frame_id.as_str());
            if previous_stack_key.is_some_and(|previous| previous > stack_key) {
                return Err(CoreError::InvalidSnapshot(format!(
                    "Pilha visual de Frames inválida na Lâmina {}",
                    sheet.sheet_id
                )));
            }
            previous_stack_key = Some(stack_key);

            if let Some(photo) = &frame.photo {
                let media_id = photo.media_id.to_string();
                validate_positive_dimensions(
                    photo.draw_rect.width,
                    photo.draw_rect.height,
                    "Foto composta",
                    &media_id,
                    CoreError::InvalidSnapshot,
                )?;
                if !photo.rotation_degrees.is_finite() {
                    return Err(CoreError::InvalidSnapshot(format!(
                        "rotação inválida para a Foto {}",
                        photo.media_id
                    )));
                }
                validate_palette(&photo.palette, &media_id, CoreError::InvalidSnapshot)?;
            }
        }
    }

    validate_frame_border(frame_border, CoreError::InvalidSnapshot)?;
    Ok(())
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
