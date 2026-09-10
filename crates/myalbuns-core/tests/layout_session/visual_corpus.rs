use myalbuns_core::{
    EditableProject, FrameStyleChange, FrameStyleEdit, LayoutSelection, OpenProjectRequest,
    PhotoSourceMetadata, ProjectCore, ProjectIntent,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn fixture_project(root: &Path, count: usize) -> EditableProject {
    let mut document: Value = serde_json::from_str(include_str!(
        "../fixtures/project_document_v6_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    let source = document["project"]["sheets"][0]["frames"][0].clone();
    document["project"]["sheets"][0]["frames"] = json!(
        (0..count)
            .map(|i| {
                let mut frame = source.clone();
                frame["id"] = json!(format!("00000000-0000-4000-8000-{:012}", 101 + i));
                frame["rect"] = json!(match i % 4 {
                    0 => json!({"x":30000,"y":40000,"width":100000,"height":150000}),
                    1 => json!({"x":320000,"y":60000,"width":150000,"height":100000}),
                    2 => json!({"x":360000,"y":170000,"width":90000,"height":90000}),
                    _ => json!({"x":180000,"y":170000,"width":150000,"height":100000}),
                });
                if i % 2 == 1 {
                    frame["photo"] = Value::Null;
                }
                frame
            })
            .collect::<Vec<_>>()
    );
    let path = root.join("Layouts.myalbuns");
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .open_editable(OpenProjectRequest::new(super::location(&path)))
        .unwrap();
    let before = project.projection();
    project
        .observe_photo_source(
            before.state.album.media[0].id,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    if let Some(frame) = before.state.album.sheets[0].frames.first() {
        for change in [
            FrameStyleChange::Opacity {
                opacity_percent: 60,
            },
            FrameStyleChange::BorderColor {
                rgb: "#205070".into(),
            },
            FrameStyleChange::BorderWidth { width_um: 2000 },
        ] {
            project
                .apply(ProjectIntent::SetFrameStyle {
                    edit: FrameStyleEdit {
                        frame_ids: vec![frame.id.clone()],
                        change,
                    },
                })
                .unwrap();
        }
    }
    project
}

fn record(project: &mut EditableProject, name: &str) -> Value {
    record_expansion(project, name, None).0
}

fn record_expansion(
    project: &mut EditableProject,
    name: &str,
    expansion: Option<myalbuns_core::LayoutExpansion>,
) -> (Value, Option<LayoutSelection>) {
    let projection = project.projection();
    let mut queries = serde_json::Map::new();
    let mut selection = None;
    for sheet in &projection.state.album.sheets {
        let extra = (sheet.id == projection.state.album.sheets[0].id)
            .then(|| expansion.clone())
            .flatten();
        let query = project
            .query_layouts_with_expansion(&sheet.id, extra)
            .unwrap();
        let previews: Vec<_> = (0..query.listing.candidates.len())
            .map(|index| {
                project
                    .preview_layout(&LayoutSelection {
                        query_id: query.query_id.clone(),
                        candidate_index: index,
                    })
                    .unwrap()
            })
            .collect();
        if sheet.id == projection.state.album.sheets[0].id && !query.listing.candidates.is_empty() {
            selection = Some(LayoutSelection {
                query_id: query.query_id.clone(),
                candidate_index: 0,
            });
        }
        let mut query = serde_json::to_value(query).unwrap();
        query["queryId"] = json!(format!("{name}-{}", sheet.id));
        let mut sample = json!({"query":query,"previews":previews});
        let mut ids = BTreeMap::new();
        for (candidate, preview) in previews.iter().enumerate() {
            for (index, frame) in preview.iter().enumerate().skip(sheet.frames.len()) {
                ids.insert(
                    frame.frame_id.clone(),
                    format!(
                        "{name}-{}-candidate-{candidate}-placeholder-{index}",
                        sheet.id
                    ),
                );
            }
        }
        normalize(&mut sample, &ids);
        queries.insert(sheet.id.clone(), sample);
    }
    assert_eq!(
        project.projection(),
        projection,
        "querying and composing never touch history"
    );
    (
        json!({"projection":projection,"queries":queries}),
        selection,
    )
}

#[test]
fn layout_panel_corpus_is_produced_by_the_public_core() {
    let mut cases = serde_json::Map::new();
    for (name, count) in [
        ("mixed", 4),
        ("single", 4),
        ("empty", 0),
        ("outside", 31),
        ("expanded", 4),
        ("empty-lock", 0),
        ("custom", 4),
        ("custom-save", 4),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut project = fixture_project(root.path(), count);
        let sheet = project.projection().state.album.sheets[0].id.clone();
        if name == "single" {
            project
                .apply(ProjectIntent::ConvertEdgeSheet {
                    sheet_id: sheet.clone(),
                })
                .unwrap();
        }
        let custom = if matches!(name, "custom" | "custom-save") {
            let captured = project.capture_custom_layout(&sheet).unwrap();
            let generated = project.query_layouts(&sheet).unwrap().listing.candidates[0]
                .layout
                .definition
                .clone();
            Some(myalbuns_core::LayoutCatalogSnapshot {
                revision: 1,
                entries: vec![
                    myalbuns_core::CustomLayout {
                        id: serde_json::from_value(json!("00000000-0000-4000-8000-000000000801"))
                            .unwrap(),
                        definition: captured,
                    },
                    myalbuns_core::CustomLayout {
                        id: serde_json::from_value(json!("00000000-0000-4000-8000-000000000802"))
                            .unwrap(),
                        definition: generated,
                    },
                ],
            })
        } else {
            None
        };
        if name == "custom" {
            project
                .refresh_layout_catalog(custom.clone().unwrap())
                .unwrap();
        }
        let before = record(&mut project, name);
        let query = project.query_layouts(&sheet).unwrap();
        let applied = if query.listing.candidates.is_empty()
            || matches!(name, "expanded" | "empty-lock" | "custom-save")
        {
            Value::Null
        } else {
            let preview = project
                .preview_layout(&LayoutSelection {
                    query_id: query.query_id.clone(),
                    candidate_index: 0,
                })
                .unwrap();
            let previous = project.projection();
            project
                .apply(ProjectIntent::ApplyLayout {
                    selection: LayoutSelection {
                        query_id: query.query_id,
                        candidate_index: 0,
                    },
                })
                .unwrap();
            let applied = project.projection();
            assert_eq!(applied.composition.sheets[0].frames, preview);
            for (a, b) in previous.state.album.sheets[0]
                .frames
                .iter()
                .zip(&applied.state.album.sheets[0].frames)
            {
                assert_eq!(
                    (&a.id, a.z_index, &a.photo, &a.style),
                    (&b.id, b.z_index, &b.photo, &b.style)
                );
            }
            record(&mut project, &format!("{name}-applied"))
        };
        let mut entry = json!({"before":before,"applied":applied});
        if let Some(custom) = custom {
            if !entry["applied"].is_null() {
                project.undo().unwrap();
            }
            project.refresh_layout_catalog(custom.clone()).unwrap();
            entry["catalogSaved"] = record(&mut project, &format!("{name}-saved"));
            entry["saveResult"] = serde_json::to_value(myalbuns_core::SaveCustomLayoutResult {
                catalog_revision: custom.revision,
                layout_id: custom.entries[0].id,
                created: name == "custom-save",
            })
            .unwrap();
            project
                .refresh_layout_catalog(myalbuns_core::LayoutCatalogSnapshot {
                    revision: 2,
                    entries: vec![custom.entries[1].clone()],
                })
                .unwrap();
            entry["catalogDeleted"] = record(&mut project, &format!("{name}-deleted"));
        }
        if matches!(name, "mixed" | "expanded" | "empty-lock") {
            if !entry["applied"].is_null() {
                project.undo().unwrap();
            }
            let extra = matches!(name, "expanded" | "empty-lock").then_some(
                myalbuns_core::LayoutExpansion {
                    additional_positions: 2,
                    orientation: myalbuns_core::FrameOrientation::Horizontal,
                },
            );
            let (ready, selection) =
                record_expansion(&mut project, &format!("{name}-ready"), extra.clone());
            entry["lockReady"] = ready;
            // Queries for other Sheets replace the session's handle, so prepare the
            // target last and retain precisely its previews and generated Frame IDs.
            let query = project.query_layouts_with_expansion(&sheet, extra).unwrap();
            let selection = LayoutSelection {
                query_id: query.query_id.clone(),
                candidate_index: selection.unwrap().candidate_index,
            };
            let previews: Vec<_> = (0..query.listing.candidates.len())
                .map(|candidate_index| {
                    project
                        .preview_layout(&LayoutSelection {
                            query_id: query.query_id.clone(),
                            candidate_index,
                        })
                        .unwrap()
                })
                .collect();
            let mut serialized_query = serde_json::to_value(&query).unwrap();
            serialized_query["queryId"] = json!(format!("{name}-ready-{sheet}"));
            entry["lockReady"]["queries"][&sheet] =
                json!({"query":serialized_query,"previews":previews});
            let locked = project
                .apply(ProjectIntent::LockLayout { selection })
                .unwrap();
            assert_eq!(locked.composition.sheets[0].frames, previews[0]);
            entry["locked"] = record(&mut project, &format!("{name}-locked"));
            let problems = project
                .freeze_rendering()
                .validate_export_sheets(std::slice::from_ref(&sheet))
                .unwrap();
            entry["exportProblems"] = serde_json::to_value(problems).unwrap();
            project
                .apply(ProjectIntent::UnlockLayout {
                    sheet_id: sheet.clone(),
                })
                .unwrap();
            entry["unlocked"] = record(&mut project, &format!("{name}-unlocked"));
            project.undo().unwrap();
            let media_id = project
                .projection()
                .state
                .album
                .media
                .iter()
                .find(|m| m.kind == myalbuns_core::MediaKind::Photo)
                .unwrap()
                .id;
            while project.projection().state.album.sheets[0]
                .frames
                .iter()
                .any(|frame| frame.photo.is_none())
            {
                project
                    .apply(ProjectIntent::AddPhoto {
                        sheet_id: sheet.clone(),
                        media_id,
                        mode: myalbuns_core::PhotoPlacementMode::Normal,
                    })
                    .unwrap();
            }
            entry["filled"] = record(&mut project, &format!("{name}-filled"));
            assert!(project.freeze_rendering().into_sheet(&sheet).is_ok());
            let first = project.projection().state.album.sheets[0].frames[0]
                .id
                .clone();
            project
                .apply(ProjectIntent::DeleteFrames {
                    frame_ids: vec![first],
                    mode: myalbuns_core::PhotoPlacementMode::Edit,
                })
                .unwrap();
            entry["cleared"] = record(&mut project, &format!("{name}-cleared"));
            // Normalize only nondeterministic UUID allocation, never geometry.
            let mut ids = BTreeMap::new();
            for (candidate, preview) in previews.iter().enumerate().skip(1) {
                for (index, frame) in preview.iter().enumerate().skip(count) {
                    ids.insert(
                        frame.frame_id.clone(),
                        format!("candidate-{candidate}-placeholder-{}", index + 1),
                    );
                }
            }
            for (index, frame) in locked.state.album.sheets[0]
                .frames
                .iter()
                .enumerate()
                .skip(count)
            {
                ids.insert(frame.id.clone(), format!("lock-placeholder-{}", index + 1));
            }
            normalize(&mut entry, &ids);
        }
        cases.insert(name.into(), entry);
    }
    cases.insert("favorites".into(), favorite_case());
    let mut value = json!({"cases":cases});
    let before = &value["cases"]["mixed"]["before"]["projection"];
    let ids: BTreeMap<String, String> = before["state"]["album"]["sheets"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, sheet)| {
            (
                sheet["id"].as_str().unwrap().to_owned(),
                format!("sheet-{:03}", i + 1),
            )
        })
        .collect();
    normalize(&mut value, &ids);
    let serialized = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/layout-panel-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_LAYOUT_FIXTURE").is_some() {
        fs::write(&path, &serialized).unwrap();
    }
    assert!(
        serialized == fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
        "Layout corpus changed; regenerate with MYALBUNS_UPDATE_LAYOUT_FIXTURE and inspect the fixture diff"
    );
}

fn normalize(value: &mut Value, ids: &BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            for (id, replacement) in ids {
                *text = text.replace(id, replacement);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize(item, ids);
            }
        }
        Value::Object(fields) => {
            let previous = std::mem::take(fields);
            for (key, mut value) in previous {
                normalize(&mut value, ids);
                fields.insert(ids.get(&key).cloned().unwrap_or(key), value);
            }
        }
        _ => {}
    }
}

