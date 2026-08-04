use myalbuns_paths::{
    ExpectedObject, ExportPathPlan, OperationPathContext, PhysicalIdentityEvidence, ResolveError,
};

#[test]
fn resolves_existing_objects_through_the_frozen_plan_and_checks_their_type() {
    let root = tempfile::tempdir().expect("temporary path root");
    let media = root.path().join("Foto não ASCII.jpg");
    std::fs::write(&media, b"photo").expect("the media fixture is writable");
    let folder = root.path().join("Destino");
    std::fs::create_dir(&folder).expect("the destination fixture is writable");

    let mut owner = OperationPathContext::new();
    owner
        .capture(&media)
        .expect("the operation owner captures the native root");
    let plan = owner.freeze();

    let resolved_media = plan
        .resolve_existing(&media, ExpectedObject::RegularFile)
        .expect("the participant opens the expected regular file");
    assert_eq!(resolved_media.logical_path(), media);
    assert_eq!(resolved_media.operational_path(), media);
    assert_eq!(resolved_media.object_type(), ExpectedObject::RegularFile);

    let resolved_folder = plan
        .resolve_existing(&folder, ExpectedObject::Directory)
        .expect("the participant opens the expected directory");
    assert_eq!(resolved_folder.object_type(), ExpectedObject::Directory);

    assert_eq!(
        plan.resolve_existing(&media, ExpectedObject::Directory)
            .unwrap_err(),
        ResolveError::UnexpectedObjectType {
            expected: ExpectedObject::Directory,
        }
    );
}

#[test]
fn operation_owner_resolves_and_accumulates_a_binding_before_freeze() {
    let root = tempfile::tempdir().expect("temporary owner path root");
    let project = root.path().join("Projeto.myalbum");
    std::fs::write(&project, b"project").expect("the project fixture is writable");
    let mut owner = OperationPathContext::new();

    let opened = owner
        .resolve_existing(&project, ExpectedObject::RegularFile)
        .expect("the owner captures and opens the project in one operation");
    assert_eq!(opened.logical_path(), project);

    let plan = owner.freeze();
    assert!(plan.covers(&project));
}

#[test]
fn prepares_a_new_file_from_its_parent_handle_and_verifies_the_created_child() {
    let root = tempfile::tempdir().expect("temporary destination root");
    let logical_target = if cfg!(windows) {
        std::path::PathBuf::from(r"R:\Álbuns\Novo Projeto.myalbuns")
    } else {
        std::path::PathBuf::from("/Álbuns/Novo Projeto.myalbuns")
    };
    let operational_parent = root.path().join("Álbuns");
    std::fs::create_dir(&operational_parent).expect("the authorized parent exists");

    let mut owner = OperationPathContext::new();
    owner
        .capture_with_binding(&logical_target, root.path())
        .expect("the logical root is frozen to the fixture root");
    let destination = owner
        .freeze()
        .prepare_file_destination(&logical_target)
        .expect("the new file destination is prepared from its existing parent");

    assert_eq!(destination.logical_path(), logical_target);
    assert_eq!(
        destination.operational_path(),
        operational_parent.join("Novo Projeto.myalbuns")
    );
    let temporary = destination.sibling_temporary_path();
    assert_eq!(temporary.parent(), destination.operational_path().parent());
    assert_ne!(temporary, destination.operational_path());
    assert!(
        temporary
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".myalbuns-create-") && name.ends_with(".tmp"))
    );
    assert!(
        destination
            .resolve_existing()
            .expect("the missing child is conclusive")
            .is_none()
    );

    std::fs::write(destination.operational_path(), b"project")
        .expect("the direct child is materialized");
    let created = destination
        .resolve_created()
        .expect("the final handle proves physical containment");
    assert_eq!(created.object_type(), ExpectedObject::RegularFile);
    assert_eq!(
        created.read_to_string().expect("the child is readable"),
        "project"
    );
}

#[test]
fn reads_a_regular_file_through_the_resolved_handle_after_path_replacement() {
    let root = tempfile::tempdir().expect("temporary handle-bound read root");
    let project = root.path().join("Projeto.myalbum");
    let archived = root.path().join("Projeto-anterior.myalbum");
    let replacement = root.path().join("Projeto-novo.myalbum");
    std::fs::write(&project, "revisão original").expect("the original fixture is writable");
    std::fs::write(&replacement, "revisão substituta")
        .expect("the replacement fixture is writable");

    let mut owner = OperationPathContext::new();
    let resolved = owner
        .resolve_existing(&project, ExpectedObject::RegularFile)
        .expect("the original Project resolves by handle");
    std::fs::rename(&project, &archived).expect("the original path can move while shared");
    std::fs::rename(&replacement, &project).expect("the replacement takes the original path");

    assert_eq!(
        resolved
            .read_to_string()
            .expect("the resolved handle remains readable"),
        "revisão original",
        "reading a resolved object must not follow a later pathname replacement"
    );
}

