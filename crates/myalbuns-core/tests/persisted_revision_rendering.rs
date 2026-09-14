use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, LoadProjectRequest, ProjectCore,
    ProjectIntent, ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

#[test]
fn persisted_rendering_ignores_unsaved_changes_and_identifies_the_exact_loaded_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Album.myalbuns");
    let core = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"));
    let location = || {
        let mut paths = OperationPathContext::new();
        paths.capture(&path).unwrap();
        ProjectLocation::new(path.clone(), paths.freeze())
    };
    let mut editor = core
        .create_editable(CreateProjectRequest::new(
            location(),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    editor.apply(ProjectIntent::SetDpi { dpi: 240 }).unwrap();
    let persisted = core
        .load_persisted_revision(LoadProjectRequest::new(location()))
        .unwrap();
    let original_digest = persisted.content_sha256().to_owned();
    let frozen = persisted.freeze_rendering();
    let sheet_ids = frozen
        .render_snapshot()
        .composition
        .sheets
        .iter()
        .map(|sheet| sheet.sheet_id.clone())
        .collect::<Vec<_>>();
    let (snapshot, _) = frozen.into_export(&sheet_ids).unwrap();
    assert_eq!(snapshot.dpi, 300);
    assert_eq!(snapshot.revision, 0);
    assert_eq!(snapshot.project_name, "Album");
    assert!(editor.has_unsaved_changes());
    editor.save(editor.revision()).unwrap();
    let saved = core
        .load_persisted_revision(LoadProjectRequest::new(location()))
        .unwrap();
    assert_ne!(saved.content_sha256(), original_digest);
    assert_eq!(saved.freeze_rendering().render_snapshot().dpi, 240);
    // A same-revision external rewrite must also invalidate the preflight token.
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n ")
        .unwrap();
    let rewritten = core
        .load_persisted_revision(LoadProjectRequest::new(location()))
        .unwrap();
    assert_eq!(rewritten.revision(), saved.revision());
    assert_ne!(rewritten.content_sha256(), saved.content_sha256());
}
