#![cfg(windows)]

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, DisplayUnit, EditableProject,
    EditorProjection, EndSheetFormat, FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget,
    ImportPhoto, InitialProject, InitialProjectConfiguration, OpenProjectRequest,
    PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
    SaveAsAuthorization, SaveAsProjectRequest,
};
use myalbuns_paths::OperationPathContext;
use std::{fs, path::Path};

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

fn metadata() -> PhotoSourceMetadata {
    PhotoSourceMetadata::new(
        600,
        400,
        ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
    )
    .unwrap()
}

fn empty_project(root: &Path, name: &str) -> EditableProject {
    core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join(name)),
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
        .unwrap()
}

fn project_with_frames(root: &Path) -> EditableProject {
    let mut project = empty_project(root, "Clipboard.myalbuns");
    let photo = root.join("Foto.jpg");
    fs::write(&photo, b"original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(photo, metadata()))
        .unwrap();
    let sheet_ids: Vec<_> = imported
        .projection
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect();
    for sheet_id in sheet_ids {
        for (index, dx) in [-45_000, 45_000].into_iter().enumerate() {
            let added = project
                .apply_with_outcome(ProjectIntent::AddFrame {
                    sheet_id: sheet_id.clone(),
                })
                .unwrap();
            let id = added.affected_frame_id.unwrap();
            let frame = added
                .projection
                .state
                .album
                .sheets
                .iter()
                .flat_map(|sheet| &sheet.frames)
                .find(|frame| frame.id == id)
                .unwrap();
            if index == 0 {
                project
                    .apply(ProjectIntent::DropPhoto {
                        sheet_id: sheet_id.clone(),
                        media_id: imported.media_id,
                        x_um: frame.rect.x + frame.rect.width / 2,
                        y_um: frame.rect.y + frame.rect.height / 2,
                        mode: PhotoPlacementMode::Edit,
                    })
                    .unwrap();
                project
                    .apply(ProjectIntent::TransformPhoto {
                        frame_id: id.clone(),
                        delta_pan_x: 0.2,
                        delta_pan_y: -0.1,
                        delta_zoom: 0.5,
                    })
                    .unwrap();
            }
            project
                .apply(ProjectIntent::EditFrameGeometry {
                    edit: FrameGeometryEdit {
                        frames: vec![FrameGeometryTarget {
                            frame_id: id,
                            expected_rect: frame.rect.clone(),
                        }],
                        gesture: FrameGeometryGesture::Move {
                            delta_x_um: dx,
                            delta_y_um: 0,
                        },
                    },
                })
                .unwrap();
        }
    }
    project
}

fn copy_sheet(project: &mut EditableProject, index: usize) -> EditorProjection {
    let ids = project.projection().state.album.sheets[index]
        .frames
        .iter()
        .rev()
        .map(|frame| frame.id.clone())
        .collect();
    project
        .apply(ProjectIntent::CopyFrames { frame_ids: ids })
        .unwrap()
}

fn paste(
    project: &mut EditableProject,
    index: usize,
    offset: u64,
) -> myalbuns_core::ProjectMutationOutcome {
    let sheet_id = project.projection().state.album.sheets[index].id.clone();
    project
        .apply_with_outcome(ProjectIntent::PasteFrames {
            sheet_id,
            desired_offset_um: offset,
        })
        .unwrap()
}

#[test]
fn copy_is_session_only_preserves_redo_and_paste_is_one_persisted_history_action() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    project.undo().unwrap();
    project.save(project.revision()).unwrap();
    let before = project.projection();
    let frozen = project.render_snapshot();
    let copied = copy_sheet(&mut project, 1);
    assert_eq!(copied.state, before.state);
    assert_eq!(copied.composition, before.composition);
    assert!(copied.can_paste_frames);
    assert!(!copied.state.dirty);
    assert!(copied.state.can_redo);
    let outcome = paste(&mut project, 1, 8_000);
    let after = outcome.projection;
    let originals = &before.state.album.sheets[1].frames;
    let result = &after.state.album.sheets[1].frames;
    assert_eq!(&result[..2], originals);
    assert_eq!(
        outcome.affected_frame_ids.unwrap(),
        result[2..]
            .iter()
            .map(|frame| frame.id.clone())
            .collect::<Vec<_>>()
    );
    for (index, frame) in result[2..].iter().enumerate() {
        assert_ne!(frame.id, originals[index].id);
        assert_eq!(frame.photo, originals[index].photo);
        assert_eq!(frame.z_index, (index + 2) as u32);
        assert_eq!(frame.rect.x, originals[index].rect.x + 8_000);
        assert_eq!(frame.rect.y, originals[index].rect.y + 8_000);
        assert_eq!(frame.rect.width, originals[index].rect.width);
        assert_eq!(frame.rect.height, originals[index].rect.height);
    }
    assert_eq!(after.state.album.media, before.state.album.media);
    assert_eq!(
        after.state.album.visual_defaults,
        before.state.album.visual_defaults
    );
    assert_eq!(frozen.composition, before.composition);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, after.state.album);
    project.save(project.revision()).unwrap();
    drop(project);
    let mut reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(
            &root.path().join("Clipboard.myalbuns"),
        )))
        .unwrap();
    reopened
        .observe_photo_source(after.state.album.media[0].id, metadata())
        .unwrap();
    assert_eq!(reopened.projection().state.album, after.state.album);
    assert_eq!(reopened.projection().composition, after.composition);
    assert!(!reopened.projection().can_paste_frames);
    assert_eq!(fs::read(root.path().join("Foto.jpg")).unwrap(), b"original");
}

