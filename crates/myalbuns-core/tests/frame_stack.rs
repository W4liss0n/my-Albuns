#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, EditableProject, FrameStackAction,
    ImportPhoto, InitialProject, OpenProjectRequest, PhotoDropTarget, PhotoPlacementMode,
    PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn project_with_frames(root: &Path, count: usize) -> EditableProject {
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"));
    let mut project = core
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Pilha.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let photo = root.join("Foto.jpg");
    fs::write(&photo, b"original").unwrap();
    let imported = project
        .import_photo(ImportPhoto::new(photo, photo_metadata()))
        .unwrap();
    let sheet_id = imported.projection.state.album.sheets[0].id.clone();
    for _ in 0..count {
        project
            .apply(ProjectIntent::AddPhoto {
                sheet_id: sheet_id.clone(),
                media_id: imported.media_id,
                mode: PhotoPlacementMode::Edit,
            })
            .unwrap();
    }
    project
}

fn photo_metadata() -> PhotoSourceMetadata {
    PhotoSourceMetadata::new(
        600,
        400,
        ["#173B4B".into(), "#84AAA7".into(), "#E7C58B".into()],
    )
    .unwrap()
}

#[test]
fn arranging_frames_moves_selected_blocks_and_preserves_every_placement() {
    use FrameStackAction::*;
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), 6);
    let before = project.projection();
    let original = &before.state.album.sheets[0].frames;
    let ids = original
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    for (action, expected) in [
        (BringToFront, [0, 3, 5, 1, 2, 4]),
        (Advance, [0, 3, 1, 2, 5, 4]),
        (Recede, [1, 2, 0, 4, 3, 5]),
        (SendToBack, [1, 2, 4, 0, 3, 5]),
    ] {
        let changed = project
            .apply(ProjectIntent::ArrangeFrames {
                frame_ids: vec![ids[4].clone(), ids[1].clone(), ids[2].clone()],
                action,
            })
            .unwrap();
        let arranged = &changed.state.album.sheets[0].frames;
        assert_eq!(
            arranged.iter().map(|frame| &frame.id).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|index| &ids[*index])
                .collect::<Vec<_>>(),
            "{action:?}"
        );
        for (index, frame) in arranged.iter().enumerate() {
            let source = &original[expected[index]];
            assert_eq!(frame.rect, source.rect);
            assert_eq!(frame.photo, source.photo);
            assert_eq!(frame.z_index, index as u32);
            let composed = &changed.composition.sheets[0].frames[index];
            assert_eq!(composed.frame_id, frame.id);
            assert_eq!(
                composed.photo,
                before.composition.sheets[0].frames[expected[index]].photo
            );
        }
        assert_eq!(project.undo().unwrap().state.album, before.state.album);
        assert_eq!(project.redo().unwrap().state.album, changed.state.album);
        project.undo().unwrap();
    }
}

#[test]
fn a_block_at_the_stack_limit_does_not_stop_other_blocks_and_noop_keeps_history() {
    use FrameStackAction::*;
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), 6);
    let before = project.projection();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    for (action, expected) in [(Advance, [1, 0, 2, 3, 4, 5]), (Recede, [0, 1, 2, 3, 5, 4])] {
        let changed = project
            .apply(ProjectIntent::ArrangeFrames {
                frame_ids: vec![ids[0].clone(), ids[5].clone()],
                action,
            })
            .unwrap();
        assert_eq!(
            changed.state.album.sheets[0]
                .frames
                .iter()
                .map(|frame| &frame.id)
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|index| &ids[*index])
                .collect::<Vec<_>>()
        );
        project.undo().unwrap();
    }
    let before_noop = project.projection();
    assert!(before_noop.state.can_redo);
    for (action, frame_ids) in [
        (Advance, vec![ids[5].clone()]),
        (BringToFront, vec![ids[5].clone()]),
        (Recede, vec![ids[0].clone()]),
        (SendToBack, vec![ids[0].clone()]),
        (Advance, ids.clone()),
        (Recede, ids.clone()),
        (BringToFront, ids.clone()),
        (SendToBack, ids.clone()),
    ] {
        let unchanged = project
            .apply(ProjectIntent::ArrangeFrames { frame_ids, action })
            .unwrap();
        assert_eq!(
            unchanged, before_noop,
            "{action:?} must not create history at the limit"
        );
    }
    assert!(project.redo().is_some());
}

