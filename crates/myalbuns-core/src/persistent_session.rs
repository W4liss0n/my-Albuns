use crate::{
    model::{CoreError, ProjectIntent, RelinkMedia},
    project_document::{FrameClipboard, MAX_SAFE_INTEGER, ProjectDocument, ProjectRevision},
};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProjectIntentOutcome {
    pub(crate) affected_frame_id: Option<Uuid>,
    pub(crate) affected_sheet_id: Option<Uuid>,
    pub(crate) affected_frame_ids: Option<Vec<Uuid>>,
}

#[derive(Clone, Debug)]
pub(crate) struct PersistentProjectSession {
    current: ProjectRevision,
    latest_revision: u64,
    saved_revision: u64,
    schema_upgrade_required: bool,
    recovered_unsaved: bool,
    undo: Vec<ProjectRevision>,
    redo: Vec<ProjectRevision>,
    frame_clipboard: Option<FrameClipboard>,
    prepared_layout_query: Option<PreparedLayoutQuery>,
    layout_catalog: crate::LayoutCatalogSnapshot,
}

#[derive(Clone, Debug)]
struct PreparedLayoutQuery {
    id: String,
    revision: u64,
    sheet_id: Uuid,
    frame_ids: Vec<Uuid>,
    patches: Vec<crate::LayoutPatch>,
}

impl PersistentProjectSession {
    pub(crate) fn from_persisted(current: ProjectRevision, schema_upgrade_required: bool) -> Self {
        let saved_revision = current.revision;
        let latest_revision = current.revision;
        Self {
            current,
            latest_revision,
            saved_revision,
            schema_upgrade_required,
            recovered_unsaved: false,
            undo: Vec::new(),
            redo: Vec::new(),
            frame_clipboard: None,
            prepared_layout_query: None,
            layout_catalog: crate::LayoutCatalogSnapshot::default(),
        }
    }

    pub(crate) fn from_recovery(current: ProjectRevision, saved_revision: u64) -> Self {
        let latest_revision = current.revision.max(saved_revision);
        Self {
            current,
            latest_revision,
            saved_revision,
            schema_upgrade_required: false,
            recovered_unsaved: true,
            undo: Vec::new(),
            redo: Vec::new(),
            frame_clipboard: None,
            prepared_layout_query: None,
            layout_catalog: crate::LayoutCatalogSnapshot::default(),
        }
    }

    pub(crate) fn project_id(&self) -> Uuid {
        self.current.project_id
    }

    pub(crate) fn revision(&self) -> u64 {
        self.current.revision
    }

    pub(crate) fn saved_revision(&self) -> u64 {
        self.saved_revision
    }

    pub(crate) fn project(&self) -> &ProjectDocument {
        &self.current.project
    }

    pub(crate) fn current_revision(&self) -> ProjectRevision {
        self.current.clone()
    }

    pub(crate) fn has_unsaved_changes(&self) -> bool {
        self.recovered_unsaved || self.current.revision != self.saved_revision
    }

