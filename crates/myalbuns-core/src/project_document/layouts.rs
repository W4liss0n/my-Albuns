use super::*;
use crate::{
    CoreError, FrameOrientation, LayoutPatch, LayoutQuery, LayoutSettings, LayoutSurface,
    LayoutSurfaceKind, StoredLayout,
};

impl ProjectSheet {
    pub fn layout_locked(&self) -> bool {
        self.layout_locked
    }

    pub fn last_layout(&self) -> Option<&StoredLayout> {
        self.last_layout.as_ref()
    }
}

impl ProjectDocument {
    pub(crate) fn restore_layout_locks(mut self, locks: Vec<bool>) -> Result<Self, ()> {
        if locks.len() != self.sheets.len() {
            return Err(());
        }
        for (sheet, locked) in self.sheets.iter_mut().zip(locks) {
            sheet.layout_locked = locked;
        }
        validate_project_state(&self)?;
        Ok(self)
    }

    pub(crate) fn validate_locked_structure(&self, next: &Self) -> Result<(), CoreError> {
        for sheet in self.sheets.iter().filter(|sheet| sheet.layout_locked) {
            let Some(updated) = next.sheets.iter().find(|updated| updated.id == sheet.id) else {
                continue;
            };
            // Converting an edge explicitly replaces its active surface and unlocks it.
            if updated.active_sides != sheet.active_sides {
                continue;
            }
            if sheet.frames.len() != updated.frames.len()
                || sheet.frames.iter().any(|frame| {
                    !updated
                        .frames
                        .iter()
                        .any(|other| other.id == frame.id && other.rect == frame.rect)
                })
            {
                return Err(CoreError::LayoutLocked);
            }
        }
        Ok(())
    }

