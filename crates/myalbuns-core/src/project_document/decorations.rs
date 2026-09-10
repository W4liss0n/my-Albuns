use super::*;
use crate::{CoreError, DecorativeRole, DecorativeScope, SheetVisuals};

impl ProjectSheet {
    pub fn visuals(&self) -> &SheetVisuals {
        &self.visuals
    }
}

impl ProjectDocument {
    pub(crate) fn decorative_drop_zone(
        &self,
        request: &crate::DecorativeDropRequest,
    ) -> Result<Option<crate::sheet_visuals::DecorativeDropZone>, CoreError> {
        let sheet = self
            .sheets
            .iter()
            .find(|sheet| sheet.id.to_string() == request.sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(request.sheet_id.clone()))?;
        let width = self.document.sheet_width_um() as i64
            / if sheet.active_sides == ActiveSides::Both {
                1
            } else {
                2
            };
        let height = self.document.sheet_height_um() as i64;
        if request.x_um < 0 || request.x_um >= width || request.y_um < 0 || request.y_um >= height {
            return Ok(None);
        }
        let (scope, x, zone_width, center) = match sheet.active_sides {
            ActiveSides::Left => (DecorativeScope::Left, 0, width, None),
            ActiveSides::Right => (DecorativeScope::Right, 0, width, None),
            ActiveSides::Both => {
                let center_left = width * 2 / 5;
                let center_right = width * 3 / 5;
                let center = Some(crate::RectUm {
                    x: center_left,
                    y: 0,
                    width: center_right - center_left,
                    height,
                });
                if request.x_um < center_left {
                    (DecorativeScope::Left, 0, center_left, center)
                } else if request.x_um < center_right {
                    (
                        DecorativeScope::BothSides,
                        center_left,
                        center_right - center_left,
                        center,
                    )
                } else {
                    (
                        DecorativeScope::Right,
                        center_right,
                        width - center_right,
                        center,
                    )
                }
            }
        };
        Ok(Some(crate::sheet_visuals::DecorativeDropZone {
            scope,
            rect: crate::RectUm {
                x,
                y: 0,
                width: zone_width,
                height,
            },
            center,
        }))
    }

    pub(crate) fn with_dropped_decorative(
        &self,
        request: &crate::DecorativeDropRequest,
    ) -> Result<Self, CoreError> {
        let Some(zone) = self.decorative_drop_zone(request)? else {
            return Ok(self.clone());
        };
        self.with_applied_decorative(
            &request.sheet_id,
            request.media_id,
            request.role,
            zone.scope,
        )
    }

    pub(crate) fn with_applied_decorative(
        &self,
        sheet_id: &str,
        media_id: MediaId,
        role: DecorativeRole,
        scope: DecorativeScope,
    ) -> Result<Self, CoreError> {
        if !self
            .media
            .iter()
            .any(|item| item.id == media_id.into_uuid() && item.kind == MediaKind::Decorative)
        {
            return Err(CoreError::MediaNotFound(media_id.to_string()));
        }
        let mut next = self.clone();
        let sheet = next
            .sheets
            .iter_mut()
            .find(|sheet| sheet.id.to_string() == sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.into()))?;
        if matches!(
            (sheet.active_sides, scope),
            (ActiveSides::Left, DecorativeScope::Right)
                | (ActiveSides::Right, DecorativeScope::Left)
        ) {
            return Err(CoreError::InvalidProject(
                "Este lado da Lâmina está desativado.".into(),
            ));
        }
        match role {
            DecorativeRole::Background => sheet
                .visuals
                .background
                .apply(scope, ProjectedBackgroundContent::Media { media_id }),
            DecorativeRole::Overlay => sheet
                .visuals
                .overlay
                .apply(scope, Some(ProjectedOverlayContent::Media { media_id })),
        }
        Ok(next)
    }

    pub(crate) fn restore_sheet_visuals(
        mut self,
        entries: Vec<(Uuid, SheetVisuals)>,
    ) -> Result<Self, ()> {
        let mut seen = HashSet::new();
        for (id, visuals) in entries {
            if !seen.insert(id) {
                return Err(());
            }
            self.sheets
                .iter_mut()
                .find(|sheet| sheet.id == id)
                .ok_or(())?
                .visuals = visuals;
        }
        validate_project_state(&self)?;
        Ok(self)
    }

    pub(crate) fn sheet_visuals_are_valid(&self) -> bool {
        let valid_media = |id: &MediaId| {
            self.media
                .iter()
                .any(|media| media.id == id.into_uuid() && media.kind == MediaKind::Decorative)
        };
        self.sheets.iter().all(|sheet| {
            sheet
                .visuals
                .background
                .contents()
                .into_iter()
                .all(|content| match content {
                    ProjectedBackgroundContent::Color { rgb } => {
                        Rgb::parse_canonical(rgb).is_some()
                    }
                    ProjectedBackgroundContent::Media { media_id } => valid_media(media_id),
                })
                && sheet
                    .visuals
                    .overlay
                    .contents()
                    .into_iter()
                    .all(|content| match content {
                        None => true,
                        Some(ProjectedOverlayContent::Media { media_id }) => valid_media(media_id),
                    })
        })
    }
}
