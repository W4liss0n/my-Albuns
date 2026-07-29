use std::{io::Write, path::Path};

use directories::BaseDirs;
use myalbuns_paths::{AppPaths, AppPathsError};

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
    assert_eq!(paths.logs_dir(), Path::new(r"C:\Local\MyAlbuns2\Logs"));
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
