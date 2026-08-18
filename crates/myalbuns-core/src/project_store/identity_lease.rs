use std::{
    cell::Cell,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

#[cfg(windows)]
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use myalbuns_paths::{
    PhysicalFileIdentity, PhysicalIdentityEvidence, ProcessInstanceId, ProjectFileLock,
    ProjectFileLockError, publish_new_file, replace_existing_file,
};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::Threading::{CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject},
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
    SamePhysicalTarget { owner_process: ProcessInstanceId },
    DifferentPhysicalTarget,
}

#[cfg(windows)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActiveIdentityTarget {
    version: u32,
    physical_identity: String,
    owner_process: ProcessInstanceId,
}

#[cfg(windows)]
#[derive(Debug)]
struct IdentityPublicationMutex {
    handle: HANDLE,
}

#[cfg(windows)]
impl IdentityPublicationMutex {
    fn acquire(project_id: Uuid) -> Result<Self, IdentityLeaseError> {
        let name = format!(
            "Local\\MyAlbuns.ProjectIdentityPublication.{}",
            project_id.hyphenated()
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(IdentityLeaseError::Unavailable);
        }
        let wait = unsafe { WaitForSingleObject(handle, INFINITE) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            unsafe {
                CloseHandle(handle);
            }
            return Err(IdentityLeaseError::Unavailable);
        }
        Ok(Self { handle })
    }
}

#[cfg(windows)]
impl Drop for IdentityPublicationMutex {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
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
}

#[derive(Debug)]
pub(crate) struct PendingProjectIdentityLease {
    #[cfg(windows)]
    lease: Option<ProjectIdentityLease>,
    #[cfg(windows)]
    created_for_attempt: bool,
    #[cfg(windows)]
    target_bound: Cell<bool>,
    #[cfg(windows)]
    _publication_lock: IdentityPublicationMutex,
}

pub(crate) trait IdentityTargetBinder {
    fn bind_target(
        &self,
        identity: myalbuns_paths::PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError>;
}

impl ProjectIdentityLease {
    #[cfg(windows)]
    pub(crate) fn acquire(
        root: &Path,
        project_id: Uuid,
    ) -> Result<PendingProjectIdentityLease, IdentityLeaseError> {
        fs::create_dir_all(root).map_err(|_| IdentityLeaseError::Unavailable)?;
        let publication_lock = IdentityPublicationMutex::acquire(project_id)?;
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
        Ok(PendingProjectIdentityLease {
            lease: Some(Self {
                project_id,
                _lock: lock,
                lease_path,
                target_path,
            }),
            created_for_attempt,
            target_bound: Cell::new(false),
            _publication_lock: publication_lock,
        })
    }

