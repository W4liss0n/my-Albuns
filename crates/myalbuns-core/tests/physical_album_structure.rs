use std::fs;

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat, ImportPhoto,
    InitialProject, InitialProjectConfiguration, OpenProjectRequest, PhotoPlacementMode,
    PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation, ProjectedActiveSides,
    SaveProjectOutcome, SheetInsertionPosition, SheetRole,
};
use myalbuns_paths::OperationPathContext;

fn project_location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the Project path is captured at the public boundary");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn physical_album(sheet_count: i64) -> InitialProject {
    InitialProject::configured(InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        600_000,
        300_000,
        300,
        3_000,
        3_000,
        sheet_count,
        EndSheetFormat::SinglePage,
        EndSheetFormat::SinglePage,
    ))
}

fn create_project(
    core: &ProjectCore,
    path: &std::path::Path,
    sheet_count: i64,
) -> myalbuns_core::EditableProject {
    core.create_editable(CreateProjectRequest::new(
        project_location(path),
        physical_album(sheet_count),
        CreateAuthorization::CreateOnly,
    ))
    .expect("the physical Album is created through ProjectCore")
}

#[test]
fn adding_a_blank_double_sheet_respects_single_page_edges_and_is_one_revision() {
    let root = tempfile::tempdir().expect("temporary physical Album root");
    let project_path = root.path().join("Adicionar Lâmina.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path, 3);
    let before = project.projection();
    let first_id = before.state.album.sheets[0].id.clone();
    let last_id = before
        .state
        .album
        .sheets
        .last()
        .expect("the physical Album has a final Sheet")
        .id
        .clone();

    assert_eq!(
        project
            .apply(ProjectIntent::AddSheet {
                anchor_sheet_id: first_id.clone(),
                position: SheetInsertionPosition::Before,
            })
            .expect_err("nothing may be inserted outside the initial single Page"),
        CoreError::InvalidSheetInsertion
    );
    assert_eq!(project.projection(), before, "a rejected add is atomic");
    assert_eq!(
        project
            .apply(ProjectIntent::AddSheet {
                anchor_sheet_id: last_id,
                position: SheetInsertionPosition::After,
            })
            .expect_err("nothing may be inserted outside the final single Page"),
        CoreError::InvalidSheetInsertion
    );
    assert_eq!(
        project.projection(),
        before,
        "both rejected edges are atomic"
    );

    let added = project
        .apply_with_outcome(ProjectIntent::AddSheet {
            anchor_sheet_id: first_id,
            position: SheetInsertionPosition::After,
        })
        .expect("a blank double Sheet is inserted inside the Album");
    let added_id = added
        .affected_sheet_id
        .expect("the public outcome identifies the Sheet to center");

    assert_eq!(added.projection.state.revision, 1);
    assert_eq!(added.projection.state.album.sheets.len(), 4);
    assert_eq!(
        added
            .projection
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| (
                sheet.id.as_str(),
                sheet.number,
                sheet.role,
                sheet.active_sides,
                sheet.page_numbers.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                added.projection.state.album.sheets[0].id.as_str(),
                1,
                SheetRole::Initial,
                ProjectedActiveSides::Right,
                vec![1],
            ),
            (
                added_id.as_str(),
                2,
                SheetRole::Internal,
                ProjectedActiveSides::Both,
                vec![2, 3],
            ),
            (
                added.projection.state.album.sheets[2].id.as_str(),
                3,
                SheetRole::Internal,
                ProjectedActiveSides::Both,
                vec![4, 5],
            ),
            (
                added.projection.state.album.sheets[3].id.as_str(),
                4,
                SheetRole::Final,
                ProjectedActiveSides::Left,
                vec![6],
            ),
        ]
    );
    let new_sheet = added
        .projection
        .composition
        .sheets
        .iter()
        .find(|sheet| sheet.sheet_id == added_id)
        .expect("the added Sheet is composed");
    assert!(new_sheet.frames.is_empty(), "the new Sheet is blank");
    assert_eq!(
        new_sheet.backgrounds.len(),
        1,
        "Album defaults are inherited"
    );

    let undone = project.undo().expect("the add is one Undo step");
    assert_eq!(undone.state.album.sheets.len(), 3);
    let redone = project.redo().expect("the add is one Redo step");
    assert_eq!(redone.state.album.sheets.len(), 4);
}

