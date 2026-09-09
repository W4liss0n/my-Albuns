#![cfg(windows)]

#[path = "layout_session/visual_corpus.rs"]
mod visual_corpus;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, LayoutSelection, ProjectCore,
    ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;
use std::path::Path;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project(root: &Path) -> myalbuns_core::EditableProject {
    ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Layouts.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap()
}

#[test]
fn preview_is_transient_and_commit_matches_it_in_one_undo_action() {
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..3 {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
    }
    let before = project.projection();
    let document = project.project().clone();
    let query = project.query_layouts(&sheet).unwrap();
    assert!(!query.listing.candidates.is_empty());
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    let preview = project.preview_layout(&selection).unwrap();
    assert_eq!(project.project(), &document);
    assert_eq!(project.projection(), before);
    let applied = project
        .apply(ProjectIntent::ApplyLayout { selection })
        .unwrap();
    assert_eq!(applied.composition.sheets[0].frames, preview);
    assert_eq!(applied.state.revision, before.state.revision + 1);
    let undone = project.undo().unwrap();
    assert_eq!(project.project(), &document);
    assert_eq!(undone.state.revision, before.state.revision);
    assert!(undone.state.can_redo);
    assert_eq!(undone.composition, before.composition);
    assert_eq!(project.redo().unwrap(), applied);
    let again = project.query_layouts(&sheet).unwrap();
    assert!(again.listing.candidates[0].is_last_applied);
}

#[test]
fn a_changed_revision_or_a_different_query_cannot_confirm_an_old_preview() {
    use myalbuns_core::CoreError;
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheets = project.projection().state.album.sheets;
    let sheet = sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    let query = project.query_layouts(&sheet).unwrap();
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    project.apply(ProjectIntent::SetDpi { dpi: 240 }).unwrap();
    let after_edit = project.projection();
    assert_eq!(
        project.preview_layout(&selection),
        Err(CoreError::StaleLayoutPreview)
    );
    assert_eq!(
        project.apply(ProjectIntent::ApplyLayout { selection }),
        Err(CoreError::StaleLayoutPreview)
    );
    assert_eq!(project.projection(), after_edit);
    let query = project.query_layouts(&sheet).unwrap();
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    project.query_layouts(&sheets[1].id).unwrap();
    assert_eq!(
        project.apply(ProjectIntent::ApplyLayout { selection }),
        Err(CoreError::StaleLayoutPreview)
    );
    assert_eq!(project.projection(), after_edit);
}

#[test]
fn saving_and_reopening_preserves_last_layout_and_generation_settings() {
    use myalbuns_core::{LayoutParameters, LayoutPermission, LayoutSettings, OpenProjectRequest};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut project = project(root);
    let sheet = project.projection().state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    let settings = LayoutSettings {
        permission: LayoutPermission::PagesOnly,
        parameters: LayoutParameters {
            margin_um: 19000,
            gap_um: 6000,
            minimum_side_um: 25000,
        },
    };
    project
        .apply(ProjectIntent::SetLayoutSettings {
            settings: settings.clone(),
        })
        .unwrap();
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let expected = project.project().clone();
    let composition = project.projection().composition;
    project.save(project.revision()).unwrap();
    drop(project);
    let mut reopened = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .open_editable(OpenProjectRequest::new(location(
            &root.join("Layouts.myalbuns"),
        )))
        .unwrap();
    assert_eq!(reopened.project(), &expected);
    assert_eq!(reopened.projection().composition, composition);
    let query = reopened.query_layouts(&sheet).unwrap();
    assert_eq!(query.settings, settings);
    assert!(query.listing.candidates[0].is_last_applied);
    let bytes = std::fs::read(root.join("Layouts.myalbuns")).unwrap();
    let persisted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(persisted["schemaVersion"], 8);
}

