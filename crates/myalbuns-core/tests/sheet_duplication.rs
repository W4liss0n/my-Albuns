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
fn duplicate_inserts_an_independent_sheet_after_its_source_in_one_persistent_history_action() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Duplicação.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let source_id = project.projection().state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: source_id.clone(),
        })
        .unwrap();
    project
        .apply(ProjectIntent::EditSheetVisual {
            sheet_id: source_id.clone(),
            scope: DecorativeScope::Left,
            change: SheetVisualChange::BackgroundColor {
                rgb: "#123456".into(),
            },
        })
        .unwrap();
    let before = project.projection();
    let intent =
        serde_json::from_value(serde_json::json!({ "kind":"duplicateSheet", "sheetId":source_id }))
            .unwrap();
    let outcome = project.apply_with_outcome(intent).unwrap();
    let after = outcome.projection;
    assert_eq!(after.state.revision, before.state.revision + 1);
    assert_eq!(after.state.album.sheets.len(), 3);
    let copy = &after.state.album.sheets[1];
    assert_eq!(outcome.affected_sheet_id.as_deref(), Some(copy.id.as_str()));
    assert_ne!(copy.id, source_id);
    assert_ne!(copy.frames[0].id, before.state.album.sheets[0].frames[0].id);
    assert_eq!(copy.visuals, before.state.album.sheets[0].visuals);
    assert_eq!(
        copy.frames[0].rect,
        before.state.album.sheets[0].frames[0].rect
    );
    let mut original = after.state.album.sheets[0].clone();
    assert!(original.structure.availability.can_delete);
    assert!(
        !before.state.album.sheets[0]
            .structure
            .availability
            .can_delete
    );
    original.structure = before.state.album.sheets[0].structure.clone();
    assert_eq!(original, before.state.album.sheets[0]);
    assert_eq!(
        after.state.album.sheets[2].id,
        before.state.album.sheets[1].id
    );
    assert_eq!(copy.role, SheetRole::Internal);
    assert_eq!(copy.page_numbers, vec![3, 4]);
    assert_eq!(after.state.album.sheets[2].page_numbers, vec![5, 6]);
    assert_eq!(after.state.album.media, before.state.album.media);
    assert_eq!(project.undo().unwrap().composition, before.composition);
    assert_eq!(project.redo().unwrap().composition, after.composition);
    project.save(project.revision()).unwrap();
    drop(project);
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(project.projection().composition, after.composition);
    let edited = project
        .apply(ProjectIntent::EditSheetVisual {
            sheet_id: copy.id.clone(),
            scope: DecorativeScope::BothSides,
            change: SheetVisualChange::BackgroundColor {
                rgb: "#ABCDEF".into(),
            },
        })
        .unwrap();
    assert_eq!(edited.composition.sheets[0], before.composition.sheets[0]);
    assert_ne!(
        edited.composition.sheets[1].backgrounds,
        after.composition.sheets[1].backgrounds
    );
}

