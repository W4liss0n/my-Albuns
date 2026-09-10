use super::{location, project};
use myalbuns_core::*;
use serde_json::{Value, json};
use std::fs;

fn prepare(root: &std::path::Path, count: usize) -> (EditableProject, String) {
    let mut project = project(root);
    let sheet = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..count {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
    }
    (project, sheet)
}

fn toggle(
    project: &mut EditableProject,
    sheet: &str,
    predicate: impl Fn(&LayoutCandidate) -> bool,
) {
    let query = project.query_layouts(sheet).unwrap();
    let index = query.listing.candidates.iter().position(predicate).unwrap();
    project
        .apply(ProjectIntent::ToggleLayoutFavorite {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: index,
            },
        })
        .unwrap();
}

#[test]
fn starring_is_one_undoable_project_edit_without_applying_or_locking_the_layout() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheet) = prepare(root.path(), 1);
    project.save(project.revision()).unwrap();
    let before = project.project().clone();
    let projection = project.projection();
    let query = project
        .query_layouts_with_frame_request(
            &sheet,
            Some(LayoutFrameRequest {
                frame_count: 3,
                orientation: FrameOrientation::Horizontal,
            }),
        )
        .unwrap();
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    let chosen = query.listing.candidates[0].layout.clone();
    project
        .apply(ProjectIntent::ToggleLayoutFavorite {
            selection: selection.clone(),
        })
        .unwrap();
    assert_eq!(project.revision(), projection.state.revision + 1);
    assert!(project.has_unsaved_changes());
    assert_eq!(project.projection().composition, projection.composition);
    assert_eq!(project.projection().state.album, projection.state.album);
    let favorite = project.project().favorite_layouts()[0].clone();
    assert_eq!(favorite.layout, chosen);
    assert_eq!(favorite.layout.definition.positions.len(), 3);
    assert!(matches!(
        project.apply(ProjectIntent::ToggleLayoutFavorite { selection }),
        Err(CoreError::StaleLayoutPreview)
    ));
    project.undo().unwrap();
    assert_eq!(project.project(), &before);
    assert!(!project.has_unsaved_changes());
    project.redo().unwrap();
    assert_eq!(project.project().favorite_layouts(), &[favorite]);
}

#[test]
fn equal_geometry_has_independent_stars_per_origin_and_orphans_survive_only_as_favorite_or_last() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheet) = prepare(root.path(), 1);
    let generated = project.query_layouts(&sheet).unwrap().listing.candidates[0]
        .layout
        .clone();
    let custom_id = CustomLayoutId::generate();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 1,
            entries: vec![CustomLayout {
                id: custom_id,
                definition: generated.definition.clone(),
            }],
        })
        .unwrap();
    toggle(&mut project, &sheet, |item| {
        item.custom_id == Some(custom_id)
    });
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Automatic
            && LayoutRules::same_definition(&item.layout.definition, &generated.definition)
    });
    let favorites = project.project().favorite_layouts().to_vec();
    assert_eq!(favorites.len(), 2);
    assert_ne!(favorites[0].id, favorites[1].id);
    let before = project.projection();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 2,
            entries: vec![],
        })
        .unwrap();
    assert_eq!(project.projection(), before);
    let query = project.query_layouts(&sheet).unwrap();
    assert_eq!(
        query
            .listing
            .candidates
            .iter()
            .filter(|item| item.favorite_id.is_some())
            .count(),
        2
    );
    assert!(
        query
            .listing
            .candidates
            .iter()
            .all(|item| item.custom_id.is_none())
    );
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Custom
    });
    assert!(
        project
            .query_layouts(&sheet)
            .unwrap()
            .listing
            .candidates
            .iter()
            .all(|item| item.layout.origin != LayoutOrigin::Custom)
    );
    project.undo().unwrap();
    let query = project.query_layouts(&sheet).unwrap();
    let index = query
        .listing
        .candidates
        .iter()
        .position(|item| item.layout.origin == LayoutOrigin::Custom)
        .unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: index,
            },
        })
        .unwrap();
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Custom
    });
    let query = project.query_layouts(&sheet).unwrap();
    assert!(query.listing.candidates[0].is_last_applied);
    assert_eq!(
        query.listing.candidates[0].layout.origin,
        LayoutOrigin::Custom
    );
    assert_eq!(query.listing.candidates[0].favorite_id, None);
    let other = project.projection().state.album.sheets[1].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: other.clone(),
        })
        .unwrap();
    assert!(
        project
            .query_layouts(&other)
            .unwrap()
            .listing
            .candidates
            .iter()
            .all(|item| item.layout.origin != LayoutOrigin::Custom)
    );
}

