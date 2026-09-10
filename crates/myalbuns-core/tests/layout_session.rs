#![cfg(windows)]

#[path = "layout_session/favorites.rs"]
mod favorites;
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
fn saving_and_reapplying_a_page_layout_preserves_the_frame_positions() {
    use myalbuns_core::{CustomLayout, CustomLayoutId, LayoutCatalogSnapshot, LayoutScope};
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..4 {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
    }
    let generated = project.query_layouts(&sheet).unwrap();
    let candidate_index = generated
        .listing
        .candidates
        .iter()
        .position(|candidate| candidate.layout.definition.scope == LayoutScope::Page)
        .unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: generated.query_id,
                candidate_index,
            },
        })
        .unwrap();
    let initial = project.projection().state.album.sheets[0].clone();
    for right in [false, true] {
        project
            .apply(ProjectIntent::EditFrameGeometry {
                edit: myalbuns_core::FrameGeometryEdit {
                    frames: initial
                        .frames
                        .iter()
                        .filter(|frame| (frame.rect.x * 2 >= initial.width_um) == right)
                        .map(|frame| myalbuns_core::FrameGeometryTarget {
                            frame_id: frame.id.clone(),
                            expected_rect: frame.rect.clone(),
                        })
                        .collect(),
                    gesture: myalbuns_core::FrameGeometryGesture::Move {
                        delta_x_um: if right { -2_000 } else { 2_000 },
                        delta_y_um: 1_000,
                    },
                },
            })
            .unwrap();
    }
    let before = project.projection().state.album.sheets[0].frames.clone();
    let definition = project.capture_custom_layout(&sheet).unwrap();
    assert_eq!(
        definition.positions,
        before
            .iter()
            .map(|frame| frame.rect.clone())
            .collect::<Vec<_>>()
    );
    let id = CustomLayoutId::generate();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 1,
            entries: vec![CustomLayout { id, definition }],
        })
        .unwrap();
    let saved = project.query_layouts(&sheet).unwrap();
    let candidate_index = saved
        .listing
        .candidates
        .iter()
        .position(|candidate| candidate.custom_id == Some(id))
        .unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: saved.query_id,
                candidate_index,
            },
        })
        .unwrap();
    let after = project.projection().state.album.sheets[0].frames.clone();
    assert_eq!(
        after.iter().map(|frame| &frame.rect).collect::<Vec<_>>(),
        before.iter().map(|frame| &frame.rect).collect::<Vec<_>>()
    );
}

#[test]
fn catalog_refresh_is_independent_of_project_history_and_deletion_preserves_the_last_copy() {
    use myalbuns_core::{CustomLayout, CustomLayoutId, LayoutCatalogSnapshot, LayoutOrigin};
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    assert!(project.capture_custom_layout(&sheet).is_err());
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    let captured = project.capture_custom_layout(&sheet).unwrap();
    let before = project.projection();
    let bytes = std::fs::read(root.path().join("Layouts.myalbuns")).unwrap();
    let id = CustomLayoutId::generate();
    let snapshot = LayoutCatalogSnapshot {
        revision: 1,
        entries: vec![CustomLayout {
            id,
            definition: captured.clone(),
        }],
    };
    assert!(project.refresh_layout_catalog(snapshot.clone()).unwrap());
    assert!(!project.refresh_layout_catalog(snapshot).unwrap());
    assert_eq!(project.projection(), before);
    assert_eq!(
        std::fs::read(root.path().join("Layouts.myalbuns")).unwrap(),
        bytes
    );
    let query = project.query_layouts(&sheet).unwrap();
    assert_eq!(query.catalog_revision, 1);
    let index = query
        .listing
        .candidates
        .iter()
        .position(|item| item.custom_id == Some(id))
        .unwrap();
    project
        .apply(ProjectIntent::ApplyLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: index,
            },
        })
        .unwrap();
    let applied = project.projection();
    project
        .refresh_layout_catalog(LayoutCatalogSnapshot {
            revision: 2,
            entries: vec![],
        })
        .unwrap();
    assert_eq!(project.projection(), applied);
    let query = project.query_layouts(&sheet).unwrap();
    let last = &query.listing.candidates[0];
    assert!(last.is_last_applied);
    assert_eq!(last.layout.origin, LayoutOrigin::Custom);
    assert_eq!(last.layout.definition, captured);
    assert_eq!(last.custom_id, None);
    project.undo().unwrap();
    let mut undone = before;
    undone.state.can_redo = true;
    assert_eq!(project.projection(), undone);
    assert!(
        project
            .query_layouts(&sheet)
            .unwrap()
            .listing
            .candidates
            .iter()
            .all(|item| item.custom_id.is_none())
    );
}

