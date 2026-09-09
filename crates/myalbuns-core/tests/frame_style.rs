#![cfg(windows)]

use std::path::Path;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, EditableProject, FrameStyleChange, FrameStyleEdit,
    FrameStyleSource, InitialProject, ProjectCore, ProjectIntent, ProjectLocation,
    ProjectedFrameBorder,
};
use myalbuns_paths::OperationPathContext;

fn project(root: &Path) -> EditableProject {
    let path = root.join("Estilo.myalbuns");
    let mut context = OperationPathContext::new();
    context.capture(&path).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.join("leases"), root.join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(path, context.freeze()),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let sheet_id = project.projection().state.album.sheets[0].id.clone();
    for _ in 0..2 {
        project
            .apply(ProjectIntent::AddFrame {
                sheet_id: sheet_id.clone(),
            })
            .unwrap();
    }
    project
}

#[test]
fn copying_preserves_frame_style_while_swapping_content_leaves_it_with_each_frame() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Conteudo.myalbuns");
    std::fs::write(
        &path,
        include_bytes!("fixtures/project_document_v6_photo_migration_expected.myalbuns"),
    )
    .unwrap();
    let mut context = OperationPathContext::new();
    context.capture(&path).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(myalbuns_core::OpenProjectRequest::new(
            ProjectLocation::new(path, context.freeze()),
        ))
        .unwrap();
    let initial = project.projection();
    let ids: Vec<_> = initial.state.album.sheets[0]
        .frames
        .iter()
        .map(|f| f.id.clone())
        .collect();
    for change in [
        FrameStyleChange::Opacity {
            opacity_percent: 50,
        },
        FrameStyleChange::BorderColor {
            rgb: "#205070".into(),
        },
        FrameStyleChange::BorderWidth { width_um: 5_000 },
    ] {
        project
            .apply(ProjectIntent::SetFrameStyle {
                edit: FrameStyleEdit {
                    frame_ids: vec![ids[0].clone()],
                    change,
                },
            })
            .unwrap();
    }
    let styled = project.projection();
    let expected_styles: Vec<_> = styled.state.album.sheets[0]
        .frames
        .iter()
        .map(|f| f.style.clone())
        .collect();
    project
        .apply(ProjectIntent::SwapFrameContents {
            frame_ids: ids.clone(),
        })
        .unwrap();
    let swapped = project.projection();
    assert!(swapped.state.album.sheets[0].frames[0].photo.is_none());
    assert!(swapped.state.album.sheets[0].frames[1].photo.is_some());
    assert_eq!(
        swapped.state.album.sheets[0]
            .frames
            .iter()
            .map(|f| f.style.clone())
            .collect::<Vec<_>>(),
        expected_styles
    );
    project
        .apply(ProjectIntent::CopyFrames { frame_ids: ids })
        .unwrap();
    project
        .apply(ProjectIntent::PasteFrames {
            sheet_id: initial.state.album.sheets[1].id.clone(),
            desired_offset_um: 0,
        })
        .unwrap();
    let pasted = project.projection();
    assert_eq!(
        pasted.state.album.sheets[1]
            .frames
            .iter()
            .map(|f| f.style.clone())
            .collect::<Vec<_>>(),
        expected_styles
    );
    assert_eq!(
        pasted.state.album.sheets[1]
            .frames
            .iter()
            .map(|f| f.photo.clone())
            .collect::<Vec<_>>(),
        swapped.state.album.sheets[0]
            .frames
            .iter()
            .map(|f| f.photo.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn style_commands_validate_atomically_and_only_semantic_changes_consume_history() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let initial = project.projection();
    let id = initial.state.album.sheets[0].frames[0].id.clone();
    let other = project
        .apply_with_outcome(ProjectIntent::AddFrame {
            sheet_id: initial.state.album.sheets[1].id.clone(),
        })
        .unwrap()
        .affected_frame_id
        .unwrap();
    let before = project.projection();
    for frame_ids in [
        vec![],
        vec![id.clone(), id.clone()],
        vec![id.clone(), other],
        vec![id.clone(), "missing".into()],
    ] {
        let edit = FrameStyleEdit {
            frame_ids,
            change: FrameStyleChange::Opacity {
                opacity_percent: 50,
            },
        };
        assert!(project.preview_frame_style(&edit).is_err());
        assert!(
            project
                .apply(ProjectIntent::SetFrameStyle { edit })
                .is_err()
        );
        assert_eq!(project.projection(), before);
    }
    for change in [
        FrameStyleChange::Opacity {
            opacity_percent: 101,
        },
        FrameStyleChange::BorderColor { rgb: "bad".into() },
        FrameStyleChange::BorderWidth {
            width_um: 9_007_199_254_740_992,
        },
    ] {
        let edit = FrameStyleEdit {
            frame_ids: vec![id.clone()],
            change,
        };
        assert!(project.preview_frame_style(&edit).is_err());
        assert!(
            project
                .apply(ProjectIntent::SetFrameStyle { edit })
                .is_err()
        );
        assert_eq!(project.projection(), before);
    }
    let edit = FrameStyleEdit {
        frame_ids: vec![id],
        change: FrameStyleChange::Opacity {
            opacity_percent: 100,
        },
    };
    project
        .apply(ProjectIntent::SetFrameStyle { edit: edit.clone() })
        .unwrap();
    let customized = project.projection();
    assert_eq!(customized.state.revision, before.state.revision + 1);
    assert_eq!(
        customized.state.album.sheets[0].frames[0].style.source,
        FrameStyleSource::Custom
    );
    project
        .apply(ProjectIntent::SetFrameStyle { edit })
        .unwrap();
    assert_eq!(project.projection(), customized);
}

#[test]
fn v7_requires_a_closed_valid_style_and_legacy_versions_reject_it_without_writing() {
    use myalbuns_core::{DocumentFailure, LoadProjectError, LoadProjectRequest};
    let valid: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/project_document_v7_photo_migration_expected.myalbuns"
    ))
    .unwrap();
    let mut cases = Vec::new();
    for style in [
        serde_json::json!({"kind":"unknown"}),
        serde_json::json!({"kind":"album","opacityPercent":100}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":0}}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":0},"opacityPercent":101}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":0},"opacityPercent":0.5}),
        serde_json::json!({"kind":"custom","border":{"rgb":"bad","widthUm":0},"opacityPercent":50}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":-1},"opacityPercent":50}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":9007199254740992u64},"opacityPercent":50}),
        serde_json::json!({"kind":"custom","border":{"rgb":"#FFFFFF","widthUm":0,"extra":true},"opacityPercent":50}),
        serde_json::Value::Null,
    ] {
        let mut invalid = valid.clone();
        invalid["project"]["sheets"][0]["frames"][0]["style"] = style;
        cases.push(invalid);
    }
    let mut missing = valid.clone();
    missing["project"]["sheets"][0]["frames"][0]
        .as_object_mut()
        .unwrap()
        .remove("style");
    cases.push(missing);
    let mut legacy = valid;
    legacy["schemaVersion"] = serde_json::json!(6);
    cases.push(legacy);
    let root = tempfile::tempdir().unwrap();
    for (index, value) in cases.into_iter().enumerate() {
        let path = root.path().join(format!("Invalid-{index}.myalbuns"));
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let mut context = OperationPathContext::new();
        context.capture(&path).unwrap();
        let result = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .load_persisted_revision(LoadProjectRequest::new(ProjectLocation::new(
                path.clone(),
                context.freeze(),
            )));
        assert!(
            matches!(
                result,
                Err(LoadProjectError::Document(
                    DocumentFailure::InvalidProjectDocument
                ))
            ),
            "case {index}: {result:?}"
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn opacity_personalizes_the_whole_style_and_restoration_resumes_album_inheritance() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let before = project.projection();
    let ids: Vec<_> = before.state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    let edit = FrameStyleEdit {
        frame_ids: vec![ids[0].clone()],
        change: FrameStyleChange::Opacity {
            opacity_percent: 50,
        },
    };
    let preview = project.preview_frame_style(&edit).unwrap();
    assert_eq!(project.projection(), before);
    project
        .apply(ProjectIntent::SetFrameStyle { edit })
        .unwrap();
    let custom = project.projection();
    assert_eq!(custom.state.revision, before.state.revision + 1);
    assert_eq!(custom.composition.sheets[0].frames[0].opacity_byte, 128);
    assert_eq!(custom.composition.sheets[0].frames[0], preview[0]);
    assert_eq!(
        custom.state.album.sheets[0].frames[0].style.source,
        FrameStyleSource::Custom
    );
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    assert_eq!(project.redo().unwrap().state.album, custom.state.album);

    let mut defaults = custom.state.album.visual_defaults.clone();
    defaults.frame_border = ProjectedFrameBorder::Solid {
        rgb: "#C02030".into(),
        width_um: 2_000,
    };
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults.clone(),
        })
        .unwrap();
    let changed = project.projection();
    assert_eq!(
        changed.composition.sheets[0].frames[0].border,
        ProjectedFrameBorder::None
    );
    assert_eq!(
        changed.composition.sheets[0].frames[1].border,
        defaults.frame_border
    );
    project
        .apply(ProjectIntent::SetFrameStyle {
            edit: FrameStyleEdit {
                frame_ids: ids,
                change: FrameStyleChange::RestoreAlbum,
            },
        })
        .unwrap();
    let restored = project.projection();
    for frame in &restored.state.album.sheets[0].frames {
        assert_eq!(frame.style.source, FrameStyleSource::Album);
        assert_eq!(frame.style.border_width_um, 2_000);
        assert_eq!(frame.style.border_rgb, "#C02030");
        assert_eq!(frame.style.opacity_percent, 100);
    }
    assert_eq!(project.undo().unwrap().state.album, changed.state.album);
}

