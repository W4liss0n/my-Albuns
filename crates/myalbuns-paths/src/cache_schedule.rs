use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use crate::{
    AppPaths, AppPathsError,
    guarded_fs::{
        DirectoryGuard, GuardedFsError, create_new_deletable_file, delete_open_file,
        ensure_direct_child, open_deletable_file, open_directory, open_existing_direct_child,
        open_readable_file, rename_open_file,
    },
};

const MARKER_FILE_NAME: &str = "clear-cache-on-startup.v1";
const MAX_MARKER_BYTES: usize = 1024;

/// Owns the physical `State` directory while the Cache cleanup marker is read,
/// published, or removed.
///
/// This is deliberately narrower than a general State store: it authorizes one
/// fixed marker and keeps every mutation relative to the validated directory
/// handle even if the visible pathname is later replaced by a junction.
#[derive(Debug)]
pub struct CacheClearScheduleStorage {
    state: DirectoryGuard,
}

impl AppPaths {
    pub fn open_cache_clear_schedule_storage(
        &self,
    ) -> Result<Option<CacheClearScheduleStorage>, AppPathsError> {
        open_schedule_storage(self, false)
    }

    pub fn prepare_cache_clear_schedule_storage(
        &self,
    ) -> Result<CacheClearScheduleStorage, AppPathsError> {
        open_schedule_storage(self, true)?.ok_or(AppPathsError::CacheStorageUnavailable)
    }
}

impl CacheClearScheduleStorage {
    pub fn read_marker(&self) -> Result<Option<Vec<u8>>, AppPathsError> {
        let path = self.marker_path();
        let mut file = match open_readable_file(&self.state, &path) {
            Ok(file) => file,
            Err(GuardedFsError::NotFound) => return Ok(None),
            Err(error) => return Err(map_guarded_error(error)),
        };
        read_bounded(&mut file).map(Some)
    }

    /// Publishes one create-only marker. `false` means a concurrent marker is
    /// already present and must be inspected by the caller.
    pub fn publish_marker(&self, bytes: &[u8]) -> Result<bool, AppPathsError> {
        validate_marker_bytes(bytes)?;
        let temporary = self.state.logical_path.join(format!(
            ".clear-cache-on-startup.{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let mut file =
            create_new_deletable_file(&self.state, &temporary).map_err(map_guarded_error)?;
        if file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .is_err()
        {
            let _ = delete_open_file(&self.state, &temporary, &file);
            return Err(AppPathsError::CacheStorageUnavailable);
        }
        let published = match rename_open_file(
            &self.state,
            &temporary,
            &file,
            std::ffi::OsStr::new(MARKER_FILE_NAME),
        ) {
            Ok(()) => true,
            Err(GuardedFsError::AlreadyExists) => {
                delete_open_file(&self.state, &temporary, &file).map_err(map_guarded_error)?;
                false
            }
            Err(error) => {
                let _ = delete_open_file(&self.state, &temporary, &file);
                return Err(map_guarded_error(error));
            }
        };
        Ok(published)
    }

    /// Removes only the exact marker handle and only when its bytes still
    /// match the content previously authorized by the Cache service.
    pub fn remove_marker_if_matches(&self, expected: &[u8]) -> Result<bool, AppPathsError> {
        validate_marker_bytes(expected)?;
        let path = self.marker_path();
        let mut file = match open_deletable_file(&self.state, &path) {
            Ok(file) => file,
            Err(GuardedFsError::NotFound) => return Ok(false),
            Err(error) => return Err(map_guarded_error(error)),
        };
        if read_bounded(&mut file)? != expected {
            return Err(AppPathsError::CacheStorageUnavailable);
        }
        delete_open_file(&self.state, &path, &file).map_err(map_guarded_error)?;
        Ok(true)
    }

    fn marker_path(&self) -> PathBuf {
        self.state.logical_path.join(MARKER_FILE_NAME)
    }
}

fn open_schedule_storage(
    app_paths: &AppPaths,
    create: bool,
) -> Result<Option<CacheClearScheduleStorage>, AppPathsError> {
    let local_data_root = app_paths
        .local_root
        .parent()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    let local_data = open_directory(local_data_root).map_err(map_guarded_error)?;
    let application = if create {
        ensure_direct_child(&local_data, &app_paths.local_root).map_err(map_guarded_error)?
    } else {
        let Some(application) = open_existing_direct_child(&local_data, &app_paths.local_root)
            .map_err(map_guarded_error)?
        else {
            return Ok(None);
        };
        application
    };
    let state_path = app_paths.state_dir();
    let state = if create {
        ensure_direct_child(&application, &state_path).map_err(map_guarded_error)?
    } else {
        let Some(state) =
            open_existing_direct_child(&application, &state_path).map_err(map_guarded_error)?
        else {
            return Ok(None);
        };
        state
    };
    Ok(Some(CacheClearScheduleStorage { state }))
}

fn read_bounded(file: &mut File) -> Result<Vec<u8>, AppPathsError> {
    let mut bytes = Vec::with_capacity(MAX_MARKER_BYTES);
    file.take((MAX_MARKER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    validate_marker_bytes(&bytes)?;
    Ok(bytes)
}

fn validate_marker_bytes(bytes: &[u8]) -> Result<(), AppPathsError> {
    if bytes.is_empty() || bytes.len() > MAX_MARKER_BYTES {
        return Err(AppPathsError::CacheStorageUnavailable);
    }
    Ok(())
}

fn map_guarded_error(error: GuardedFsError) -> AppPathsError {
    match error {
        GuardedFsError::OutsideRoot => AppPathsError::CacheStorageOutsideRoot,
        GuardedFsError::AlreadyExists | GuardedFsError::NotFound | GuardedFsError::Unavailable => {
            AppPathsError::CacheStorageUnavailable
        }
    }
}