#[test]
fn export_rejects_placeholders_after_unlock_and_on_manual_frames() {
    for lock_then_unlock in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let mut project = project(root.path());
        let sheet = project.projection().state.album.sheets[0].id.clone();
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
        if lock_then_unlock {
            let query = project
                .query_layouts_with_expansion(
                    &sheet,
                    Some(myalbuns_core::LayoutExpansion {
                        additional_positions: 2,
                        orientation: myalbuns_core::FrameOrientation::Horizontal,
                    }),
                )
                .unwrap();
            project
                .apply(ProjectIntent::LockLayout {
                    selection: LayoutSelection {
                        query_id: query.query_id,
                        candidate_index: 0,
                    },
                })
                .unwrap();
            project
                .apply(ProjectIntent::UnlockLayout {
                    sheet_id: sheet.clone(),
                })
                .unwrap();
        }
        let frozen = project.freeze_rendering();
        assert_eq!(
            frozen
                .validate_export_sheets(std::slice::from_ref(&sheet))
                .unwrap()
                .len(),
            if lock_then_unlock { 3 } else { 1 }
        );
        assert!(matches!(
            frozen.into_sheet(&sheet),
            Err(myalbuns_core::CoreError::UnfilledLayoutPositions { .. })
        ));
        let path = root.path().join("Foto.jpg");
        std::fs::write(&path, b"linked original").unwrap();
        let media_id = project
            .import_photo(myalbuns_core::ImportPhoto::new(
                path,
                myalbuns_core::PhotoSourceMetadata::new(
                    600,
                    400,
                    ["#112233".into(), "#223344".into(), "#334455".into()],
                )
                .unwrap(),
            ))
            .unwrap()
            .media_id;
        for _ in 0..if lock_then_unlock { 3 } else { 1 } {
            project
                .apply(ProjectIntent::AddPhoto {
                    sheet_id: sheet.clone(),
                    media_id,
                    mode: myalbuns_core::PhotoPlacementMode::Normal,
                })
                .unwrap();
        }
        assert!(
            project.freeze_rendering().into_sheet(&sheet).is_ok(),
            "filling the same selection releases Export"
        );
    }
}

