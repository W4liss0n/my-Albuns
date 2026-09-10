#![cfg(windows)]

use myalbuns_core::*;
use myalbuns_paths::OperationPathContext;
use std::path::Path;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

#[test]
fn v10_migrates_without_local_visuals_and_explicit_save_matches_the_v11_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in [
        "migration",
        "photo_migration",
        "angle_migration",
        "effect_migration",
    ] {
        let root = tempfile::tempdir().unwrap();
        let input =
            std::fs::read(fixtures.join(format!("project_document_v10_{name}_expected.myalbuns")))
                .unwrap();
        let path = root.path().join("Legado.myalbuns");
        std::fs::write(&path, &input).unwrap();
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let mut project = core
            .open_editable(OpenProjectRequest::new(location(&path)))
            .unwrap();
        assert!(
            project
                .project()
                .sheets()
                .iter()
                .all(|sheet| sheet.visuals().is_default())
        );
        assert!(!project.has_unsaved_changes());
        assert!(!project.projection().state.can_undo);
        assert_eq!(std::fs::read(&path).unwrap(), input);
        let before = project.projection();
        project.save(project.revision()).unwrap();
        assert_eq!(project.projection(), before);
        let output = std::fs::read(&path).unwrap();
        let expected = fixtures.join(format!("project_document_v11_{name}_expected.myalbuns"));
        if std::env::var_os("MYALBUNS_UPDATE_PROJECT_V11_FIXTURES").is_some() {
            std::fs::write(&expected, &output).unwrap();
        }
        assert_eq!(output, std::fs::read(expected).unwrap());
    }
}