    #[cfg(windows)]
    pub(crate) fn bind_target(
        &self,
        identity: PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        let owner_process =
            ProcessInstanceId::current().map_err(|_| IdentityLeaseError::Unavailable)?;
        publish_target_atomically(
            &self.target_path,
            &ActiveIdentityTarget {
                version: 2,
                physical_identity: identity.to_local_token(),
                owner_process,
            },
            || {},
        )
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
                let target: ActiveIdentityTarget =
                    serde_json::from_str(&source).map_err(|_| IdentityLeaseError::Unavailable)?;
                if target.version != 2 {
                    return Err(IdentityLeaseError::Unavailable);
                }
                let active = PhysicalFileIdentity::from_local_token(&target.physical_identity)
                    .filter(|identity| identity.to_local_token() == target.physical_identity)
                    .ok_or(IdentityLeaseError::Unavailable)?;
                match active.compare(candidate) {
                    PhysicalIdentityEvidence::Same => {
                        Ok(IdentityLeaseObservation::SamePhysicalTarget {
                            owner_process: target.owner_process,
                        })
                    }
                    PhysicalIdentityEvidence::Different => {
                        Ok(IdentityLeaseObservation::DifferentPhysicalTarget)
                    }
                    PhysicalIdentityEvidence::Indeterminate => Err(IdentityLeaseError::Unavailable),
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
    pub(crate) fn acquire(
        _root: &Path,
        _project_id: Uuid,
    ) -> Result<PendingProjectIdentityLease, IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }
    pub(crate) fn project_id(&self) -> Uuid {
        self.project_id
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

#[cfg(windows)]
fn publish_target_atomically(
    target_path: &Path,
    target: &ActiveIdentityTarget,
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
        let bytes = serde_json::to_vec(target).map_err(|_| IdentityLeaseError::Unavailable)?;
        file.write_all(&bytes)
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

impl IdentityTargetBinder for ProjectIdentityLease {
    fn bind_target(
        &self,
        identity: myalbuns_paths::PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        ProjectIdentityLease::bind_target(self, identity)
    }
}

impl PendingProjectIdentityLease {
    #[cfg(windows)]
    pub(crate) fn bind_target(
        &self,
        identity: PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        let lease = self.lease.as_ref().ok_or(IdentityLeaseError::Unavailable)?;
        lease.bind_target(identity)?;
        self.target_bound.set(true);
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn into_published(mut self) -> Result<ProjectIdentityLease, IdentityLeaseError> {
        if !self.target_bound.get() {
            return Err(IdentityLeaseError::Unavailable);
        }
        self.lease.take().ok_or(IdentityLeaseError::Unavailable)
    }

    pub(crate) fn discard_unpublished(self) {}

    #[cfg(not(windows))]
    pub(crate) fn bind_target(
        &self,
        _identity: myalbuns_paths::PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }

    #[cfg(not(windows))]
    pub(crate) fn into_published(self) -> Result<ProjectIdentityLease, IdentityLeaseError> {
        Err(IdentityLeaseError::Unavailable)
    }
}

impl IdentityTargetBinder for PendingProjectIdentityLease {
    fn bind_target(
        &self,
        identity: myalbuns_paths::PhysicalFileIdentity,
    ) -> Result<(), IdentityLeaseError> {
        PendingProjectIdentityLease::bind_target(self, identity)
    }
}

#[cfg(windows)]
impl Drop for PendingProjectIdentityLease {
    fn drop(&mut self) {
        if !self.created_for_attempt {
            return;
        }
        let Some(lease) = self.lease.as_ref() else {
            return;
        };
        let _ = fs::remove_file(&lease.target_path);
        let _ = fs::remove_file(&lease.lease_path);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::mpsc;

    use myalbuns_paths::{ExpectedObject, OperationPathContext, ProcessInstanceId};

    use super::{
        ActiveIdentityTarget, IdentityLeaseError, IdentityLeaseObservation, ProjectIdentityLease,
        publish_target_atomically, target_path,
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
        let active_target = super::ActiveIdentityTarget {
            version: 2,
            physical_identity: identity.to_local_token(),
            owner_process: ProcessInstanceId::current().expect("the process instance is captured"),
        };
        let (reached_sender, reached_receiver) = mpsc::sync_channel(0);
        let (release_sender, release_receiver) = mpsc::sync_channel(0);

        let publisher = std::thread::spawn(move || {
            publish_target_atomically(&publishing_target, &active_target, || {
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
            Ok(IdentityLeaseObservation::SamePhysicalTarget {
                owner_process: ProcessInstanceId::current()
                    .expect("the observing process instance is captured"),
            })
        );
        drop(lease);
    }

    #[test]
    fn an_active_lease_never_focuses_from_an_unusable_physical_identity_token() {
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
            .expect("the candidate has authoritative physical identity evidence");
        let project_id = uuid::Uuid::new_v4();
        let lease = ProjectIdentityLease::acquire(&root, project_id)
            .expect("the fixture retains the active lease");
        let target = target_path(&root, project_id);
        let owner_process = ProcessInstanceId::current().expect("the process instance is captured");

        for token in [
            "windows-file-id-v1:0000000000000007:00000000000000000000000000000000",
            "windows-file-id-v1:0000000000000007:ffffffffffffffffffffffffffffffff",
            "windows-file-index-v1:0000000000000007:0303030303030303",
        ] {
            publish_target_atomically(
                &target,
                &ActiveIdentityTarget {
                    version: 2,
                    physical_identity: token.to_owned(),
                    owner_process,
                },
                || {},
            )
            .expect("the malformed external target fixture is published atomically");

            assert_eq!(
                ProjectIdentityLease::observe(&root, project_id, Some(identity)),
                Err(IdentityLeaseError::Unavailable),
                "unusable evidence must never become FocusExisting"
            );
        }
        drop(lease);
    }
}
