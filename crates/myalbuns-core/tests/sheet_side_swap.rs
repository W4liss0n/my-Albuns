#![cfg(windows)]

use std::{fs, path::Path};

use myalbuns_core::{
    CoreError, CreateAuthorization, CreateProjectRequest, EditableProject, FrameGeometryEdit,
    FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle, ImportPhoto, InitialProject,
    OpenProjectRequest, PhotoPlacementMode, PhotoSourceMetadata, ProjectCore, ProjectIntent,
    ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

fn empty_project(root: &Path) -> EditableProject {
    core(root)
        .create_editable(CreateProjectRequest::new(
            location(&root.join("Troca de lados.myalbuns")),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap()
}

fn add_frame(project: &mut EditableProject, sheet_index: usize, rect: [i64; 4]) -> String {
    let sheet_id = project.projection().state.album.sheets[sheet_index]
        .id
        .clone();
    let added = project
        .apply_with_outcome(ProjectIntent::AddFrame { sheet_id })
        .unwrap();
    let frame_id = added.affected_frame_id.unwrap();
    let [x, y, width, height] = rect;
    for resizing in [true, false] {
        let current = project.projection().state.album.sheets[sheet_index]
            .frames
            .iter()
            .find(|frame| frame.id == frame_id)
            .unwrap()
            .rect
            .clone();
        let gesture = if resizing {
            FrameGeometryGesture::Resize {
                handle: FrameResizeHandle::BottomRight,
                delta_x_um: width - current.width,
                delta_y_um: height - current.height,
                preserve_aspect_ratio: false,
                from_center: false,
            }
        } else {
            FrameGeometryGesture::Move {
                delta_x_um: x - current.x,
                delta_y_um: y - current.y,
            }
        };
        project
            .apply(ProjectIntent::EditFrameGeometry {
                edit: FrameGeometryEdit {
                    snap: None,
                    frames: vec![FrameGeometryTarget {
                        frame_id: frame_id.clone(),
                        expected_rect: current,
                    }],
                    gesture,
                },
            })
            .unwrap();
    }
    frame_id
}

#[test]
fn swaps_whole_page_frames_in_one_edit_without_changing_crossings_or_page_numbers() {
    let root = tempfile::tempdir().unwrap();
    let mut project = empty_project(root.path());
    for rect in [
        [20_000, 40_000, 100_000, 90_000],
        [410_000, 50_000, 100_000, 120_000],
        [280_000, 80_000, 80_000, 100_000],
        [200_000, 170_000, 100_000, 50_000],
        [300_000, 180_000, 50_000, 60_000],
    ] {
        add_frame(&mut project, 0, rect);
    }
    add_frame(&mut project, 1, [30_000, 40_000, 100_000, 90_000]);
    project.save(project.revision()).unwrap();
    let before = project.projection();
    let mut expected = before.state.album.clone();
    for (frame, expected_x) in expected.sheets[0]
        .frames
        .iter_mut()
        .zip([320_000, 110_000, 280_000, 500_000, 0])
    {
        frame.rect.x = expected_x;
    }
    let after = project
        .apply(ProjectIntent::SwapSheetSides {
            sheet_id: before.state.album.sheets[0].id.clone(),
        })
        .unwrap();
    assert_eq!(after.state.album, expected);
    assert_eq!(after.state.revision, before.state.revision + 1);
    assert!(after.state.dirty);
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, expected);
}

fn photo_metadata(index: usize) -> PhotoSourceMetadata {
    let palette = if index == 0 {
        ["#173B4B", "#84AAA7", "#E7C58B"]
    } else {
        ["#66314A", "#D88370", "#EAD8B5"]
    };
    PhotoSourceMetadata::new(600, 400, palette.map(String::from)).unwrap()
}

fn mixed_project(root: &Path) -> EditableProject {
    let mut project = empty_project(root);
    for (index, rect) in [
        [30_000, 50_000, 150_000, 120_000],
        [400_000, 100_000, 100_000, 150_000],
        [260_000, 50_000, 80_000, 100_000],
        [200_000, 210_000, 100_000, 60_000],
    ]
    .into_iter()
    .enumerate()
    {
        let frame_id = add_frame(&mut project, 0, rect);
        if index < 2 {
            let path = root.join(format!("Foto-{index}.jpg"));
            fs::write(&path, format!("original-{index}")).unwrap();
            let media_id = project
                .import_photo(ImportPhoto::new(path, photo_metadata(index)))
                .unwrap()
                .media_id;
            let sheet_id = project.projection().state.album.sheets[0].id.clone();
            project
                .apply(ProjectIntent::DropPhoto {
                    sheet_id,
                    media_id,
                    x_um: rect[0] + rect[2] / 2,
                    y_um: rect[1] + rect[3] / 2,
                    mode: PhotoPlacementMode::Edit,
                })
                .unwrap();
            project
                .apply(ProjectIntent::TransformPhoto {
                    frame_id,
                    delta_pan_x: if index == 0 { -0.5 } else { 0.6 },
                    delta_pan_y: 0.3,
                    delta_zoom: 0.75,
                })
                .unwrap();
        }
    }
    add_frame(&mut project, 1, [40_000, 80_000, 170_000, 120_000]);
    add_frame(&mut project, 1, [370_000, 60_000, 140_000, 160_000]);
    project
}

#[test]
fn preserves_photo_occurrences_and_export_composition_through_save_reopen_and_two_swaps() {
    let root = tempfile::tempdir().unwrap();
    let mut project = mixed_project(root.path());
    let before = project.projection();
    let sheet_id = before.state.album.sheets[0].id.clone();
    let frozen = project.render_snapshot();
    let after = project
        .apply_with_outcome(ProjectIntent::SwapSheetSides {
            sheet_id: sheet_id.clone(),
        })
        .unwrap();
    assert!(after.affected_frame_id.is_none());
    assert!(after.affected_sheet_id.is_none());
    for (original, swapped) in before.state.album.sheets[0]
        .frames
        .iter()
        .zip(&after.projection.state.album.sheets[0].frames)
    {
        assert_eq!(original.photo, swapped.photo);
        assert_eq!(original.id, swapped.id);
        assert_eq!(original.z_index, swapped.z_index);
        assert_eq!(original.rect.y, swapped.rect.y);
        assert_eq!(original.rect.width, swapped.rect.width);
        assert_eq!(original.rect.height, swapped.rect.height);
    }
    assert_eq!(after.projection.media_usage, before.media_usage);
    assert_eq!(frozen.composition, before.composition);
    let after_export = project.render_snapshot();
    assert_eq!(
        after_export.composition.sheets[0].frames[0].clip_rect.x,
        330_000
    );
    assert_eq!(
        after_export.composition.sheets[0].frames[1].clip_rect.x,
        100_000
    );
    project.save(project.revision()).unwrap();
    drop(project);
    let mut reopened = core(root.path())
        .open_editable(OpenProjectRequest::new(location(
            &root.path().join("Troca de lados.myalbuns"),
        )))
        .unwrap();
    for (index, media) in before.state.album.media.iter().enumerate() {
        reopened
            .observe_photo_source(media.id, photo_metadata(index))
            .unwrap();
    }
    assert_eq!(
        reopened.projection().state.album,
        after.projection.state.album
    );
    assert_eq!(
        reopened.render_snapshot().composition,
        after_export.composition
    );
    let twice = reopened
        .apply(ProjectIntent::SwapSheetSides { sheet_id })
        .unwrap();
    assert_eq!(twice.state.album, before.state.album);
    assert_eq!(reopened.render_snapshot().composition, frozen.composition);
    for index in 0..2 {
        assert_eq!(
            fs::read_to_string(root.path().join(format!("Foto-{index}.jpg"))).unwrap(),
            format!("original-{index}")
        );
    }
}

#[test]
fn empty_and_crossing_only_sheets_leave_revision_dirty_state_and_redo_unchanged() {
    for crossing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut project = empty_project(root.path());
        let sheet_id = project.projection().state.album.sheets[0].id.clone();
        if crossing {
            project
                .apply(ProjectIntent::AddFrame {
                    sheet_id: sheet_id.clone(),
                })
                .unwrap();
        }
        project.save(project.revision()).unwrap();
        project.apply(ProjectIntent::SetDpi { dpi: 301 }).unwrap();
        let before = project.undo().unwrap();
        let result = project
            .apply(ProjectIntent::SwapSheetSides { sheet_id })
            .unwrap();
        assert_eq!(result, before);
        assert!(project.redo().is_some());
    }
}

#[test]
fn single_pages_and_unknown_targets_fail_atomically_without_discarding_redo() {
    let root = tempfile::tempdir().unwrap();
    let mut project = empty_project(root.path());
    let sheet_ids: Vec<_> = project
        .projection()
        .state
        .album
        .sheets
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect();
    for sheet_id in &sheet_ids {
        project
            .apply(ProjectIntent::ConvertEdgeSheet {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
    }
    project.save(project.revision()).unwrap();
    project.apply(ProjectIntent::SetDpi { dpi: 301 }).unwrap();
    let before = project.undo().unwrap();
    for sheet_id in sheet_ids {
        assert_eq!(
            project.apply(ProjectIntent::SwapSheetSides { sheet_id }),
            Err(CoreError::InvalidSheetSideSwap)
        );
        assert_eq!(project.projection(), before);
    }
    for sheet_id in ["invalid", "00000000-0000-4000-8000-000000000001"] {
        assert_eq!(
            project.apply(ProjectIntent::SwapSheetSides {
                sheet_id: sheet_id.into()
            }),
            Err(CoreError::SheetNotFound(sheet_id.into()))
        );
        assert_eq!(project.projection(), before);
    }
    assert!(project.redo().is_some());
}

#[test]
fn sheet_side_swap_preview_corpus_matches_the_public_core() {
    let root = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    for (name, target) in [
        ("mixed", 0),
        ("other-sheet", 1),
        ("empty", 0),
        ("only-crossings", 0),
        ("right-page", 0),
        ("left-page", 1),
    ] {
        let case_root = root.path().join(name);
        fs::create_dir(&case_root).unwrap();
        let mut project = if name == "mixed" || name == "other-sheet" {
            mixed_project(&case_root)
        } else {
            empty_project(&case_root)
        };
        let target_sheet_id = project.projection().state.album.sheets[target].id.clone();
        if name == "only-crossings" {
            project
                .apply(ProjectIntent::AddFrame {
                    sheet_id: target_sheet_id.clone(),
                })
                .unwrap();
        }
        if name.ends_with("-page") {
            project
                .apply(ProjectIntent::ConvertEdgeSheet {
                    sheet_id: target_sheet_id.clone(),
                })
                .unwrap();
        }
        project.save(project.revision()).unwrap();
        let media_ids: Vec<_> = project
            .projection()
            .state
            .album
            .media
            .iter()
            .map(|media| media.id)
            .collect();
        drop(project);
        let mut project = core(&case_root)
            .open_editable(OpenProjectRequest::new(location(
                &case_root.join("Troca de lados.myalbuns"),
            )))
            .unwrap();
        for (index, media_id) in media_ids.iter().enumerate() {
            project
                .observe_photo_source(*media_id, photo_metadata(index))
                .unwrap();
        }
        let before = project.projection();
        let (after, outcome) = match project.apply(ProjectIntent::SwapSheetSides {
            sheet_id: target_sheet_id.clone(),
        }) {
            Ok(after) => {
                let outcome = if after.state.revision == before.state.revision {
                    "unchanged"
                } else {
                    "changed"
                };
                (after, outcome)
            }
            Err(CoreError::InvalidSheetSideSwap) if name.ends_with("-page") => {
                (project.projection(), "unavailable")
            }
            result => panic!("unexpected side swap result: {result:?}"),
        };
        let mut text = serde_json::to_string(&serde_json::json!({
            "name": name, "targetSheetId": target_sheet_id, "before": before,
            "after": after, "outcome": outcome,
        }))
        .unwrap()
        .replace(&before.state.project_id, "sheet-side-swap-project");
        for (sheet_index, sheet) in before.state.album.sheets.iter().enumerate() {
            text = text.replace(&sheet.id, &format!("sheet-{:03}", sheet_index + 1));
            for (frame_index, frame) in sheet.frames.iter().enumerate() {
                text = text.replace(
                    &frame.id,
                    &format!("side-frame-{sheet_index}-{frame_index}"),
                );
            }
        }
        for (index, media_id) in media_ids.iter().enumerate() {
            text = text.replace(
                &media_id.to_string(),
                &format!("00000000-0000-4000-8000-{:012}", index + 1),
            );
        }
        cases.push(serde_json::from_str::<serde_json::Value>(&text).unwrap());
    }
    let before = cases[0]["before"].clone();
    assert_eq!(cases[1]["before"], before);
    for case in &mut cases[..2] {
        case.as_object_mut().unwrap().remove("before");
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({"before": before, "cases": cases}))
            .unwrap()
    );
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sheet-side-swap-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_SHEET_SIDE_SWAP_FIXTURE").is_some() {
        fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        fs::read_to_string(fixture).unwrap().replace("\r\n", "\n")
    );
}
