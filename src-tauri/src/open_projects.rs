//! Knows whether other Project Hosts are still open, so closing a Project
//! returns to the Welcome screen only when it was the last one.
//!
//! Each Project Host holds an exclusive, delete-on-close marker file for its
//! whole life. Windows removes the marker when the process ends, even after a
//! crash, so a marker that cannot be opened belongs to a live Project Host.

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use myalbuns_paths::AppPaths;

#[cfg(windows)]
use crate::local_store_io::{CrossProcessStoreGuard, store_mutex_name};

const MARKER_EXTENSION: &str = "open";

/// The marker of this Project Host, released when the Project closes.
pub(crate) struct OpenProjectPresence {
    marker: Mutex<Option<(PathBuf, File)>>,
}

impl OpenProjectPresence {
    pub(crate) fn register(app_paths: &AppPaths) -> io::Result<Self> {
        let directory = app_paths.open_projects_dir();
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!(
            "{}-{}.{MARKER_EXTENSION}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let file = create_marker(&path)?;
        Ok(Self {
            marker: Mutex::new(Some((path, file))),
        })
    }

    pub(crate) fn unregistered() -> Self {
        Self {
            marker: Mutex::new(None),
        }
    }

    /// Releases this Project Host and reports whether no other Project is
    /// still open. Hosts closing at the same time decide one after the other,
    /// so the last of them always returns to the Welcome screen.
    pub(crate) fn release_and_check_last(&self, app_paths: &AppPaths) -> io::Result<bool> {
        #[cfg(windows)]
        let _decision = CrossProcessStoreGuard::acquire(
            &store_mutex_name("OpenProjects", app_paths.local_root()),
            "OpenProjects",
        )?;
        let released = self
            .marker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let own_path = released.map(|(path, file)| {
            drop(file);
            path
        });
        Ok(!any_other_open(
            &app_paths.open_projects_dir(),
            own_path.as_deref(),
        ))
    }
}

#[cfg(windows)]
fn create_marker(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_FLAG_DELETE_ON_CLOSE};

    const GENERIC_WRITE: u32 = 0x4000_0000;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_WRITE | DELETE)
        .share_mode(0)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(path)
}

#[cfg(not(windows))]
fn create_marker(path: &Path) -> io::Result<File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

fn any_other_open(directory: &Path, own_path: Option<&Path>) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    let mut any_open = false;
    for path in entries.flatten().map(|entry| entry.path()) {
        if Some(path.as_path()) == own_path
            || path.extension().and_then(|extension| extension.to_str()) != Some(MARKER_EXTENSION)
        {
            continue;
        }
        if marker_is_held(&path) {
            any_open = true;
        } else {
            let _ = fs::remove_file(&path);
        }
    }
    any_open
}

#[cfg(windows)]
fn marker_is_held(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;

    const ERROR_SHARING_VIOLATION: i32 = 32;
    match fs::OpenOptions::new().read(true).share_mode(0).open(path) {
        Ok(_) => false,
        Err(error) => error.raw_os_error() == Some(ERROR_SHARING_VIOLATION),
    }
}

#[cfg(not(windows))]
fn marker_is_held(_path: &Path) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs;

    use myalbuns_paths::AppPaths;

    use super::OpenProjectPresence;

    fn paths() -> (tempfile::TempDir, AppPaths) {
        let root = tempfile::tempdir().expect("temporary application data root");
        let paths = AppPaths::from_roots(root.path(), root.path());
        (root, paths)
    }

    #[test]
    fn only_the_last_open_project_returns_to_the_welcome_screen() {
        let (_root, paths) = paths();
        let first = OpenProjectPresence::register(&paths).expect("first Project registers");
        let second = OpenProjectPresence::register(&paths).expect("second Project registers");

        assert!(!first.release_and_check_last(&paths).unwrap());
        assert!(second.release_and_check_last(&paths).unwrap());
        assert_eq!(fs::read_dir(paths.open_projects_dir()).unwrap().count(), 0);
    }

    #[test]
    fn markers_without_a_live_host_do_not_keep_the_welcome_screen_away() {
        let (_root, paths) = paths();
        let presence = OpenProjectPresence::register(&paths).expect("the Project registers");
        let stale = paths.open_projects_dir().join("1-stale.open");
        fs::write(&stale, b"").expect("stale marker fixture");

        assert!(presence.release_and_check_last(&paths).unwrap());
        assert!(!stale.exists());
    }

    #[test]
    fn a_host_without_a_marker_still_sees_the_other_projects() {
        let (_root, paths) = paths();
        let other = OpenProjectPresence::register(&paths).expect("other Project registers");

        assert!(
            !OpenProjectPresence::unregistered()
                .release_and_check_last(&paths)
                .unwrap()
        );
        assert!(other.release_and_check_last(&paths).unwrap());
    }
}