#[test]
fn duplication_preserves_complete_composition_layout_and_media_and_emits_the_visual_corpus() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Duplicação completa.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    // A persisted input with stable media identity also exercises unavailable originals.
    let mut input: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/project_file_v1/photo.myalbuns")).unwrap();
    input["project"]["media"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id":"00000000-0000-4000-8000-000000000020", "kind":"decorative",
            "path":"C:\\Fotos\\Overlay.png"
        }));
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let initial = project.projection();
    let source_id = initial.state.album.sheets[0].id.clone();
    let photo_id = initial.state.album.media[0].id;
    let decorative_id = initial
        .state
        .album
        .media
        .iter()
        .find(|media| media.kind == MediaKind::Decorative)
        .unwrap()
        .id;
    project
        .observe_photo_source(
            photo_id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: source_id.clone(),
            media_id: photo_id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: source_id.clone(),
        })
        .unwrap();
    let frame_id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    for action in [
        PhotoOrientationAction::RotateCounterClockwise,
        PhotoOrientationAction::ToggleHorizontalMirror,
    ] {
        project
            .apply(ProjectIntent::OrientPhotos {
                frame_ids: vec![frame_id.clone()],
                action,
            })
            .unwrap();
    }
    project
        .apply(ProjectIntent::SetPhotoAngle {
            edit: PhotoAngleEdit {
                frame_ids: vec![frame_id.clone()],
                angle_tenths: 123,
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![frame_id.clone()],
        })
        .unwrap();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: frame_id.clone(),
            delta_pan_x: 0.04,
            delta_pan_y: 0.02,
            delta_zoom: 0.3,
        })
        .unwrap();
    for change in [
        FrameStyleChange::BorderColor {
            rgb: "#205070".into(),
        },
        FrameStyleChange::BorderWidth { width_um: 2000 },
        FrameStyleChange::Opacity {
            opacity_percent: 65,
        },
    ] {
        project
            .apply(ProjectIntent::SetFrameStyle {
                edit: FrameStyleEdit {
                    frame_ids: vec![frame_id.clone()],
                    change,
                },
            })
            .unwrap();
    }
    project
        .apply(ProjectIntent::ArrangeFrames {
            frame_ids: vec![frame_id],
            action: FrameStackAction::BringToFront,
        })
        .unwrap();
    let query = project.query_layouts(&source_id).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let defaults = ProjectedVisualDefaults {
        background: ProjectedBackground::BothSides {
            both: ProjectedBackgroundContent::Media {
                media_id: decorative_id,
            },
        },
        overlay: ProjectedOverlay::BothSides {
            both: Some(ProjectedOverlayContent::Media {
                media_id: decorative_id,
            }),
        },
        ..ProjectedVisualDefaults::default()
    };
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults,
        })
        .unwrap();
    project
        .apply(ProjectIntent::EditSheetVisual {
            sheet_id: source_id.clone(),
            scope: DecorativeScope::Left,
            change: SheetVisualChange::BackgroundColor {
                rgb: "#E7D6C4".into(),
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: source_id.clone(),
            media_id: decorative_id,
            role: DecorativeRole::Overlay,
            scope: DecorativeScope::Right,
        })
        .unwrap();
    let before = project.projection();
    assert!(before.state.album.sheets[0].layout_locked);
    let last_layout = project.project().sheets()[0].last_layout().cloned();
    assert!(last_layout.is_some());
    assert!(
        before.state.album.sheets[0]
            .frames
            .iter()
            .any(|f| f.photo.is_none())
    );
    let outcome = project
        .apply_with_outcome(ProjectIntent::DuplicateSheet {
            sheet_id: source_id.clone(),
        })
        .unwrap();
    let after = outcome.projection.clone();
    assert_eq!(
        project.project().sheets()[1].last_layout(),
        last_layout.as_ref()
    );
    let copy_id = outcome.affected_sheet_id.clone().unwrap();
    let mut comparable = after.state.album.sheets[1].clone();
    let source = &before.state.album.sheets[0];
    comparable.id = source.id.clone();
    comparable.number = source.number;
    comparable.role = source.role;
    comparable.page_numbers.clone_from(&source.page_numbers);
    assert!(!comparable.structure.availability.can_convert_edge);
    assert!(comparable.edge_conversion_loss.is_none());
    comparable.structure = source.structure.clone();
    comparable.edge_conversion_loss = source.edge_conversion_loss.clone();
    for (copy, original) in comparable.frames.iter_mut().zip(&source.frames) {
        assert_ne!(copy.id, original.id);
        copy.id.clone_from(&original.id);
    }
    assert_eq!(
        &comparable, source,
        "creative content, including stack, Photos, styles and Layout, is preserved"
    );
    assert_eq!(after.state.album.media, before.state.album.media);
    assert_eq!(project.undo().unwrap().composition, before.composition);
    assert_eq!(project.redo().unwrap().composition, after.composition);
    project.save(project.revision()).unwrap();
    drop(project);
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    project
        .observe_photo_source(
            photo_id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    let reopened = project.projection();
    assert_eq!(reopened.composition, after.composition);
    assert_eq!(reopened.state.album, after.state.album);
    assert_eq!(
        project.project().sheets()[1].last_layout(),
        last_layout.as_ref()
    );
    project
        .apply(ProjectIntent::UnlockLayout {
            sheet_id: copy_id.clone(),
        })
        .unwrap();
    let copy_photo = after.state.album.sheets[1]
        .frames
        .iter()
        .find(|f| f.photo.is_some())
        .unwrap()
        .id
        .clone();
    let edited = project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![copy_photo],
        })
        .unwrap();
    assert_eq!(edited.state.album.sheets[0], after.state.album.sheets[0]);
    assert_ne!(edited.state.album.sheets[1], after.state.album.sheets[1]);
    let inherited = project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: ProjectedVisualDefaults {
                background: ProjectedBackground::BothSides {
                    both: ProjectedBackgroundContent::Color {
                        rgb: "#112233".into(),
                    },
                },
                ..edited.state.album.visual_defaults.clone()
            },
        })
        .unwrap();
    for index in [0, 1] {
        assert!(
            matches!(&inherited.composition.sheets[index].backgrounds[0], ComposedBackground::Color {rgb,..} if rgb=="#E7D6C4")
        );
        assert!(
            matches!(&inherited.composition.sheets[index].backgrounds[1], ComposedBackground::Color {rgb,..} if rgb=="#112233")
        );
    }
    project.undo().unwrap();
    let single = project
        .apply(ProjectIntent::ConvertEdgeSheet {
            sheet_id: source_id,
        })
        .unwrap();

    let mut corpus = serde_json::json!({"before":before, "after":after, "outcome":outcome, "reopened":reopened, "edited":edited, "single":single});
    let mut ids = std::collections::BTreeMap::new();
    for (index, sheet) in after.state.album.sheets.iter().enumerate() {
        ids.insert(sheet.id.clone(), format!("dup-sheet-{}", index + 1));
        for (index, frame) in sheet.frames.iter().enumerate() {
            ids.insert(
                frame.id.clone(),
                format!("{}-frame-{}", ids[&sheet.id], index + 1),
            );
        }
    }
    normalize_ids(&mut corpus, &ids);
    let output = format!("{}\n", serde_json::to_string_pretty(&corpus).unwrap());
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sheet-duplication-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_DUPLICATION_FIXTURE").is_some() {
        std::fs::write(&fixture, &output).unwrap();
    }
    assert!(
        output
            == std::fs::read_to_string(fixture)
                .unwrap()
                .replace("\r\n", "\n"),
        "The public duplication corpus changed; inspect the projection before regenerating it."
    );
}

