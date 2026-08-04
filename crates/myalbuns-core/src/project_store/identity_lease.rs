use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

#[cfg(windows)]
use myalbuns_paths::{PhysicalFileIdentity, ProjectFileLock, ProjectFileLockError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityLeaseError {
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityLeaseObservation {
    Inactive,
    SamePhysicalTarget,
    DifferentPhysicalTarget,
}

#[derive(Debug)]
pub(crate) struct ProjectIdentityLease {
    #[cfg(windows)]
    _lock: ProjectFileLock,
    #[cfg(windows)]
    target_path: PathBuf,
}

impl ProjectIdentityLease {
    #[cfg(windows)]
    pub(crate) fn acquire(root: &Path, project_id: Uuid) -> Result<Self, IdentityLeaseError> {
        fs::create_dir_all(root).map_err(|_| IdentityLeaseError::Unavailable)?;
        let lease_path = lease_path(root, project_id);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lease_path)
        {
            Ok(mut file) => {
                if file
                    .write_all(project_id.hyphenated().to_string().as_bytes())
                    .and_then(|_| file.sync_all())
                    .is_err()
                {
                    drop(file);
                    let _ = fs::remove_file(&lease_path);
                    return Err(IdentityLeaseError::Unavailable);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(IdentityLeaseError::Unavailable),
        }
        let lock = ProjectFileLock::try_acquire(&lease_path).map_err(|error| match error {
            ProjectFileLockError::Conflict => IdentityLeaseError::Conflict,
            ProjectFileLockError::Unavailable { .. } => IdentityLeaseError::Unavailable,
        })?;
        Ok(Self {
            _lock: lock,
            target_path: target_path(root, project_id),
        })
    }

    #[cfg(windows)]
    pub(crate) fn bind_target(
        &self,
        identity: PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        let token = identity.to_local_token();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.target_path)
            .map_err(|_| IdentityLeaseError::Unavailable)?;
        file.write_all(token.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|_| IdentityLeaseError::Unavailable)
    }

    #[cfg(windows)]
    pub(crate) fn observe(
        root: &Path,
        project_id: Uuid,
        candidate: Option<PhysicalFileIdentity>,
    ) -> Result<IdentityLeaseObservation, IdentityLeaseError> {
        let lease_path = lease_path(root, project_id);
        match fs::metadata(&lease_path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(IdentityLeaseError::Unavailable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(IdentityLeaseObservation::Inactive);
            }
            Err(_) => return Err(IdentityLeaseError::Unavailable),
        }

        match ProjectFileLock::try_acquire(&lease_path) {
            Ok(lock) => {
                drop(lock);
                Ok(IdentityLeaseObservation::Inactive)
            }
            Err(ProjectFileLockError::Conflict) => {
                let candidate = candidate.ok_or(IdentityLeaseError::Unavailable)?;
                let source = fs::read_to_string(target_path(root, project_id))
                    .map_err(|_| IdentityLeaseError::Unavailable)?;
                let token = source
                    .strip_suffix('\n')
                    .filter(|value| !value.contains(['\r', '\n']))
                    .ok_or(IdentityLeaseError::Unavailable)?;
                let active = PhysicalFileIdentity::from_local_token(token)
                    .filter(|identity| identity.to_local_token() == token)
                    .ok_or(IdentityLeaseError::Unavailable)?;
                if active == candidate {
                    Ok(IdentityLeaseObservation::SamePhysicalTarget)
                } else {
                    Ok(IdentityLeaseObservation::DifferentPhysicalTarget)
                }
            }
            Err(ProjectFileLockError::Unavailable { .. }) => match fs::metadata(&lease_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(IdentityLeaseObservation::Inactive)
                }
                _ => Err(IdentityLeaseError::Unavailable),
            },
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn acquire(_root: &Path, _project_id: Uuid) -> Result<Self, IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }

    #[cfg(not(windows))]
    pub(crate) fn bind_target(
        &self,
        _identity: myalbuns_paths::PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }

    #[cfg(not(windows))]
    pub(crate) fn observe(
        _root: &Path,
        _project_id: Uuid,
        _candidate: Option<myalbuns_paths::PhysicalFileIdentity>,
    ) -> Result<IdentityLeaseObservation, IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }
}

fn lease_path(root: &Path, project_id: Uuid) -> PathBuf {
    root.join(format!("{}.lease", project_id.hyphenated()))
}

fn target_path(root: &Path, project_id: Uuid) -> PathBuf {
    root.join(format!("{}.target", project_id.hyphenated()))
}
