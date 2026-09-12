use std::{
    io,
    sync::atomic::{AtomicBool, Ordering},
};

use super::*;

#[derive(Default)]
struct Platform {
    candidates: Mutex<Vec<PathBuf>>,
    launched: Mutex<Vec<(PathBuf, PathBuf)>>,
    fail_launch: AtomicBool,
}

impl PhotoshopPlatform for Platform {
    fn candidates(&self) -> Vec<PathBuf> {
        self.candidates.lock().unwrap().clone()
    }
    fn inspect(&self, executable: &Path) -> Option<ExecutableInfo> {
        let version = serde_json::from_slice::<[u16; 4]>(&fs::read(executable).ok()?).ok()?;
        Some(ExecutableInfo {
            name: format!("Adobe Photoshop {}", version[0]),
            version,
        })
    }
    fn launch(&self, executable: &Path, original: &Path) -> io::Result<()> {
        if self.fail_launch.load(Ordering::SeqCst) {
            return Err(io::Error::other("launch rejected"));
        }
        self.launched
            .lock()
            .unwrap()
            .push((executable.into(), original.into()));
        Ok(())
    }
}

struct Fixture {
    root: tempfile::TempDir,
    paths: AppPaths,
    platform: Arc<Platform>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("Roaming"), &root.path().join("Local"));
        Self {
            root,
            paths,
            platform: Arc::new(Platform::default()),
        }
    }
    fn store(&self) -> PhotoshopStateStore {
        let mut store = PhotoshopStateStore::new(&self.paths);
        store.platform = self.platform.clone();
        store
    }
    fn install(&self, name: &str, version: [u16; 4], detected: bool) -> PathBuf {
        let path = self.root.path().join(name).join("Photoshop.exe");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_vec(&version).unwrap()).unwrap();
        if detected {
            self.platform.candidates.lock().unwrap().push(path.clone());
        }
        path
    }
    fn photo(&self, path: PathBuf) -> MediaBinding {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"original photo bytes").unwrap();
        MediaBinding {
            media_id: "photo-a".into(),
            kind: MediaKind::Photo,
            logical_path: path,
        }
    }
}

#[test]
fn newest_numeric_version_is_selected_and_a_different_choice_survives_new_hosts() {
    let fixture = Fixture::new();
    fixture.install("2026-old", [27, 9, 0, 0], true);
    let older = fixture.install("2025", [26, 12, 0, 0], true);
    fixture.install("2026", [27, 10, 0, 0], true);
    let store = fixture.store();
    let first = store.status().unwrap();
    assert_eq!(
        first
            .installations
            .iter()
            .map(|item| item.version.as_str())
            .collect::<Vec<_>>(),
        ["27.10.0.0", "27.9.0.0", "26.12.0.0"]
    );
    assert_eq!(
        first.selected_installation_id.as_deref(),
        Some(first.installations[0].id.as_str())
    );
    assert_eq!(first.revision, 1);
    let selected = store.select(&first.installations[2].id).unwrap();
    assert_eq!(selected.revision, 2);
    let reopened = fixture.store().status().unwrap();
    assert_eq!(selected, reopened);
    assert_eq!(
        fixture.store().load().unwrap().selected.unwrap().as_path(),
        older
    );
    assert!(
        fixture
            .paths
            .photoshop_file()
            .starts_with(fixture.paths.state_dir())
    );
    assert!(!fixture.paths.settings_file().exists());
}

#[test]
fn no_installation_is_a_valid_optional_state_and_manual_location_is_persistent() {
    let fixture = Fixture::new();
    let empty = fixture.store().status().unwrap();
    assert!(empty.installations.is_empty());
    assert_eq!(empty.selected_installation_id, None);
    assert!(!fixture.paths.photoshop_file().exists());
    let manual = fixture.install("custom installation", [27, 1, 0, 0], false);
    let selected = fixture.store().choose(manual).unwrap();
    assert_eq!(selected.installations.len(), 1);
    assert_eq!(fixture.store().status().unwrap(), selected);
}

#[test]
fn manual_invalid_executable_does_not_overwrite_a_working_preference() {
    let fixture = Fixture::new();
    fixture.install("2026", [27, 1, 0, 0], true);
    let store = fixture.store();
    let before = store.status().unwrap();
    let stored = fs::read(fixture.paths.photoshop_file()).unwrap();
    let invalid = fixture.root.path().join("not-photoshop.exe");
    fs::write(&invalid, b"not an application").unwrap();
    assert_eq!(
        store.choose(invalid).unwrap_err().code,
        PhotoshopErrorCode::InvalidInstallation
    );
    assert_eq!(fs::read(fixture.paths.photoshop_file()).unwrap(), stored);
    assert_eq!(store.status().unwrap(), before);
}

