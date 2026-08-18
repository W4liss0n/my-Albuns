use std::{
    collections::HashSet,
    io::Write,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use myalbuns_paths::{
    AppPaths, AppPathsError, CacheArtifactFormat, ExportPathPlan, ExportWriteAuthorization,
    OperationPathContext, PathRootKind, RootBindingPlan, project_data_namespace,
};

#[test]
fn derives_application_roots_from_injected_known_folders() {
    let paths = AppPaths::from_roots(
        Path::new(r"C:\Users\Pessoa\AppData\Roaming"),
        Path::new(r"C:\Users\Pessoa\AppData\Local"),
    );

    assert_eq!(
        paths.roaming_root(),
        Path::new(r"C:\Users\Pessoa\AppData\Roaming\MyAlbuns2")
    );
    assert_eq!(
        paths.local_root(),
        Path::new(r"C:\Users\Pessoa\AppData\Local\MyAlbuns2")
    );
}

#[test]
fn exposes_each_data_category_under_its_approved_root() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    assert_eq!(
        paths.settings_file(),
        Path::new(r"C:\Roaming\MyAlbuns2\settings.json")
    );
    assert_eq!(
        paths.layouts_dir(),
        Path::new(r"C:\Roaming\MyAlbuns2\Layouts")
    );
    assert_eq!(paths.cache_dir(), Path::new(r"C:\Local\MyAlbuns2\Cache"));
    assert_eq!(
        paths.recovery_dir(),
        Path::new(r"C:\Local\MyAlbuns2\Recovery")
    );
    assert_eq!(paths.state_dir(), Path::new(r"C:\Local\MyAlbuns2\State"));
    assert_eq!(
        paths
            .webview_data_directory("project-host-01")
            .expect("the host namespace is safe"),
        Path::new(r"C:\Local\MyAlbuns2\State\WebView2\project-host-01")
    );
    assert_eq!(paths.logs_dir(), Path::new(r"C:\Local\MyAlbuns2\Logs"));
}

#[test]
fn derives_the_recent_projects_state_file_from_the_central_local_root() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    assert_eq!(
        paths.recent_projects_file(),
        Path::new(r"C:\Local\MyAlbuns2\State\recent-projects.json")
    );
}

#[test]
fn derives_the_project_identity_lease_root_from_the_central_local_state() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    assert_eq!(
        paths.project_identity_leases_dir(),
        Path::new(r"C:\Local\MyAlbuns2\State\ProjectIdentityLeases")
    );
}

#[test]
fn derives_only_safe_webview_host_namespaces() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    for unsafe_namespace in [
        "",
        ".",
        "..",
        "../escape",
        r"nested\escape",
        "C:escape",
        "álbum",
        "CON",
        "trailing.",
    ] {
        assert_eq!(
            paths.webview_data_directory(unsafe_namespace).unwrap_err(),
            AppPathsError::InvalidStateNamespace,
            "{unsafe_namespace:?} must not become a WebView data namespace"
        );
    }
}

#[test]
fn project_identity_derives_stable_isolated_internal_namespaces() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));
    let first = project_data_namespace("project-01");

    assert_eq!(
        first,
        "project-deea8d493d4e6f5453e54b288cf92a4a91bd32c5dbc87f2150cfb33580e9f9cb"
    );
    assert_eq!(first, project_data_namespace("project-01"));
    assert_ne!(first, project_data_namespace("project-02"));

    for project_id in ["../escape", "CON", "album com espaco", "\u{00e1}lbum"] {
        let namespace = project_data_namespace(project_id);
        paths
            .project_cache(&namespace)
            .expect("an opaque Project namespace is safe for Cache");
        paths
            .webview_data_directory(&namespace)
            .expect("an opaque Project namespace is safe for WebView2");
    }
}

