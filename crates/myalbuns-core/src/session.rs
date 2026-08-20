use crate::composition::{CompositionCore, build_render_snapshot, derive_media_usage};
use crate::model::{
    AlbumSnapshot, CompositionPlan, CoreError, EditorProjection, EditorState, FrameSnapshot,
    MediaKind, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN, PhotoSnapshot,
    ProjectIntent, RenderSnapshot,
};
use crate::project::serialize_persisted_revision;

pub(crate) struct ProjectSession {
    state: EditorState,
    latest_revision: u64,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
}

struct HistoryEntry {
    album: AlbumSnapshot,
    revision: u64,
}

impl ProjectSession {
    pub(crate) fn from_state(state: EditorState) -> Self {
        let latest_revision = state.revision;
        Self {
            state,
            latest_revision,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub(crate) fn state(&self) -> EditorState {
        self.state.clone()
    }

    pub(crate) fn apply(&mut self, intent: ProjectIntent) -> Result<EditorState, CoreError> {
        let next_revision = self.latest_revision.checked_add(1).ok_or_else(|| {
            CoreError::InvalidProject("A SessÃ£o do Projeto esgotou o espaÃ§o de revisÃµes.".into())
        })?;
        let previous = HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        };

        match intent {
            ProjectIntent::SetAlbumInformation { .. } => {
                return Err(CoreError::UnsupportedProjectIntent);
            }
            ProjectIntent::SetVisualDefaults { .. } => {
                return Err(CoreError::UnsupportedProjectIntent);
            }
            ProjectIntent::SetDpi { .. } => {
                return Err(CoreError::UnsupportedProjectIntent);
            }
            ProjectIntent::TransformPhoto {
                frame_id,
                delta_pan_x,
                delta_pan_y,
                delta_zoom,
            } => {
                let frame = find_frame_mut(&mut self.state.album, &frame_id)
                    .ok_or_else(|| CoreError::FrameNotFound(frame_id.clone()))?;
                let photo = frame
                    .photo
                    .as_mut()
                    .ok_or_else(|| CoreError::FrameHasNoPhoto(frame_id.clone()))?;

                photo.transform.pan_x =
                    (photo.transform.pan_x + delta_pan_x).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX);
                photo.transform.pan_y =
                    (photo.transform.pan_y + delta_pan_y).clamp(PHOTO_PAN_MIN, PHOTO_PAN_MAX);
                photo.transform.user_zoom =
                    (photo.transform.user_zoom + delta_zoom).clamp(PHOTO_ZOOM_MIN, PHOTO_ZOOM_MAX);
            }
            ProjectIntent::FillLeftmostPlaceholder { sheet_id, media_id } => {
                if !self
                    .state
                    .album
                    .media
                    .iter()
                    .any(|item| item.id == media_id && item.kind == MediaKind::Photo)
                {
                    return Err(CoreError::MediaNotFound(media_id));
                }
                let sheet = self
                    .state
                    .album
                    .sheets
                    .iter_mut()
                    .find(|sheet| sheet.id == sheet_id)
                    .ok_or_else(|| CoreError::SheetNotFound(sheet_id.clone()))?;
                let frame = sheet
                    .frames
                    .iter_mut()
                    .filter(|frame| frame.photo.is_none())
                    .min_by_key(|frame| (frame.rect.x, frame.rect.y))
                    .ok_or_else(|| CoreError::PlaceholderNotFound(sheet_id.clone()))?;
                frame.photo = Some(PhotoSnapshot::for_media(media_id));
            }
        }

        self.undo.push(previous);
        self.redo.clear();
        self.latest_revision = next_revision;
        self.state.revision = next_revision;
        self.refresh_history_flags();
        Ok(self.state())
    }

    pub(crate) fn undo(&mut self) -> Option<EditorState> {
        let previous = self.undo.pop()?;
        self.redo.push(HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        });
        self.state.album = previous.album;
        self.state.revision = previous.revision;
        self.refresh_history_flags();
        Some(self.state())
    }

    pub(crate) fn redo(&mut self) -> Option<EditorState> {
        let next = self.redo.pop()?;
        self.undo.push(HistoryEntry {
            album: self.state.album.clone(),
            revision: self.state.revision,
        });
        self.state.album = next.album;
        self.state.revision = next.revision;
        self.refresh_history_flags();
        Some(self.state())
    }

    fn composition_plan(&self) -> CompositionPlan {
        CompositionCore::compose(&self.state.album)
    }

    pub(crate) fn projection(&self) -> EditorProjection {
        let composition = self.composition_plan();
        EditorProjection {
            state: self.state(),
            media_usage: derive_media_usage(&self.state.album, &composition),
            composition,
        }
    }

    pub(crate) fn render_snapshot(&self) -> RenderSnapshot {
        build_render_snapshot(
            &self.state.project_id,
            &self.state.project_name,
            self.state.revision,
            self.state.document.dpi,
            &self.state.album,
        )
    }

    pub(crate) fn persisted_revision(&self) -> Result<String, CoreError> {
        serialize_persisted_revision(&self.state)
    }

    pub(crate) fn confirm_saved_revision(
        &mut self,
        revision: u64,
    ) -> Result<EditorState, CoreError> {
        if revision != self.state.revision {
            return Err(CoreError::SavedRevisionMismatch {
                current: self.state.revision,
                confirmed: revision,
            });
        }
        self.state.saved_revision = revision;
        self.state.dirty = false;
        Ok(self.state())
    }

    fn refresh_history_flags(&mut self) {
        self.state.can_undo = !self.undo.is_empty();
        self.state.can_redo = !self.redo.is_empty();
        self.state.dirty = self.state.revision != self.state.saved_revision;
    }
}

fn find_frame_mut<'a>(
    album: &'a mut AlbumSnapshot,
    frame_id: &str,
) -> Option<&'a mut FrameSnapshot> {
    album
        .sheets
        .iter_mut()
        .flat_map(|sheet| sheet.frames.iter_mut())
        .find(|frame| frame.id == frame_id)
}
