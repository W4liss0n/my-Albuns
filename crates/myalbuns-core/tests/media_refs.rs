#![cfg(windows)]

use std::fs;

use myalbuns_core::{
    DocumentFailure, LoadProjectError, MediaId, MediaKind, OpenProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation, RelinkMedia, RenderSnapshotRef,
};
use myalbuns_paths::OperationPathContext;

const PROJECT_WITH_PHOTO_AND_DECORATIVE: &str = r##"{
  "documentType": "myalbuns.project",
  "schemaVersion": 1,
  "projectId": "550e8400-e29b-41d4-a716-446655440000",
  "revision": 0,
  "project": {
    "album": {
      "displayUnit": "mm",
      "sheetWidthUm": 600000,
      "sheetHeightUm": 300000,
      "dpi": 300,
      "bleedUm": 3000,
      "safetyUm": 3000
    },
    "layoutSettings": {
      "permission": "pagesAndSheet",
      "marginUm": 15000,
      "gapUm": 5000,
      "minimumSideUm": 20000
    },
    "visualDefaults": {
      "background": {
        "sides": "both",
        "both": { "kind": "media", "mediaId": "00000000-0000-4000-8000-000000000011" }
      },
      "overlay": {
        "sides": "both",
        "both": { "kind": "media", "mediaId": "00000000-0000-4000-8000-000000000011" }
      },
      "frameBorder": { "kind": "none" }
    },
    "media": [
      {
        "id": "00000000-0000-4000-8000-000000000010",
        "kind": "photo",
        "path": "C:\\Fotos\\Foto.jpg"
      },
      {
        "id": "00000000-0000-4000-8000-000000000011",
        "kind": "decorative",
        "path": "C:\\Fotos\\Overlay.png"
      }
    ],
    "sheets": [
      { "id": "00000000-0000-4000-8000-000000000001", "activeSides": "both" },
      { "id": "00000000-0000-4000-8000-000000000002", "activeSides": "both" }
    ]
  }
}"##;