#[cfg(windows)]
#[test]
fn distinguishes_unbound_missing_and_unsupported_windows_paths() {
    let root = tempfile::tempdir().expect("temporary typed-error root");
    let existing = root.path().join("Projeto.myalbum");
    let missing = root.path().join("Ausente.myalbum");
    std::fs::write(&existing, b"project").expect("the Project fixture is writable");
    let mut owner = OperationPathContext::new();
    owner
        .capture(&existing)
        .expect("the local root is captured");
    let plan = owner.freeze();

    assert_eq!(
        plan.resolve_existing(&missing, ExpectedObject::RegularFile)
            .unwrap_err(),
        ResolveError::NotFound
    );
    assert_eq!(
        plan.resolve_existing(
            std::path::Path::new(r"\\.\C:\Projeto.myalbum"),
            ExpectedObject::RegularFile,
        )
        .unwrap_err(),
        ResolveError::UnsupportedNamespace
    );
    assert_eq!(
        plan.resolve_existing(
            std::path::Path::new(r"D:\Projeto.myalbum"),
            ExpectedObject::RegularFile,
        )
        .unwrap_err(),
        ResolveError::UnboundRoot
    );
}

#[test]
fn compares_physical_identity_by_open_handles_and_keeps_failures_indeterminate() {
    let root = tempfile::tempdir().expect("temporary identity root");
    let project = root.path().join("Álbum.myalbum");
    let alias = root.path().join("Atalho.myalbum");
    let other = root.path().join("Outro.myalbum");
    let missing = root.path().join("Ausente.myalbum");
    std::fs::write(&project, b"project").expect("the project fixture is writable");
    std::fs::hard_link(&project, &alias).expect("the physical alias is materialized");
    std::fs::write(&other, b"other").expect("the distinct fixture is writable");

    let mut owner = OperationPathContext::new();
    owner
        .capture(&project)
        .expect("the operation owner captures the native root");
    let plan = owner.freeze();

    assert_eq!(
        plan.compare_existing(&project, &project, ExpectedObject::RegularFile),
        PhysicalIdentityEvidence::Same
    );
    assert_eq!(
        plan.compare_existing(&project, &alias, ExpectedObject::RegularFile),
        PhysicalIdentityEvidence::Same
    );
    assert_eq!(
        plan.compare_existing(&project, &other, ExpectedObject::RegularFile),
        PhysicalIdentityEvidence::Different
    );
    assert_eq!(
        plan.compare_existing(&project, &missing, ExpectedObject::RegularFile),
        PhysicalIdentityEvidence::Indeterminate
    );
}

#[cfg(windows)]
#[test]
fn opens_and_publishes_beyond_the_legacy_windows_path_limit() {
    let root = tempfile::tempdir().expect("temporary long-path root");
    let mut destination = root.path().to_path_buf();
    for index in 0..9 {
        destination.push(format!("segmento-não-ascii-{index:02}-complementar"));
    }
    assert!(
        destination.as_os_str().len() > 260,
        "the fixture must cross the legacy MAX_PATH boundary"
    );
    std::fs::create_dir_all(&destination).expect("the long destination is materialized");
    let source = destination.join("Foto de origem.jpg");
    std::fs::write(&source, b"photo").expect("the long source is writable");

    let mut owner = OperationPathContext::new();
    owner
        .capture(&source)
        .expect("the long native root is captured");
    let plan = owner.freeze();
    plan.resolve_existing(&source, ExpectedObject::RegularFile)
        .expect("the long source opens through the frozen plan");

    let output = destination.join("Álbum exportado.png");
    let export =
        ExportPathPlan::new(output.clone(), "long-path").expect("the long Export path is planned");
    let prepared = export
        .prepare()
        .expect("staging is reserved inside the long destination");
    std::fs::write(export.prepared_output_path(), b"published")
        .expect("the long preparation is writable");
    prepared
        .publish()
        .expect("the long preparation is published");

    assert_eq!(
        std::fs::read(output).expect("the long output remains readable"),
        b"published"
    );
}