fn normalize_ids(value: &mut serde_json::Value, ids: &std::collections::BTreeMap<String, String>) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(id) = ids.get(text) {
                *text = id.clone();
            }
        }
        serde_json::Value::Array(items) => {
            items.iter_mut().for_each(|item| normalize_ids(item, ids))
        }
        serde_json::Value::Object(items) => {
            items.values_mut().for_each(|item| normalize_ids(item, ids))
        }
        _ => {}
    }
}

#[test]
fn duplication_rejects_single_page_edges_without_history_and_preserves_the_final_edge() {
    let root = tempfile::tempdir().unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let initial = InitialProject::configured(InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        600_000,
        300_000,
        300,
        3000,
        3000,
        3,
        EndSheetFormat::SinglePage,
        EndSheetFormat::SinglePage,
    ));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&root.path().join("Extremidades.myalbuns")),
            initial,
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let before = project.projection();
    for index in [0, 2] {
        assert!(
            project
                .apply(ProjectIntent::DuplicateSheet {
                    sheet_id: before.state.album.sheets[index].id.clone()
                })
                .is_err()
        );
        assert_eq!(project.projection(), before);
    }
    assert!(
        project
            .apply(ProjectIntent::DuplicateSheet {
                sheet_id: "00000000-0000-4000-8000-000000999999".into()
            })
            .is_err()
    );
    assert_eq!(project.projection(), before);
    let copy = project
        .apply(ProjectIntent::DuplicateSheet {
            sheet_id: before.state.album.sheets[1].id.clone(),
        })
        .unwrap();
    assert_eq!(
        copy.state
            .album
            .sheets
            .iter()
            .map(|s| s.page_numbers.clone())
            .collect::<Vec<_>>(),
        vec![vec![1], vec![2, 3], vec![4, 5], vec![6]]
    );
    let final_id = before.state.album.sheets[2].id.clone();
    project
        .apply(ProjectIntent::ConvertEdgeSheet {
            sheet_id: final_id.clone(),
        })
        .unwrap();
    let duplicated_final = project
        .apply(ProjectIntent::DuplicateSheet {
            sheet_id: final_id.clone(),
        })
        .unwrap();
    assert_eq!(duplicated_final.state.album.sheets[3].id, final_id);
    assert_eq!(
        duplicated_final.state.album.sheets[3].role,
        SheetRole::Internal
    );
    assert_eq!(
        duplicated_final.state.album.sheets[4].role,
        SheetRole::Final
    );
    assert!(duplicated_final.state.album.sheets[4].frames.is_empty());
}