#[test]
fn applying_to_one_side_preserves_the_other_crop_and_history_through_save() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Decorativos.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let source =
        PhotoSourceMetadata::new(600, 400, std::array::from_fn(|_| "#FFFFFF".into())).unwrap();
    let imported = project
        .import_media(
            MediaKind::Decorative,
            ["A.png", "B.png"]
                .map(|name| {
                    let path = root.path().join(name);
                    std::fs::write(&path, b"original").unwrap();
                    ImportMedia::new(path, source.clone())
                })
                .to_vec(),
        )
        .unwrap();
    let ids = imported
        .projection
        .state
        .album
        .media
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: ids[1],
            role: DecorativeRole::Overlay,
            scope: DecorativeScope::Left,
        })
        .unwrap();
    let whole = project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: ids[0],
            role: DecorativeRole::Background,
            scope: DecorativeScope::BothSides,
        })
        .unwrap();
    let divided = project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: ids[1],
            role: DecorativeRole::Background,
            scope: DecorativeScope::Left,
        })
        .unwrap();
    assert_eq!(divided.state.revision, whole.state.revision + 1);
    let backgrounds = &divided.composition.sheets[0].backgrounds;
    assert_eq!(backgrounds.len(), 2);
    assert!(
        matches!(&backgrounds[0], ComposedBackground::Media { media_id, draw_rect, clip_rect: None, .. } if *media_id == ids[1] && draw_rect.width == 300_000)
    );
    assert!(
        matches!(&backgrounds[1], ComposedBackground::Media { media_id, draw_rect, clip_rect: Some(clip), .. } if *media_id == ids[0] && draw_rect.x == 0 && draw_rect.width == 600_000 && clip.x == 300_000 && clip.width == 300_000)
    );
    assert_eq!(project.undo().unwrap().composition, whole.composition);
    assert_eq!(project.redo().unwrap().composition, divided.composition);
    project.save(project.projection().state.revision).unwrap();
    drop(project);
    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.projection().composition, divided.composition);
    for through_information in [false, true] {
        let intent = if through_information {
            ProjectIntent::SetAlbumInformation {
                information: AlbumInformation {
                    display_unit: DisplayUnit::Mm,
                    sheet_width_um: 600_000,
                    sheet_height_um: 300_000,
                    dpi: 300,
                    bleed_um: 3_000,
                    safety_um: 3_000,
                    first_sheet: EndSheetFormat::SinglePage,
                    last_sheet: EndSheetFormat::Double,
                },
            }
        } else {
            ProjectIntent::ConvertEdgeSheet {
                sheet_id: sheet_id.clone(),
            }
        };
        let single = reopened.apply(intent).unwrap();
        assert_eq!(
            single.state.album.sheets[0].active_sides,
            ProjectedActiveSides::Right
        );
        assert_eq!(
            single
                .media_usage
                .iter()
                .find(|usage| usage.media_id == ids[1])
                .unwrap()
                .count,
            0
        );
        assert!(single.composition.sheets[0].overlays.is_empty());
        assert!(
            matches!(&single.composition.sheets[0].backgrounds[0], ComposedBackground::Media { draw_rect, clip_rect: Some(clip), .. } if draw_rect.x == -300_000 && draw_rect.width == 600_000 && clip.x == 0 && clip.width == 300_000)
        );
        let expanded = reopened
            .apply(ProjectIntent::ConvertEdgeSheet {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
        assert!(
            matches!(&expanded.composition.sheets[0].backgrounds[0], ComposedBackground::Color { rgb, .. } if rgb == "#FFFFFF")
        );
        assert!(
            matches!(&expanded.composition.sheets[0].backgrounds[1], ComposedBackground::Media { media_id, .. } if *media_id == ids[0])
        );
        assert!(expanded.composition.sheets[0].overlays.is_empty());
        reopened.undo().unwrap();
        assert_eq!(reopened.undo().unwrap().composition, divided.composition);
    }
}

#[test]
fn removing_local_decoratives_restores_the_current_defaults_in_one_action() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Remoção.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let source =
        PhotoSourceMetadata::new(600, 400, std::array::from_fn(|_| "#FFFFFF".into())).unwrap();
    let imported = project
        .import_media(
            MediaKind::Decorative,
            ["Padrão.png", "Local.png"]
                .map(|name| {
                    let path = root.path().join(name);
                    std::fs::write(&path, b"original").unwrap();
                    ImportMedia::new(path, source.clone())
                })
                .to_vec(),
        )
        .unwrap();
    let ids = imported
        .projection
        .state
        .album
        .media
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    let mut defaults = ProjectedVisualDefaults {
        background: ProjectedBackground::BothSides {
            both: ProjectedBackgroundContent::Media { media_id: ids[0] },
        },
        ..ProjectedVisualDefaults::default()
    };
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults.clone(),
        })
        .unwrap();
    project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: ids[1],
            role: DecorativeRole::Background,
            scope: DecorativeScope::Left,
        })
        .unwrap();
    let local = project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: ids[1],
            role: DecorativeRole::Overlay,
            scope: DecorativeScope::BothSides,
        })
        .unwrap();
    assert!(
        matches!(&local.composition.sheets[0].backgrounds[1], ComposedBackground::Media { media_id, draw_rect, clip_rect: Some(clip), .. } if *media_id == ids[0] && draw_rect.width == 600_000 && clip.x == 300_000)
    );
    defaults.background = ProjectedBackground::PerSide {
        left: ProjectedBackgroundContent::Color {
            rgb: "#00AA00".into(),
        },
        right: ProjectedBackgroundContent::Media { media_id: ids[0] },
    };
    let changed = project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults,
        })
        .unwrap();
    assert!(
        matches!(&changed.composition.sheets[0].backgrounds[1], ComposedBackground::Media { media_id, draw_rect, clip_rect: None, .. } if *media_id == ids[0] && draw_rect.x == 300_000 && draw_rect.width == 300_000)
    );
    assert_eq!(
        changed.composition.sheets[0].overlays,
        local.composition.sheets[0].overlays
    );
    let usage = changed
        .media_usage
        .iter()
        .find(|usage| usage.media_id == ids[1])
        .unwrap();
    assert_eq!(usage.count, 2);
    assert_eq!(usage.breakdown.as_ref().unwrap().backgrounds, 1);
    assert_eq!(usage.breakdown.as_ref().unwrap().overlays, 1);
    assert_eq!(
        changed
            .media_usage
            .iter()
            .find(|usage| usage.media_id == ids[0])
            .unwrap()
            .breakdown
            .as_ref()
            .unwrap()
            .album_backgrounds,
        1
    );
    let removed = project
        .apply(ProjectIntent::RemoveMedia {
            media_ids: vec![ids[1]],
            mode: MediaRemovalMode::RemoveAll,
        })
        .unwrap();
    assert_eq!(removed.state.revision, changed.state.revision + 1);
    assert!(removed.composition.sheets[0].overlays.is_empty());
    assert!(
        matches!(&removed.composition.sheets[0].backgrounds[0], ComposedBackground::Color { rgb, .. } if rgb == "#00AA00")
    );
    assert_eq!(project.undo().unwrap().composition, changed.composition);
    assert_eq!(project.redo().unwrap().composition, removed.composition);
    project.save(project.projection().state.revision).unwrap();
    drop(project);
    let reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.projection().composition, removed.composition);
}