#[test]
fn derives_only_safe_project_cache_namespaces() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    let cache = paths
        .project_cache("project-01.ABC")
        .expect("a safe opaque identity is accepted");
    assert_eq!(
        cache.media_directory(),
        Path::new(r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media")
    );
    assert_eq!(
        cache
            .preview_file(
                "media-001",
                "0123456789abcdef-v1-1600",
                CacheArtifactFormat::Jpeg,
            )
            .expect("safe artifact identities are accepted"),
        Path::new(
            r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media\media-ddedb0a5b1fd0e11bd569d4b06eec63d02c0e5a272186ce3e2ef6529439afafa.0123456789abcdef-v1-1600.jpg"
        )
    );
    assert_eq!(
        cache
            .preview_temporary_file(
                "media-001",
                "0123456789abcdef-v1-1600",
                CacheArtifactFormat::Jpeg,
                42,
            )
            .expect("temporary names are derived by the path module"),
        Path::new(
            r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media\media-ddedb0a5b1fd0e11bd569d4b06eec63d02c0e5a272186ce3e2ef6529439afafa.0123456789abcdef-v1-1600.jpg.tmp-42"
        )
    );
    assert_eq!(
        cache
            .preview_file(
                "decorative-001",
                "0123456789abcdef-v1-1600",
                CacheArtifactFormat::Png,
            )
            .expect("safe PNG artifact identities are accepted"),
        Path::new(
            r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media\media-12a79c4913ad160c8f60a357adb14fa9ea6a07a156d2feee32b8312e9c00da19.0123456789abcdef-v1-1600.png"
        )
    );
    assert_eq!(
        cache.metadata_file(),
        Path::new(r"C:\Local\MyAlbuns2\Cache\project-01.ABC\metadata.json")
    );
    assert!(
        cache
            .preview_file("", "generation-01", CacheArtifactFormat::Jpeg)
            .is_err(),
        "an empty Media identity is invalid"
    );
    for opaque_media_id in [
        "../escape",
        r"nested\escape",
        "a.b",
        "album com espaco",
        "CON",
    ] {
        let artifact = cache
            .preview_file(opaque_media_id, "generation-01", CacheArtifactFormat::Jpeg)
            .expect("a Media identity is converted to an opaque artifact key");
        assert!(
            artifact
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.starts_with("media-")),
            "{opaque_media_id:?} must be represented by an opaque artifact key"
        );
    }
    for unsafe_generation in ["", "../escape", r"nested\escape", "a.b", "CON"] {
        assert!(
            cache
                .preview_file("media-01", unsafe_generation, CacheArtifactFormat::Jpeg)
                .is_err(),
            "{unsafe_generation:?} must not become a generation path component"
        );
    }
    for unsafe_namespace in [
        "",
        ".",
        "..",
        "../escape",
        r"nested\escape",
        "C:escape",
        "álbum",
        "CON",
        "lpt1.preview",
        "trailing.",
    ] {
        assert!(
            paths.project_cache(unsafe_namespace).is_err(),
            "{unsafe_namespace:?} must not become a Cache path component"
        );
    }
}

#[test]
fn validates_cache_artifacts_without_choosing_a_transport_protocol() {
    let paths = AppPaths::from_roots(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));
    let preview = paths
        .project_cache("project-01")
        .expect("project namespace is safe")
        .preview_file(
            "media-001",
            "0123456789abcdef-v1-1600",
            CacheArtifactFormat::Jpeg,
        )
        .expect("artifact identity is safe");

    paths
        .validate_cache_artifact(&preview)
        .expect("the artifact belongs to the authorized Cache root");
    assert_eq!(
        paths
            .validate_cache_artifact(Path::new(r"C:\Photos\private.jpg"))
            .unwrap_err(),
        AppPathsError::CacheArtifactOutsideRoot
    );
    assert_eq!(
        paths
            .validate_cache_artifact(Path::new(r"C:\Local\MyAlbuns2\Cache\..\private.jpg"))
            .unwrap_err(),
        AppPathsError::CacheArtifactOutsideRoot
    );
}

