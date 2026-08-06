use std::{fs::OpenOptions, path::Path};

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
/// during Project publication.
#[derive(Debug)]
pub struct ProjectTransitionBarrier {
    _lock: ProjectFileLock,
}

impl ProjectTransitionBarrier {
    pub fn try_acquire(
        project_path: &Path,
        project_id: &str,
    ) -> Result<Self, ProjectTransitionBarrierError> {
        let parent = project_path
            .parent()
            .ok_or(ProjectTransitionBarrierError::Unavailable)?;
        let namespace = project_data_namespace(project_id);
        let barrier_path = parent.join(format!(".myalbuns-{namespace}.lock"));
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