#[test]
fn decorative_drop_preview_uses_sheet_zones_and_single_page_scope_without_history() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&root.path().join("Preview.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let path = root.path().join("Decorativo.png");
    std::fs::write(&path, b"original").unwrap();
    let imported = project
        .import_media(
            MediaKind::Decorative,
            vec![ImportMedia::new(
                path,
                PhotoSourceMetadata::new(600, 400, std::array::from_fn(|_| "#FFFFFF".into()))
                    .unwrap(),
            )],
        )
        .unwrap();
    let media_id = imported.projection.state.album.media[0].id;
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet_id.clone(),
        })
        .unwrap();
    let before = project.projection();
    let mut request = DecorativeDropRequest {
        sheet_id: sheet_id.clone(),
        media_id,
        role: DecorativeRole::Overlay,
        x_um: 300_000,
        y_um: 150_000,
    };
    for (x, scope) in [
        (100_000, DecorativeScope::Left),
        (300_000, DecorativeScope::BothSides),
        (500_000, DecorativeScope::Right),
    ] {
        request.x_um = x;
        let preview = project.preview_decorative_drop(&request).unwrap().unwrap();
        assert_eq!(preview.scope, scope);
        assert_eq!(preview.sheet.frames, before.composition.sheets[0].frames);
        assert_eq!(preview.center_rect.unwrap().width, 120_000);
        assert_eq!(preview.sheet.overlays.len(), 1);
        assert_eq!(project.projection(), before);
    }
    request.x_um = -1;
    assert!(project.preview_decorative_drop(&request).unwrap().is_none());
    request.x_um = 500_000;
    let preview = project.preview_decorative_drop(&request).unwrap().unwrap();
    let committed = project
        .apply(ProjectIntent::DropDecorative {
            request: request.clone(),
        })
        .unwrap();
    assert_eq!(committed.composition.sheets[0], preview.sheet);
    assert_eq!(committed.state.revision, before.state.revision + 1);
    project
        .apply(ProjectIntent::ConvertEdgeSheet {
            sheet_id: sheet_id.clone(),
        })
        .unwrap();
    request.x_um = 150_000;
    let preview = project.preview_decorative_drop(&request).unwrap().unwrap();
    assert_eq!(preview.scope, DecorativeScope::Right);
    assert!(preview.center_rect.is_none());
    assert_eq!(preview.sheet.width_um, 300_000);
    project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id,
            role: DecorativeRole::Background,
            scope: DecorativeScope::BothSides,
        })
        .unwrap();
    let expanded = project
        .apply(ProjectIntent::ConvertEdgeSheet { sheet_id })
        .unwrap();
    assert!(
        matches!(&expanded.composition.sheets[0].backgrounds[0], ComposedBackground::Media { draw_rect, .. } if draw_rect.width == 600_000)
    );
}