    pub(crate) fn with_layout_unlocked(&self, sheet_id: &str) -> Result<Self, CoreError> {
        let mut next = self.clone();
        let sheet = next
            .sheets
            .iter_mut()
            .find(|s| s.id.to_string() == sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.into()))?;
        sheet.layout_locked = false;
        Ok(next)
    }

    pub(crate) fn with_locked_layout_patch(
        &self,
        sheet_id: Uuid,
        patch: &LayoutPatch,
    ) -> Result<Self, CoreError> {
        self.ensure_layout_unlocked(sheet_id)?;
        let mut next = self.with_layout_preview(sheet_id, patch)?;
        next.sheets
            .iter_mut()
            .find(|s| s.id == sheet_id)
            .unwrap()
            .layout_locked = true;
        Ok(next)
    }

    pub fn layout_settings(&self) -> &LayoutSettings {
        &self.layout_settings
    }

    pub(crate) fn current_layout(&self, sheet_id: Uuid) -> Result<StoredLayout, CoreError> {
        let query = self.layout_query(sheet_id)?;
        let sheet = self
            .sheets
            .iter()
            .find(|sheet| sheet.id == sheet_id)
            .unwrap();
        let positions: Vec<crate::RectUm> =
            sheet.frames.iter().map(|frame| frame.rect.into()).collect();
        let crosses = query.surface.kind == LayoutSurfaceKind::DoubleSheet
            && positions.iter().any(|r| {
                2 * r.x < query.surface.width_um && 2 * (r.x + r.width) > query.surface.width_um
            });
        Ok(StoredLayout {
            definition: crate::LayoutDefinition {
                surface: query.surface,
                scope: if crosses {
                    crate::LayoutScope::Sheet
                } else {
                    crate::LayoutScope::Page
                },
                positions,
            },
            origin: sheet
                .last_layout
                .as_ref()
                .map_or(crate::LayoutOrigin::Automatic, |last| last.origin),
        })
    }

    pub(crate) fn ensure_layout_unlocked(&self, sheet_id: Uuid) -> Result<(), CoreError> {
        let sheet = self
            .sheets
            .iter()
            .find(|sheet| sheet.id == sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.to_string()))?;
        if sheet.layout_locked {
            return Err(CoreError::LayoutLocked);
        }
        Ok(())
    }

    pub(super) fn layout_state_is_valid(&self) -> bool {
        self.layout_settings.parameters.is_valid()
            && self.sheets.iter().all(|sheet| {
                (!sheet.layout_locked
                    || sheet.last_layout.as_ref().is_some_and(|layout| {
                        !sheet.frames.is_empty()
                            && layout.definition.positions.len() == sheet.frames.len()
                    }))
                    && sheet.last_layout.as_ref().is_none_or(|layout| {
                        crate::LayoutRules::definition_is_valid(&layout.definition)
                    })
            })
    }

    pub(crate) fn restore_layout_state(
        mut self,
        settings: LayoutSettings,
        last_layouts: Vec<Option<StoredLayout>>,
    ) -> Result<Self, ()> {
        if last_layouts.len() != self.sheets.len() {
            return Err(());
        }
        self.layout_settings = settings;
        for (sheet, last) in self.sheets.iter_mut().zip(last_layouts) {
            sheet.last_layout = last;
        }
        validate_project_state(&self)?;
        Ok(self)
    }

    pub(crate) fn layout_query(&self, sheet_id: Uuid) -> Result<LayoutQuery, CoreError> {
        let sheet = self
            .sheets
            .iter()
            .find(|s| s.id == sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.to_string()))?;
        Ok(LayoutQuery {
            surface: LayoutSurface {
                kind: if sheet.active_sides == ActiveSides::Both {
                    LayoutSurfaceKind::DoubleSheet
                } else {
                    LayoutSurfaceKind::SinglePage
                },
                width_um: active_surface_width(sheet, self.document.sheet_width_um) as i64,
                height_um: self.document.sheet_height_um as i64,
            },
            frame_orientations: sheet
                .frames
                .iter()
                .map(|f| match f.rect.width.cmp(&f.rect.height) {
                    std::cmp::Ordering::Less => FrameOrientation::Vertical,
                    std::cmp::Ordering::Greater => FrameOrientation::Horizontal,
                    std::cmp::Ordering::Equal => FrameOrientation::Square,
                })
                .collect(),
            permission: self.layout_settings.permission,
            parameters: self.layout_settings.parameters.clone(),
        })
    }

    pub(crate) fn with_layout_settings(&self, settings: LayoutSettings) -> Result<Self, CoreError> {
        if !settings.parameters.is_valid() {
            return Err(CoreError::InvalidLayoutQuery);
        }
        let mut next = self.clone();
        next.layout_settings = settings;
        Ok(next)
    }

    pub(crate) fn with_layout_patch(
        &self,
        sheet_id: Uuid,
        patch: &LayoutPatch,
    ) -> Result<Self, CoreError> {
        self.ensure_layout_unlocked(sheet_id)?;
        if !patch.placeholder_ids().is_empty() {
            return Err(CoreError::LayoutRequiresLock);
        }
        self.with_layout_preview(sheet_id, patch)
    }

    pub(crate) fn with_layout_preview(
        &self,
        sheet_id: Uuid,
        patch: &LayoutPatch,
    ) -> Result<Self, CoreError> {
        let mut next = self.clone();
        next.sheets
            .iter_mut()
            .find(|sheet| sheet.id == sheet_id)
            .ok_or(CoreError::StaleLayoutPreview)?
            .layout_locked = false;
        next.apply_layout_patch(sheet_id, patch)?;
        Ok(next)
    }

    pub(super) fn reorganize_sheet(&mut self, sheet_id: Uuid) -> Result<(), CoreError> {
        let query = self.layout_query(sheet_id)?;
        let sheet = self.sheets.iter().find(|s| s.id == sheet_id).unwrap();
        let ids = sheet.frames.iter().map(|f| f.id).collect::<Vec<_>>();
        let patch = crate::LayoutRules::automatic(&query, sheet.last_layout(), &ids)?;
        self.apply_layout_patch(sheet_id, &patch)
    }

    fn apply_layout_patch(&mut self, sheet_id: Uuid, patch: &LayoutPatch) -> Result<(), CoreError> {
        let sheet = self
            .sheets
            .iter_mut()
            .find(|s| s.id == sheet_id)
            .ok_or(CoreError::StaleLayoutPreview)?;
        if sheet.layout_locked {
            return Err(CoreError::LayoutLocked);
        }
        if !sheet
            .frames
            .iter()
            .map(|f| f.id)
            .eq(patch.frame_ids().iter().copied())
        {
            return Err(CoreError::StaleLayoutPreview);
        }
        for (frame, rect) in sheet.frames.iter_mut().zip(&patch.definition().positions) {
            frame.rect = ProjectRect::new(
                rect.x as u64,
                rect.y as u64,
                rect.width as u64,
                rect.height as u64,
            );
        }
        for (id, rect) in patch
            .placeholder_ids()
            .iter()
            .zip(patch.definition().positions.iter().skip(sheet.frames.len()))
        {
            sheet.frames.push(ProjectFrame::new(
                *id,
                ProjectRect::new(
                    rect.x as u64,
                    rect.y as u64,
                    rect.width as u64,
                    rect.height as u64,
                ),
                None,
            ));
        }
        if let Some(last) = patch.last_layout() {
            sheet.last_layout = Some(last.clone());
        }
        validate_project_state(self).map_err(|_| CoreError::IncompatibleLayout)
    }
}
