use super::*;
use crate::{
    CoreError, FrameOrientation, LayoutPatch, LayoutQuery, LayoutSettings, LayoutSurface,
    LayoutSurfaceKind, StoredLayout,
};

impl ProjectSheet {
    pub fn last_layout(&self) -> Option<&StoredLayout> {
        self.last_layout.as_ref()
    }
}

impl ProjectDocument {
    pub fn layout_settings(&self) -> &LayoutSettings {
        &self.layout_settings
    }

    pub(super) fn layout_state_is_valid(&self) -> bool {
        self.layout_settings.parameters.is_valid()
            && self.sheets.iter().all(|sheet| {
                sheet.last_layout.as_ref().is_none_or(|layout| {
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
        let mut next = self.clone();
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
        if let Some(last) = patch.last_layout() {
            sheet.last_layout = Some(last.clone());
        }
        validate_project_state(self).map_err(|_| CoreError::IncompatibleLayout)
    }
}
