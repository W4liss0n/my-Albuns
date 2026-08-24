use std::{collections::HashMap, path::PathBuf};

use crate::{
    composition::resolve_editor_projection,
    model::{
        AlbumSnapshot, DocumentSnapshot, EditorProjection, EditorState, FrameSnapshot,
        MediaCatalogItem, MediaId, MediaKind, MediaTransform, PhotoSnapshot, PhotoSourceMetadata,
        ProjectedActiveSides, ProjectedBackground, ProjectedBackgroundContent,
        ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent, ProjectedVisualDefaults,
        RectUm, SheetRole, SheetSnapshot,
    },
    persistent_session::PersistentProjectSession,
    project_document::{
        ActiveSides, Background, BackgroundContent, FrameBorder, Overlay, OverlayContent,
        VisualDefaults,
    },
};

pub(crate) fn editor_projection(
    session: &PersistentProjectSession,
    history_enabled: bool,
    project_name: &str,
    photo_sources: &HashMap<MediaId, HashMap<PathBuf, PhotoSourceMetadata>>,
) -> EditorProjection {
    let project = session.project();
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
                frames: sheet
                    .frames()
                    .iter()
                    .enumerate()
                    .map(|(z_index, frame)| FrameSnapshot {
                        id: frame.id().hyphenated().to_string(),
                        rect: RectUm {
                            x: i64::try_from(frame.rect().x()).expect("validated Frame x fits i64"),
                            y: i64::try_from(frame.rect().y()).expect("validated Frame y fits i64"),
                            width: i64::try_from(frame.rect().width())
                                .expect("validated Frame width fits i64"),
                            height: i64::try_from(frame.rect().height())
                                .expect("validated Frame height fits i64"),
                        },
                        z_index: u32::try_from(z_index).expect("validated Frame stack fits u32"),
                        photo: frame.photo().map(|photo| PhotoSnapshot {
                            media_id: MediaId::from_uuid(photo.media_id()),
                            transform: MediaTransform {
                                pan_x: photo.transform().pan_x(),
                                pan_y: photo.transform().pan_y(),
                                user_zoom: photo.transform().user_zoom(),
                                quarter_turns: 0,
                                fine_rotation_degrees: 0.0,
                                mirror_x: false,
                            },
                        }),
                    })
                    .collect(),
            })
            .collect(),
        media: project
            .media()
            .iter()
            .map(|media| {
                let media_id = MediaId::from_uuid(media.id());
                let source = photo_sources
                    .get(&media_id)
                    .and_then(|observations| observations.get(media.path()));
                MediaCatalogItem {
                    id: media_id,
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
                    source_width_px: (media.kind() == MediaKind::Photo)
                        .then(|| source.map_or(1, PhotoSourceMetadata::source_width_px)),
                    source_height_px: (media.kind() == MediaKind::Photo)
                        .then(|| source.map_or(1, PhotoSourceMetadata::source_height_px)),
                    palette: (media.kind() == MediaKind::Photo).then(|| {
                        source
                            .map(|source| source.palette().clone())
                            .unwrap_or_else(|| {
                                ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()]
                            })
                    }),
                }
            })
            .collect(),
        visual_defaults: projected_visual_defaults(project.visual_defaults()),
    };
    let state = EditorState {
        project_id: session.project_id().hyphenated().to_string(),
        project_name: project_name.into(),
        document: DocumentSnapshot::from_settings(settings),
        revision: session.revision(),
        saved_revision: session.saved_revision(),
        dirty: session.has_unsaved_changes(),
        can_undo: history_enabled && session.can_undo(),
        can_redo: history_enabled && session.can_redo(),
        album,
    };
    resolve_editor_projection(state)
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
            media_id: MediaId::from_uuid(*media_id),
        },
    }
}

fn projected_overlay_content(content: &OverlayContent) -> ProjectedOverlayContent {
    match content {
        OverlayContent::Media { media_id } => ProjectedOverlayContent::Media {
            media_id: MediaId::from_uuid(*media_id),
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
        OverlayContent, ProjectDocument, ProjectRevision, ProjectSheet, Rgb, VisualDefaults,
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

        let session = PersistentProjectSession::from_persisted(
            ProjectRevision::new(
                Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("project id"),
                0,
                project,
            ),
            false,
        );
        let projection = editor_projection(&session, true, "Projeto", &HashMap::new());
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