#[test]
fn invalid_selections_never_partially_reorder_frames() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), 3);
    let initial = project.projection();
    project
        .apply(ProjectIntent::AddPhoto {
            sheet_id: initial.state.album.sheets[1].id.clone(),
            media_id: initial.state.album.media[0].id,
            mode: PhotoPlacementMode::Edit,
        })
        .unwrap();
    let before = project.projection();
    let id = before.state.album.sheets[0].frames[0].id.clone();
    let other_sheet = before.state.album.sheets[1].frames[0].id.clone();
    for frame_ids in [
        vec![],
        vec![id.clone(), id.to_uppercase()],
        vec![id.clone(), other_sheet],
        vec![id.clone(), "00000000-0000-0000-0000-000000000000".into()],
        vec![id, "invalid".into()],
    ] {
        assert_eq!(
            project.apply(ProjectIntent::ArrangeFrames {
                frame_ids,
                action: FrameStackAction::BringToFront,
            }),
            Err(CoreError::InvalidFrameStackSelection)
        );
        assert_eq!(project.projection(), before);
    }
}

#[test]
fn stack_order_preserves_placeholders_and_controls_hit_testing_saved_and_frozen_composition() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), 3);
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.path().join("Pilha.myalbuns");
    let mut payload: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    payload["project"]["sheets"][0]["frames"][1]["photo"] = serde_json::Value::Null;
    fs::write(&path, serde_json::to_vec(&payload).unwrap()).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let media_id = project.projection().state.album.media[0].id;
    project
        .observe_photo_source(media_id, photo_metadata())
        .unwrap();
    let before = project.projection();
    let sheet_id = before.state.album.sheets[0].id.clone();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    let frozen = project.render_snapshot();
    assert_eq!(
        project
            .photo_drop_target(&sheet_id, 250_000, 120_000)
            .unwrap(),
        PhotoDropTarget::Frame {
            frame_id: ids[2].clone()
        }
    );
    let arranged = project
        .apply(ProjectIntent::ArrangeFrames {
            frame_ids: vec![ids[0].clone(), ids[1].clone()],
            action: FrameStackAction::BringToFront,
        })
        .unwrap();
    assert!(arranged.state.album.sheets[0].frames[2].photo.is_none());
    assert_eq!(
        project
            .photo_drop_target(&sheet_id, 250_000, 120_000)
            .unwrap(),
        PhotoDropTarget::Frame {
            frame_id: ids[1].clone()
        }
    );
    project.save(project.revision()).unwrap();
    let saved = project.render_snapshot();
    assert_eq!(frozen.composition, before.composition);
    drop(project);
    let mut reopened = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    reopened
        .observe_photo_source(media_id, photo_metadata())
        .unwrap();
    assert_eq!(
        reopened.projection().state.album.sheets,
        arranged.state.album.sheets
    );
    assert_eq!(reopened.render_snapshot().composition, saved.composition);
}

#[test]
fn headless_stack_corpus_matches_core_commands() {
    use FrameStackAction::*;
    let root = tempfile::tempdir().unwrap();
    let mut project = project_with_frames(root.path(), 3);
    project.save(project.revision()).unwrap();
    drop(project);
    let path = root.path().join("Pilha.myalbuns");
    let mut payload: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (index, [x, y, width, height]) in [
        [130_000, 40_000, 340_000, 220_000],
        [210_000, 80_000, 240_000, 180_000],
        [260_000, 100_000, 150_000, 100_000],
    ]
    .into_iter()
    .enumerate()
    {
        payload["project"]["sheets"][0]["frames"][index]["rect"] =
            serde_json::json!({ "x": x, "y": y, "width": width, "height": height });
    }
    payload["project"]["sheets"][0]["frames"][1]["photo"] = serde_json::Value::Null;
    fs::write(&path, serde_json::to_vec(&payload).unwrap()).unwrap();
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let mut project = core
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let media_id = project.projection().state.album.media[0].id;
    project
        .observe_photo_source(media_id, photo_metadata())
        .unwrap();
    let before = project.projection();
    let ids = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    let normalize = |projection: &myalbuns_core::EditorProjection| {
        let mut frames = projection.composition.sheets[0].frames.clone();
        for frame in &mut frames {
            frame.frame_id = format!(
                "stack-frame-{}",
                ids.iter().position(|id| id == &frame.frame_id).unwrap()
            );
            if let Some(photo) = &mut frame.photo {
                photo.media_id = "00000000-0000-4000-8000-000000000001".parse().unwrap();
            }
        }
        frames
    };
    let mut cases = Vec::new();
    for (action, selected) in [
        (BringToFront, vec![0, 1]),
        (Advance, vec![1]),
        (Recede, vec![1]),
        (SendToBack, vec![1, 2]),
    ] {
        let changed = project
            .apply(ProjectIntent::ArrangeFrames {
                frame_ids: selected.iter().map(|index| ids[*index].clone()).collect(),
                action,
            })
            .unwrap();
        cases.push(serde_json::json!({ "action": action,
            "selectedFrameIds": selected.iter().map(|index| format!("stack-frame-{index}")).collect::<Vec<_>>(),
            "frames": normalize(&changed) }));
        project.undo().unwrap();
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(
            &serde_json::json!({ "before": normalize(&before), "cases": cases })
        )
        .unwrap()
    );
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/frame-stack-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_STACK_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
