#![cfg(windows)]
//! Contract of the public Project File, schema version 1.
//!
//! `complete.myalbuns` exercises every persisted feature. It was produced by
//! the public Core through `write_complete_fixture`; regenerate it only when the
//! contract changes on purpose:
//! `cargo test -p myalbuns-core --test project_file_v1 -- --ignored write_complete_fixture`.

use std::{fs, path::Path};

use myalbuns_core::*;
use myalbuns_paths::OperationPathContext;

const FIXTURES: [(&str, &[u8]); 6] = [
    (
        "base",
        include_bytes!("fixtures/project_file_v1/base.myalbuns"),
    ),
    (
        "photo",
        include_bytes!("fixtures/project_file_v1/photo.myalbuns"),
    ),
    (
        "photo_angle",
        include_bytes!("fixtures/project_file_v1/photo_angle.myalbuns"),
    ),
    (
        "photo_effect",
        include_bytes!("fixtures/project_file_v1/photo_effect.myalbuns"),
    ),
    (
        "folders_valid",
        include_bytes!("fixtures/project_file_v1/folders_valid.myalbuns"),
    ),
    (
        "complete",
        include_bytes!("fixtures/project_file_v1/complete.myalbuns"),
    ),
];

fn location(path: &Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}

fn core(root: &Path) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(root.join("leases"), root.join("identities"))
}

#[test]
fn every_valid_fixture_is_written_back_byte_for_byte() {
    for (name, bytes) in FIXTURES {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(format!("{name}.myalbuns"));
        fs::write(&path, bytes).unwrap();
        let mut project = core(root.path())
            .open_editable(OpenProjectRequest::new(location(&path)))
            .unwrap_or_else(|error| panic!("{name} opens: {error:?}"));
        assert!(!project.has_unsaved_changes(), "{name}");
        let original_id = project.project_id().hyphenated().to_string();
        let expected = project.project().clone();

        let copy = root.path().join(format!("{name} copy.myalbuns"));
        project
            .save_as(SaveAsProjectRequest::new(
                project.revision(),
                location(&copy),
                SaveAsAuthorization::CreateOnly,
            ))
            .unwrap();
        let copied_id = project.project_id().hyphenated().to_string();
        drop(project);

        // Save As gives the copy its own Identity and changes nothing else.
        let written = fs::read_to_string(&copy).unwrap();
        assert_eq!(written.matches(&copied_id).count(), 1, "{name}");
        assert_eq!(
            written.replacen(&copied_id, &original_id, 1).as_bytes(),
            bytes,
            "{name} is not written back byte for byte"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes, "{name} source changed");

        let reopened = core(root.path())
            .open_editable(OpenProjectRequest::new(location(&copy)))
            .unwrap();
        assert_eq!(reopened.project(), &expected, "{name}");
    }
}