    pub(crate) fn requires_save(&self) -> bool {
        self.has_unsaved_changes() || self.schema_upgrade_required
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn can_paste_frames(&self) -> bool {
        self.frame_clipboard.is_some()
    }

    pub(crate) fn apply(
        &mut self,
        intent: ProjectIntent,
    ) -> Result<ProjectIntentOutcome, CoreError> {
        let mut outcome = ProjectIntentOutcome::default();
        if let ProjectIntent::AddFrame { sheet_id } | ProjectIntent::PasteFrames { sheet_id, .. } =
            &intent
        {
            let id =
                parse_uuid(sheet_id).map_err(|_| CoreError::SheetNotFound(sheet_id.clone()))?;
            self.project().ensure_layout_unlocked(id)?;
        }
        if let ProjectIntent::AddPhoto { sheet_id, .. } = &intent
            && let Some(sheet) = self
                .project()
                .sheets()
                .iter()
                .find(|sheet| sheet.id().to_string() == *sheet_id)
            && sheet.layout_locked()
            && sheet.frames().iter().all(|frame| frame.photo().is_some())
        {
            return Err(CoreError::LockedLayoutHasNoPlaceholder);
        }
        if let ProjectIntent::UnlockLayout { sheet_id } = &intent {
            let next = self.project().with_layout_unlocked(sheet_id)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::ApplyLayout { selection } | ProjectIntent::LockLayout { selection } =
            &intent
        {
            let (sheet_id, patch) = self.checked_layout_patch(selection)?;
            let next = if matches!(intent, ProjectIntent::LockLayout { .. }) {
                self.project().with_locked_layout_patch(sheet_id, patch)?
            } else {
                self.project().with_layout_patch(sheet_id, patch)?
            };
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            outcome.affected_sheet_id = Some(sheet_id);
            return Ok(outcome);
        }
        if let ProjectIntent::SetLayoutSettings { settings } = &intent {
            let next = self.project().with_layout_settings(settings.clone())?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::SetFrameStyle { edit } = &intent {
            let next = self.project().with_frame_style(edit)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::TogglePhotoBlackAndWhite { frame_ids } = &intent {
            let next = self
                .project()
                .with_toggled_photo_black_and_white(frame_ids)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::SetPhotoAngle { edit } = &intent {
            let next = self.project().with_photo_angle(edit)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::OrientPhotos { frame_ids, action } = &intent {
            let next = self.project().with_oriented_photos(frame_ids, *action)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::SwapSheetSides { sheet_id } = &intent {
            let next = self.project().with_swapped_sheet_sides(sheet_id)?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::CopyFrames { frame_ids } = &intent {
            self.frame_clipboard = Some(self.project().copy_frames(frame_ids)?);
            return Ok(outcome);
        }
        if let ProjectIntent::PasteFrames {
            sheet_id,
            desired_offset_um,
        } = &intent
        {
            let clipboard = self
                .frame_clipboard
                .clone()
                .ok_or(CoreError::FrameClipboardEmpty)?;
            self.commit_edit(|project| {
                let (next, ids) =
                    project.with_pasted_frames(&clipboard, sheet_id, *desired_offset_um)?;
                outcome.affected_frame_ids = Some(ids);
                Ok(next)
            })?;
            return Ok(outcome);
        }
        if let ProjectIntent::ArrangeFrames { frame_ids, action } = &intent {
            let next = self.project().with_arranged_frames(frame_ids, *action)?;
            if next == *self.project() {
                return Ok(outcome);
            }
        }
        if let ProjectIntent::DeleteFrames { frame_ids, mode } = &intent {
            let next = self.project().with_deleted_frames(
                frame_ids,
                *mode,
                &self.layout_catalog.entries,
            )?;
            if next != *self.project() {
                self.commit_edit(|_| Ok(next))?;
            }
            return Ok(outcome);
        }
        if let ProjectIntent::EditFrameGeometry { edit } = &intent {
            let rects = self.project().frame_geometry_edit(edit)?;
            if rects
                .iter()
                .zip(&edit.frames)
                .all(|((_, rect), target)| crate::RectUm::from(*rect) == target.expected_rect)
            {
                return Ok(outcome);
            }
        }
        let custom = self.layout_catalog.entries.clone();
        self.commit_edit(|project| match intent {
            ProjectIntent::ApplyLayout { .. }
            | ProjectIntent::LockLayout { .. }
            | ProjectIntent::UnlockLayout { .. }
            | ProjectIntent::SetLayoutSettings { .. } => {
                unreachable!("Layout commands validate the captured query before committing")
            }
            ProjectIntent::SetFrameStyle { .. } => {
                unreachable!("Frame style handles unchanged selections before committing")
            }
            ProjectIntent::TogglePhotoBlackAndWhite { .. } => {
                unreachable!("Photo effects handle unchanged selections before committing")
            }
            ProjectIntent::SetPhotoAngle { .. } => {
                unreachable!("Photo angle handles unchanged compositions before committing")
            }
            ProjectIntent::OrientPhotos { .. } => {
                unreachable!("Photo orientation handles unchanged compositions before committing")
            }
            ProjectIntent::SwapSheetSides { .. } => {
                unreachable!("side swapping handles unchanged compositions before committing")
            }
            ProjectIntent::CopyFrames { .. } | ProjectIntent::PasteFrames { .. } => {
                unreachable!("clipboard commands are handled before document intents")
            }
            ProjectIntent::SwapFrameContents { frame_ids } => {
                project.with_swapped_frame_contents(&frame_ids)
            }
            ProjectIntent::DeleteFrames { .. } => {
                unreachable!("Frame deletion commits its prepared document once")
            }
            ProjectIntent::ArrangeFrames { frame_ids, action } => {
                project.with_arranged_frames(&frame_ids, action)
            }
            ProjectIntent::EditFrameGeometry { edit } => project.with_edited_frame_geometry(&edit),
            ProjectIntent::SetAlbumInformation { information } => project
                .with_album_information(information, &custom)
                .map_err(CoreError::InvalidAlbumInformation),
            ProjectIntent::SetVisualDefaults { visual_defaults } => project
                .with_visual_defaults(visual_defaults)
                .map_err(|()| CoreError::InvalidVisualDefaults),
            ProjectIntent::SetDpi { dpi } => project
                .with_dpi(dpi)
                .map_err(|()| CoreError::InvalidDpi(dpi)),
            ProjectIntent::AddSheet {
                anchor_sheet_id,
                position,
            } => {
                let parsed = parse_uuid(&anchor_sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(anchor_sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(anchor_sheet_id));
                }
                let (next, sheet_id) = project
                    .with_added_sheet(parsed, position)
                    .map_err(|()| CoreError::InvalidSheetInsertion)?;
                outcome.affected_sheet_id = Some(sheet_id);
                Ok(next)
            }
            ProjectIntent::DeleteSheet { sheet_id } => {
                let parsed = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(sheet_id));
                }
                if project.sheets().len() <= 2 {
                    return Err(CoreError::MinimumSheetCount);
                }
                let (next, neighbor_id) = project.with_deleted_sheet(parsed).map_err(|()| {
                    CoreError::InvalidProject("a Lâmina não pode ser excluída".into())
                })?;
                outcome.affected_sheet_id = Some(neighbor_id);
                Ok(next)
            }
            ProjectIntent::ConvertEdgeSheet { sheet_id } => {
                let parsed = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(sheet_id));
                }
                let next = project
                    .with_converted_edge_sheet(parsed, &custom)
                    .map_err(|()| CoreError::InvalidEdgeConversion)?;
                outcome.affected_sheet_id = Some(parsed);
                Ok(next)
            }
            ProjectIntent::ReorderSheet {
                sheet_id,
                target_index,
            } => {
                let parsed = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(sheet_id));
                }
                let next = project
                    .with_reordered_sheet(parsed, target_index)
                    .map_err(|()| CoreError::InvalidSheetReorder)?;
                outcome.affected_sheet_id = Some(parsed);
                Ok(next)
            }
            ProjectIntent::TransformPhoto {
                frame_id,
                delta_pan_x,
                delta_pan_y,
                delta_zoom,
            } => {
                let parsed = parse_uuid(&frame_id)
                    .map_err(|()| CoreError::FrameNotFound(frame_id.clone()))?;
                let next = project
                    .with_transformed_photo(parsed, delta_pan_x, delta_pan_y, delta_zoom)
                    .map_err(|()| CoreError::FrameNotFound(frame_id))?;
                outcome.affected_frame_id = Some(parsed);
                Ok(next)
            }
            ProjectIntent::AddFrame { sheet_id } => {
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                let (next, frame_id) = project
                    .with_added_frame(parsed_sheet)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id))?;
                outcome.affected_frame_id = Some(frame_id);
                Ok(next)
            }
            ProjectIntent::AddPhoto {
                sheet_id,
                media_id,
                mode,
            } => {
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                let (next, frame_id) = project
                    .with_added_photo(parsed_sheet, media_id.into_uuid(), mode, &custom)
                    .map_err(|()| {
                        CoreError::InvalidProject(
                            "não foi possível adicionar a Foto à Lâmina".into(),
                        )
                    })?;
                outcome.affected_frame_id = Some(frame_id);
                Ok(next)
            }
            ProjectIntent::DropPhoto {
                sheet_id,
                media_id,
                x_um,
                y_um,
                mode,
            } => {
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                let (next, frame_id) = project
                    .with_dropped_photo(
                        parsed_sheet,
                        media_id.into_uuid(),
                        x_um,
                        y_um,
                        mode,
                        &custom,
                    )
                    .map_err(|()| {
                        CoreError::InvalidProject("o alvo da Foto não é válido".into())
                    })?;
                outcome.affected_frame_id = Some(frame_id);
                Ok(next)
            }
        })?;
        Ok(outcome)
    }