#[test]
fn adding_outside_double_edges_transfers_the_derived_edge_role() {
    let root = tempfile::tempdir().expect("temporary double-edge Album root");
    let project_path = root.path().join("Adicionar nas extremidades.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let configured = InitialProject::configured(InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        600_000,
        300_000,
        300,
        3_000,
        3_000,
        3,
        EndSheetFormat::Double,
        EndSheetFormat::Double,
    ));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            configured,
            CreateAuthorization::CreateOnly,
        ))
        .expect("the double-edge Album is created");
    let initial = project.projection();
    let original_first_id = initial.state.album.sheets[0].id.clone();

    let before = project
        .apply_with_outcome(ProjectIntent::AddSheet {
            anchor_sheet_id: original_first_id.clone(),
            position: SheetInsertionPosition::Before,
        })
        .expect("a double initial Sheet may be pushed inward");
    let added_first_id = before
        .affected_sheet_id
        .expect("the new initial Sheet is identified");
    assert_eq!(before.projection.state.album.sheets[0].id, added_first_id);
    assert_eq!(
        before.projection.state.album.sheets[0].role,
        SheetRole::Initial
    );
    assert_eq!(
        before.projection.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Both
    );
    assert_eq!(
        before.projection.state.album.sheets[1].id,
        original_first_id
    );
    assert_eq!(
        before.projection.state.album.sheets[1].role,
        SheetRole::Internal
    );

    let current_last_id = before
        .projection
        .state
        .album
        .sheets
        .last()
        .expect("the final double Sheet remains available")
        .id
        .clone();
    let after = project
        .apply_with_outcome(ProjectIntent::AddSheet {
            anchor_sheet_id: current_last_id.clone(),
            position: SheetInsertionPosition::After,
        })
        .expect("a double final Sheet may be pushed inward");
    let added_last_id = after
        .affected_sheet_id
        .expect("the new final Sheet is identified");
    let final_sheet = after
        .projection
        .state
        .album
        .sheets
        .last()
        .expect("the inserted final Sheet exists");
    assert_eq!(final_sheet.id, added_last_id);
    assert_eq!(final_sheet.role, SheetRole::Final);
    assert_eq!(final_sheet.active_sides, ProjectedActiveSides::Both);
    assert_eq!(
        after.projection.state.album.sheets[after.projection.state.album.sheets.len() - 2].id,
        current_last_id
    );
    assert_eq!(
        after.projection.state.album.sheets[after.projection.state.album.sheets.len() - 2].role,
        SheetRole::Internal
    );
}