#[test]
fn saving_and_reopening_preserves_custom_frame_style_in_the_current_schema() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::SetFrameStyle {
            edit: FrameStyleEdit {
                frame_ids: vec![id],
                change: FrameStyleChange::Opacity { opacity_percent: 0 },
            },
        })
        .unwrap();
    let expected = project.projection().state.album;
    project.save(project.revision()).unwrap();
    let path = project.project_path().to_path_buf();
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(document["schemaVersion"], 8);
    assert_eq!(
        document["project"]["sheets"][0]["frames"][0]["style"],
        serde_json::json!({
            "kind": "custom", "border": { "rgb": "#000000", "widthUm": 0 }, "opacityPercent": 0
        })
    );
    drop(project);
    let mut context = OperationPathContext::new();
    context.capture(&path).unwrap();
    let reopened = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(myalbuns_core::OpenProjectRequest::new(
            ProjectLocation::new(path, context.freeze()),
        ))
        .unwrap();
    assert_eq!(reopened.projection().state.album, expected);
}

#[test]
fn border_controls_preserve_other_properties_and_remember_color_without_a_border() {
    let root = tempfile::tempdir().unwrap();
    let mut project = project(root.path());
    let ids: Vec<_> = project.projection().state.album.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect();
    let apply = |project: &mut EditableProject, ids: Vec<String>, change| {
        project
            .apply(ProjectIntent::SetFrameStyle {
                edit: FrameStyleEdit {
                    frame_ids: ids,
                    change,
                },
            })
            .unwrap();
    };
    apply(
        &mut project,
        ids.clone(),
        FrameStyleChange::BorderColor {
            rgb: "#105030".into(),
        },
    );
    assert!(
        project.projection().composition.sheets[0]
            .frames
            .iter()
            .all(|frame| frame.border == ProjectedFrameBorder::None)
    );
    apply(
        &mut project,
        vec![ids[0].clone()],
        FrameStyleChange::Opacity {
            opacity_percent: 25,
        },
    );
    let before = project.projection();
    apply(
        &mut project,
        ids.clone(),
        FrameStyleChange::BorderWidth { width_um: 2_000 },
    );
    let after = project.projection();
    assert_eq!(after.state.revision, before.state.revision + 1);
    assert_eq!(after.composition.sheets[0].frames[0].opacity_byte, 64);
    assert_eq!(after.composition.sheets[0].frames[1].opacity_byte, 255);
    assert!(
        after.composition.sheets[0]
            .frames
            .iter()
            .all(|frame| frame.border
                == ProjectedFrameBorder::Solid {
                    rgb: "#105030".into(),
                    width_um: 2_000
                })
    );
    assert_eq!(project.undo().unwrap().state.album, before.state.album);
    project.redo().unwrap();
    apply(
        &mut project,
        ids.clone(),
        FrameStyleChange::BorderWidth { width_um: 0 },
    );
    apply(
        &mut project,
        ids,
        FrameStyleChange::BorderWidth { width_um: 1_000 },
    );
    assert!(
        project.projection().composition.sheets[0]
            .frames
            .iter()
            .all(|frame| frame.border
                == ProjectedFrameBorder::Solid {
                    rgb: "#105030".into(),
                    width_um: 1_000
                })
    );
}