#[test]
fn v8_requires_complete_layout_payloads_and_rejects_corrupt_geometry() {
    use myalbuns_core::{DocumentFailure, LoadProjectError, LoadProjectRequest};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let mut project = project(root);
    let sheet = project.projection().state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    project.save(project.revision()).unwrap();
    drop(project);
    let valid: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("Layouts.myalbuns")).unwrap()).unwrap();
    let mut missing = valid.clone();
    missing["project"]["sheets"][0]
        .as_object_mut()
        .unwrap()
        .remove("lastLayout");
    let mut negative = valid.clone();
    negative["project"]["layoutSettings"]["gapUm"] = serde_json::json!(-1);
    let mut unknown = valid.clone();
    unknown["project"]["layoutSettings"]["weight"] = serde_json::json!(0.5);
    let mut empty = valid.clone();
    empty["project"]["sheets"][0]["lastLayout"]["definition"]["positions"] = serde_json::json!([]);
    let mut geometry = valid.clone();
    geometry["project"]["sheets"][0]["lastLayout"]["definition"]["positions"][0]["width"] =
        serde_json::json!(9007199254740991i64);
    for (i, invalid) in [missing, negative, unknown, empty, geometry]
        .into_iter()
        .enumerate()
    {
        let path = root.join(format!("invalid-{i}.myalbuns"));
        let bytes = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let core = ProjectCore::new()
            .with_identity_storage_roots(root.join("leases"), root.join("identities"));
        assert!(
            matches!(
                core.load_persisted_revision(LoadProjectRequest::new(location(&path))),
                Err(LoadProjectError::Document(
                    DocumentFailure::InvalidProjectDocument | DocumentFailure::InvalidProjectState
                ))
            ),
            "invalid case {i}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn normal_insertion_uses_real_initial_frame_profiles_and_remembers_the_chosen_layout() {
    use myalbuns_core::{ImportPhoto, PhotoPlacementMode, PhotoSourceMetadata};
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let path = directory.path().join("Portrait.jpg");
    std::fs::write(&path, b"linked original").unwrap();
    project
        .import_photo(ImportPhoto::new(
            path,
            PhotoSourceMetadata::new(
                800,
                1200,
                ["#FF0000".into(), "#00FF00".into(), "#0000FF".into()],
            )
            .unwrap(),
        ))
        .unwrap();
    let before = project.projection();
    let sheet = before.state.album.sheets[0].id.clone();
    let media = before.state.album.media[0].id;
    for _ in 0..2 {
        project
            .apply(ProjectIntent::AddPhoto {
                sheet_id: sheet.clone(),
                media_id: media,
                mode: PhotoPlacementMode::Normal,
            })
            .unwrap();
    }
    let current = project.projection();
    let frames = &current.state.album.sheets[0].frames;
    assert_eq!(frames.len(), 2);
    assert!(
        frames.iter().all(|f| f.rect.width > f.rect.height),
        "new Frames start with the canonical manual 3:2 profile"
    );
    let query = project.query_layouts(&sheet).unwrap();
    assert!(query.listing.candidates[0].is_last_applied);
    assert_eq!(
        project
            .preview_layout(&LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0
            })
            .unwrap(),
        current.composition.sheets[0].frames
    );
}

