#![cfg(windows)]

use std::fs;

use myalbuns_core::{
    DocumentFailure, LoadProjectError, MediaId, MediaKind, OpenProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation, SaveProjectOutcome,
};
use myalbuns_paths::OperationPathContext;

const PROJECT_V1_MIGRATION_INPUT: &[u8] =
    include_bytes!("fixtures/project_document_v1_migration_input.myalbuns");
const PROJECT_V2_MIGRATION_EXPECTED: &[u8] =
    include_bytes!("fixtures/project_document_v2_migration_expected.myalbuns");

const PROJECT_WITH_PHOTO_AND_DECORATIVE_V2: &str = r##"{
  "documentType": "myalbuns.project",
  "schemaVersion": 2,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 0,
  "project": {
    "document": {
      "displayUnit": "mm",
      "sheetWidthUm": 600000,
      "sheetHeightUm": 300000,
      "dpi": 300,
      "bleedUm": 3000,
      "safetyUm": 3000
    },
    "visualDefaults": {
      "background": {
        "scope": "bothSides",
        "both": {
          "kind": "media",
          "mediaId": "00000000-0000-4000-8000-000000000011"
        }
      },
      "overlay": {
        "scope": "bothSides",
        "both": {
          "kind": "media",
          "mediaId": "00000000-0000-4000-8000-000000000011"
        }
      },
      "frameBorder": { "kind": "none" }
    },
    "media": [
      {
        "id": "00000000-0000-4000-8000-000000000010",
        "kind": "photo",
        "path": {
          "encoding": "windowsUtf16",
          "units": [67, 58, 92, 70, 111, 116, 111, 115, 92, 70, 111, 116, 111, 46, 106, 112, 103]
        }
      },
      {
        "id": "00000000-0000-4000-8000-000000000011",
        "kind": "decorative",
        "path": {
          "encoding": "windowsUtf16",
          "units": [67, 58, 92, 70, 111, 116, 111, 115, 92, 79, 118, 101, 114, 108, 97, 121, 46, 112, 110, 103]
        }
      }
    ],
    "sheets": [
      {
        "id": "00000000-0000-4000-8000-000000000001",
        "activeSides": "both"
      },
      {
        "id": "00000000-0000-4000-8000-000000000002",
        "activeSides": "both"
      }
    ]
  }
}"##;

