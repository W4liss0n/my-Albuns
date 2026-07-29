use std::path::Path;

use directories::BaseDirs;
use myalbuns_paths::AppPaths;

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
