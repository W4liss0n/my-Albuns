#![cfg(windows)]

use std::path::Path;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectIntent, ProjectLocation, SaveCopyAsError,
    SaveCopyAsRequest,
};
use myalbuns_paths::{OperationPathContext, ProcessInstanceId, project_data_namespace};

fn project_location(path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths
        .capture(path)
        .expect("the public path seam captures the Project root");
    ProjectLocation::new(path.to_path_buf(), paths.freeze())
}

fn project_core(storage_root: &Path) -> ProjectCore {
    ProjectCore::new()
        .with_identity_storage_roots(storage_root.join("leases"), storage_root.join("identities"))
}

#[allow(
    clippy::permissions_set_readonly_false,
    reason = "this integration suite runs only on Windows, where clearing FILE_ATTRIBUTE_READONLY is the intended operation"
)]
fn make_writable(path: &Path) {
    let mut permissions = std::fs::metadata(path)
        .expect("the read-only fixture still has metadata")
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).expect("the Windows fixture becomes writable");
}

#[test]
fn moving_a_closed_project_preserves_its_identity_and_namespace() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("original.myalbuns");
    let moved_path = fixture.path().join("movido.myalbuns");
    let core = project_core(fixture.path());

    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created through ProjectCore");
    let original_id = original.project_id();
    let original_namespace = project_data_namespace(&original_id.hyphenated().to_string());
    drop(original);

    std::fs::rename(&original_path, &moved_path)
        .expect("the operating system moves the closed Project");

    let moved = core
        .open_editable(OpenProjectRequest::new(project_location(&moved_path)))
        .expect("a confirmed movement opens without creating another Project");

    assert_eq!(moved.project_id(), original_id);
    assert_eq!(moved.identity_authority().project_id(), original_id);
    assert_eq!(moved.project_path(), moved_path);
    assert_eq!(
        project_data_namespace(&moved.project_id().hyphenated().to_string()),
        original_namespace,
        "movement keeps the Cache/Recovery/WebView namespace"
    );
}

#[test]
fn a_second_physical_alias_focuses_the_existing_session() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let alias_path = fixture.path().join("Alias.myalbuns");
    let core = project_core(fixture.path());
    let opened = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let project_id = opened.project_id();
    std::fs::hard_link(&original_path, &alias_path)
        .expect("the alias names the same physical Project file");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&alias_path)))
            .expect_err("an alias must not create a second editable Sessão"),
        OpenProjectError::FocusExisting {
            project_id,
            owner_process: ProcessInstanceId::current()
                .expect("the owning process instance is captured"),
        }
    );
}

#[test]
fn a_writable_external_copy_gets_a_new_identity_without_pending_creative_changes() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let copy_path = fixture.path().join("Copia externa.myalbuns");
    let core = project_core(fixture.path());
    let mut original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    original
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the original has a pending creative change");
    assert_eq!(original.projection().state.document.dpi, 240);
    assert!(original.has_unsaved_changes());

    std::fs::copy(&original_path, &copy_path)
        .expect("Windows creates an external physical copy from persisted bytes");
    let copied = core
        .open_editable(OpenProjectRequest::new(project_location(&copy_path)))
        .expect("a writable external copy is promoted before opening");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(
        copied.identity_authority().project_id(),
        copied.project_id()
    );
    assert_eq!(copied.revision(), 0);
    assert_eq!(copied.saved_revision(), 0);
    assert_eq!(copied.projection().state.document.dpi, 300);
    assert!(!copied.has_unsaved_changes());
    assert!(!copied.can_undo());
    assert_ne!(
        project_data_namespace(&copied.project_id().hyphenated().to_string()),
        project_data_namespace(&original_id.hyphenated().to_string()),
        "the promoted copy authorizes a distinct local namespace"
    );

    let original_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&original_path).expect("the original remains readable"),
    )
    .expect("the original remains valid JSON");
    let copy_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&copy_path).expect("the promoted copy remains readable"),
    )
    .expect("the promoted copy remains valid JSON");
    assert_eq!(
        original_json["projectId"],
        serde_json::Value::String(original_id.hyphenated().to_string())
    );
    assert_eq!(copy_json["projectId"], copied.project_id().to_string());
    assert_eq!(copy_json["schemaVersion"], original_json["schemaVersion"]);
    assert_eq!(copy_json["revision"], original_json["revision"]);
    assert_eq!(copy_json["project"], original_json["project"]);
}