#[test]
fn paste_preserves_coordinates_or_maps_the_complete_group_to_the_correct_logical_page() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    for (source, destination, divisor, origin) in [
        (1, 2, 1, 0),
        (1, 0, 2, 0),
        (1, 3, 2, 0),
        (0, 1, 1, 300_000),
        (3, 1, 1, 0),
        (0, 3, 1, 0),
    ] {
        let copied = copy_sheet(&mut project, source);
        let pasted = paste(&mut project, destination, 15_000).projection;
        for (source_frame, target) in copied.state.album.sheets[source]
            .frames
            .iter()
            .zip(&pasted.state.album.sheets[destination].frames[2..])
        {
            assert_eq!(target.rect.x, origin + source_frame.rect.x / divisor);
            assert_eq!(target.rect.width, source_frame.rect.width / divisor);
            assert_eq!(target.rect.y, source_frame.rect.y);
            assert_eq!(target.rect.height, source_frame.rect.height);
            assert_eq!(target.photo, source_frame.photo);
        }
        project.undo().unwrap();
    }
}

#[test]
fn copy_survives_source_deletion_and_clamps_the_same_sheet_group_offset() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let copied = copy_sheet(&mut project, 1);
    let source = &copied.state.album.sheets[1].frames;
    let after = paste(&mut project, 1, u64::MAX).projection;
    let right = source
        .iter()
        .map(|frame| frame.rect.x + frame.rect.width)
        .max()
        .unwrap();
    let bottom = source
        .iter()
        .map(|frame| frame.rect.y + frame.rect.height)
        .max()
        .unwrap();
    let offset = (600_000 - right).min(300_000 - bottom);
    assert!(offset > 0);
    assert_eq!(
        after.state.album.sheets[1].frames[2].rect.x,
        source[0].rect.x + offset
    );
    copy_sheet(&mut project, 1);
    let overlapped = paste(&mut project, 1, 9_000).projection;
    for (original, duplicate) in after.state.album.sheets[1]
        .frames
        .iter()
        .zip(&overlapped.state.album.sheets[1].frames[4..])
    {
        assert_eq!(original.rect, duplicate.rect);
    }
    project
        .apply(ProjectIntent::DeleteSheet {
            sheet_id: copied.state.album.sheets[1].id.clone(),
        })
        .unwrap();
    let restored = paste(&mut project, 1, 1_000).projection;
    assert_eq!(restored.state.album.sheets[1].frames.len(), 6);
    assert_eq!(
        restored.state.album.sheets[1].frames[2].photo,
        source[0].photo
    );
}

#[test]
fn clipboard_restores_a_binding_removed_by_undo_and_reuses_a_reimported_path() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    let copied = copy_sheet(&mut project, 1);
    while project.undo().is_some() {}
    assert!(project.projection().state.album.media.is_empty());
    let restored = paste(&mut project, 1, 0).projection;
    assert_eq!(restored.state.album.media, copied.state.album.media);
    assert_eq!(
        restored.state.album.sheets[1].frames[0].photo,
        copied.state.album.sheets[1].frames[0].photo
    );
    project.undo().unwrap();
    let reimported = project
        .import_photo(ImportPhoto::new(root.path().join("Foto.jpg"), metadata()))
        .unwrap();
    let pasted = paste(&mut project, 1, 0).projection;
    assert_eq!(pasted.state.album.media.len(), 1);
    assert_eq!(
        pasted.state.album.sheets[1].frames[0]
            .photo
            .as_ref()
            .unwrap()
            .media_id,
        reimported.media_id
    );
    assert_eq!(fs::read(root.path().join("Foto.jpg")).unwrap(), b"original");
}