    pub(crate) fn import_photos(
        &mut self,
        links: Vec<(Uuid, std::path::PathBuf)>,
    ) -> Result<(), CoreError> {
        self.commit_edit(move |project| {
            project.with_imported_photos(links).map_err(|()| {
                CoreError::InvalidProject("o vínculo externo da Foto não é válido".into())
            })
        })
    }

    pub(crate) fn refresh_layout_catalog(
        &mut self,
        snapshot: crate::LayoutCatalogSnapshot,
    ) -> Result<bool, CoreError> {
        if !snapshot.is_valid()
            || snapshot.revision == self.layout_catalog.revision && snapshot != self.layout_catalog
        {
            return Err(CoreError::InvalidLayoutQuery);
        }
        if snapshot.revision <= self.layout_catalog.revision {
            return Ok(false);
        }
        self.layout_catalog = snapshot;
        self.prepared_layout_query = None;
        Ok(true)
    }

    pub(crate) fn query_layouts(
        &mut self,
        sheet_id: &str,
        expansion: Option<crate::LayoutExpansion>,
    ) -> Result<crate::LayoutQueryResult, CoreError> {
        let parsed = parse_uuid(sheet_id).map_err(|_| CoreError::SheetNotFound(sheet_id.into()))?;
        let mut query = self.project().layout_query(parsed)?;
        let frame_count = query.frame_orientations.len();
        if let Some(expansion) = expansion {
            if expansion.additional_positions > 30
                || frame_count.saturating_add(expansion.additional_positions) > 30
            {
                return Err(CoreError::InvalidLayoutQuery);
            }
            query.frame_orientations.extend(std::iter::repeat_n(
                expansion.orientation,
                expansion.additional_positions,
            ));
        }
        let sheet = self
            .project()
            .sheets()
            .iter()
            .find(|s| s.id() == parsed)
            .unwrap();
        let locked = sheet.layout_locked();
        if locked && frame_count != query.frame_orientations.len() {
            return Err(CoreError::LayoutLocked);
        }
        let ids: Vec<_> = sheet.frames().iter().map(|f| f.id()).collect();
        let sources = crate::LayoutSources {
            last: sheet.last_layout(),
            custom: &self.layout_catalog.entries,
        };
        let mut listing = crate::LayoutRules::list_for_lock(&query, sources);
        if locked {
            let current = self.project().current_layout(parsed)?;
            listing.candidates.retain(|candidate| {
                candidate.layout.origin != current.origin
                    || !crate::LayoutRules::same_definition(
                        &candidate.layout.definition,
                        &current.definition,
                    )
            });
            for candidate in &mut listing.candidates {
                candidate.is_last_applied = false;
            }
            listing.candidates.insert(
                0,
                crate::LayoutCandidate {
                    custom_id: sources.custom_id(&current),
                    layout: current,
                    is_last_applied: true,
                },
            );
        }
        let patches = listing
            .candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                let placeholders = (ids.len()..candidate.layout.definition.positions.len())
                    .map(|_| Uuid::new_v4())
                    .collect::<Vec<_>>();
                crate::LayoutRules::resolve_for_lock(
                    &candidate.layout,
                    &query.surface,
                    &ids,
                    &placeholders,
                    if locked && index == 0 {
                        crate::LayoutPermission::PagesAndSheet
                    } else {
                        query.permission
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let id = Uuid::new_v4().to_string();
        self.prepared_layout_query = Some(PreparedLayoutQuery {
            id: id.clone(),
            revision: self.revision(),
            sheet_id: parsed,
            frame_ids: ids,
            patches,
        });
        Ok(crate::LayoutQueryResult {
            query_id: id,
            project_id: self.project_id().to_string(),
            revision: self.revision(),
            sheet_id: sheet_id.into(),
            catalog_revision: self.layout_catalog.revision,
            frame_count,
            locked,
            settings: self.project().layout_settings().clone(),
            listing,
        })
    }

    pub(crate) fn checked_layout_patch(
        &self,
        selection: &crate::LayoutSelection,
    ) -> Result<(Uuid, &crate::LayoutPatch), CoreError> {
        let prepared = self
            .prepared_layout_query
            .as_ref()
            .ok_or(CoreError::StaleLayoutPreview)?;
        if prepared.id != selection.query_id || prepared.revision != self.revision() {
            return Err(CoreError::StaleLayoutPreview);
        }
        let sheet = self
            .project()
            .sheets()
            .iter()
            .find(|s| s.id() == prepared.sheet_id)
            .ok_or(CoreError::StaleLayoutPreview)?;
        if !sheet
            .frames()
            .iter()
            .map(|f| f.id())
            .eq(prepared.frame_ids.iter().copied())
        {
            return Err(CoreError::StaleLayoutPreview);
        }
        Ok((
            prepared.sheet_id,
            prepared
                .patches
                .get(selection.candidate_index)
                .ok_or(CoreError::StaleLayoutPreview)?,
        ))
    }

    pub(crate) fn relink_media(&mut self, command: RelinkMedia) -> Result<(), CoreError> {
        self.commit_edit(move |project| {
            if !project
                .media()
                .iter()
                .any(|media| media.id() == command.media_id.into_uuid())
            {
                return Err(CoreError::MediaNotFound(command.media_id.to_string()));
            }
            project
                .with_relinked_media(command.media_id.into_uuid(), command.replacement_path)
                .map_err(|()| {
                    CoreError::InvalidProject("a nova referência de mídia não é válida".into())
                })
        })
    }

    fn commit_edit(
        &mut self,
        edit: impl FnOnce(&ProjectDocument) -> Result<ProjectDocument, CoreError>,
    ) -> Result<(), CoreError> {
        let next_revision = self
            .latest_revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_SAFE_INTEGER)
            .ok_or(CoreError::RevisionSpaceExhausted)?;
        let project = edit(&self.current.project)?;
        self.current.project.validate_locked_structure(&project)?;

        self.undo.push(self.current.clone());
        self.redo.clear();
        self.current = ProjectRevision::new(self.current.project_id, next_revision, project);
        self.latest_revision = next_revision;
        Ok(())
    }

    pub(crate) fn undo(&mut self) -> Option<()> {
        let previous = self.undo.pop()?;
        let current = std::mem::replace(&mut self.current, previous);
        self.redo.push(current);
        Some(())
    }

    pub(crate) fn redo(&mut self) -> Option<()> {
        let next = self.redo.pop()?;
        let current = std::mem::replace(&mut self.current, next);
        self.undo.push(current);
        Some(())
    }

    pub(crate) fn confirm_saved(&mut self, candidate: &ProjectRevision) -> Result<(), ()> {
        if self.current != *candidate {
            return Err(());
        }
        self.saved_revision = candidate.revision;
        self.schema_upgrade_required = false;
        self.recovered_unsaved = false;
        Ok(())
    }

    pub(crate) fn adopt_saved_as(&mut self, candidate: &ProjectRevision) -> Result<(), ()> {
        if self.current.revision != candidate.revision || self.current.project != candidate.project
        {
            return Err(());
        }
        self.current.project_id = candidate.project_id;
        self.frame_clipboard = None;
        for revision in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            revision.project_id = candidate.project_id;
        }
        self.saved_revision = candidate.revision;
        self.schema_upgrade_required = false;
        self.recovered_unsaved = false;
        Ok(())
    }
}

fn parse_uuid(source: &str) -> Result<Uuid, ()> {
    let parsed = Uuid::parse_str(source).map_err(|_| ())?;
    (parsed.hyphenated().to_string() == source)
        .then_some(parsed)
        .ok_or(())
}
