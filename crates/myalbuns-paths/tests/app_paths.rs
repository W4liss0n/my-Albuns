use std::{
    io::Write,
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use myalbuns_paths::{AppPaths, AppPathsError, ExportPathPlan, OperationPathContext, PathRootKind};

#[test]
fn derives_temporary_application_roots_from_known_folders() {
    let paths = AppPaths::from_known_folders(
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
    let paths = AppPaths::from_known_folders(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

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
fn derives_only_safe_webview_host_namespaces() {
    let paths = AppPaths::from_known_folders(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

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
fn derives_only_safe_project_cache_namespaces() {
    let paths = AppPaths::from_known_folders(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));

    let cache = paths
        .project_cache("project-01.ABC")
        .expect("a safe opaque identity is accepted");
    assert_eq!(
        cache.media_directory(),
        Path::new(r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media")
    );
    assert_eq!(
        cache
            .preview_file("media-001", "0123456789abcdef-v1-1600")
            .expect("safe artifact identities are accepted"),
        Path::new(
            r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media\media-001.0123456789abcdef-v1-1600.jpg"
        )
    );
    assert_eq!(
        cache
            .preview_temporary_file("media-001", "0123456789abcdef-v1-1600", 42)
            .expect("temporary names are derived by the path module"),
        Path::new(
            r"C:\Local\MyAlbuns2\Cache\project-01.ABC\Media\media-001.0123456789abcdef-v1-1600.jpg.tmp-42"
        )
    );
    assert_eq!(
        cache.metadata_file(),
        Path::new(r"C:\Local\MyAlbuns2\Cache\project-01.ABC\metadata.json")
    );
    for unsafe_artifact in ["", "../escape", r"nested\escape", "a.b", "álbum", "CON"] {
        assert!(
            cache
                .preview_file(unsafe_artifact, "generation-01")
                .is_err(),
            "{unsafe_artifact:?} must not become an artifact path component"
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
fn exposes_cache_artifacts_as_scoped_urls_instead_of_lossy_paths() {
    let paths = AppPaths::from_known_folders(Path::new(r"C:\Roaming"), Path::new(r"C:\Local"));
    let preview = paths
        .project_cache("project-01")
        .expect("project namespace is safe")
        .preview_file("media-001", "0123456789abcdef-v1-1600")
        .expect("artifact identity is safe");

    assert_eq!(
        paths
            .cache_asset_url(&preview)
            .expect("the authorized Cache path becomes an asset URL"),
        "http://asset.localhost/C%3A%5CLocal%5CMyAlbuns2%5CCache%5Cproject-01%5CMedia%5Cmedia-001.0123456789abcdef-v1-1600.jpg"
    );
    assert!(
        paths
            .cache_asset_url(Path::new(r"C:\Photos\private.jpg"))
            .is_err()
    );
    assert!(
        paths
            .cache_asset_url(Path::new(r"C:\Local\MyAlbuns2\Cache\..\private.jpg"))
            .is_err()
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
    let plan =
        ExportPathPlan::new(output.clone(), "export-success").expect("the Export plan is valid");
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
    let paths = AppPaths::from_known_folders(root.path(), root.path());
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
fn rejects_a_cache_plan_from_another_local_data_root() {
    let authorized_root = tempfile::tempdir().expect("authorized LocalAppData root");
    let other_root = tempfile::tempdir().expect("different LocalAppData root");
    let paths = AppPaths::from_known_folders(authorized_root.path(), authorized_root.path());
    let other_paths = AppPaths::from_known_folders(other_root.path(), other_root.path());
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
    let paths = AppPaths::from_known_folders(root.path(), root.path());
    let cache = paths
        .project_cache("project-01")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let temporary = cache
        .preview_temporary_file("media-01", "generation-01", 42)
        .expect("the temporary path is valid");
    let published = cache
        .preview_file("media-01", "generation-01")
        .expect("the published path is valid");

    let mut file = storage
        .create_temporary_file(&temporary)
        .expect("the temporary file is created without following aliases");
    file.write_all(b"preview").expect("the preview is written");
    file.sync_all().expect("the preview is synchronized");
    drop(file);
    storage
        .replace_file(&temporary, &published)
        .expect("the exact temporary file is published");

    assert_eq!(
        std::fs::read(&published).expect("the published preview is readable"),
        b"preview"
    );
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
fn discards_only_cache_temporaries_left_by_a_terminated_processor() {
    let root = tempfile::tempdir().expect("temporary LocalAppData root");
    let paths = AppPaths::from_known_folders(root.path(), root.path());
    let cache = paths
        .project_cache("project-recovery")
        .expect("the Cache plan is valid");
    let storage = paths
        .prepare_cache_storage(&cache)
        .expect("the Cache directory chain is held");
    let published = cache
        .preview_file("media-01", "generation-01")
        .expect("the published path is valid");
    let preview_temporary = cache
        .preview_temporary_file("media-01", "generation-02", 4242)
        .expect("the preview temporary path is valid");
    let other_process_temporary = cache
        .preview_temporary_file("media-01", "generation-03", 4343)
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
        let mut file = storage
            .create_temporary_file(temporary)
            .expect("the stale temporary is materialized safely");
        file.write_all(b"incomplete")
            .expect("the stale temporary is writable");
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
    let paths = AppPaths::from_known_folders(root.path(), root.path());
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
