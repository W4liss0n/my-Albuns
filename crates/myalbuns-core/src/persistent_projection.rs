use uuid::Uuid;

use crate::{
    composition::CompositionCore,
    model::{
        AlbumSnapshot, DocumentSnapshot, EditorProjection, EditorState, MediaUsage,
        ProjectedActiveSides, SheetRole, SheetSnapshot,
    },
    project_document::{ActiveSides, ProjectDocument},
};

pub(crate) fn editor_projection(
    project_id: Uuid,
    revision: u64,
    saved_revision: u64,
    can_undo: bool,
    can_redo: bool,
    project: &ProjectDocument,
) -> EditorProjection {
    let settings = project.document();
    let last_sheet = project.sheets().len().saturating_sub(1);
    let album = AlbumSnapshot {
        sheets: project
            .sheets()
            .iter()
            .enumerate()
            .map(|(index, sheet)| SheetSnapshot {
                id: sheet.id().hyphenated().to_string(),
                number: index + 1,
                role: if index == 0 {
                    SheetRole::Initial
                } else if index == last_sheet {
                    SheetRole::Final
                } else {
                    SheetRole::Internal
                },
                active_sides: projected_active_sides(sheet.active_sides()),
                width_um: settings.sheet_width_um() as i64,
                height_um: settings.sheet_height_um() as i64,
                frames: Vec::new(),
                overlay_media_id: None,
            })
            .collect(),
        media: Vec::new(),
    };
    let state = EditorState {
        project_id: project_id.hyphenated().to_string(),
        project_name: "Projeto".into(),
        document: DocumentSnapshot::from_settings(settings),
        revision,
        saved_revision,
        dirty: revision != saved_revision,
        can_undo,
        can_redo,
        album,
    };
    EditorProjection {
        composition: CompositionCore::compose(&state.album),
        state,
        media_usage: Vec::<MediaUsage>::new(),
    }
}

fn projected_active_sides(active_sides: ActiveSides) -> ProjectedActiveSides {
    match active_sides {
        ActiveSides::Both => ProjectedActiveSides::Both,
        ActiveSides::Left => ProjectedActiveSides::Left,
        ActiveSides::Right => ProjectedActiveSides::Right,
    }
}