#[test]
fn favorite_copies_survive_save_reopen_save_as_and_external_copy_and_remain_project_local() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheet) = prepare(root.path(), 1);
    let definition = project.capture_custom_layout(&sheet).unwrap();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 1,
            entries: vec![CustomLayout {
                id: CustomLayoutId::generate(),
                definition,
            }],
        })
        .unwrap();
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Custom
    });
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Automatic
    });
    let favorites = project.project().favorite_layouts().to_vec();
    project.save(project.revision()).unwrap();
    let path = root.path().join("Layouts.myalbuns");
    let bytes = fs::read(&path).unwrap();
    let original_id = project.project_id();
    drop(project);
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.project().favorite_layouts(), favorites);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let copy_path = root.path().join("Cópia externa.myalbuns");
    fs::copy(&path, &copy_path).unwrap();
    let mut copied = core
        .open_editable(OpenProjectRequest::new(location(&copy_path)))
        .unwrap();
    assert_ne!(copied.project_id(), original_id);
    assert_eq!(copied.project().favorite_layouts(), favorites);
    toggle(&mut copied, &sheet, |item| item.favorite_id.is_some());
    assert_eq!(copied.project().favorite_layouts().len(), 1);
    assert_eq!(reopened.project().favorite_layouts(), favorites);
    let save_as_path = root.path().join("Salvar como.myalbuns");
    reopened
        .save_as(SaveAsProjectRequest::new(
            reopened.revision(),
            location(&save_as_path),
            SaveAsAuthorization::CreateOnly,
        ))
        .unwrap();
    assert_ne!(reopened.project_id(), original_id);
    assert_eq!(reopened.project().favorite_layouts(), favorites);
    drop(reopened);
    let saved_as = core
        .open_editable(OpenProjectRequest::new(location(&save_as_path)))
        .unwrap();
    assert_eq!(saved_as.project().favorite_layouts(), favorites);
    let unrelated_root = tempfile::tempdir().unwrap();
    assert!(
        super::project(unrelated_root.path())
            .project()
            .favorite_layouts()
            .is_empty()
    );
}

