use std::collections::VecDeque;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditPublication {
    GuardedIfChanged,
    GuardedAlways,
    AlbumInformation,
}

/// Upper bound for the Project snapshots that Undo and Redo keep together.
/// Each snapshot is the whole Project, so a long session would otherwise grow
/// without limit. The oldest Undo steps go first; dropping them never changes
/// the current Project or its saved state.
const HISTORY_BUDGET_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
struct HistoryEntry {
    revision: ProjectRevision,
    bytes: usize,
}

impl HistoryEntry {
    fn new(revision: ProjectRevision) -> Self {
        let bytes = revision.project.approximate_bytes();
        Self { revision, bytes }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PersistentProjectSession {
    current: ProjectRevision,
    latest_revision: u64,
    saved_revision: u64,
    recovered_unsaved: bool,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_bytes: usize,
    history_budget: usize,
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
    pub(crate) fn from_persisted(current: ProjectRevision) -> Self {
        let saved_revision = current.revision;
        let latest_revision = current.revision;
        Self {
            current,
            latest_revision,
            saved_revision,
            recovered_unsaved: false,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_bytes: 0,
            history_budget: HISTORY_BUDGET_BYTES,
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
            recovered_unsaved: true,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_bytes: 0,
            history_budget: HISTORY_BUDGET_BYTES,
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
        self.has_unsaved_changes()
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
        sources: &crate::project_document::PhotoDimensions,
    ) -> Result<ProjectIntentOutcome, CoreError> {
        let mut outcome = ProjectIntentOutcome::default();
        let project = self.project();
        let custom = &self.layout_catalog.entries;
        let mut publication = EditPublication::GuardedIfChanged;
        let candidate = match intent {
            ProjectIntent::SetAlbumInformation {
                information,
                expected_dimension_key,
            } => {
                publication = EditPublication::AlbumInformation;
                if let Some(expected) = expected_dimension_key {
                    let (candidate, validation) = project
                        .prepare_album_information(&information, custom, sources)
                        .map_err(|_| CoreError::AlbumInformationReviewChanged)?;
                    let current = validation
                        .impact
                        .and_then(|impact| impact.dimensional_change);
                    if current.as_ref().map(|change| &change.confirmation_key) != Some(&expected) {
                        return Err(CoreError::AlbumInformationReviewChanged);
                    }
                    Ok(candidate)
                } else {
                    project
                        .with_album_information(information, custom, sources)
                        .map_err(CoreError::InvalidAlbumInformation)
                }
            }
            ProjectIntent::ToggleLayoutFavorite { selection } => {
                let (_, patch) = self.checked_layout_patch(&selection)?;
                let layout = patch.last_layout().ok_or(CoreError::IncompatibleLayout)?;
                publication = EditPublication::GuardedAlways;
                project.with_toggled_layout_favorite(layout)
            }
            ProjectIntent::UnlockLayout { sheet_id } => project.with_layout_unlocked(&sheet_id),
            ProjectIntent::ApplyLayout { selection } => {
                let (sheet_id, patch) = self.checked_layout_patch(&selection)?;
                outcome.affected_sheet_id = Some(sheet_id);
                project.with_layout_patch(sheet_id, patch)
            }
            ProjectIntent::LockLayout { selection } => {
                let (sheet_id, patch) = self.checked_layout_patch(&selection)?;
                outcome.affected_sheet_id = Some(sheet_id);
                project.with_locked_layout_patch(sheet_id, patch)
            }
            ProjectIntent::SetLayoutSettings { settings } => project.with_layout_settings(settings),
            ProjectIntent::SetFrameStyle { edit } => project.with_frame_style(&edit),
            ProjectIntent::TogglePhotoBlackAndWhite { frame_ids } => {
                project.with_toggled_photo_black_and_white(&frame_ids)
            }
            ProjectIntent::SetPhotoZoom { edit } => project.with_photo_zoom(&edit),
            ProjectIntent::SetPhotoAngle { edit } => project.with_photo_angle(&edit),
            ProjectIntent::OrientPhotos { frame_ids, action } => {
                project.with_oriented_photos(&frame_ids, action)
            }
            ProjectIntent::SwapSheetSides { sheet_id } => {
                project.with_swapped_sheet_sides(&sheet_id)
            }
            ProjectIntent::CopyFrames { frame_ids } => {
                self.frame_clipboard = Some(project.copy_frames(&frame_ids)?);
                return Ok(outcome);
            }
            ProjectIntent::PasteFrames {
                sheet_id,
                desired_offset_um,
                mode,
            } => {
                let id = parse_uuid(&sheet_id)
                    .map_err(|_| CoreError::SheetNotFound(sheet_id.clone()))?;
                project.ensure_layout_unlocked(id)?;
                let clipboard = self
                    .frame_clipboard
                    .as_ref()
                    .ok_or(CoreError::FrameClipboardEmpty)?;
                let (next, ids) = project.with_pasted_frames(
                    clipboard,
                    &sheet_id,
                    desired_offset_um,
                    mode,
                    custom,
                )?;
                outcome.affected_frame_ids = Some(ids);
                publication = EditPublication::GuardedAlways;
                Ok(next)
            }
            ProjectIntent::ArrangeFrames { frame_ids, action } => {
                project.with_arranged_frames(&frame_ids, action)
            }
            ProjectIntent::EditSheetVisual {
                sheet_id,
                scope,
                change,
            } => project.with_edited_sheet_visual(&sheet_id, scope, &change),
            ProjectIntent::DropDecorative { request } => project.with_dropped_decorative(&request),
            ProjectIntent::ApplyDecorative {
                sheet_id,
                media_id,
                role,
                scope,
            } => project.with_applied_decorative(&sheet_id, media_id, role, scope),
            ProjectIntent::EditMediaFolder { edit } => project.with_media_folder_edit(&edit),
            ProjectIntent::RemoveMedia { media_ids, mode } => {
                project.with_removed_media(&media_ids, mode)
            }
            ProjectIntent::DeleteFrames { frame_ids, mode } => {
                project.with_deleted_frames(&frame_ids, mode, custom)
            }
            ProjectIntent::EditFrameGeometry { edit } => project.with_edited_frame_geometry(&edit),
            ProjectIntent::SetAlbumDesign {
                visual_defaults,
                frame_gap_um,
            } => project.with_album_design(visual_defaults, frame_gap_um),
            ProjectIntent::SwapFrameContents { frame_ids } => {
                publication = EditPublication::GuardedAlways;
                project.with_swapped_frame_contents(&frame_ids)
            }
            ProjectIntent::SetVisualDefaults { visual_defaults } => {
                publication = EditPublication::GuardedAlways;
                project
                    .with_visual_defaults(visual_defaults)
                    .map_err(|()| CoreError::InvalidVisualDefaults)
            }
            ProjectIntent::SetDpi { dpi } => {
                publication = EditPublication::GuardedAlways;
                project
                    .with_dpi(dpi)
                    .map_err(|()| CoreError::InvalidDpi(dpi))
            }
            ProjectIntent::AddSheet {
                anchor_sheet_id,
                position,
            } => {
                publication = EditPublication::GuardedAlways;
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
            ProjectIntent::DuplicateSheet { sheet_id } => {
                publication = EditPublication::GuardedAlways;
                let parsed = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(sheet_id));
                }
                let (next, copy_id) = project
                    .with_duplicated_sheet(parsed)
                    .map_err(|()| CoreError::InvalidSheetDuplication)?;
                outcome.affected_sheet_id = Some(copy_id);
                Ok(next)
            }
            ProjectIntent::DeleteSheet { sheet_id } => {
                publication = EditPublication::GuardedAlways;
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
                publication = EditPublication::GuardedAlways;
                let parsed = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if !project.sheets().iter().any(|sheet| sheet.id() == parsed) {
                    return Err(CoreError::SheetNotFound(sheet_id));
                }
                let next = project
                    .with_converted_edge_sheet(parsed, custom)
                    .map_err(|()| CoreError::InvalidEdgeConversion)?;
                outcome.affected_sheet_id = Some(parsed);
                Ok(next)
            }
            ProjectIntent::ReorderSheet {
                sheet_id,
                target_index,
            } => {
                publication = EditPublication::GuardedAlways;
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
                publication = EditPublication::GuardedAlways;
                let parsed = parse_uuid(&frame_id)
                    .map_err(|()| CoreError::FrameNotFound(frame_id.clone()))?;
                let next = project
                    .with_transformed_photo(parsed, delta_pan_x, delta_pan_y, delta_zoom)
                    .map_err(|()| CoreError::FrameNotFound(frame_id))?;
                outcome.affected_frame_id = Some(parsed);
                Ok(next)
            }
            ProjectIntent::AddFrame { sheet_id } => {
                publication = EditPublication::GuardedAlways;
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                project.ensure_layout_unlocked(parsed_sheet)?;
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
                publication = EditPublication::GuardedAlways;
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                if let Some(sheet) = project
                    .sheets()
                    .iter()
                    .find(|sheet| sheet.id() == parsed_sheet)
                    && sheet.layout_locked()
                    && sheet.frames().iter().all(|frame| frame.photo().is_some())
                {
                    return Err(CoreError::LockedLayoutHasNoPlaceholder);
                }
                let (next, frame_id) = project
                    .with_added_photo(parsed_sheet, media_id.into_uuid(), mode, custom)
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
                publication = EditPublication::GuardedAlways;
                let parsed_sheet = parse_uuid(&sheet_id)
                    .map_err(|()| CoreError::SheetNotFound(sheet_id.clone()))?;
                let (next, frame_id) = project
                    .with_dropped_photo(
                        parsed_sheet,
                        media_id.into_uuid(),
                        x_um,
                        y_um,
                        mode,
                        custom,
                    )
                    .map_err(|()| {
                        CoreError::InvalidProject("o alvo da Foto não é válido".into())
                    })?;
                outcome.affected_frame_id = Some(frame_id);
                Ok(next)
            }
        }?;
        if publication != EditPublication::GuardedAlways && candidate == *self.project() {
            return Ok(outcome);
        }
        match publication {
            // Only the complete validated global transformation can resize locked Frames.
            EditPublication::AlbumInformation => self.publish_edit(candidate)?,
            EditPublication::GuardedAlways | EditPublication::GuardedIfChanged => {
                self.commit_edit(|_| Ok(candidate))?;
            }
        }
        Ok(outcome)
    }

    pub(crate) fn import_media(
        &mut self,
        kind: crate::MediaKind,
        links: Vec<(Uuid, std::path::PathBuf)>,
    ) -> Result<(), CoreError> {
        self.commit_edit(move |project| {
            project.with_imported_media(kind, links).map_err(|()| {
                CoreError::InvalidProject("o vínculo externo da imagem não é válido".into())
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
        frame_request: Option<crate::LayoutFrameRequest>,
    ) -> Result<crate::LayoutQueryResult, CoreError> {
        let parsed = parse_uuid(sheet_id).map_err(|_| CoreError::SheetNotFound(sheet_id.into()))?;
        let mut query = self.project().layout_query(parsed)?;
        let frame_count = query.frame_orientations.len();
        let sheet = self
            .project()
            .sheets()
            .iter()
            .find(|s| s.id() == parsed)
            .unwrap();
        let locked = sheet.layout_locked();
        let requested_count = frame_request
            .as_ref()
            .map_or(frame_count, |request| request.frame_count);
        if frame_request.is_some() {
            sheet.validate_layout_frame_count(requested_count)?;
        }
        let filled_count = sheet
            .frames()
            .iter()
            .filter(|frame| frame.photo().is_some())
            .count();
        let captured_ids: Vec<_> = sheet.frames().iter().map(|frame| frame.id()).collect();
        let mut ids = Vec::new();
        let mut orientations = Vec::new();
        let mut remaining_placeholders = requested_count.saturating_sub(filled_count);
        for (frame, orientation) in sheet.frames().iter().zip(&query.frame_orientations) {
            if frame.photo().is_none() {
                if remaining_placeholders == 0 {
                    continue;
                }
                remaining_placeholders -= 1;
            }
            ids.push(frame.id());
            orientations.push(*orientation);
        }
        if let Some(request) = &frame_request {
            orientations.resize(requested_count, request.orientation);
        }
        query.frame_orientations = orientations;
        let sources = crate::LayoutSources {
            last: sheet.last_layout(),
            custom: &self.layout_catalog.entries,
            favorites: self.project().favorite_layouts(),
        };
        let mut listing = if frame_request.is_some() {
            crate::LayoutRules::list(&query, sources)
        } else {
            crate::LayoutRules::list_for_lock(&query, sources)
        };
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
                    favorite_id: sources.favorite_id(&current),
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
        let candidate_requires_lock = patches
            .iter()
            .map(crate::LayoutPatch::requires_lock)
            .collect();
        self.prepared_layout_query = Some(PreparedLayoutQuery {
            id: id.clone(),
            revision: self.revision(),
            sheet_id: parsed,
            frame_ids: captured_ids,
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
            candidate_requires_lock,
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
        let project = edit(&self.current.project)?;
        self.current.project.validate_locked_structure(&project)?;
        self.publish_edit(project)
    }

    fn publish_edit(&mut self, project: ProjectDocument) -> Result<(), CoreError> {
        let next_revision = self
            .latest_revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_SAFE_INTEGER)
            .ok_or(CoreError::RevisionSpaceExhausted)?;

        self.redo.clear();
        self.history_bytes = self.undo.iter().map(|entry| entry.bytes).sum();
        self.push_undo(HistoryEntry::new(self.current.clone()));
        self.current = ProjectRevision::new(self.current.project_id, next_revision, project);
        self.latest_revision = next_revision;
        Ok(())
    }

    fn push_undo(&mut self, entry: HistoryEntry) {
        self.history_bytes += entry.bytes;
        self.undo.push_back(entry);
        // The latest step stays available even if it alone exceeds the budget.
        while self.history_bytes > self.history_budget && self.undo.len() > 1 {
            let oldest = self
                .undo
                .pop_front()
                .expect("more than one Undo step remains");
            self.history_bytes -= oldest.bytes;
        }
    }

    pub(crate) fn validate_album_information(
        &self,
        information: &crate::AlbumInformation,
        sources: &crate::project_document::PhotoDimensions,
    ) -> crate::AlbumInformationValidation {
        self.project().validate_album_information(
            information,
            &self.layout_catalog.entries,
            sources,
        )
    }

    pub(crate) fn undo(&mut self) -> Option<()> {
        let previous = self.undo.pop_back()?;
        self.history_bytes -= previous.bytes;
        let current = HistoryEntry::new(std::mem::replace(&mut self.current, previous.revision));
        self.history_bytes += current.bytes;
        self.redo.push(current);
        Some(())
    }

    pub(crate) fn redo(&mut self) -> Option<()> {
        let next = self.redo.pop()?;
        self.history_bytes -= next.bytes;
        let current = HistoryEntry::new(std::mem::replace(&mut self.current, next.revision));
        self.push_undo(current);
        Some(())
    }

    pub(crate) fn confirm_saved(&mut self, candidate: &ProjectRevision) -> Result<(), ()> {
        if self.current != *candidate {
            return Err(());
        }
        self.saved_revision = candidate.revision;
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
        for entry in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            entry.revision.project_id = candidate.project_id;
        }
        self.saved_revision = candidate.revision;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_drops_the_oldest_undo_steps_beyond_its_budget() {
        let revision = crate::project_store::decode(include_bytes!(
            "../tests/fixtures/project_file_v1/base.myalbuns"
        ))
        .unwrap();
        let saved_revision = revision.revision;
        let mut session = PersistentProjectSession::from_persisted(revision);
        session.history_budget = session.current.project.approximate_bytes() * 3;

        for dpi in [240, 300, 240, 300, 240, 300] {
            let project = session.current.project.with_dpi(dpi).unwrap();
            session.publish_edit(project).unwrap();
        }

        assert_eq!(session.undo.len(), 3);
        assert!(session.history_bytes <= session.history_budget);
        assert_eq!(session.saved_revision, saved_revision);
        let latest = session.current.clone();
        for _ in 0..3 {
            session.undo().unwrap();
        }
        assert!(session.undo().is_none(), "the dropped steps stay gone");
        for _ in 0..3 {
            session.redo().unwrap();
        }
        assert_eq!(session.current, latest);
        assert!(session.history_bytes <= session.history_budget);
    }
}
