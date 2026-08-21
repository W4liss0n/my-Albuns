use crate::{
    model::{CoreError, ProjectIntent, RelinkMedia},
    project_document::{MAX_SAFE_INTEGER, ProjectDocument, ProjectRevision},
};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct PersistentProjectSession {
    current: ProjectRevision,
    latest_revision: u64,
    saved_revision: u64,
    schema_upgrade_required: bool,
    undo: Vec<ProjectRevision>,
    redo: Vec<ProjectRevision>,
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
            undo: Vec::new(),
            redo: Vec::new(),
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
        self.current.revision != self.saved_revision
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

    pub(crate) fn apply(&mut self, intent: ProjectIntent) -> Result<Option<Uuid>, CoreError> {
        let mut affected_frame_id = None;
        self.commit_edit(|project| match intent {
            ProjectIntent::SetDpi { dpi } => project
                .with_dpi(dpi)
                .map_err(|()| CoreError::InvalidDpi(dpi)),
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
                affected_frame_id = Some(parsed);
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
                    .with_added_photo(parsed_sheet, media_id.into_uuid(), mode)
                    .map_err(|()| {
                        CoreError::InvalidProject(
                            "não foi possível adicionar a Foto à Lâmina".into(),
                        )
                    })?;
                affected_frame_id = Some(frame_id);
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
                    .with_dropped_photo(parsed_sheet, media_id.into_uuid(), x_um, y_um, mode)
                    .map_err(|()| {
                        CoreError::InvalidProject("o alvo da Foto não é válido".into())
                    })?;
                affected_frame_id = Some(frame_id);
                Ok(next)
            }
        })?;
        Ok(affected_frame_id)
    }

    pub(crate) fn import_photo(
        &mut self,
        media_id: Uuid,
        path: std::path::PathBuf,
    ) -> Result<(), CoreError> {
        self.commit_edit(move |project| {
            project.with_imported_photo(media_id, path).map_err(|()| {
                CoreError::InvalidProject("o vínculo externo da Foto não é válido".into())
            })
        })
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
        Ok(())
    }
}

fn parse_uuid(source: &str) -> Result<Uuid, ()> {
    let parsed = Uuid::parse_str(source).map_err(|_| ())?;
    (parsed.hyphenated().to_string() == source)
        .then_some(parsed)
        .ok_or(())
}