#[test]
fn the_complete_fixture_carries_every_persisted_feature() {
    let document: serde_json::Value = serde_json::from_slice(FIXTURES[5].1).unwrap();
    let project = &document["project"];
    let sheet = &project["sheets"][0];
    let frame = &sheet["frames"][0];
    assert_eq!(document["schemaVersion"], 1);
    assert_eq!(project["visualDefaults"]["background"]["sides"], "both");
    assert_eq!(project["visualDefaults"]["frameBorder"]["kind"], "solid");
    assert!(project["mediaFolders"].as_array().unwrap().len() >= 2);
    assert_eq!(project["favoriteLayouts"].as_array().unwrap().len(), 1);
    assert_eq!(sheet["layoutLocked"], true);
    assert!(sheet["lastLayout"]["positions"].is_array());
    assert_eq!(sheet["visuals"]["background"]["sides"], "perSide");
    assert_eq!(sheet["visuals"]["overlay"]["sides"], "perSide");
    assert!(frame["style"]["opacityPercent"].is_number());
    for field in [
        "panX",
        "userZoom",
        "quarterTurns",
        "mirrorX",
        "angleTenths",
        "blackAndWhite",
    ] {
        assert!(
            !frame["photo"]["transform"][field].is_null(),
            "{field} is persisted"
        );
    }
    let paths = project["media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| &media["path"])
        .collect::<Vec<_>>();
    assert!(
        paths
            .iter()
            .any(|path| path.as_str().is_some_and(|text| text.contains('ô')))
    );
    assert!(paths.iter().any(|path| path["windowsUtf16"].is_array()));
}

#[test]
fn an_invalid_folder_membership_is_rejected_without_writing() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Pastas.myalbuns");
    let bytes =
        include_bytes!("fixtures/project_file_v1/folders_invalid_duplicate_membership.myalbuns");
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        core(root.path())
            .load_persisted_revision(LoadProjectRequest::new(location(&path)))
            .unwrap_err(),
        LoadProjectError::Document(DocumentFailure::InvalidProjectState)
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn development_files_before_the_first_public_version_are_rejected_without_writing() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Desenvolvimento.myalbuns");
    let mut document: serde_json::Value = serde_json::from_slice(FIXTURES[1].1).unwrap();
    document["schemaVersion"] = serde_json::json!(12);
    let bytes = serde_json::to_vec(&document).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        core(root.path())
            .load_persisted_revision(LoadProjectRequest::new(location(&path)))
            .unwrap_err(),
        LoadProjectError::Document(DocumentFailure::UnsupportedFutureSchema { version: 12 })
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
#[ignore = "regenerates tests/fixtures/project_file_v1/complete.myalbuns"]
fn write_complete_fixture() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Completo.myalbuns");
    let mut input: serde_json::Value =
        serde_json::from_slice(include_bytes!("fixtures/project_file_v1/photo.myalbuns")).unwrap();
    let media = input["project"]["media"].as_array_mut().unwrap();
    media[0]["path"] = serde_json::json!("C:\\Fotos\\Cerimônia\\Noivos.jpg");
    media.push(serde_json::json!({
        "id": "00000000-0000-4000-8000-000000000020", "kind": "decorative",
        "path": "\\\\servidor\\Texturas\\papel.png"
    }));
    // A name that is not valid UTF-16 keeps its exact code units.
    media.push(serde_json::json!({
        "id": "00000000-0000-4000-8000-000000000021", "kind": "decorative",
        "path": { "windowsUtf16": [67, 58, 92, 99, 97, 112, 97, 55296, 46, 112, 110, 103] }
    }));
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let mut project = core(root.path())
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap();
    let initial = project.projection();
    let sheet_id = initial.state.album.sheets[0].id.clone();
    let photo_id = initial.state.album.media[0].id;
    let decorative_id: MediaId = "00000000-0000-4000-8000-000000000020".parse().unwrap();
    let frame_id = initial.state.album.sheets[0].frames[0].id.clone();
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
        .apply(ProjectIntent::SetLayoutSettings {
            settings: LayoutSettings {
                permission: LayoutPermission::PagesOnly,
                parameters: LayoutParameters {
                    margin_um: 19_000,
                    gap_um: 6_000,
                    minimum_side_um: 25_000,
                },
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::SetVisualDefaults {
            visual_defaults: ProjectedVisualDefaults {
                background: ProjectedBackground::BothSides {
                    both: ProjectedBackgroundContent::Media {
                        media_id: decorative_id,
                    },
                },
                overlay: ProjectedOverlay::BothSides { both: None },
                frame_border: ProjectedFrameBorder::Solid {
                    rgb: "#204060".into(),
                    width_um: 1_500,
                },
            },
        })
        .unwrap();
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
        FrameStyleChange::BorderWidth { width_um: 2_000 },
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
    let query = project.query_layouts(&sheet_id).unwrap();
    project
        .apply(ProjectIntent::ToggleLayoutFavorite {
            selection: LayoutSelection {
                query_id: query.query_id.clone(),
                candidate_index: 0,
            },
        })
        .unwrap();
    let query = project.query_layouts(&sheet_id).unwrap();
    project
        .apply(ProjectIntent::LockLayout {
            selection: LayoutSelection {
                query_id: query.query_id,
                candidate_index: 0,
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::EditSheetVisual {
            sheet_id: sheet_id.clone(),
            scope: DecorativeScope::Left,
            change: SheetVisualChange::BackgroundColor {
                rgb: "#E7D6C4".into(),
            },
        })
        .unwrap();
    project
        .apply(ProjectIntent::ApplyDecorative {
            sheet_id: sheet_id.clone(),
            media_id: decorative_id,
            role: DecorativeRole::Overlay,
            scope: DecorativeScope::Right,
        })
        .unwrap();
    for (kind, name) in [
        (MediaKind::Photo, "Cerimônia"),
        (MediaKind::Decorative, "Texturas"),
    ] {
        project
            .apply(ProjectIntent::EditMediaFolder {
                edit: MediaFolderEdit::Create {
                    media_kind: kind,
                    name: name.into(),
                },
            })
            .unwrap();
    }
    let folder_id = project.project().media_folders()[0].id.clone();
    project
        .apply(ProjectIntent::EditMediaFolder {
            edit: MediaFolderEdit::MoveMedia {
                media_ids: vec![photo_id],
                folder_id: Some(folder_id),
            },
        })
        .unwrap();
    project.save(project.revision()).unwrap();
    drop(project);

    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/project_file_v1/complete.myalbuns");
    fs::copy(&path, target).unwrap();
}