#[test]
fn invalid_commands_are_atomic_and_clipboards_are_isolated_between_projects() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path());
    project.undo().unwrap();
    let copied = copy_sheet(&mut project, 1);
    let id = copied.state.album.sheets[1].frames[0].id.clone();
    for frame_ids in [
        vec![],
        vec![id.clone(), id.to_uppercase()],
        vec![id.clone(), "invalid".into()],
        vec![
            id.clone(),
            copied.state.album.sheets[0].frames[0].id.clone(),
        ],
    ] {
        assert_eq!(
            project.apply(ProjectIntent::CopyFrames { frame_ids }),
            Err(CoreError::InvalidFrameCopySelection)
        );
        assert_eq!(project.projection(), copied);
    }
    assert!(
        project
            .apply(ProjectIntent::PasteFrames {
                sheet_id: "invalid".into(),
                desired_offset_um: 0
            })
            .is_err()
    );
    assert_eq!(project.projection(), copied);
    let mut other = empty_project(root.path(), "Outro.myalbuns");
    let before = other.projection();
    assert_eq!(
        other.apply(ProjectIntent::PasteFrames {
            sheet_id: before.state.album.sheets[1].id.clone(),
            desired_offset_um: 0
        }),
        Err(CoreError::FrameClipboardEmpty)
    );
    assert_eq!(other.projection(), before);
    assert!(project.redo().is_some());
    project
        .save_as(SaveAsProjectRequest::new(
            project.revision(),
            location(&root.path().join("Nova identidade.myalbuns")),
            SaveAsAuthorization::CreateOnly,
        ))
        .unwrap();
    assert!(!project.projection().can_paste_frames);
    assert!(!project.undo().unwrap().can_paste_frames);
}

#[test]
fn clipboard_preview_corpus_matches_the_public_core() {
    let root = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    for (name, source, target, single) in [
        ("same-group", 1, 1, false),
        ("same-single", 1, 1, true),
        ("other-double", 1, 2, false),
        ("double-right", 1, 0, false),
        ("double-left", 1, 3, false),
        ("right-double", 0, 1, false),
        ("left-double", 3, 1, false),
        ("same-no-offset", 1, 1, false),
    ] {
        let case_root = root.path().join(name);
        fs::create_dir(&case_root).unwrap();
        let mut project = project_with_frames(&case_root);
        if name == "same-no-offset" {
            let frame = project.projection().state.album.sheets[source].frames[1].clone();
            project
                .apply(ProjectIntent::EditFrameGeometry {
                    edit: FrameGeometryEdit {
                        frames: vec![FrameGeometryTarget {
                            frame_id: frame.id,
                            expected_rect: frame.rect.clone(),
                        }],
                        gesture: FrameGeometryGesture::Move {
                            delta_x_um: 600_000 - frame.rect.x - frame.rect.width,
                            delta_y_um: 0,
                        },
                    },
                })
                .unwrap();
        }
        project.save(project.revision()).unwrap();
        let before = project.projection();
        let selected: Vec<_> = before.state.album.sheets[source]
            .frames
            .iter()
            .take(if single { 1 } else { 2 })
            .map(|frame| frame.id.clone())
            .collect();
        let copied = project
            .apply(ProjectIntent::CopyFrames {
                frame_ids: selected.clone(),
            })
            .unwrap();
        let outcome = paste(&mut project, target, 8_000);
        let new_ids = outcome.affected_frame_ids.unwrap();
        let mut text = serde_json::to_string(&serde_json::json!({ "name": name, "before": before, "copied": copied,
            "after": outcome.projection, "sourceSheetId": before.state.album.sheets[source].id,
            "targetSheetId": before.state.album.sheets[target].id, "selectedFrameIds": selected, "pastedFrameIds": new_ids, "desiredOffsetUm": 8_000 })).unwrap();
        text = text.replace(&before.state.project_id, "frame-clipboard-project");
        for (sheet_index, sheet) in before.state.album.sheets.iter().enumerate() {
            text = text.replace(&sheet.id, &format!("sheet-{:03}", sheet_index + 1));
            for (frame_index, frame) in sheet.frames.iter().enumerate() {
                text = text.replace(
                    &frame.id,
                    &format!("clipboard-frame-{sheet_index}-{frame_index}"),
                );
            }
        }
        for (index, id) in new_ids.iter().enumerate() {
            text = text.replace(id, &format!("pasted-frame-{index}"));
        }
        text = text.replace(
            &before.state.album.media[0].id.to_string(),
            "00000000-0000-4000-8000-000000000001",
        );
        cases.push(serde_json::from_str::<serde_json::Value>(&text).unwrap());
    }
    let before = cases[0]["before"].clone();
    let copied = cases[0]["copied"].clone();
    for case in &mut cases {
        if case["name"] == "same-no-offset" {
            continue;
        }
        assert_eq!(case["before"], before);
        assert_eq!(case["copied"], copied);
        case.as_object_mut().unwrap().remove("before");
        case.as_object_mut().unwrap().remove("copied");
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(
            &serde_json::json!({"before": before, "copied": copied, "cases": cases})
        )
        .unwrap()
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/frame-clipboard-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_CLIPBOARD_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
