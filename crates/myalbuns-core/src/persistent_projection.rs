use uuid::Uuid;

use crate::{
    composition::{CompositionCore, derive_media_usage},
    model::{
        AlbumSnapshot, DocumentSnapshot, EditorProjection, EditorState, MediaCatalogItem,
        MediaKind, ProjectedActiveSides, ProjectedBackground, ProjectedBackgroundContent,
        ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent, ProjectedVisualDefaults,
        SheetRole, SheetSnapshot,
    },
    project_document::{
        ActiveSides, Background, BackgroundContent, FrameBorder, Overlay, OverlayContent,
        ProjectDocument, VisualDefaults,
    },
};

pub(crate) fn editor_projection(
    project_id: Uuid,
    revision: u64,
    saved_revision: u64,
    can_undo: bool,
    can_redo: bool,
    project_name: &str,
    project: &ProjectDocument,
) -> EditorProjection {
    let settings = project.document();
    let last_sheet = project.sheets().len().saturating_sub(1);
    let mut next_page_number = 1;
    let album = AlbumSnapshot {
        sheets: project
            .sheets()
            .iter()
            .enumerate()
            .map(|(index, sheet)| {
                let active_sides = projected_active_sides(sheet.active_sides());
                let page_count = match active_sides {
                    ProjectedActiveSides::Both => 2,
                    ProjectedActiveSides::Left | ProjectedActiveSides::Right => 1,
                };
                let page_numbers = (next_page_number..next_page_number + page_count).collect();
                next_page_number += page_count;

                SheetSnapshot {
                    id: sheet.id().hyphenated().to_string(),
                    number: index + 1,
                    role: if index == 0 {
                        SheetRole::Initial
                    } else if index == last_sheet {
                        SheetRole::Final
                    } else {
                        SheetRole::Internal
                    },
                    active_sides,
                    page_numbers,
                    width_um: settings.sheet_width_um() as i64,
                    height_um: settings.sheet_height_um() as i64,
                    frames: Vec::new(),
                }
            })
            .collect(),
        media: project
            .media()
            .iter()
            .map(|media| MediaCatalogItem {
                id: media.id().hyphenated().to_string(),
                kind: media.kind(),
                name: media
                    .path()
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| match media.kind() {
                        MediaKind::Photo => "Foto".into(),
                        MediaKind::Decorative => "Decorativo".into(),
                    }),
                source_width_px: None,
                source_height_px: None,
                palette: None,
            })
            .collect(),
        visual_defaults: projected_visual_defaults(project.visual_defaults()),
    };
    let state = EditorState {
        project_id: project_id.hyphenated().to_string(),
        project_name: project_name.into(),
        document: DocumentSnapshot::from_settings(settings),
        revision,
        saved_revision,
        dirty: revision != saved_revision,
        can_undo,
        can_redo,
        album,
    };
    let composition = CompositionCore::compose(&state.album);
    let media_usage = derive_media_usage(&state.album, &composition);
    EditorProjection {
        composition,
        state,
        media_usage,
    }
}

fn projected_active_sides(active_sides: ActiveSides) -> ProjectedActiveSides {
    match active_sides {
        ActiveSides::Both => ProjectedActiveSides::Both,
        ActiveSides::Left => ProjectedActiveSides::Left,
        ActiveSides::Right => ProjectedActiveSides::Right,
    }
}

fn projected_visual_defaults(defaults: &VisualDefaults) -> ProjectedVisualDefaults {
    ProjectedVisualDefaults {
        background: match defaults.background() {
            Background::BothSides { both } => ProjectedBackground::BothSides {
                both: projected_background_content(both),
            },
            Background::PerSide { left, right } => ProjectedBackground::PerSide {
                left: projected_background_content(left),
                right: projected_background_content(right),
            },
        },
        overlay: match defaults.overlay() {
            Overlay::BothSides { both } => ProjectedOverlay::BothSides {
                both: both.as_ref().map(projected_overlay_content),
            },
            Overlay::PerSide { left, right } => ProjectedOverlay::PerSide {
                left: left.as_ref().map(projected_overlay_content),
                right: right.as_ref().map(projected_overlay_content),
            },
        },
        frame_border: match defaults.frame_border() {
            FrameBorder::None => ProjectedFrameBorder::None,
            FrameBorder::Solid { rgb, width_um } => ProjectedFrameBorder::Solid {
                rgb: rgb.canonical_hex(),
                width_um: *width_um,
            },
        },
    }
}

fn projected_background_content(content: &BackgroundContent) -> ProjectedBackgroundContent {
    match content {
        BackgroundContent::Color { rgb } => ProjectedBackgroundContent::Color {
            rgb: rgb.canonical_hex(),
        },
        BackgroundContent::Media { media_id } => ProjectedBackgroundContent::Media {
            media_id: media_id.hyphenated().to_string(),
        },
    }
}