#[test]
fn discovers_roaming_and_local_roots_from_the_operating_system() {
    let known_folders =
        BaseDirs::new().expect("the test environment must expose user data folders");

    let paths = AppPaths::discover().expect("AppPaths must discover the same folders");

    assert_eq!(
        paths.roaming_root(),
        known_folders.data_dir().join("MyAlbuns2")
    );
    assert_eq!(
        paths.local_root(),
        known_folders.data_local_dir().join("MyAlbuns2")
    );
}

#[test]
fn derives_a_unique_export_preparation_inside_the_destination() {
    let destination = tempfile::tempdir().expect("temporary Export destination");
    let output = destination.path().join("Álbum 01.png");

    let plan =
        ExportPathPlan::new(output.clone(), "export-42").expect("the Export path plan is valid");

    assert_eq!(plan.output_path(), output);
    assert_eq!(
        plan.preparation_directory(),
        destination.path().join(".myalbuns-export-export-42.tmp")
    );
    assert_eq!(
        plan.prepared_output_path(),
        destination
            .path()
            .join(".myalbuns-export-export-42.tmp")
            .join("Álbum 01.png")
    );
}

#[cfg(windows)]
#[test]
fn rejects_prohibited_windows_export_paths_before_planning() {
    for path in [
        r"relative\Album.png",
        r"C:Album.png",
        r"\Album.png",
        r"\\.\C:\Exports\Album.png",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\Album.png",
        r"C:\Exports\Album*.png",
        r"C:\Exports\Album.png:stream",
        r"C:\Exports\CON.png",
        r"C:\Exports\folder.\Album.png",
    ] {
        assert_eq!(
            ExportPathPlan::new(PathBuf::from(path), "export-safe").unwrap_err(),
            AppPathsError::InvalidExportPath,
            "{path:?} must be rejected before any filesystem operation"
        );
    }
}

#[cfg(windows)]
#[test]
fn accepts_supported_windows_export_path_forms() {
    for path in [
        r"C:\Exports\Album.png",
        r"\\server\share\Album.png",
        r"\\?\C:\long\Album.png",
        r"\\?\UNC\server\share\Album.png",
    ] {
        ExportPathPlan::new(PathBuf::from(path), "export-safe")
            .unwrap_or_else(|error| panic!("{path:?} must be supported: {error}"));
    }
}

#[cfg(windows)]
#[test]
fn freezes_one_binding_per_logical_root_for_an_operation() {
    let first = Path::new(r"Z:\Álbuns\Casamento\foto-01.jpg");
    let second = Path::new(r"Z:\Álbuns\Casamento\foto-02.jpg");
    let network = Path::new(r"\\servidor\acervo\foto-03.jpg");
    let mut context = OperationPathContext::new();

    context
        .capture(first)
        .expect("the mapped-drive source is representable");
    context
        .capture(second)
        .expect("the same logical root reuses its binding");
    context
        .capture(network)
        .expect("the UNC source is representable");
    let bindings = context.freeze();

    assert_eq!(bindings.bindings().len(), 2);
    assert_eq!(bindings.bindings()[0].kind(), PathRootKind::Disk);
    assert_eq!(bindings.bindings()[1].kind(), PathRootKind::Unc);
    assert_eq!(
        bindings
            .resolve(first)
            .expect("the frozen plan resolves the captured path"),
        first
    );
    assert_eq!(
        bindings
            .resolve(Path::new(r"z:\Álbuns\Casamento\foto-04.jpg"))
            .expect("drive-letter casing does not create another root"),
        Path::new(r"Z:\Álbuns\Casamento\foto-04.jpg")
    );
}

#[cfg(windows)]
#[test]
fn a_frozen_binding_plan_refuses_an_uncaptured_root() {
    let mut context = OperationPathContext::new();
    context
        .capture(Path::new(r"C:\Álbuns\foto.jpg"))
        .expect("the source root is captured");
    let bindings = context.freeze();

    assert_eq!(
        bindings
            .resolve(Path::new(r"D:\Outro\foto.jpg"))
            .unwrap_err(),
        AppPathsError::PathRootNotBound
    );
}