#[test]
fn a_read_only_external_copy_can_be_saved_as_a_new_editable_project() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let read_only_path = fixture.path().join("Copia somente leitura.myalbuns");
    let destination_path = fixture.path().join("Copia editavel.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    let original_revision = original.revision();
    drop(original);
    std::fs::copy(&original_path, &read_only_path)
        .expect("Windows creates the external physical copy");
    let mut permissions = std::fs::metadata(&read_only_path)
        .expect("the copied file has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&read_only_path, permissions)
        .expect("the external copy becomes read-only");
    let source_bytes = std::fs::read(&read_only_path).expect("the source is readable");

    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&read_only_path)))
        .expect_err("a read-only external copy cannot be corrected in place")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    let copied = core
        .save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&destination_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect("Salvar cópia como... publishes a new editable Project");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(copied.revision(), original_revision);
    assert_eq!(copied.saved_revision(), original_revision);
    assert!(!copied.has_unsaved_changes());
    assert!(!copied.can_undo());
    assert_eq!(
        std::fs::read(&read_only_path).expect("the source remains readable"),
        source_bytes,
        "the validated read-only source remains byte-for-byte intact"
    );
    let destination_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&destination_path).expect("the destination is readable"),
    )
    .expect("the destination is valid JSON");
    assert_eq!(destination_json["schemaVersion"], 2);
    assert_eq!(destination_json["revision"], original_revision);
    assert_eq!(
        destination_json["projectId"],
        copied.project_id().hyphenated().to_string()
    );
}

#[test]
fn save_copy_as_never_rewrites_a_source_that_became_writable() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let source_path = fixture.path().join("Fonte externa.myalbuns");
    let destination_path = fixture.path().join("Destino editavel.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    drop(original);
    std::fs::copy(&original_path, &source_path).expect("the external source is copied");
    let source_bytes = std::fs::read(&source_path).expect("the source baseline is captured");
    let mut permissions = std::fs::metadata(&source_path)
        .expect("the source has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source_path, permissions)
        .expect("the source first refuses in-place Identidade correction");
    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("the opening returns the validated opaque source")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    make_writable(&source_path);
    let copied = core
        .save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&destination_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the external source is saved at a distinct destination");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(
        std::fs::read(&source_path).expect("the source remains readable"),
        source_bytes,
        "Salvar cópia como... never corrects Identidade in the source pathname"
    );
}

#[test]
fn cancelling_or_failing_save_copy_as_leaves_no_source_session_or_path_mutation() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let source_path = fixture.path().join("Fonte externa.myalbuns");
    let occupied_path = fixture.path().join("Destino ocupado.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    drop(original);
    std::fs::copy(&original_path, &source_path).expect("the external source is copied");
    let mut permissions = std::fs::metadata(&source_path)
        .expect("the source has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source_path, permissions)
        .expect("the external source becomes read-only");
    std::fs::write(&occupied_path, b"occupied destination")
        .expect("the destination conflict is explicit");
    let source_bytes = std::fs::read(&source_path).expect("source bytes are captured");
    let occupied_bytes = std::fs::read(&occupied_path).expect("destination bytes are captured");

    let cancelled = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("the refused opening returns the cancellable source")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    drop(cancelled);
    assert_eq!(
        std::fs::read(&source_path).expect("the cancelled source remains readable"),
        source_bytes
    );

    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("cancellation retained no editable ownership")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    assert_eq!(
        core.save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&occupied_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect_err("an occupied destination fails before a Sessão exists"),
        SaveCopyAsError::DestinationConflict
    );
    assert_eq!(
        std::fs::read(&source_path).expect("the failed source remains readable"),
        source_bytes
    );
    assert_eq!(
        std::fs::read(&occupied_path).expect("the occupied destination remains readable"),
        occupied_bytes
    );

    let retry = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("failure retained no editable ownership or duplicated source Sessão")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    drop(retry);
    make_writable(&source_path);
}

#[test]
fn a_reused_previous_location_is_indeterminate_and_never_rewrites_the_candidate() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let previous_path = fixture.path().join("Local anterior.myalbuns");
    let candidate_path = fixture.path().join("Candidato.myalbuns");
    let replacement_path = fixture.path().join("Outro projeto.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&previous_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project establishes durable Identidade evidence");
    std::fs::copy(&previous_path, &candidate_path)
        .expect("the candidate preserves the original persisted Identidade");
    drop(original);
    let replacement = core
        .create_editable(CreateProjectRequest::new(
            project_location(&replacement_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("another Project is created");
    drop(replacement);
    std::fs::copy(&replacement_path, &previous_path)
        .expect("the previous pathname is reused by another valid Project");
    let candidate_bytes = std::fs::read(&candidate_path).expect("candidate bytes are captured");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&candidate_path)))
            .expect_err("a reused pathname cannot prove movement or an external copy"),
        OpenProjectError::IdentityIndeterminate
    );
    assert_eq!(
        std::fs::read(&candidate_path).expect("the rejected candidate remains readable"),
        candidate_bytes,
        "Indeterminate fails closed without rewriting Identidade"
    );
}