#[test]
fn locked_photo_content_can_be_filled_replaced_and_cleared_while_export_reports_each_selected_placeholder()
 {
    use myalbuns_core::{CoreError, ImportPhoto, PhotoPlacementMode, PhotoSourceMetadata};
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    let other = project.projection().state.album.sheets[1].id.clone();
    for _ in 0..2 {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet.clone(),
            })
            .unwrap();
    }
    let path = directory.path().join("Foto.jpg");
    std::fs::write(&path, b"original").unwrap();
    let media = project
        .import_photo(ImportPhoto::new(
            path,
            PhotoSourceMetadata::new(
                600,
                400,
                ["#112233".into(), "#223344".into(), "#334455".into()],
            )
            .unwrap(),
        ))
        .unwrap()
        .media_id;
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let before = project.projection();
    let frozen = project.freeze_rendering();
    let problems = frozen
        .validate_export_sheets(std::slice::from_ref(&sheet))
        .unwrap();
    assert_eq!(problems.len(), 2);
    assert_eq!(
        problems[0].frame_id,
        before.state.album.sheets[0].frames[0].id
    );
    assert_eq!(problems[1].frame_number, 2);
    assert!(matches!(
        frozen.clone().into_sheet(&sheet),
        Err(CoreError::UnfilledLayoutPositions { .. })
    ));
    assert!(
        frozen.into_sheet(&other).is_ok(),
        "placeholders outside the selection do not block it"
    );
    for _ in 0..2 {
        project
            .apply(ProjectIntent::AddPhoto {
                sheet_id: sheet.clone(),
                media_id: media,
                mode: PhotoPlacementMode::Normal,
            })
            .unwrap();
    }
    let filled = project.projection();
    assert!(project.freeze_rendering().into_sheet(&sheet).is_ok());
    assert_eq!(
        project.apply(ProjectIntent::AddPhoto {
            sheet_id: sheet.clone(),
            media_id: media,
            mode: PhotoPlacementMode::Edit
        }),
        Err(CoreError::LockedLayoutHasNoPlaceholder)
    );
    assert_eq!(project.projection(), filled);
    assert!(
        project
            .apply(ProjectIntent::DropPhoto {
                sheet_id: sheet.clone(),
                media_id: media,
                x_um: 0,
                y_um: 0,
                mode: PhotoPlacementMode::Normal
            })
            .is_err()
    );
    assert_eq!(project.projection(), filled);
    let first = &filled.state.album.sheets[0].frames[0];
    project
        .apply(ProjectIntent::DropPhoto {
            sheet_id: sheet.clone(),
            media_id: media,
            x_um: first.rect.x + 1,
            y_um: first.rect.y + 1,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    project
        .apply(ProjectIntent::TransformPhoto {
            frame_id: first.id.clone(),
            delta_pan_x: 0.02,
            delta_pan_y: 0.0,
            delta_zoom: 0.1,
        })
        .unwrap();
    for change in [
        myalbuns_core::FrameStyleChange::BorderColor {
            rgb: "#336699".into(),
        },
        myalbuns_core::FrameStyleChange::BorderWidth { width_um: 2_000 },
        myalbuns_core::FrameStyleChange::Opacity {
            opacity_percent: 60,
        },
    ] {
        project
            .apply(ProjectIntent::SetFrameStyle {
                edit: myalbuns_core::FrameStyleEdit {
                    frame_ids: vec![first.id.clone()],
                    change,
                },
            })
            .unwrap();
    }
    for action in [
        myalbuns_core::PhotoOrientationAction::RotateCounterClockwise,
        myalbuns_core::PhotoOrientationAction::ToggleHorizontalMirror,
    ] {
        project
            .apply(ProjectIntent::OrientPhotos {
                frame_ids: vec![first.id.clone()],
                action,
            })
            .unwrap();
    }
    project
        .apply(ProjectIntent::SetPhotoAngle {
            edit: myalbuns_core::PhotoAngleEdit {
                frame_ids: vec![first.id.clone()],
                angle_tenths: 123,
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::TogglePhotoBlackAndWhite {
            frame_ids: vec![first.id.clone()],
        })
        .unwrap();
    let content = project.projection();
    assert_ne!(content.state.album.sheets[0].frames[0].style, first.style);
    let transform = &content.state.album.sheets[0].frames[0]
        .photo
        .as_ref()
        .unwrap()
        .transform;
    assert!(transform.black_and_white && transform.mirror_x);
    assert_eq!(transform.fine_rotation_degrees, 12.3);
    for (original, current) in before.state.album.sheets[0]
        .frames
        .iter()
        .zip(&content.state.album.sheets[0].frames)
    {
        assert_eq!(original.rect, current.rect);
        assert_eq!(original.id, current.id);
    }
    project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: content.state.album.sheets[0]
                .frames
                .iter()
                .map(|frame| frame.id.clone())
                .collect(),
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    let mut expected_placeholders = content.state.album.sheets[0].frames.clone();
    for frame in &mut expected_placeholders {
        frame.photo = None;
    }
    assert_eq!(
        project.projection().state.album.sheets[0].frames,
        expected_placeholders
    );
    assert!(project.project().sheets()[0].layout_locked());
    project.undo().unwrap();
    assert_eq!(
        project.projection().state.album.sheets[0].frames,
        content.state.album.sheets[0].frames
    );
    assert!(project.freeze_rendering().into_sheet(&sheet).is_ok());
}

#[test]
fn the_locked_preview_tracks_frame_order_and_remains_available_after_permission_changes() {
    use myalbuns_core::{CoreError, FrameStackAction, LayoutPermission, LayoutSettings};
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
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::ArrangeFrames {
            frame_ids: vec![id],
            action: FrameStackAction::BringToFront,
        })
        .unwrap();
    project
        .apply(ProjectIntent::SetLayoutSettings {
            settings: LayoutSettings {
                permission: LayoutPermission::PagesOnly,
                ..Default::default()
            },
        })
        .unwrap();
    let before = project.projection();
    let query = project.query_layouts(&sheet).unwrap();
    assert!(query.locked);
    assert!(query.listing.candidates[0].is_last_applied);
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    assert_eq!(
        project.preview_layout(&selection).unwrap(),
        before.composition.sheets[0].frames
    );
    assert_eq!(
        project.apply(ProjectIntent::ApplyLayout { selection }),
        Err(CoreError::LayoutLocked)
    );
    assert_eq!(project.projection(), before);
    project
        .apply(ProjectIntent::UnlockLayout { sheet_id: sheet })
        .unwrap();
    assert_eq!(project.projection().composition, before.composition);
}

#[test]
fn expanded_preview_creates_inherited_placeholders_only_when_confirmed_by_the_lock() {
    use myalbuns_core::{CoreError, FrameOrientation, FrameStyleSource, LayoutExpansion};
    let directory = tempfile::tempdir().unwrap();
    let mut project = project(directory.path());
    let sheet = project.projection().state.album.sheets[0].id.clone();
    project
        .apply(ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    let before = project.projection();
    let query = project
        .query_layouts_with_expansion(
            &sheet,
            Some(LayoutExpansion {
                additional_positions: 2,
                orientation: FrameOrientation::Horizontal,
            }),
        )
        .unwrap();
    assert_eq!(query.frame_count, 1);
    assert!(
        query.listing.candidates.iter().all(|candidate| candidate
            .layout
            .definition
            .positions
            .len()
            == 3)
    );
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    let preview = project.preview_layout(&selection).unwrap();
    assert_eq!(preview.len(), 3);
    assert!(preview.iter().all(|frame| frame.photo.is_none()));
    assert_eq!(project.projection(), before);
    assert_eq!(
        project.apply(ProjectIntent::ApplyLayout {
            selection: selection.clone()
        }),
        Err(CoreError::LayoutRequiresLock)
    );
    assert_eq!(project.projection(), before);
    let applied = project
        .apply(ProjectIntent::LockLayout { selection })
        .unwrap();
    assert_eq!(applied.composition.sheets[0].frames, preview);
    assert_eq!(applied.state.revision, before.state.revision + 1);
    assert_eq!(
        applied.state.album.sheets[0].frames[0].id,
        before.state.album.sheets[0].frames[0].id
    );
    assert!(
        applied.state.album.sheets[0]
            .frames
            .iter()
            .all(|frame| frame.style.source == FrameStyleSource::Album)
    );
    project.undo().unwrap();
    assert_eq!(project.projection().state.album, before.state.album);
    assert_eq!(project.projection().composition, before.composition);
    assert_eq!(project.redo().unwrap(), applied);
}

#[test]
fn saving_a_locked_layout_uses_current_schema_and_migrates_v8_without_inventing_a_lock() {
    use myalbuns_core::OpenProjectRequest;
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
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let expected = project.project().clone();
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.join("Layouts.myalbuns");
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["schemaVersion"], 10);
    assert_eq!(saved["project"]["sheets"][0]["layoutLocked"], true);
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert_eq!(reopened.project(), &expected);
    drop(reopened);
    for sheet in saved["project"]["sheets"].as_array_mut().unwrap() {
        sheet.as_object_mut().unwrap().remove("layoutLocked");
    }
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    assert!(
        core.open_editable(OpenProjectRequest::new(location(&path)))
            .is_err(),
        "the current schema cannot silently default a missing lock"
    );
    saved["project"]
        .as_object_mut()
        .unwrap()
        .remove("favoriteLayouts");
    saved["schemaVersion"] = 8.into();
    let legacy = serde_json::to_vec(&saved).unwrap();
    std::fs::write(&path, &legacy).unwrap();
    let mut migrated = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    assert!(
        migrated
            .project()
            .sheets()
            .iter()
            .all(|sheet| !sheet.layout_locked())
    );
    assert_eq!(std::fs::read(&path).unwrap(), legacy);
    migrated.save(migrated.revision()).unwrap();
    let upgraded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(upgraded["schemaVersion"], 10);
    assert_eq!(upgraded["project"]["sheets"][0]["layoutLocked"], false);
}

#[test]
fn locked_structure_rejects_geometry_creation_and_paste_but_keeps_selection_order_and_placeholders()
{
    use myalbuns_core::{
        CoreError, FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
        FrameStackAction, PhotoDropTarget, PhotoPlacementMode,
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
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    let before = project.projection();
    let first = &before.state.album.sheets[0].frames[0];
    project
        .apply(ProjectIntent::CopyFrames {
            frame_ids: vec![first.id.clone()],
        })
        .unwrap();
    let before = project.projection();
    for intent in [
        ProjectIntent::AddFrame {
            sheet_id: sheet.clone(),
        },
        ProjectIntent::SwapSheetSides {
            sheet_id: sheet.clone(),
        },
        ProjectIntent::PasteFrames {
            sheet_id: sheet.clone(),
            desired_offset_um: 5_000,
        },
    ] {
        assert_eq!(project.apply(intent), Err(CoreError::LayoutLocked));
        assert_eq!(project.projection(), before);
    }
    for gesture in [
        FrameGeometryGesture::Move {
            delta_x_um: 10_000,
            delta_y_um: 0,
        },
        FrameGeometryGesture::Resize {
            handle: FrameResizeHandle::BottomRight,
            delta_x_um: -10_000,
            delta_y_um: -10_000,
            preserve_aspect_ratio: false,
            from_center: false,
        },
    ] {
        let edit = FrameGeometryEdit {
            frames: vec![FrameGeometryTarget {
                frame_id: first.id.clone(),
                expected_rect: first.rect.clone(),
            }],
            gesture,
        };
        assert_eq!(
            project.preview_frame_geometry(&edit),
            Err(CoreError::LayoutLocked)
        );
        assert_eq!(
            project.apply(ProjectIntent::EditFrameGeometry { edit }),
            Err(CoreError::LayoutLocked)
        );
        assert_eq!(project.projection(), before);
    }
    assert_eq!(
        project.photo_drop_target(&sheet, 0, 0).unwrap(),
        PhotoDropTarget::Invalid
    );
    project
        .apply(ProjectIntent::DeleteFrames {
            frame_ids: vec![first.id.clone()],
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    assert_eq!(
        project.projection(),
        before,
        "Delete on placeholders is a no-op"
    );
    project
        .apply(ProjectIntent::ArrangeFrames {
            frame_ids: vec![first.id.clone()],
            action: FrameStackAction::BringToFront,
        })
        .unwrap();
    let reordered = project.projection();
    assert_eq!(
        reordered.state.album.sheets[0].frames.last().unwrap().id,
        first.id
    );
    assert_eq!(
        reordered.state.album.sheets[0].frames.last().unwrap().rect,
        first.rect
    );
    assert!(project.project().sheets()[0].layout_locked());
}

#[test]
fn locking_commits_the_preview_and_unlocking_preserves_it_with_undo_redo() {
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
    let before = project.project().clone();
    let revision = project.revision();
    let query = project.query_layouts(&sheet).unwrap();
    let selection = LayoutSelection {
        query_id: query.query_id,
        candidate_index: 0,
    };
    let preview = project.preview_layout(&selection).unwrap();
    let applied = project
        .apply(ProjectIntent::LockLayout { selection })
        .unwrap();
    assert_eq!(applied.composition.sheets[0].frames, preview);
    assert!(project.project().sheets()[0].layout_locked());
    assert_eq!(project.revision(), revision + 1);
    let locked = project.project().clone();
    project.undo().unwrap();
    assert_eq!(project.project(), &before);
    project.redo().unwrap();
    assert_eq!(project.project(), &locked);
    project
        .apply(ProjectIntent::UnlockLayout {
            sheet_id: sheet.clone(),
        })
        .unwrap();
    assert!(!project.project().sheets()[0].layout_locked());
    assert_eq!(
        project.project().sheets()[0].frames(),
        locked.sheets()[0].frames()
    );
    assert_eq!(
        project.project().sheets()[0].last_layout(),
        locked.sheets()[0].last_layout()
    );
    project.undo().unwrap();
    assert_eq!(project.project(), &locked);
    project.redo().unwrap();
    assert!(!project.project().sheets()[0].layout_locked());
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
    assert_eq!(persisted["schemaVersion"], 10);
}

#[test]
fn current_schema_requires_complete_layout_payloads_and_rejects_corrupt_geometry() {
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
    let query = project.query_layouts(&sheet).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
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
    assert!(!converted.state.album.sheets[0].layout_locked);
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
        let query = project.query_layouts(&sheet.id).unwrap();
        project
            .apply(ProjectIntent::LockLayout {
                selection: LayoutSelection {
                    query_id: query.query_id,
                    candidate_index: 0,
                },
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
        assert!(!sheet.layout_locked);
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