#[test]
fn current_schema_rejects_incomplete_or_corrupt_favorites_without_rewriting_the_source() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheet) = prepare(root.path(), 2);
    toggle(&mut project, &sheet, |_| true);
    project.save(project.revision()).unwrap();
    let valid: Value = serde_json::from_slice(&fs::read(project.project_path()).unwrap()).unwrap();
    assert_eq!(valid["schemaVersion"], 11);
    let original = valid["project"]["favoriteLayouts"][0].clone();
    let mut cases = Vec::new();
    let mut missing = valid.clone();
    missing["project"]
        .as_object_mut()
        .unwrap()
        .remove("favoriteLayouts");
    cases.push(missing);
    for (field, value) in [
        ("id", json!("not-a-uuid")),
        ("id", json!("550E8400-E29B-41D4-A716-446655440000")),
        ("id", json!("00000000-0000-1000-8000-000000000001")),
        ("order", json!(-1)),
        ("order", json!(9007199254740992u64)),
        ("name", json!("extra")),
    ] {
        let mut invalid = valid.clone();
        invalid["project"]["favoriteLayouts"][0][field] = value;
        cases.push(invalid);
    }
    let mut duplicate = valid.clone();
    duplicate["project"]["favoriteLayouts"]
        .as_array_mut()
        .unwrap()
        .push(original.clone());
    cases.push(duplicate);
    let mut duplicate_geometry = valid.clone();
    let mut second = original.clone();
    second["id"] = json!(LayoutFavoriteId::generate());
    duplicate_geometry["project"]["favoriteLayouts"]
        .as_array_mut()
        .unwrap()
        .push(second);
    cases.push(duplicate_geometry);
    let mut bad_geometry = valid.clone();
    bad_geometry["project"]["favoriteLayouts"][0]["layout"]["definition"]["positions"] = json!([]);
    cases.push(bad_geometry);
    let mut unknown_nested = valid.clone();
    unknown_nested["project"]["favoriteLayouts"][0]["layout"]["name"] = json!("extra");
    cases.push(unknown_nested);
    let mut missing_order = valid.clone();
    missing_order["project"]["favoriteLayouts"][0]
        .as_object_mut()
        .unwrap()
        .remove("order");
    cases.push(missing_order);
    for (i, invalid) in cases.into_iter().enumerate() {
        let path = root.path().join(format!("invalid-favorite-{i}.myalbuns"));
        let bytes = serde_json::to_vec(&invalid).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(
            ProjectCore::new()
                .load_persisted_revision(LoadProjectRequest::new(location(&path)))
                .is_err(),
            "case {i}"
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn v9_opens_with_no_favorites_and_upgrades_only_on_explicit_save() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Legado.myalbuns");
    let bytes = include_bytes!("../fixtures/project_document_v9_migration_expected.myalbuns");
    fs::write(&path, bytes).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert!(project.project().favorite_layouts().is_empty());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    project.save(project.revision()).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["schemaVersion"], 11);
    assert_eq!(saved["project"]["favoriteLayouts"], json!([]));
}

#[test]
fn changing_global_geometry_and_generation_settings_keeps_the_captured_favorites() {
    let root = tempfile::tempdir().unwrap();
    let (mut project, sheet) = prepare(root.path(), 2);
    let custom = CustomLayout {
        id: CustomLayoutId::generate(),
        definition: project.capture_custom_layout(&sheet).unwrap(),
    };
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 1,
            entries: vec![custom.clone()],
        })
        .unwrap();
    toggle(&mut project, &sheet, |item| item.custom_id.is_some());
    toggle(&mut project, &sheet, |item| {
        item.layout.origin == LayoutOrigin::Automatic
    });
    let favorites = project.project().favorite_layouts().to_vec();
    let before = project.projection().composition;
    let mut changed = custom;
    changed.definition.positions[0].width /= 2;
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 2,
            entries: vec![changed.clone()],
        })
        .unwrap();
    project
        .apply(ProjectIntent::SetLayoutSettings {
            settings: LayoutSettings {
                permission: LayoutPermission::PagesAndSheet,
                parameters: LayoutParameters {
                    margin_um: 25000,
                    gap_um: 9000,
                    minimum_side_um: 25000,
                },
            },
        })
        .unwrap();
    assert_eq!(project.project().favorite_layouts(), favorites);
    assert_eq!(project.projection().composition, before);
    let query = project.query_layouts(&sheet).unwrap();
    assert_eq!(
        query
            .listing
            .candidates
            .iter()
            .filter_map(|item| item.favorite_id)
            .collect::<Vec<_>>(),
        favorites.iter().map(|item| item.id).collect::<Vec<_>>()
    );
    let updated_origin = query
        .listing
        .candidates
        .iter()
        .find(|item| item.custom_id == Some(changed.id))
        .unwrap();
    assert_eq!(updated_origin.favorite_id, None);
    assert_eq!(updated_origin.layout.definition, changed.definition);
}