#[test]
fn photo_and_decorative_persist_as_media_refs_without_observed_state() {
    let root = tempfile::tempdir().expect("temporary Project");
    let project_path = root.path().join("Projeto tracer.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE).expect("the fixture is written");

    let loaded = ProjectCore::new()
        .load_persisted_revision(myalbuns_core::LoadProjectRequest::new(location(
            &project_path,
        )))
        .expect("the Project loads read-only");

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
fn one_photo_and_one_decorative_may_reference_the_same_native_path() {
    let root = tempfile::tempdir().expect("temporary cross-tab Project");
    let project_path = root.path().join("Projeto midia entre abas.myalbuns");
    let photo_path = r#""C:\\Fotos\\Foto.jpg""#;
    let overlay_path = r#""C:\\Fotos\\Overlay.png""#;
    let project = PROJECT_WITH_PHOTO_AND_DECORATIVE.replacen(overlay_path, photo_path, 1);
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
fn a_photo_is_rejected_as_background_or_overlay() {
    let root = tempfile::tempdir().expect("temporary visual-role Project");
    let project_path = root.path().join("Projeto foto como padrao.myalbuns");
    let photo_id = "00000000-0000-4000-8000-000000000010";
    let decorative_id = "00000000-0000-4000-8000-000000000011";
    let project = PROJECT_WITH_PHOTO_AND_DECORATIVE.replacen(decorative_id, photo_id, 2);
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
fn an_authorized_editable_project_saves_media_refs_and_keeps_opaque_identity_authority() {
    let root = tempfile::tempdir().expect("temporary editable Project");
    let project_path = root.path().join("Projeto tracer.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE).expect("the fixture is written");
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the Project is positively authorized");

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
        .expect("a creative change advances the Project");
    project
        .save(changed.state.revision)
        .expect("the revision is saved");

    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(&project_path).expect("the saved Project is readable"))
            .expect("the saved Project remains JSON");
    assert_eq!(persisted["schemaVersion"], 1);
    assert_eq!(persisted["project"]["media"][0]["kind"], "photo");
    assert_eq!(persisted["project"]["media"][1]["kind"], "decorative");
}

#[test]
fn public_relink_command_updates_only_the_selected_occurrence_and_participates_in_history() {
    let root = tempfile::tempdir().expect("temporary relink Project");
    let project_path = root.path().join("Projeto religado.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE).expect("the fixture is written");
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the Project is positively authorized");
    let selected_id: MediaId = "00000000-0000-4000-8000-000000000010"
        .parse()
        .expect("the selected occurrence has a canonical identity");
    let replacement = root.path().join("Foto religada.jpg");
    let untouched_path = project.project().media()[1].path().to_path_buf();

    let relinked = project
        .relink_media(RelinkMedia::new(selected_id, replacement.clone()))
        .expect("the public Session command relinks the selected occurrence");

    assert_eq!(relinked.state.revision, 1);
    assert!(relinked.state.dirty);
    assert!(relinked.state.can_undo);
    assert_eq!(project.project().media()[0].path(), replacement);
    assert_eq!(project.project().media()[1].path(), untouched_path);

    project.undo().expect("RelinkMedia participates in Undo");
    assert_ne!(project.project().media()[0].path(), replacement);
    project.redo().expect("RelinkMedia participates in Redo");
    assert_eq!(project.project().media()[0].path(), replacement);
    project
        .save(project.revision())
        .expect("RelinkMedia persists through the normal Save handshake");
    drop(project);

    let reopened = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the relinked Project reopens");
    assert_eq!(reopened.project().media()[0].path(), replacement);
    assert_eq!(reopened.project().media()[1].path(), untouched_path);
}

#[test]
fn frozen_rendering_borrows_one_resolved_plan_for_canvas_and_export() {
    let root = tempfile::tempdir().expect("temporary frozen Project");
    let project_path = root.path().join("Projeto congelado.myalbuns");
    fs::write(&project_path, PROJECT_WITH_PHOTO_AND_DECORATIVE).expect("the fixture is written");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&project_path)))
        .expect("the Project is positively authorized");
    let selected_sheet_id = "00000000-0000-4000-8000-000000000002".to_owned();

    let frozen = project.freeze_rendering();
    let snapshot: RenderSnapshotRef<'_> = frozen.render_snapshot();
    assert!(
        std::ptr::eq(snapshot.composition, frozen.render_snapshot().composition),
        "Canvas and Export must borrow the same resolved CompositionPlan"
    );
    assert_eq!(snapshot.project_id, "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(snapshot.project_name, "Projeto congelado");
    assert_eq!(snapshot.revision, 0);
    assert_eq!(snapshot.dpi, 300);
    assert_eq!(snapshot.composition.sheets.len(), 2);
    assert_eq!(snapshot.composition.sheets[1].sheet_id, selected_sheet_id);
    assert_eq!(snapshot.composition.sheets[1].width_um, 600_000);
    assert_eq!(snapshot.composition.sheets[1].height_um, 300_000);
    let canvas_sheet = &frozen.render_snapshot().composition.sheets[1];
    assert_eq!(canvas_sheet.sheet_id, selected_sheet_id);
    assert_eq!(canvas_sheet.number, 2);
    assert_eq!(canvas_sheet.width_um, 600_000);
    assert_eq!(canvas_sheet.height_um, 300_000);
    let (snapshot, sources) = frozen
        .into_export(std::slice::from_ref(&selected_sheet_id))
        .expect("the selected sheet owns its exact sources");

    let unit = snapshot.output_unit(&selected_sheet_id).unwrap();
    assert_eq!(unit.sheet.sheet_id, selected_sheet_id);
    let referenced = unit.sheet.referenced_media_ids().collect::<Vec<MediaId>>();
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
        sources
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

fn location(path: &std::path::Path) -> ProjectLocation {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .expect("the Project root is captured without opening its media");
    ProjectLocation::new(path.to_path_buf(), context.freeze())
}
