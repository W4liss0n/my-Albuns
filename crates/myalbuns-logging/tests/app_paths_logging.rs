use std::ffi::OsString;

use myalbuns_logging::{ProcessRole, init_local_logging};
use myalbuns_paths::AppPaths;

#[test]
fn logging_materializes_only_the_logs_category_under_temporary_roots() {
    let roaming_data = tempfile::tempdir().expect("temporary roaming data root");
    let local_data = tempfile::tempdir().expect("temporary local data root");
    let app_paths = AppPaths::from_roots(roaming_data.path(), local_data.path());

    assert!(!app_paths.roaming_root().exists());
    assert!(!app_paths.local_root().exists());

    let logging_guard = init_local_logging(&app_paths.logs_dir(), ProcessRole::DesktopHost)
        .expect("logging must initialize under AppPaths");
    drop(logging_guard);

    assert!(app_paths.logs_dir().is_dir());
    assert!(!app_paths.roaming_root().exists());
    assert!(!app_paths.settings_file().exists());
    assert!(!app_paths.layouts_dir().exists());
    assert!(!app_paths.cache_dir().exists());
    assert!(!app_paths.recovery_dir().exists());
    assert!(!app_paths.state_dir().exists());

    let local_categories = std::fs::read_dir(app_paths.local_root())
        .expect("local application root must be readable")
        .map(|entry| entry.expect("local category must be readable").file_name())
        .collect::<Vec<_>>();
    assert_eq!(local_categories, [OsString::from("Logs")]);
}