#[cfg(windows)]
#[test]
fn a_mapped_drive_binding_keeps_its_unc_target_for_the_whole_attempt() {
    let logical = Path::new(r"Z:\Álbuns\Casamento\foto-01.jpg");
    let mut context = OperationPathContext::new();
    context
        .capture_with_binding(logical, Path::new(r"\\servidor\acervo\"))
        .expect("the platform-resolved mapped-drive binding is captured");
    let bindings = context.freeze();

    assert_eq!(
        bindings
            .resolve(Path::new(r"Z:\Álbuns\Casamento\foto-02.jpg"))
            .expect("the logical suffix is applied to the frozen UNC root"),
        Path::new(r"\\servidor\acervo\Álbuns\Casamento\foto-02.jpg")
    );
    assert_eq!(bindings.bindings()[0].kind(), PathRootKind::Disk);
}

#[cfg(windows)]
#[test]
fn a_mapped_drive_can_bind_to_a_directory_below_the_unc_share_root() {
    let logical = Path::new(r"Z:\Casamento\foto-01.jpg");
    let mut context = OperationPathContext::new();
    context
        .capture_with_binding(logical, Path::new(r"\\servidor\acervo\2026\cliente"))
        .expect("a mapped drive may target a directory inside its UNC share");
    let bindings = context.freeze();

    assert_eq!(
        bindings
            .resolve(Path::new(r"Z:\Casamento\foto-02.jpg"))
            .expect("the suffix is applied below the captured UNC base"),
        Path::new(r"\\servidor\acervo\2026\cliente\Casamento\foto-02.jpg")
    );
}

#[cfg(windows)]
#[test]
fn root_binding_plan_round_trips_native_windows_paths_without_loss() {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };

    let mut native_units = r"\\server\share\opaque-".encode_utf16().collect::<Vec<_>>();
    native_units.push(0xd800);
    let operational_base = PathBuf::from(OsString::from_wide(&native_units));
    let logical_path = PathBuf::from(r"R:\Photo.jpg");
    let mut context = OperationPathContext::new();
    context
        .capture_with_binding(&logical_path, &operational_base)
        .expect("an opaque native Windows binding component is accepted");
    let original = context.freeze();

    let wire = serde_json::to_vec(&original).expect("the native path has a reversible wire form");
    assert!(
        !String::from_utf8_lossy(&wire).contains('\u{fffd}'),
        "the wire representation must not substitute native units"
    );
    let restored: RootBindingPlan =
        serde_json::from_slice(&wire).expect("the reversible native path wire form decodes");
    assert_eq!(
        restored.bindings()[0]
            .operational_root()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        native_units
    );
    let resolved = restored
        .resolve(&logical_path)
        .expect("the restored plan still binds the exact native path");

    assert_eq!(resolved, operational_base.join("Photo.jpg"));
}

#[test]
fn discarding_an_export_preparation_preserves_the_previous_output() {
    let destination = tempfile::tempdir().expect("temporary Export destination");
    let output = destination.path().join("Álbum 01.png");
    std::fs::write(&output, b"previous export").expect("the previous Export is writable");
    let plan =
        ExportPathPlan::new(output.clone(), "export-failed").expect("the Export plan is valid");
    let preparation = plan
        .prepare()
        .expect("the Export preparation is reserved safely");
    std::fs::write(plan.prepared_output_path(), b"incomplete export")
        .expect("the processor can write its preparation");

    assert!(
        preparation
            .discard()
            .expect("the exact Export preparation is discarded")
    );
    assert_eq!(
        std::fs::read(output).expect("the previous Export remains readable"),
        b"previous export"
    );
    assert!(!plan.preparation_directory().exists());
}

#[test]
fn publishing_a_verified_preparation_replaces_the_previous_output() {
    let destination = tempfile::tempdir().expect("temporary Export destination");
    let output = destination.path().join("Álbum 01.png");
    std::fs::write(&output, b"previous export").expect("the previous Export is writable");
    let plan = ExportPathPlan::new_authorized(
        output.clone(),
        "export-success",
        ExportWriteAuthorization::ReplaceConfirmed,
    )
    .expect("the Export plan is valid");
    let preparation = plan
        .prepare()
        .expect("the Export preparation is reserved safely");
    std::fs::write(plan.prepared_output_path(), b"verified export")
        .expect("the verified preparation is writable");

    preparation
        .publish()
        .expect("the verified preparation is published");

    assert_eq!(
        std::fs::read(output).expect("the published Export is readable"),
        b"verified export"
    );
    assert!(!plan.preparation_directory().exists());
}

#[test]
fn create_only_publication_never_reinfers_permission_to_replace() {
    let destination = tempfile::tempdir().expect("temporary Export destination");
    let output = destination.path().join("Álbum 01.jpg");
    std::fs::write(&output, b"external final").expect("the conflicting final is writable");
    let plan = ExportPathPlan::new_authorized(
        output.clone(),
        "export-create-only",
        ExportWriteAuthorization::CreateOnly,
    )
    .expect("the CreateOnly Export plan is valid");
    assert_eq!(plan.authorization(), ExportWriteAuthorization::CreateOnly);
    let preparation = plan
        .prepare()
        .expect("the Export preparation is reserved safely");
    std::fs::write(plan.prepared_output_path(), b"verified export")
        .expect("the verified preparation is writable");

    assert_eq!(
        preparation.publish().unwrap_err(),
        AppPathsError::ExportTargetConflict
    );
    assert_eq!(
        std::fs::read(&output).expect("the external final remains readable"),
        b"external final"
    );
    assert!(!plan.preparation_directory().exists());
}

#[test]
fn a_publication_failure_discards_only_its_preparation() {
    let destination = tempfile::tempdir().expect("temporary Export destination");
    let output = destination.path().join("Album.png");
    std::fs::create_dir(&output).expect("the conflicting final target is a directory");
    let plan =
        ExportPathPlan::new(output.clone(), "export-conflict").expect("the Export plan is valid");
    let preparation = plan
        .prepare()
        .expect("the Export preparation is reserved safely");
    std::fs::write(plan.prepared_output_path(), b"verified export")
        .expect("the verified preparation is writable");

    assert_eq!(
        preparation.publish().unwrap_err(),
        AppPathsError::ExportStorageOutsideDestination
    );
    assert!(output.is_dir());
    assert!(!plan.preparation_directory().exists());
}

#[test]
fn prepares_the_cache_only_as_directories_below_the_authorized_root() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-01")
        .expect("the Cache plan is valid");

    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is created and held");

    assert!(cache.media_directory().is_dir());
    drop(storage);
}