#[test]
fn a_thick_inward_border_saturates_without_overlapping_its_own_paint() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Borda.myalbuns");
    std::fs::write(
        &path,
        include_bytes!("fixtures/project_document_v6_photo_migration_expected.myalbuns"),
    )
    .unwrap();
    let mut context = OperationPathContext::new();
    context.capture(&path).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(myalbuns_core::OpenProjectRequest::new(
            ProjectLocation::new(path, context.freeze()),
        ))
        .unwrap();
    let id = project.projection().state.album.sheets[0].frames[0]
        .id
        .clone();
    project
        .apply(ProjectIntent::SetFrameStyle {
            edit: FrameStyleEdit {
                frame_ids: vec![id],
                change: FrameStyleChange::BorderWidth { width_um: 100_000 },
            },
        })
        .unwrap();
    let projection = project.projection();
    let frame = &projection.composition.sheets[0].frames[0];
    assert_eq!(
        frame.clip_rect,
        myalbuns_core::RectUm {
            x: 30_000,
            y: 40_000,
            width: 150_000,
            height: 100_000
        }
    );
    assert_eq!(
        frame.border_fill_rects,
        vec![
            myalbuns_core::RectUm {
                x: 30_000,
                y: 40_000,
                width: 150_000,
                height: 50_000
            },
            myalbuns_core::RectUm {
                x: 30_000,
                y: 90_000,
                width: 150_000,
                height: 50_000
            },
        ]
    );
}