#[test]
fn public_decorative_projections_match_the_visual_corpus() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Decorativos.myalbuns");
    let mut input: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v10_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    for (suffix, name) in [(20, "Textura.png"), (30, "Overlay.png")] {
        input["project"]["media"].as_array_mut().unwrap().push(serde_json::json!({
            "id": format!("00000000-0000-4000-8000-{suffix:012}"), "kind": "decorative",
            "path": { "encoding": "windowsUtf16", "units": format!("C:\\Fotos\\{name}").encode_utf16().collect::<Vec<_>>() }
        }));
    }
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let initial = project.projection();
    project
        .observe_photo_source(
            initial.state.album.media[0].id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    let media_ids: Vec<_> = initial
        .state
        .album
        .media
        .iter()
        .filter(|media| media.kind == MediaKind::Decorative)
        .map(|media| media.id)
        .collect();
    let sheet_id = initial.state.album.sheets[0].id.clone();
    let mut states = serde_json::Map::new();
    let mut previews = Vec::new();
    let mut transitions = Vec::new();
    for name in ["neutral", "whole", "split", "overlay", "single"] {
        match name {
            "whole" | "split" | "overlay" => {
                project
                    .apply(ProjectIntent::ApplyDecorative {
                        sheet_id: sheet_id.clone(),
                        media_id: if name == "whole" {
                            media_ids[0]
                        } else {
                            media_ids[1]
                        },
                        role: if name == "overlay" {
                            DecorativeRole::Overlay
                        } else {
                            DecorativeRole::Background
                        },
                        scope: if name == "split" {
                            DecorativeScope::Left
                        } else {
                            DecorativeScope::BothSides
                        },
                    })
                    .unwrap();
            }
            "single" => {
                project
                    .apply(ProjectIntent::ConvertEdgeSheet {
                        sheet_id: sheet_id.clone(),
                    })
                    .unwrap();
            }
            _ => {}
        }
        states.insert(
            name.into(),
            serde_json::to_value(project.projection()).unwrap(),
        );
        for media_id in &media_ids {
            for role in [DecorativeRole::Background, DecorativeRole::Overlay] {
                let positions = if name == "single" {
                    vec![150_000]
                } else {
                    vec![100_000, 300_000, 500_000]
                };
                for x_um in positions {
                    let request = DecorativeDropRequest {
                        sheet_id: sheet_id.clone(),
                        media_id: *media_id,
                        role,
                        x_um,
                        y_um: 150_000,
                    };
                    let preview = project.preview_decorative_drop(&request).unwrap().unwrap();
                    previews.push(
                        serde_json::json!({ "from": name, "request": request, "preview": preview }),
                    );
                }
                for scope in [
                    DecorativeScope::BothSides,
                    DecorativeScope::Left,
                    DecorativeScope::Right,
                ] {
                    if name == "single" && scope == DecorativeScope::Left {
                        continue;
                    }
                    let intent = ProjectIntent::ApplyDecorative {
                        sheet_id: sheet_id.clone(),
                        media_id: *media_id,
                        role,
                        scope,
                    };
                    let before = project.projection();
                    let after = project.apply(intent.clone()).unwrap();
                    transitions.push(
                        serde_json::json!({ "from": name, "intent": intent, "projection": after }),
                    );
                    if after.state.revision != before.state.revision {
                        project.undo().unwrap();
                    }
                }
            }
        }
    }
    let serialized = format!("{}\n", serde_json::to_string_pretty(&serde_json::json!({ "states": states, "previews": previews, "transitions": transitions })).unwrap());
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/decorative-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_DECORATIVE_FIXTURE").is_some() {
        std::fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        std::fs::read_to_string(fixture)
            .unwrap()
            .replace("\r\n", "\n")
    );
}