#[test]
fn deleting_a_sheet_centers_its_neighbor_and_enforces_the_two_sheet_minimum() {
    let root = tempfile::tempdir().expect("temporary physical Album root");
    let project_path = root.path().join("Excluir Lâmina.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path, 3);
    let middle_id = project.projection().state.album.sheets[1].id.clone();

    let deleted = project
        .apply_with_outcome(ProjectIntent::DeleteSheet {
            sheet_id: middle_id.clone(),
        })
        .expect("an Album with three Sheets may lose one");
    assert_eq!(deleted.projection.state.album.sheets.len(), 2);
    assert!(
        !deleted
            .projection
            .state
            .album
            .sheets
            .iter()
            .any(|sheet| sheet.id == middle_id)
    );
    assert_eq!(
        deleted.affected_sheet_id.as_deref(),
        Some(deleted.projection.state.album.sheets[1].id.as_str()),
        "the next Sheet is preferred for centering"
    );

    let remaining_id = deleted.projection.state.album.sheets[0].id.clone();
    assert_eq!(
        project
            .apply(ProjectIntent::DeleteSheet {
                sheet_id: remaining_id,
            })
            .expect_err("the minimum physical structure is two Sheets"),
        CoreError::MinimumSheetCount
    );
    assert_eq!(project.projection(), deleted.projection);

    let restored = project
        .undo()
        .expect("deletion restores the complete Sheet");
    assert_eq!(restored.state.album.sheets.len(), 3);
    assert!(
        restored
            .state
            .album
            .sheets
            .iter()
            .any(|sheet| sheet.id == middle_id)
    );
}

#[test]
fn deleting_either_edge_prefers_the_next_or_previous_neighbor_and_rederives_roles() {
    let root = tempfile::tempdir().expect("temporary edge-deletion Album root");
    let project_path = root.path().join("Excluir extremidades.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path, 4);
    let initial = project.projection();
    let first_id = initial.state.album.sheets[0].id.clone();
    let next_id = initial.state.album.sheets[1].id.clone();
    let last_id = initial
        .state
        .album
        .sheets
        .last()
        .expect("the Album has a final Sheet")
        .id
        .clone();
    let previous_id = initial.state.album.sheets[2].id.clone();

    let deleted_first = project
        .apply_with_outcome(ProjectIntent::DeleteSheet { sheet_id: first_id })
        .expect("the initial single Page may be deleted");
    assert_eq!(
        deleted_first.affected_sheet_id.as_deref(),
        Some(next_id.as_str())
    );
    assert_eq!(deleted_first.projection.state.album.sheets[0].id, next_id);
    assert_eq!(
        deleted_first.projection.state.album.sheets[0].role,
        SheetRole::Initial
    );
    assert_eq!(
        deleted_first.projection.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Both
    );

    project.undo().expect("Undo restores the initial edge");
    let deleted_last = project
        .apply_with_outcome(ProjectIntent::DeleteSheet { sheet_id: last_id })
        .expect("the final single Page may be deleted");
    assert_eq!(
        deleted_last.affected_sheet_id.as_deref(),
        Some(previous_id.as_str())
    );
    let new_last = deleted_last
        .projection
        .state
        .album
        .sheets
        .last()
        .expect("the previous Sheet becomes final");
    assert_eq!(new_last.id, previous_id);
    assert_eq!(new_last.role, SheetRole::Final);
    assert_eq!(new_last.active_sides, ProjectedActiveSides::Both);
}

#[test]
fn deleting_and_undoing_a_composed_sheet_never_removes_its_media_catalog_entry() {
    let root = tempfile::tempdir().expect("temporary composed Album root");
    let project_path = root.path().join("Excluir composição.myalbuns");
    let photo_path = root.path().join("Foto preservada.jpg");
    fs::write(&photo_path, b"trusted external JPEG fixture").expect("the linked original exists");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path, 3);
    let imported = project
        .import_photo(ImportPhoto::new(
            photo_path,
            PhotoSourceMetadata::new(
                1_200,
                800,
                ["#B83225".into(), "#477998".into(), "#F4EBD9".into()],
            )
            .expect("the observed JPEG metadata is valid"),
        ))
        .expect("the linked Photo enters the Album catalog");
    let middle_id = imported.projection.state.album.sheets[1].id.clone();
    let composed = project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: middle_id.clone(),
            media_id: imported.media_id,
            mode: PhotoPlacementMode::Normal,
        })
        .expect("the middle Sheet receives one composed Photo");
    assert_eq!(composed.state.album.sheets[1].frames.len(), 1);

    let deleted = project
        .apply(ProjectIntent::DeleteSheet {
            sheet_id: middle_id.clone(),
        })
        .expect("the composed Sheet is deleted as one revision");
    assert_eq!(deleted.state.album.media.len(), 1, "media is never pruned");
    assert_eq!(
        deleted.media_usage[0].count, 0,
        "unused media remains visible"
    );
    assert!(
        !deleted
            .composition
            .sheets
            .iter()
            .any(|sheet| sheet.sheet_id == middle_id)
    );

    let restored = project.undo().expect("Undo restores the full composition");
    let restored_sheet = restored
        .composition
        .sheets
        .iter()
        .find(|sheet| sheet.sheet_id == middle_id)
        .expect("the deleted Sheet returns");
    assert_eq!(restored_sheet.frames.len(), 1);
    assert_eq!(restored.state.album.media.len(), 1);
    assert_eq!(restored.media_usage[0].count, 1);
}