#[test]
fn installation_aliases_are_deduplicated_by_physical_identity() {
    let fixture = Fixture::new();
    let path = fixture.install("2026", [27, 1, 0, 0], true);
    let alias = fixture.root.path().join("Photoshop.exe");
    fs::hard_link(&path, &alias).unwrap();
    fixture
        .platform
        .candidates
        .lock()
        .unwrap()
        .push(alias.clone());
    assert_eq!(fixture.store().status().unwrap().installations.len(), 1);
    let manual = fixture.store().choose(alias.clone()).unwrap();
    assert_eq!(manual.installations.len(), 1);
    assert_eq!(manual.installations[0].path, alias.to_string_lossy());
}

#[test]
fn opens_the_original_with_unicode_spaces_metacharacters_and_a_long_native_path() {
    let fixture = Fixture::new();
    fixture.install("Adobe 2026", [27, 1, 0, 0], true);
    let store = fixture.store();
    store.status().unwrap();
    let original = fixture
        .root
        .path()
        .join("Fotografias de João & família")
        .join("a".repeat(100))
        .join("b".repeat(100))
        .join("Foto (1) & 2.jpg");
    let photo = fixture.photo(original);
    assert!(photo.logical_path.as_os_str().len() > 260);
    store.open_original(&photo).unwrap();
    let calls = fixture.platform.launched.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(fs::read(&calls[0].1).unwrap(), b"original photo bytes");
    assert_eq!(
        fs::canonicalize(&calls[0].1).unwrap(),
        fs::canonicalize(&photo.logical_path).unwrap()
    );
    assert_eq!(fs::read(&calls[0].0).unwrap(), b"[27,1,0,0]");
}

#[test]
fn removed_selected_installation_fails_the_attempt_without_switching_to_another() {
    let fixture = Fixture::new();
    let selected = fixture.install("2026", [27, 1, 0, 0], true);
    fixture.install("2025", [26, 1, 0, 0], true);
    let store = fixture.store();
    store.status().unwrap();
    let before = fs::read(fixture.paths.photoshop_file()).unwrap();
    fs::remove_file(selected).unwrap();
    let photo = fixture.photo(fixture.root.path().join("photo.jpg"));
    assert_eq!(
        store.open_original(&photo).unwrap_err().code,
        PhotoshopErrorCode::InstallationUnavailable
    );
    assert!(fixture.platform.launched.lock().unwrap().is_empty());
    assert_eq!(fs::read(fixture.paths.photoshop_file()).unwrap(), before);
    let refreshed = store.status().unwrap();
    assert_eq!(refreshed.installations[0].version, "26.1.0.0");
    assert_eq!(refreshed.revision, 2);
}

#[test]
fn missing_unreadable_or_non_photo_originals_do_not_launch_or_rewrite_preferences() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    fixture.install("2026", [27, 1, 0, 0], true);
    let store = fixture.store();
    store.status().unwrap();
    let before = fs::read(fixture.paths.photoshop_file()).unwrap();
    let mut photo = fixture.photo(fixture.root.path().join("photo.jpg"));
    let exclusive = fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(&photo.logical_path)
        .unwrap();
    assert_eq!(
        store.open_original(&photo).unwrap_err().code,
        PhotoshopErrorCode::OriginalUnavailable
    );
    drop(exclusive);
    fs::remove_file(&photo.logical_path).unwrap();
    assert_eq!(
        store.open_original(&photo).unwrap_err().code,
        PhotoshopErrorCode::OriginalAbsent
    );
    photo.kind = MediaKind::Decorative;
    assert_eq!(
        store.open_original(&photo).unwrap_err().code,
        PhotoshopErrorCode::InvalidContext
    );
    assert!(fixture.platform.launched.lock().unwrap().is_empty());
    assert_eq!(fs::read(fixture.paths.photoshop_file()).unwrap(), before);
}

#[test]
fn launch_failure_preserves_the_original_and_is_actionable() {
    let fixture = Fixture::new();
    fixture.install("2026", [27, 1, 0, 0], true);
    let store = fixture.store();
    store.status().unwrap();
    fixture.platform.fail_launch.store(true, Ordering::SeqCst);
    let photo = fixture.photo(fixture.root.path().join("photo.jpg"));
    let error = store.open_original(&photo).unwrap_err();
    assert_eq!(error.code, PhotoshopErrorCode::LaunchFailed);
    assert!(error.message.contains("Configurações"));
    assert_eq!(
        fs::read(photo.logical_path).unwrap(),
        b"original photo bytes"
    );
}

#[test]
fn native_detection_reads_metadata_without_starting_an_application() {
    let platform = windows::WindowsPhotoshop;
    assert!(
        platform
            .inspect(&std::env::current_exe().unwrap())
            .is_none()
    );
    // Empty CI machines are supported; local installations must pass the same inspector.
    for path in platform.candidates() {
        if let Some(info) = platform.inspect(&path) {
            assert!(info.name.starts_with("Adobe Photoshop"));
        }
    }
}