#[test]
fn converting_a_populated_extremity_keeps_content_styles_and_one_undo_action() {
    use myalbuns_core::{
        FrameStyleChange, FrameStyleEdit, ImportPhoto, PhotoPlacementMode, PhotoSourceMetadata,
    };
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..3 {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
    }
    let path = directory.path().join("Photo.jpg");
    std::fs::write(&path, b"linked original").unwrap();
    project
        .import_photo(ImportPhoto::new(
            path,
            PhotoSourceMetadata::new(
                1200,
                800,
                ["#FF0000".into(), "#00FF00".into(), "#0000FF".into()],
            )
            .unwrap(),
        ))
        .unwrap();
    let media = project.projection().state.album.media[0].id;
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: sheet.clone(),
            media_id: media,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::SetFrameStyle {
            edit: FrameStyleEdit {
                frame_ids: vec![id.clone()],
                change: FrameStyleChange::Opacity {
                    opacity_percent: 37,
                },
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: id,
            delta_pan_x: 0.4,
            delta_pan_y: -0.2,
            delta_zoom: 0.5,
        })
        .unwrap();
    let before = project.projection();
    let document = project.project().clone();
    let converted = project
        .apply(ProjectIntent::ConvertEdgeSheet {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    assert_eq!(
        converted.state.album.sheets[0].active_sides,
        myalbuns_core::ProjectedActiveSides::Right
    );
    assert_eq!(converted.state.album.sheets[0].frames.len(), 3);
    for (a, b) in before.state.album.sheets[0]
        .frames
        .iter()
        .zip(&converted.state.album.sheets[0].frames)
    {
        assert_eq!(
            (a.id.clone(), a.z_index, a.photo.clone(), a.style.clone()),
            (b.id.clone(), b.z_index, b.photo.clone(), b.style.clone())
        );
        assert!(b.rect.x >= 0 && b.rect.x + b.rect.width <= 300_000);
    }
    assert_eq!(converted.state.revision, before.state.revision + 1);
    project.undo().unwrap();
    assert_eq!(project.project(), &document);
    assert_eq!(project.redo().unwrap(), converted);
    let query = project.query_layouts(&sheet).unwrap();
    assert!(query.listing.candidates[0].is_last_applied);
}

#[test]
fn album_information_converts_both_populated_edges_in_one_history_action() {
    use myalbuns_core::{AlbumInformation, DisplayUnit, EndSheetFormat, ProjectedActiveSides};
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheets = project.projection().state.album.sheets;
    for sheet in &sheets {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.id.clone(),
            })
            .unwrap();
    }
    let before = project.project().clone();
    let revision = project.revision();
    let information = AlbumInformation {
        display_unit: DisplayUnit::Mm,
        sheet_width_um: 600_000,
        sheet_height_um: 300_000,
        dpi: 240,
        bleed_um: 3_000,
        safety_um: 3_000,
        first_sheet: EndSheetFormat::SinglePage,
        last_sheet: EndSheetFormat::SinglePage,
    };
    assert!(
        project
            .validate_album_information(&information)
            .errors
            .is_empty()
    );
    let after = project
        .apply(ProjectIntent::SetAlbumInformation { information })
        .unwrap();
    assert_eq!(after.state.revision, revision + 1);
    assert_eq!(
        after.state.album.sheets[0].active_sides,
        ProjectedActiveSides::Right
    );
    assert_eq!(
        after.state.album.sheets.last().unwrap().active_sides,
        ProjectedActiveSides::Left
    );
    for sheet in &after.state.album.sheets {
        assert!(
            sheet
                .frames
                .iter()
                .all(|frame| frame.rect.x >= 0 && frame.rect.x + frame.rect.width <= 300_000)
        );
        assert!(project.query_layouts(&sheet.id).unwrap().listing.candidates[0].is_last_applied);
    }
    project.undo().unwrap();
    assert_eq!(project.project(), &before);
}

#[test]
fn deletion_reorganizes_only_in_normal_mode_and_undo_restores_the_whole_action() {
    use myalbuns_core::PhotoPlacementMode;
    for mode in [PhotoPlacementMode::Normal, PhotoPlacementMode::Edit] {
        let directory = tempfile::tempdir().unwrap();
        let mut project = project(directory.path());
        let sheet = project.projection().state.album.sheets[0].id.clone();
        for _ in 0..3 {
            project
                .apply(ProjectIntent::AddFrame {
                    sheet_id: sheet.clone(),
                })
                .unwrap();
        }
        let before = project.projection();
        let document = project.project().clone();
        let frames = &before.state.album.sheets[0].frames;
        let after = project
            .apply(ProjectIntent::DeleteFrames {
                frame_ids: vec![frames[0].id.clone(), frames[2].id.clone()],
                mode,
            })
            .unwrap();
        assert_eq!(after.state.revision, before.state.revision + 1);
        assert_eq!(after.state.album.sheets[0].frames.len(), 1);
        assert_eq!(after.state.album.sheets[0].frames[0].id, frames[1].id);
        if mode == PhotoPlacementMode::Edit {
            assert_eq!(after.state.album.sheets[0].frames[0].rect, frames[1].rect);
        } else {
            assert_ne!(after.state.album.sheets[0].frames[0].rect, frames[1].rect);
            assert!(project.query_layouts(&sheet).unwrap().listing.candidates[0].is_last_applied);
        }
        project.undo().unwrap();
        assert_eq!(project.project(), &document);
        assert_eq!(project.redo().unwrap(), after);
    }
}