#[test]
fn public_frame_style_projections_match_the_visual_corpus() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Estilo.myalbuns");
    std::fs::write(
        &path,
        include_bytes!("fixtures/project_document_v6_photo_migration_expected.myalbuns"),
    )
    .unwrap();
    let mut context = OperationPathContext::new();
    context.capture(&path).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(myalbuns_core::OpenProjectRequest::new(
            ProjectLocation::new(path, context.freeze()),
        ))
        .unwrap();
    let initial = project.projection();
    let media_id = initial.state.album.media[0].id;
    project
        .observe_photo_source(
            media_id,
            myalbuns_core::PhotoSourceMetadata::new(
                600,
                400,
                ["#C22C24", "#248044", "#2454C2"].map(String::from),
            )
            .unwrap(),
        )
        .unwrap();
    let initial = project.projection();
    let ids: Vec<_> = initial.state.album.sheets[0]
        .frames
        .iter()
        .map(|f| f.id.clone())
        .collect();
    let single = vec![ids[0].clone()];
    let mut states = serde_json::Map::new();
    let mut previews = Vec::new();
    let mut transitions = Vec::new();
    let mut record = |name: &str, project: &EditableProject| {
        states.insert(
            name.into(),
            serde_json::to_value(project.projection()).unwrap(),
        );
        for targets in [&single, &ids] {
            for change in [
                FrameStyleChange::Opacity { opacity_percent: 0 },
                FrameStyleChange::Opacity {
                    opacity_percent: 25,
                },
                FrameStyleChange::Opacity {
                    opacity_percent: 50,
                },
                FrameStyleChange::Opacity {
                    opacity_percent: 100,
                },
                FrameStyleChange::BorderWidth { width_um: 0 },
                FrameStyleChange::BorderWidth { width_um: 1_000 },
                FrameStyleChange::BorderWidth { width_um: 2_000 },
                FrameStyleChange::BorderWidth { width_um: 5_000 },
                FrameStyleChange::BorderColor {
                    rgb: "#000000".into(),
                },
                FrameStyleChange::BorderColor {
                    rgb: "#FFFFFF".into(),
                },
                FrameStyleChange::BorderColor {
                    rgb: "#205070".into(),
                },
                FrameStyleChange::RestoreAlbum,
            ] {
                let edit = FrameStyleEdit {
                    frame_ids: targets.clone(),
                    change,
                };
                previews.push(serde_json::json!({ "from": name, "edit": edit,
                    "frames": project.preview_frame_style(&edit).unwrap() }));
            }
        }
    };
    record("neutral", &project);
    let mut defaults = initial.state.album.visual_defaults.clone();
    defaults.frame_border = ProjectedFrameBorder::Solid {
        rgb: "#D6B77B".into(),
        width_um: 2_000,
    };
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: defaults,
        })
        .unwrap();
    record("album", &project);
    let mut from = "album";
    for (to, targets, change) in [
        (
            "single-opacity",
            &single,
            FrameStyleChange::Opacity {
                opacity_percent: 50,
            },
        ),
        (
            "single-border",
            &single,
            FrameStyleChange::BorderWidth { width_um: 5_000 },
        ),
        (
            "single-custom",
            &single,
            FrameStyleChange::BorderColor {
                rgb: "#205070".into(),
            },
        ),
        (
            "group-opacity",
            &ids,
            FrameStyleChange::Opacity {
                opacity_percent: 50,
            },
        ),
        (
            "group-border",
            &ids,
            FrameStyleChange::BorderWidth { width_um: 2_000 },
        ),
        (
            "group-style",
            &ids,
            FrameStyleChange::BorderColor {
                rgb: "#205070".into(),
            },
        ),
        (
            "invisible",
            &ids,
            FrameStyleChange::Opacity { opacity_percent: 0 },
        ),
        ("restored", &ids, FrameStyleChange::RestoreAlbum),
    ] {
        let edit = FrameStyleEdit {
            frame_ids: targets.clone(),
            change,
        };
        project
            .apply(ProjectIntent::SetFrameStyle { edit: edit.clone() })
            .unwrap();
        transitions.push(serde_json::json!({ "from": from, "to": to, "edit": edit }));
        record(to, &project);
        from = to;
    }
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({
            "states": states, "previews": previews, "transitions": transitions,
            "single": single, "group": ids, "placeholders": [ids[1]],
        }))
        .unwrap()
    );
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/frame-style-cases.json");
    if std::env::var_os("MYALBUNS_UPDATE_FRAME_STYLE_FIXTURE").is_some() {
        std::fs::write(&fixture, &serialized).unwrap();
    }
    assert_eq!(
        serialized,
        std::fs::read_to_string(fixture)
            .unwrap()
            .replace("\r\n", "\n")
    );
}
