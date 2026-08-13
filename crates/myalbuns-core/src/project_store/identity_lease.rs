use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

#[cfg(windows)]
use myalbuns_paths::{
    PhysicalFileIdentity, ProjectFileLock, ProjectFileLockError, publish_new_file,
    replace_existing_file,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityLeaseError {
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityLeaseObservation {
    Inactive,
    Pending,
    SamePhysicalTarget,
    DifferentPhysicalTarget,
}

#[derive(Debug)]
pub(crate) struct ProjectIdentityLease {
    project_id: Uuid,
    #[cfg(windows)]
    _lock: ProjectFileLock,
    #[cfg(windows)]
    lease_path: PathBuf,
    #[cfg(windows)]
    target_path: PathBuf,
    #[cfg(windows)]
    created_for_attempt: bool,
}

impl ProjectIdentityLease {
    #[cfg(windows)]
    pub(crate) fn acquire(root: &Path, project_id: Uuid) -> Result<Self, IdentityLeaseError> {
        fs::create_dir_all(root).map_err(|_| IdentityLeaseError::Unavailable)?;
        let lease_path = lease_path(root, project_id);
        let created_for_attempt = match OpenOptions::new()
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
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(_) => return Err(IdentityLeaseError::Unavailable),
        };
        let lock = match ProjectFileLock::try_acquire(&lease_path) {
            Ok(lock) => lock,
            Err(error) => {
                if created_for_attempt {
                    let _ = fs::remove_file(&lease_path);
                }
                return Err(match error {
                    ProjectFileLockError::Conflict => IdentityLeaseError::Conflict,
                    ProjectFileLockError::Unavailable { .. } => IdentityLeaseError::Unavailable,
                });
            }
        };
        let target_path = target_path(root, project_id);
        if let Err(error) = fs::remove_file(&target_path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            drop(lock);
            if created_for_attempt {
                let _ = fs::remove_file(&lease_path);
            }
            return Err(IdentityLeaseError::Unavailable);
        }
        Ok(Self {
            project_id,
            _lock: lock,
            lease_path,
            target_path,
            created_for_attempt,
        })
    }

    #[cfg(windows)]
    pub(crate) fn discard_unpublished(self) {
        if !self.created_for_attempt {
            return;
        }
        let lease_path = self.lease_path.clone();
        let target_path = self.target_path.clone();
        drop(self);
        let _ = fs::remove_file(target_path);
        let _ = fs::remove_file(lease_path);
    }

    #[cfg(windows)]
    pub(crate) fn bind_target(
        &self,
        identity: PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        let token = identity.to_local_token();
        publish_target_atomically(&self.target_path, &token, || {})
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
                let source = match fs::read_to_string(target_path(root, project_id)) {
                    Ok(source) => source,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(IdentityLeaseObservation::Pending);
                    }
                    Err(_) => return Err(IdentityLeaseError::Unavailable),
                };
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
    pub(crate) fn project_id(&self) -> Uuid {
        self.project_id
    }

    #[cfg(not(windows))]
    pub(crate) fn discard_unpublished(self) {}

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

#[cfg(windows)]
fn publish_target_atomically(
    target_path: &Path,
    token: &str,
    before_publish: impl FnOnce(),
) -> Result<(), IdentityLeaseError> {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(IdentityLeaseError::Unavailable)?;
    let temporary_path =
        target_path.with_file_name(format!(".{file_name}.tmp-{}", Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|_| IdentityLeaseError::Unavailable)?;
        file.write_all(token.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|_| IdentityLeaseError::Unavailable)?;
        drop(file);
        before_publish();
        let publish = if target_path.exists() {
            replace_existing_file(&temporary_path, target_path)
        } else {
            publish_new_file(&temporary_path, target_path)
        };
        publish.map_err(|_| IdentityLeaseError::Unavailable)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    result
}

fn lease_path(root: &Path, project_id: Uuid) -> PathBuf {
    root.join(format!("{}.lease", project_id.hyphenated()))
}

fn target_path(root: &Path, project_id: Uuid) -> PathBuf {
    root.join(format!("{}.target", project_id.hyphenated()))
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::mpsc;

    use myalbuns_paths::{ExpectedObject, OperationPathContext};

    use super::{
        IdentityLeaseObservation, ProjectIdentityLease, publish_target_atomically, target_path,
    };

    #[test]
    fn observers_see_pending_until_the_complete_target_token_is_atomically_published() {
        let fixture = tempfile::tempdir().expect("temporary identity lease fixture");
        let root = fixture.path().join("leases");
        let source_path = fixture.path().join("Project.myalbuns");
        std::fs::write(&source_path, b"project bytes").expect("the candidate is writable");
        let mut context = OperationPathContext::new();
        context
            .capture(&source_path)
            .expect("the candidate root binding is captured");
        let resolved = context
            .freeze()
            .resolve_existing(&source_path, ExpectedObject::RegularFile)
            .expect("the candidate is resolved once");
        let identity = resolved
            .physical_identity()
            .expect("the candidate has physical identity evidence");
        let project_id = uuid::Uuid::new_v4();
        let lease = ProjectIdentityLease::acquire(&root, project_id)
            .expect("the first observation owns the lease");
        let target = target_path(&root, project_id);
        let publishing_target = target.clone();
        let token = identity.to_local_token();
        let (reached_sender, reached_receiver) = mpsc::sync_channel(0);
        let (release_sender, release_receiver) = mpsc::sync_channel(0);

        let publisher = std::thread::spawn(move || {
            publish_target_atomically(&publishing_target, &token, || {
                reached_sender
                    .send(())
                    .expect("the observer sees the pre-publication boundary");
                release_receiver
                    .recv()
                    .expect("the observer releases atomic publication");
            })
        });
        reached_receiver
            .recv()
            .expect("the complete temporary token is synchronized");

        assert!(!target.exists(), "no partial token is addressable");
        assert_eq!(
            ProjectIdentityLease::observe(&root, project_id, Some(identity)),
            Ok(IdentityLeaseObservation::Pending)
        );

        release_sender
            .send(())
            .expect("the target token may be published");
        publisher
            .join()
            .expect("the target publisher does not panic")
            .expect("the target token is published atomically");
        assert_eq!(
            ProjectIdentityLease::observe(&root, project_id, Some(identity)),
            Ok(IdentityLeaseObservation::SamePhysicalTarget)
        );
        drop(lease);
    }
}