#[test]
fn inspects_only_direct_cache_namespaces_and_measures_their_files() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let first = paths
        .project_cache("project-first")
        .expect("the first Cache plan is valid");
    let second = paths
        .project_cache("project-second")
        .expect("the second Cache plan is valid");
    let first_storage = paths
        .prepare_cache_storage(&first)
        .expect("the first namespace is prepared");
    let second_storage = paths
        .prepare_cache_storage(&second)
        .expect("the second namespace is prepared");
    let first_preview = first
        .preview_file("media-a", "generation-a", CacheArtifactFormat::Jpeg)
        .expect("the first preview path is valid");
    let second_preview = second
        .preview_file("media-b", "generation-b", CacheArtifactFormat::Png)
        .expect("the second preview path is valid");
    std::fs::write(first.metadata_file(), b"meta").expect("the first metadata is writable");
    std::fs::write(first_preview, b"preview-one").expect("the first preview is writable");
    std::fs::write(second_preview, b"preview-two-two").expect("the second preview is writable");
    drop(first_storage);
    drop(second_storage);

    let inspected = paths
        .inspect_cache_namespaces()
        .expect("the guarded Cache inspection succeeds");

    assert_eq!(inspected.len(), 2);
    assert_eq!(inspected[0].paths(), &first);
    assert_eq!(inspected[0].bytes(), 15);
    assert_eq!(inspected[1].paths(), &second);
    assert_eq!(inspected[1].bytes(), 15);
}