fn projected_overlay_content(content: &OverlayContent) -> ProjectedOverlayContent {
    match content {
        OverlayContent::Media { media_id } => ProjectedOverlayContent::Media {
            media_id: media_id.hyphenated().to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::project_document::{
        Background, BackgroundContent, DocumentSettings, FrameBorder, MediaRef, Overlay,
        OverlayContent, ProjectSheet, Rgb, VisualDefaults,
    };

    #[test]
    fn persisted_visual_defaults_are_projected_once_and_resolved_by_composition() {
        let background_id =
            Uuid::parse_str("00000000-0000-4000-8000-000000000010").expect("background id");
        let overlay_id =
            Uuid::parse_str("00000000-0000-4000-8000-000000000011").expect("overlay id");
        let project = ProjectDocument::new(
            DocumentSettings::neutral(),
            VisualDefaults::new(
                Background::PerSide {
                    left: BackgroundContent::Media {
                        media_id: background_id,
                    },
                    right: BackgroundContent::Color {
                        rgb: Rgb::new([0x12, 0x34, 0x56]),
                    },
                },
                Overlay::BothSides {
                    both: Some(OverlayContent::Media {
                        media_id: overlay_id,
                    }),
                },
                FrameBorder::Solid {
                    rgb: Rgb::new([0xAB, 0xCD, 0xEF]),
                    width_um: 1_200,
                },
            ),
            vec![
                MediaRef::new(
                    background_id,
                    MediaKind::Decorative,
                    PathBuf::from(r"C:\Fotos\fundo.png"),
                ),
                MediaRef::new(
                    overlay_id,
                    MediaKind::Decorative,
                    PathBuf::from(r"C:\Fotos\overlay.png"),
                ),
            ],
            vec![
                ProjectSheet::new(
                    Uuid::parse_str("00000000-0000-4000-8000-000000000001")
                        .expect("first sheet id"),
                    ActiveSides::Both,
                ),
                ProjectSheet::new(
                    Uuid::parse_str("00000000-0000-4000-8000-000000000002").expect("last sheet id"),
                    ActiveSides::Both,
                ),
            ],
        );

        let projection = editor_projection(
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("project id"),
            0,
            0,
            false,
            false,
            "Projeto",
            &project,
        );
        let value = serde_json::to_value(&projection).expect("projection serializes");

        assert_eq!(
            value["state"]["album"]["visualDefaults"],
            json!({
                "background": {
                    "scope": "perSide",
                    "left": { "kind": "media", "mediaId": background_id.to_string() },
                    "right": { "kind": "color", "rgb": "#123456" }
                },
                "overlay": {
                    "scope": "bothSides",
                    "both": { "kind": "media", "mediaId": overlay_id.to_string() }
                },
                "frameBorder": { "kind": "solid", "rgb": "#ABCDEF", "widthUm": 1_200 }
            })
        );
        assert_eq!(
            value["state"]["album"]["media"],
            json!([
                {
                    "id": background_id.to_string(),
                    "kind": "decorative",
                    "name": "fundo.png",
                    "sourceWidthPx": null,
                    "sourceHeightPx": null,
                    "palette": null
                },
                {
                    "id": overlay_id.to_string(),
                    "kind": "decorative",
                    "name": "overlay.png",
                    "sourceWidthPx": null,
                    "sourceHeightPx": null,
                    "palette": null
                }
            ])
        );
        assert_eq!(
            value["composition"]["frameBorder"],
            json!({ "kind": "solid", "rgb": "#ABCDEF", "widthUm": 1_200 })
        );
        assert_eq!(
            value["composition"]["sheets"][0]["base"],
            json!({
                "rgb": "#FFFFFF",
                "drawRect": { "x": 0, "y": 0, "width": 600_000, "height": 300_000 }
            })
        );
        assert_eq!(
            value["composition"]["sheets"][0]["backgrounds"],
            json!([
                {
                    "kind": "media",
                    "mediaId": background_id.to_string(),
                    "name": "fundo.png",
                    "drawRect": { "x": 0, "y": 0, "width": 300_000, "height": 300_000 }
                },
                {
                    "kind": "color",
                    "rgb": "#123456",
                    "drawRect": { "x": 300_000, "y": 0, "width": 300_000, "height": 300_000 }
                }
            ])
        );
        assert_eq!(
            value["composition"]["sheets"][0]["overlays"],
            json!([{
                "mediaId": overlay_id.to_string(),
                "name": "overlay.png",
                "drawRect": { "x": 0, "y": 0, "width": 600_000, "height": 300_000 }
            }])
        );
    }
}