#[test]
fn v2_persists_photo_and_decorative_as_media_refs_without_observed_state() {
    let root = tempfile::tempdir().expect("temporary v2 Project");
    let project_path = root.path().join("Projeto tracer.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE_V2)
        .expect("the v2 fixture is written");

    let loaded = ProjectCore::new()
        .load_persisted_revision(myalbuns_core::LoadProjectRequest::new(location(
            &project_path,
        )))
        .expect("the v2 Project loads read-only");

    let media = loaded.project().media();
    assert_eq!(media.len(), 2);
    assert_eq!(media[0].kind(), MediaKind::Photo);
    assert_eq!(media[1].kind(), MediaKind::Decorative);
    let debug = format!("{media:?}");
    for forbidden in [
        "availability",
        "fingerprint",
        "generation",
        "source_width",
        "source_height",
    ] {
        assert!(
            !debug.contains(forbidden),
            "observed state must stay outside MediaRef: {forbidden}"
        );
    }
}

#[test]
fn v2_allows_one_photo_and_one_decorative_to_reference_the_same_native_path() {
    let root = tempfile::tempdir().expect("temporary cross-tab Project");
    let project_path = root.path().join("Projeto midia entre abas.myalbuns");
    let photo_path =
        "[67, 58, 92, 70, 111, 116, 111, 115, 92, 70, 111, 116, 111, 46, 106, 112, 103]";
    let overlay_path = "[67, 58, 92, 70, 111, 116, 111, 115, 92, 79, 118, 101, 114, 108, 97, 121, 46, 112, 110, 103]";
    let project = PROJECT_WITH_PHOTO_AND_DECORATIVE_V2.replacen(overlay_path, photo_path, 1);
    fs::write(&project_path, project).expect("the cross-tab fixture is written");

    let loaded = ProjectCore::new()
        .load_persisted_revision(myalbuns_core::LoadProjectRequest::new(location(
            &project_path,
        )))
        .expect("the same path is valid once in each media tab");

    assert_eq!(loaded.project().media().len(), 2);
    assert_eq!(loaded.project().media()[0].kind(), MediaKind::Photo);
    assert_eq!(loaded.project().media()[1].kind(), MediaKind::Decorative);
    assert_eq!(
        loaded.project().media()[0].path(),
        loaded.project().media()[1].path()
    );
}

#[test]
fn v2_rejects_a_photo_as_background_or_overlay() {
    let root = tempfile::tempdir().expect("temporary visual-role Project");
    let project_path = root.path().join("Projeto foto como padrao.myalbuns");
    let photo_id = "00000000-0000-4000-8000-000000000010";
    let decorative_id = "00000000-0000-4000-8000-000000000011";
    let project = PROJECT_WITH_PHOTO_AND_DECORATIVE_V2.replacen(decorative_id, photo_id, 2);
    fs::write(&project_path, project).expect("the invalid role fixture is written");

    assert_eq!(
        ProjectCore::new()
            .load_persisted_revision(myalbuns_core::LoadProjectRequest::new(location(
                &project_path,
            )))
            .expect_err("a Photo cannot occupy a Decorative-only visual role"),
        LoadProjectError::Document(DocumentFailure::InvalidProjectState)
    );
}

#[test]
fn an_authorized_editable_v2_project_keeps_its_schema_and_opaque_identity_authority() {
    let root = tempfile::tempdir().expect("temporary editable v2 Project");
    let project_path = root.path().join("Projeto tracer.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE_V2)
        .expect("the v2 fixture is written");
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the v2 Project is positively authorized");

    assert_eq!(
        project.identity_authority().project_id(),
        project.project_id()
    );
    let projected_media = &project.projection().state.album.media;
    assert_eq!(projected_media.len(), 2);
    assert_eq!(projected_media[0].kind, MediaKind::Photo);
    assert_eq!(projected_media[1].kind, MediaKind::Decorative);
    let changed = project
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("a creative change advances the v2 Project");
    project
        .save(changed.state.revision)
        .expect("the v2 revision is saved");

    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(&project_path).expect("the saved v2 Project is readable"))
            .expect("the saved v2 Project remains JSON");
    assert_eq!(persisted["schemaVersion"], 2);
    assert_eq!(persisted["project"]["media"][0]["kind"], "photo");
    assert_eq!(persisted["project"]["media"][1]["kind"], "decorative");
}

#[test]
fn a_frozen_sheet_owns_the_same_composition_and_only_its_exact_originals() {
    let root = tempfile::tempdir().expect("temporary frozen v2 Project");
    let project_path = root.path().join("Projeto congelado.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE_V2)
        .expect("the v2 fixture is written");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the v2 Project is positively authorized");
    let selected_sheet_id = project.projection().composition.sheets[1].sheet_id.clone();

    let frozen = project.freeze_rendering();
    assert_eq!(
        frozen.projection().composition,
        frozen.render_snapshot().composition,
        "Canvas and Exportation must receive the same CompositionPlan",
    );
    let frozen_sheet = frozen
        .into_sheet(&selected_sheet_id)
        .expect("the selected sheet owns its exact sources");

    assert_eq!(frozen_sheet.output_unit().sheet.sheet_id, selected_sheet_id);
    let referenced = frozen_sheet
        .output_unit()
        .sheet
        .referenced_media_ids()
        .collect::<Vec<MediaId>>();
    assert_eq!(
        referenced.len(),
        2,
        "background and overlay both reference media"
    );
    assert!(
        referenced
            .iter()
            .all(|media_id| media_id.to_string() == "00000000-0000-4000-8000-000000000011")
    );
    assert_eq!(
        frozen_sheet
            .sources()
            .iter()
            .map(|source| (source.kind(), source.path()))
            .collect::<Vec<_>>(),
        vec![(
            MediaKind::Decorative,
            std::path::Path::new(r"C:\Fotos\Overlay.png"),
        ),],
        "the unreferenced Foto is not frozen for this output unit",
    );
}

#[test]
fn v1_migrates_only_in_memory_and_read_only_loading_preserves_the_source_bytes() {
    let root = tempfile::tempdir().expect("temporary v1 migration Project");
    let project_path = root.path().join("Projeto legado.myalbuns");
    fs::write(&project_path, PROJECT_V1_MIGRATION_INPUT).expect("the v1 fixture is written");

    let loaded = ProjectCore::new()
        .load_persisted_revision(myalbuns_core::LoadProjectRequest::new(location(
            &project_path,
        )))
        .expect("the v1 Project migrates in memory for read-only use");

    assert_eq!(loaded.revision(), 7);
    assert_eq!(loaded.project().media()[0].kind(), MediaKind::Decorative);
    assert_eq!(
        fs::read(&project_path).expect("the v1 source remains readable"),
        PROJECT_V1_MIGRATION_INPUT,
        "read-only migration must preserve the source byte for byte"
    );
}

#[test]
fn explicit_save_promotes_an_open_v1_project_to_the_versioned_v2_golden_result() {
    let root = tempfile::tempdir().expect("temporary editable migration Project");
    let project_path = root.path().join("Projeto legado.myalbuns");
    fs::write(&project_path, PROJECT_V1_MIGRATION_INPUT).expect("the v1 fixture is written");
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the v1 Project migrates in memory before the editable Session opens");

    assert_eq!(project.revision(), 7);
    assert_eq!(project.saved_revision(), 7);
    assert!(!project.has_unsaved_changes());
    assert_eq!(
        fs::read(&project_path).expect("the unopened migration remains v1"),
        PROJECT_V1_MIGRATION_INPUT
    );

    assert_eq!(
        project
            .save(7)
            .expect("explicit Save publishes the current schema without a creative revision"),
        SaveProjectOutcome::Saved { revision: 7 }
    );
    assert!(!project.has_unsaved_changes());
    assert_eq!(
        fs::read(&project_path).expect("the migrated Project is readable"),
        PROJECT_V2_MIGRATION_EXPECTED
    );
}

fn location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the Project root is captured without opening its media");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}