#[test]
fn rejects_a_cache_plan_from_another_local_data_root() {
    let authorized_root = tempfile::tempdir().expect("authorized LocalAppData root");
    let other_root = tempfile::tempdir().expect("different LocalAppData root");
    let paths = AppPaths::from_roots(authorized_root.path(), authorized_root.path());
    let other_paths = AppPaths::from_roots(other_root.path(), other_root.path());
    let other_cache = other_paths
        .project_cache("project-01")
        .expect("the other Cache plan is structurally valid");

    assert_eq!(
        paths.prepare_cache_storage(&other_cache).unwrap_err(),
        AppPathsError::CacheStorageOutsideRoot
    );
}

#[test]
fn creates_and_publishes_cache_files_below_the_held_directory() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-01")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let temporary = cache
        .preview_temporary_file("media-01", "generation-01", CacheArtifactFormat::Jpeg, 42)
        .expect("the temporary path is valid");
    let published = cache
        .preview_file("media-01", "generation-01", CacheArtifactFormat::Jpeg)
        .expect("the published path is valid");

    let mut publication = storage
        .begin_file_publication(&temporary, &published)
        .expect("the Cache publication begins without following aliases");
    publication
        .write_all(b"preview")
        .expect("the preview is written");
    publication
        .sync()
        .expect("the preview is synchronized")
        .publish()
        .expect("the exact temporary file is published");

    assert_eq!(
        std::fs::read(&published).expect("the published preview is readable"),
        b"preview"
    );
    assert!(!temporary.exists());
    drop(storage);
    assert!(
        paths
            .clear_project_cache(&cache)
            .expect("the native Cache cleanup succeeds")
    );
    assert!(!published.exists());
    assert!(
        !paths
            .clear_project_cache(&cache)
            .expect("clearing an absent Cache is idempotent")
    );
}

#[test]
fn dropping_a_synchronized_cache_file_discards_only_its_temporary() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-incomplete")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let temporary = cache
        .preview_temporary_file("media-01", "generation-01", CacheArtifactFormat::Jpeg, 42)
        .expect("the temporary path is valid");
    let published = cache
        .preview_file("media-01", "generation-01", CacheArtifactFormat::Jpeg)
        .expect("the published path is valid");
    std::fs::write(&published, b"previous preview")
        .expect("the previous Cache artifact is writable");

    let mut publication = storage
        .begin_file_publication(&temporary, &published)
        .expect("the Cache publication begins safely");
    publication
        .write_all(b"incomplete preview")
        .expect("the temporary preview is writable");
    let synchronized = publication
        .sync()
        .expect("the temporary preview is synchronized");
    drop(synchronized);

    assert!(!temporary.exists());
    assert_eq!(
        std::fs::read(published).expect("the previous Cache artifact remains"),
        b"previous preview"
    );
}

#[test]
fn removes_only_one_validated_cache_artifact_from_the_held_namespace() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-targeted-cleanup")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let removed = cache
        .preview_file("media-01", "generation-old", CacheArtifactFormat::Jpeg)
        .expect("the old generation path is valid");
    let preserved = cache
        .preview_file("media-02", "generation-live", CacheArtifactFormat::Png)
        .expect("the live generation path is valid");
    std::fs::write(&removed, b"old").expect("the old generation is writable");
    std::fs::write(&preserved, b"live").expect("the live generation is writable");

    assert!(
        storage
            .remove_existing_file(&removed)
            .expect("the targeted generation is removed")
    );
    assert!(preserved.is_file());
    assert!(
        !storage
            .remove_existing_file(&removed)
            .expect("targeted removal is idempotent")
    );
    assert_eq!(
        storage
            .remove_existing_file(root.path().join("outside.jpg").as_path())
            .unwrap_err(),
        AppPathsError::CacheStorageOutsideRoot
    );
}