#[test]
fn duplicating_several_sheets_repeats_each_run_after_itself_in_one_action() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Duplicar várias.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                3_000,
                3_000,
                6,
                EndSheetFormat::SinglePage,
                EndSheetFormat::SinglePage,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let before = project.projection();
    let ids: Vec<String> = before
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: ids[2].clone(),
        })
        .unwrap();
    let before = project.projection();

    let intent = serde_json::from_value(serde_json::json!({
        "kind": "duplicateSheets",
        "sheetIds": [ids[4], ids[1], ids[2]],
    }))
    .unwrap();
    let outcome = project.apply_with_outcome(intent).unwrap();
    let after = outcome.projection;
    assert_eq!(after.state.revision, before.state.revision + 1);
    let order: Vec<&str> = after
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.as_str())
        .collect();
    assert_eq!(order.len(), 9);
    assert_eq!(
        &order[..3],
        [ids[0].as_str(), ids[1].as_str(), ids[2].as_str()]
    );
    assert_eq!(order[5], ids[3].as_str());
    assert_eq!(order[6], ids[4].as_str());
    assert_eq!(order[8], ids[5].as_str());
    assert_eq!(outcome.affected_sheet_id.as_deref(), Some(order[3]));
    let copied = |index: usize| &after.state.album.sheets[index];
    for (copy, source) in [(3, 1), (4, 2), (7, 6)] {
        assert!(!ids.contains(&copied(copy).id));
        assert_eq!(copied(copy).visuals, copied(source).visuals);
        assert_eq!(copied(copy).frames.len(), copied(source).frames.len());
    }
    assert_ne!(copied(4).frames[0].id, copied(2).frames[0].id);
    assert_eq!(copied(4).frames[0].rect, copied(2).frames[0].rect);
    let restored = project.undo().unwrap();
    assert_eq!(restored.state.album, before.state.album);
    assert_eq!(restored.composition, before.composition);
}

#[test]
fn duplicating_several_sheets_refuses_single_page_edges_without_changes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Duplicar extremidade.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&path),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                3_000,
                3_000,
                4,
                EndSheetFormat::SinglePage,
                EndSheetFormat::SinglePage,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let before = project.projection();
    let ids: Vec<String> = before
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect();
    assert_eq!(
        project
            .apply(ProjectIntent::DuplicateSheets {
                sheet_ids: vec![ids[0].clone(), ids[1].clone()],
            })
            .unwrap_err(),
        CoreError::InvalidSheetDuplication
    );
    assert_eq!(project.projection(), before);
}