#[test]
fn reordering_is_atomic_persistent_and_never_interiorizes_a_single_page() {
    let root = tempfile::tempdir().expect("temporary physical Album root");
    let project_path = root.path().join("Reordenar Lâminas.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = create_project(&core, &project_path, 5);
    let initial = project.projection();
    let initial_single_id = initial.state.album.sheets[0].id.clone();
    let first_internal_id = initial.state.album.sheets[1].id.clone();
    let moved_id = initial.state.album.sheets[3].id.clone();

    assert_eq!(
        project
            .apply(ProjectIntent::ReorderSheet {
                sheet_id: initial_single_id,
                target_index: 2,
            })
            .expect_err("a single Page cannot become an internal Sheet"),
        CoreError::InvalidSheetReorder
    );
    assert_eq!(
        project.projection(),
        initial,
        "an invalid drop changes nothing"
    );
    assert_eq!(
        project
            .apply(ProjectIntent::ReorderSheet {
                sheet_id: first_internal_id,
                target_index: 0,
            })
            .expect_err("an indirect move cannot push the initial single Page inward"),
        CoreError::InvalidSheetReorder
    );
    assert_eq!(
        project.projection(),
        initial,
        "an invalid indirect reorder is atomic"
    );

    let reordered = project
        .apply_with_outcome(ProjectIntent::ReorderSheet {
            sheet_id: moved_id.clone(),
            target_index: 1,
        })
        .expect("an internal double Sheet can move between valid slots");
    assert_eq!(
        reordered.affected_sheet_id.as_deref(),
        Some(moved_id.as_str())
    );
    assert_eq!(
        reordered
            .projection
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| sheet.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            initial.state.album.sheets[0].id.as_str(),
            initial.state.album.sheets[3].id.as_str(),
            initial.state.album.sheets[1].id.as_str(),
            initial.state.album.sheets[2].id.as_str(),
            initial.state.album.sheets[4].id.as_str(),
        ]
    );
    assert_eq!(
        reordered
            .projection
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| sheet.page_numbers.clone())
            .collect::<Vec<_>>(),
        vec![vec![1], vec![2, 3], vec![4, 5], vec![6, 7], vec![8]],
    );
    let undone = project.undo().expect("the reorder is one Undo step");
    assert_eq!(
        undone
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| sheet.id.as_str())
            .collect::<Vec<_>>(),
        initial
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| sheet.id.as_str())
            .collect::<Vec<_>>(),
    );
    let redone = project.redo().expect("the reorder is one Redo step");
    assert_eq!(
        redone.state.album.sheets[1].id, moved_id,
        "Redo restores the committed physical order",
    );
    let frozen = project
        .freeze_rendering()
        .into_sheet(&moved_id)
        .expect("the active reordered Sheet is an exportable output unit");
    assert_eq!(frozen.output_unit().sheet.sheet_id, moved_id);
    assert_eq!(frozen.output_unit().sheet.number, 2);

    assert_eq!(
        project
            .save(reordered.projection.state.revision)
            .expect("the reordered Album is saved"),
        SaveProjectOutcome::Saved { revision: 1 }
    );
    drop(project);
    let reopened = core
        .open_editable(OpenProjectRequest::new(project_location(&project_path)))
        .expect("the saved physical order reopens");
    assert_eq!(
        reopened
            .projection()
            .state
            .album
            .sheets
            .iter()
            .map(|sheet| sheet.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            initial.state.album.sheets[0].id.as_str(),
            initial.state.album.sheets[3].id.as_str(),
            initial.state.album.sheets[1].id.as_str(),
            initial.state.album.sheets[2].id.as_str(),
            initial.state.album.sheets[4].id.as_str(),
        ]
    );
}
