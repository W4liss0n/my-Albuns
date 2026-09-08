use super::*;
use crate::CoreError;

/// A session-owned snapshot, independent of revisions and the source Sheet's lifetime.
#[derive(Clone, Debug)]
pub(crate) struct FrameClipboard {
    sheet_id: Uuid,
    active_sides: ActiveSides,
    width: u64,
    height: u64,
    frames: Vec<ProjectFrame>,
    media: Vec<MediaRef>,
}

impl ProjectDocument {
    pub(crate) fn copy_frames(&self, frame_ids: &[String]) -> Result<FrameClipboard, CoreError> {
        let (index, selected) = self
            .frame_selection(frame_ids)
            .map_err(|()| CoreError::InvalidFrameCopySelection)?;
        let sheet = &self.sheets[index];
        let frames: Vec<_> = sheet
            .frames
            .iter()
            .filter(|frame| selected.contains(&frame.id))
            .cloned()
            .collect();
        let media_ids: HashSet<_> = frames
            .iter()
            .filter_map(|frame| frame.photo.as_ref().map(|photo| photo.media_id))
            .collect();
        Ok(FrameClipboard {
            sheet_id: sheet.id,
            active_sides: sheet.active_sides,
            width: active_surface_width(sheet, self.document.sheet_width_um),
            height: self.document.sheet_height_um,
            frames,
            media: self
                .media
                .iter()
                .filter(|media| media_ids.contains(&media.id))
                .cloned()
                .collect(),
        })
    }

    pub(crate) fn with_pasted_frames(
        &self,
        clipboard: &FrameClipboard,
        sheet_id: &str,
        desired_offset_um: u64,
    ) -> Result<(Self, Vec<Uuid>), CoreError> {
        let id =
            Uuid::parse_str(sheet_id).map_err(|_| CoreError::SheetNotFound(sheet_id.into()))?;
        let index = self
            .sheets
            .iter()
            .position(|sheet| sheet.id == id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.into()))?;
        let mut candidate = self.clone();
        let target = &candidate.sheets[index];
        let surface_width = active_surface_width(target, self.document.sheet_width_um);
        let height = self.document.sheet_height_um;
        let single_to_double =
            clipboard.active_sides != ActiveSides::Both && target.active_sides == ActiveSides::Both;
        let width = if single_to_double {
            surface_width / 2
        } else {
            surface_width
        };
        let origin_x = if single_to_double && clipboard.active_sides == ActiveSides::Right {
            width
        } else {
            0
        };
        let mut frames = clipboard.frames.clone();
        for frame in &mut frames {
            let rect = frame.rect;
            let x = map_coordinate(rect.x, clipboard.width, width);
            let y = map_coordinate(rect.y, clipboard.height, height);
            let right = map_coordinate(rect.x + rect.width, clipboard.width, width);
            let bottom = map_coordinate(rect.y + rect.height, clipboard.height, height);
            frame.id = Uuid::new_v4();
            frame.rect = ProjectRect::new(origin_x + x, y, right - x, bottom - y);
        }
        if id == clipboard.sheet_id {
            let right = frames
                .iter()
                .map(|frame| frame.rect.x + frame.rect.width)
                .max()
                .unwrap_or(0);
            let bottom = frames
                .iter()
                .map(|frame| frame.rect.y + frame.rect.height)
                .max()
                .unwrap_or(0);
            let offset = desired_offset_um
                .min(surface_width - right)
                .min(height - bottom);
            for frame in &mut frames {
                frame.rect.x += offset;
                frame.rect.y += offset;
            }
        }
        // Undo can remove an imported binding after Copy. Reuse an existing identity
        // (including a later relink), or restore its reference without copying files.
        for copied in &clipboard.media {
            let target_id =
                if let Some(current) = candidate.media.iter().find(|media| media.id == copied.id) {
                    current.id
                } else if let Some(current) = candidate
                    .media
                    .iter()
                    .find(|media| media.kind == copied.kind && media.path == copied.path)
                {
                    current.id
                } else {
                    candidate.media.push(copied.clone());
                    copied.id
                };
            for photo in frames.iter_mut().filter_map(|frame| frame.photo.as_mut()) {
                if photo.media_id == copied.id {
                    photo.media_id = target_id;
                }
            }
        }
        let ids = frames.iter().map(|frame| frame.id).collect();
        candidate.sheets[index].frames.extend(frames);
        validate_project_state(&candidate).map_err(|()| CoreError::InvalidFramePaste)?;
        Ok((candidate, ids))
    }
}

fn map_coordinate(value: u64, source: u64, target: u64) -> u64 {
    // Map shared edges together and round once, without overflow or float drift.
    ((u128::from(value) * u128::from(target) + u128::from(source / 2)) / u128::from(source)) as u64
}
