use super::*;
use crate::{CoreError, MediaRemovalMode};

impl ProjectDocument {
    pub(crate) fn with_removed_media(
        &self,
        media_ids: &[MediaId],
        mode: MediaRemovalMode,
    ) -> Result<Self, CoreError> {
        let selected = media_ids
            .iter()
            .map(|id| id.into_uuid())
            .collect::<HashSet<_>>();
        let mut kind = None;
        for id in &selected {
            let media = self
                .media
                .iter()
                .find(|media| media.id == *id)
                .ok_or_else(|| CoreError::MediaNotFound(id.to_string()))?;
            if kind.is_some_and(|kind| kind != media.kind) {
                return Err(CoreError::InvalidProject(
                    "Selecione imagens de uma única aba para remover.".into(),
                ));
            }
            kind = Some(media.kind);
        }
        let mut next = self.clone();
        for sheet in &mut next.sheets {
            if mode == MediaRemovalMode::RemoveAll && !sheet.layout_locked {
                sheet.frames.retain(|frame| {
                    !frame
                        .photo
                        .as_ref()
                        .is_some_and(|photo| selected.contains(&photo.media_id))
                });
            } else {
                for frame in &mut sheet.frames {
                    if frame
                        .photo
                        .as_ref()
                        .is_some_and(|photo| selected.contains(&photo.media_id))
                    {
                        frame.photo = None;
                    }
                }
            }
        }
        let remove_background = |content: &mut BackgroundContent| {
            if matches!(content, BackgroundContent::Media { media_id } if selected.contains(media_id))
            {
                *content = BackgroundContent::Color { rgb: Rgb::WHITE };
            }
        };
        match &mut next.visual_defaults.background {
            Background::BothSides { both } => remove_background(both),
            Background::PerSide { left, right } => {
                remove_background(left);
                remove_background(right);
            }
        }
        let remove_overlay = |content: &mut Option<OverlayContent>| {
            if matches!(content, Some(OverlayContent::Media { media_id }) if selected.contains(media_id))
            {
                *content = None;
            }
        };
        match &mut next.visual_defaults.overlay {
            Overlay::BothSides { both } => remove_overlay(both),
            Overlay::PerSide { left, right } => {
                remove_overlay(left);
                remove_overlay(right);
            }
        }
        next.media.retain(|media| !selected.contains(&media.id));
        validate_project_state(&next).map_err(|()| {
            CoreError::InvalidProject(
                "A remoção deixaria uma referência inválida no Projeto.".into(),
            )
        })?;
        Ok(next)
    }
}