fn favorite_case() -> Value {
    use myalbuns_core::{CustomLayout, LayoutCatalogSnapshot, LayoutOrigin};
    let root = tempfile::tempdir().unwrap();
    let mut project = fixture_project(root.path(), 4);
    let sheet = project.projection().state.album.sheets[0].id.clone();
    let custom_id = serde_json::from_value(json!("00000000-0000-4000-8000-000000000801")).unwrap();
    let definition = project.capture_custom_layout(&sheet).unwrap();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 1,
            entries: vec![CustomLayout {
                id: custom_id,
                definition,
            }],
        })
        .unwrap();
    let before = record(&mut project, "favorites-before");
    let mut states = serde_json::Map::new();
    states.insert("before".into(), before.clone());
    let mut transitions = Vec::new();
    for (from, to, origin) in [
        ("before", "automatic", LayoutOrigin::Automatic),
        ("automatic", "both", LayoutOrigin::Custom),
    ] {
        let query = project.query_layouts(&sheet).unwrap();
        let index = query
            .listing
            .candidates
            .iter()
            .position(|item| item.layout.origin == origin)
            .unwrap();
        project
            .apply(ProjectIntent::ToggleLayoutFavorite {
                selection: LayoutSelection {
                    query_id: query.query_id,
                    candidate_index: index,
                },
            })
            .unwrap();
        states.insert(to.into(), record(&mut project, &format!("favorites-{to}")));
        transitions.push(json!({"from":from,"to":to,"candidateIndex":index}));
    }
    let ids: BTreeMap<_, _> = project
        .project()
        .favorite_layouts()
        .iter()
        .enumerate()
        .map(|(i, item)| {
            (
                item.id.to_string(),
                format!("00000000-0000-4000-8000-{:012}", 901 + i),
            )
        })
        .collect();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 2,
            entries: vec![],
        })
        .unwrap();
    states.insert(
        "orphaned".into(),
        record(&mut project, "favorites-orphaned"),
    );
    let query = project.query_layouts(&sheet).unwrap();
    let index = query
        .listing
        .candidates
        .iter()
        .position(|item| item.layout.origin == LayoutOrigin::Custom)
        .unwrap();
    project
        .apply(ProjectIntent::ToggleLayoutFavorite {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: index,
            },
        })
        .unwrap();
    states.insert(
        "unfavorited".into(),
        record(&mut project, "favorites-unfavorited"),
    );
    transitions.push(json!({"from":"orphaned","to":"unfavorited","candidateIndex":index}));
    let mut result = json!({"before":before,"applied":null,"favoriteStates":states,"favoriteTransitions":transitions});
    normalize(&mut result, &ids);
    result
}
