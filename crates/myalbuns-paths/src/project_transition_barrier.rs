use std::{fs, fs::OpenOptions, path::Path};

use std::os::windows::fs::OpenOptionsExt;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use crate::{ProjectFileLock, ProjectFileLockError, project_data_namespace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectTransitionBarrierError {
    Conflict,
    Unavailable,
}

/// Short-lived cross-process barrier covering the physical file-lock handoff
/// during Project publication without requiring writes beside the Project.
#[derive(Debug)]
pub struct ProjectTransitionBarrier {
    _lock: ProjectFileLock,
}

impl ProjectTransitionBarrier {
    pub fn try_acquire(
        local_state_root: &Path,
        project_id: &str,
    ) -> Result<Self, ProjectTransitionBarrierError> {
        fs::create_dir_all(local_state_root)
            .map_err(|_| ProjectTransitionBarrierError::Unavailable)?;
        let namespace = project_data_namespace(project_id);
        let barrier_path = local_state_root.join(format!("{namespace}.transition"));
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(&barrier_path)
            .map_err(|_| ProjectTransitionBarrierError::Unavailable)?;
        let lock = ProjectFileLock::try_acquire(&barrier_path).map_err(|error| match error {
            ProjectFileLockError::Conflict => ProjectTransitionBarrierError::Conflict,
            ProjectFileLockError::Unavailable { .. } => ProjectTransitionBarrierError::Unavailable,
        })?;
        Ok(Self { _lock: lock })
    }
}