#[test]
fn sweeps_only_unreferenced_final_generations_below_the_held_media_directory() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-orphan-sweep")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let orphan = cache
        .preview_file("media-01", "generation-orphan", CacheArtifactFormat::Jpeg)
        .expect("the orphan path is valid");
    let referenced = cache
        .preview_file("media-02", "generation-live", CacheArtifactFormat::Png)
        .expect("the live path is valid");
    let temporary = cache
        .preview_temporary_file(
            "media-03",
            "generation-in-flight",
            CacheArtifactFormat::Jpeg,
            4_242,
        )
        .expect("the in-flight path is valid");
    std::fs::write(&orphan, b"orphan").expect("the orphan is writable");
    std::fs::write(&referenced, b"live").expect("the live generation is writable");
    std::fs::write(&temporary, b"partial").expect("the temporary is writable");

    assert_eq!(
        storage
            .remove_unreferenced_generations(&HashSet::from([referenced.clone()]))
            .expect("the orphan sweep is contained"),
        1
    );
    assert!(!orphan.exists());
    assert!(referenced.is_file());
    assert!(temporary.is_file(), "recovery owns processor temporaries");
}

#[test]
fn rejects_using_the_same_cache_path_as_temporary_and_final() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-same-path")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let published = cache
        .preview_file("media-01", "generation-01", CacheArtifactFormat::Jpeg)
        .expect("the published path is valid");

    assert!(matches!(
        storage.begin_file_publication(&published, &published),
        Err(AppPathsError::CacheStorageOutsideRoot)
    ));
}

#[test]
fn discards_only_cache_temporaries_left_by_a_terminated_processor() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-recovery")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let published = cache
        .preview_file("media-01", "generation-01", CacheArtifactFormat::Jpeg)
        .expect("the published path is valid");
    let preview_temporary = cache
        .preview_temporary_file("media-01", "generation-02", CacheArtifactFormat::Png, 4242)
        .expect("the preview temporary path is valid");
    let other_process_temporary = cache
        .preview_temporary_file("media-01", "generation-03", CacheArtifactFormat::Jpeg, 4343)
        .expect("the other process temporary path is valid");
    let metadata = cache.metadata_file();
    let metadata_temporary = cache.metadata_temporary_file(4242);

    std::fs::write(&published, b"published preview").expect("the published preview is writable");
    std::fs::write(&metadata, b"{\"schemaVersion\":1}")
        .expect("the published metadata is writable");
    for temporary in [
        &preview_temporary,
        &other_process_temporary,
        &metadata_temporary,
    ] {
        std::fs::write(temporary, b"incomplete").expect("the stale temporary is materialized");
    }
    drop(storage);

    assert_eq!(
        paths
            .discard_project_cache_temporaries(&cache, 4242)
            .expect("stale temporaries are discarded safely"),
        2
    );
    assert_eq!(
        std::fs::read(&published).expect("the published preview remains"),
        b"published preview"
    );
    assert_eq!(
        std::fs::read(&metadata).expect("the published metadata remains"),
        b"{\"schemaVersion\":1}"
    );
    assert!(!preview_temporary.exists());
    assert!(!metadata_temporary.exists());
    assert_eq!(
        std::fs::read(other_process_temporary)
            .expect("another process temporary remains untouched"),
        b"incomplete"
    );
}

#[test]
fn rejects_a_cache_namespace_redirected_by_a_directory_link() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let external = tempfile::tempdir().expect("external directory");
    let paths = AppPaths::from_roots(root.path(), root.path());
    let cache = paths
        .project_cache("project-01")
        .expect("the Cache plan is valid");
    std::fs::create_dir_all(paths.cache_dir()).expect("the Cache parent exists");
    let project_directory = paths.cache_dir().join("project-01");
    if let Err(error) = create_directory_link(external.path(), &project_directory) {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(1314)
        {
            return;
        }
        panic!("the directory link could not be created: {error}");
    }

    assert_eq!(
        paths.prepare_cache_storage(&cache).unwrap_err(),
        AppPathsError::CacheStorageOutsideRoot
    );
}

#[cfg(windows)]
fn create_directory_link(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, destination)
}

#[cfg(unix)]
fn create_directory_link(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}
