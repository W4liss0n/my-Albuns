use std::{fs, io, path::PathBuf};

#[cfg(windows)]
use myalbuns_paths::{
    ExpectedObject, PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination,
    ProjectFileLock, ProjectFileLockError, ResolveError,
};
use uuid::Uuid;

use super::{
    DecodeFailure, DocumentFailure, PathFailure, ProjectLocation, decode, encode, map_path_failure,
    windows_publish::{publish_new, replace_existing, write_synced_new},
};
use crate::project_document::ProjectRevision;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenStoreError {
    Path(PathFailure),
    Document(DocumentFailure),
    ProjectInUse,
    IdentityIndeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateStoreError {
    Path(PathFailure),
    Document(DocumentFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    StateIndeterminate,
}

#[derive(Debug)]
pub(crate) struct ProjectStore {
    location: ProjectLocation,
    _baseline: PersistedBaseline,
}

#[derive(Debug)]
struct PersistedBaseline {
    #[cfg(windows)]
    lock: ProjectFileLock,
    _bytes: Vec<u8>,
}

impl ProjectStore {
    pub(crate) fn location(&self) -> &ProjectLocation {
        &self.location
    }

    #[cfg(windows)]
    pub(crate) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self._baseline.lock.physical_identity()
    }

    #[cfg(not(windows))]
    pub(crate) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        None
    }

    #[cfg(windows)]
    fn from_verified(location: ProjectLocation, lock: ProjectFileLock, bytes: Vec<u8>) -> Self {
        Self {
            location,
            _baseline: PersistedBaseline {
                lock,
                _bytes: bytes,
            },
        }
    }
}

pub(crate) struct OpenedProject {
    pub(crate) revision: ProjectRevision,
    pub(crate) store: ProjectStore,
}

#[derive(Debug)]
pub(crate) struct PreparedReplacement {
    location: ProjectLocation,
    expected_revision: ProjectRevision,
    expected_bytes: Vec<u8>,
    temporary: TemporaryPublication,
    destination: PreparedFileDestination,
    replaced_project_id: Option<Uuid>,
    #[cfg(windows)]
    replaced_lock: Option<ProjectFileLock>,
}

impl PreparedReplacement {
    pub(crate) fn replaced_project_id(&self) -> Option<Uuid> {
        self.replaced_project_id
    }

    #[cfg(windows)]
    pub(crate) fn publish(self) -> Result<ProjectStore, CreateStoreError> {
        let target_still_exists = self.target_still_matches_preparation()?;
        let publish_result = if target_still_exists {
            replace_existing(self.temporary.path(), self.destination.operational_path())
        } else {
            publish_new(self.temporary.path(), self.destination.operational_path())
        };
        match publish_result {
            Ok(()) => verify_created(
                &self.location,
                &self.destination,
                &self.expected_bytes,
                &self.expected_revision,
            )
            .map_err(|_| CreateStoreError::StateIndeterminate),
            Err(error) => self.reconcile_publish_error(error, target_still_exists),
        }
    }

    #[cfg(windows)]
    fn reconcile_publish_error(
        &self,
        error: io::Error,
        target_existed_before_publish: bool,
    ) -> Result<ProjectStore, CreateStoreError> {
        if !target_existed_before_publish && is_destination_conflict(&error) {
            return Err(CreateStoreError::DestinationConflict);
        }
        if let Ok(store) = verify_created(
            &self.location,
            &self.destination,
            &self.expected_bytes,
            &self.expected_revision,
        ) {
            return Ok(store);
        }

        if target_existed_before_publish {
            return match self.target_still_matches_preparation() {
                Ok(true) => Err(CreateStoreError::Path(map_io_path(error))),
                Ok(false) | Err(_) => Err(CreateStoreError::StateIndeterminate),
            };
        }

        match self.destination.resolve_existing() {
            Ok(None) => Err(CreateStoreError::Path(map_io_path(error))),
            Ok(Some(_)) | Err(_) => Err(CreateStoreError::StateIndeterminate),
        }
    }

    #[cfg(windows)]
    fn target_still_matches_preparation(&self) -> Result<bool, CreateStoreError> {
        let Some(expected_lock) = &self.replaced_lock else {
            return Ok(false);
        };
        let Some(current) = self
            .destination
            .resolve_existing()
            .map_err(|error| CreateStoreError::Path(map_path_failure(error)))?
        else {
            return Ok(false);
        };
        match expected_lock.compare_physical(&current) {
            PhysicalIdentityEvidence::Same => Ok(true),
            PhysicalIdentityEvidence::Different => Err(CreateStoreError::DestinationConflict),
            PhysicalIdentityEvidence::Indeterminate => Err(CreateStoreError::IdentityIndeterminate),
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn publish(self) -> Result<ProjectStore, CreateStoreError> {
        Err(CreateStoreError::Path(PathFailure::IoFailure))
    }
}

#[cfg(windows)]
pub(crate) fn open_editable(location: ProjectLocation) -> Result<OpenedProject, OpenStoreError> {
    let resolved = location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
        .map_err(|error| OpenStoreError::Path(map_path_failure(error)))?;
    let lock =
        ProjectFileLock::try_acquire(resolved.operational_path()).map_err(|error| match error {
            ProjectFileLockError::Conflict => OpenStoreError::ProjectInUse,
            ProjectFileLockError::Unavailable { .. } => {
                OpenStoreError::Path(PathFailure::IoFailure)
            }
        })?;
    if lock.compare_physical(&resolved) != PhysicalIdentityEvidence::Same {
        return Err(OpenStoreError::IdentityIndeterminate);
    }
    let source = lock.read_to_string().map_err(|error| {
        if error.kind() == io::ErrorKind::InvalidData {
            OpenStoreError::Document(DocumentFailure::InvalidProjectDocument)
        } else {
            OpenStoreError::Path(map_io_path(error))
        }
    })?;
    let bytes = source.into_bytes();
    let revision = decode(&bytes).map_err(map_open_decode_error)?;
    Ok(OpenedProject {
        revision,
        store: ProjectStore::from_verified(location, lock, bytes),
    })
}

#[cfg(not(windows))]
pub(crate) fn open_editable(_location: ProjectLocation) -> Result<OpenedProject, OpenStoreError> {
    Err(OpenStoreError::Path(PathFailure::IoFailure))
}

pub(crate) fn create_only(
    location: ProjectLocation,
    revision: &ProjectRevision,
) -> Result<ProjectStore, CreateStoreError> {
    let destination = location
        .prepare_file_destination()
        .map_err(CreateStoreError::Path)?;
    let bytes = encode(revision).map_err(map_create_decode_error)?;
    let temporary = prepare_temporary(&destination, &bytes)?;
    match publish_new(temporary.path(), destination.operational_path()) {
        Ok(()) => verify_created(&location, &destination, &bytes, revision)
            .map_err(|_| CreateStoreError::StateIndeterminate),
        Err(error) => reconcile_create_only_error(error, &location, &destination, &bytes, revision),
    }
}

#[cfg(windows)]
fn reconcile_create_only_error(
    error: io::Error,
    location: &ProjectLocation,
    destination: &PreparedFileDestination,
    expected_bytes: &[u8],
    expected_revision: &ProjectRevision,
) -> Result<ProjectStore, CreateStoreError> {
    if is_destination_conflict(&error) {
        return Err(CreateStoreError::DestinationConflict);
    }
    if let Ok(store) = verify_created(location, destination, expected_bytes, expected_revision) {
        return Ok(store);
    }

    match destination.resolve_existing() {
        Ok(None) => Err(CreateStoreError::Path(map_io_path(error))),
        Ok(Some(_)) | Err(_) => Err(CreateStoreError::StateIndeterminate),
    }
}

#[cfg(not(windows))]
fn reconcile_create_only_error(
    _error: io::Error,
    _location: &ProjectLocation,
    _destination: &PreparedFileDestination,
    _expected_bytes: &[u8],
    _expected_revision: &ProjectRevision,
) -> Result<ProjectStore, CreateStoreError> {
    Err(CreateStoreError::StateIndeterminate)
}

#[cfg(windows)]
pub(crate) fn prepare_replacement(
    location: ProjectLocation,
    revision: &ProjectRevision,
) -> Result<PreparedReplacement, CreateStoreError> {
    let destination = location
        .prepare_file_destination()
        .map_err(CreateStoreError::Path)?;
    let expected_bytes = encode(revision).map_err(map_create_decode_error)?;
    let temporary = prepare_temporary(&destination, &expected_bytes)?;

    let resolved = match destination.resolve_existing() {
        Ok(resolved) => resolved,
        Err(ResolveError::UnexpectedObjectType { .. }) => {
            return Err(CreateStoreError::DestinationConflict);
        }
        Err(error) => return Err(CreateStoreError::Path(map_path_failure(error))),
    };

    let (replaced_lock, replaced_project_id) = if let Some(resolved) = resolved {
        let lock =
            ProjectFileLock::try_acquire(resolved.operational_path()).map_err(
                |error| match error {
                    ProjectFileLockError::Conflict => CreateStoreError::ProjectInUse,
                    ProjectFileLockError::Unavailable { .. } => {
                        CreateStoreError::Path(PathFailure::IoFailure)
                    }
                },
            )?;
        if lock.compare_physical(&resolved) != PhysicalIdentityEvidence::Same {
            return Err(CreateStoreError::IdentityIndeterminate);
        }
        let project_id = match lock.read_to_string() {
            Ok(source) => decode(source.as_bytes())
                .ok()
                .map(|revision| revision.project_id),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => None,
            Err(error) => return Err(CreateStoreError::Path(map_io_path(error))),
        };
        (Some(lock), project_id)
    } else {
        (None, None)
    };

    Ok(PreparedReplacement {
        location,
        expected_revision: revision.clone(),
        expected_bytes,
        temporary,
        destination,
        replaced_project_id,
        replaced_lock,
    })
}

#[cfg(not(windows))]
pub(crate) fn prepare_replacement(
    _location: ProjectLocation,
    _revision: &ProjectRevision,
) -> Result<PreparedReplacement, CreateStoreError> {
    Err(CreateStoreError::Path(PathFailure::IoFailure))
}

fn prepare_temporary(
    destination: &PreparedFileDestination,
    bytes: &[u8],
) -> Result<TemporaryPublication, CreateStoreError> {
    let temporary_path = destination.sibling_temporary_path();
    let temporary = TemporaryPublication::new(temporary_path);
    write_synced_new(temporary.path(), bytes)
        .map_err(|error| CreateStoreError::Path(map_io_path(error)))?;
    Ok(temporary)
}

#[cfg(windows)]
fn verify_created(
    location: &ProjectLocation,
    destination: &PreparedFileDestination,
    expected_bytes: &[u8],
    expected_revision: &ProjectRevision,
) -> Result<ProjectStore, ()> {
    let resolved = destination.resolve_created().map_err(|_| ())?;
    let lock = ProjectFileLock::try_acquire(resolved.operational_path()).map_err(|_| ())?;
    if lock.compare_physical(&resolved) != PhysicalIdentityEvidence::Same {
        return Err(());
    }
    let bytes = lock.read_to_string().map_err(|_| ())?.into_bytes();
    if bytes != expected_bytes {
        return Err(());
    }
    let revision = decode(&bytes).map_err(|_| ())?;
    if revision != *expected_revision {
        return Err(());
    }
    Ok(ProjectStore::from_verified(location.clone(), lock, bytes))
}

#[cfg(not(windows))]
fn verify_created(
    _location: &ProjectLocation,
    _destination: &PreparedFileDestination,
    _expected_bytes: &[u8],
    _expected_revision: &ProjectRevision,
) -> Result<ProjectStore, ()> {
    Err(())
}

fn map_open_decode_error(error: DecodeFailure) -> OpenStoreError {
    match error {
        DecodeFailure::Path(error) => OpenStoreError::Path(error),
        DecodeFailure::Document(error) => OpenStoreError::Document(error),
    }
}

fn map_create_decode_error(error: DecodeFailure) -> CreateStoreError {
    match error {
        DecodeFailure::Path(error) => CreateStoreError::Path(error),
        DecodeFailure::Document(error) => CreateStoreError::Document(error),
    }
}

fn map_io_path(error: io::Error) -> PathFailure {
    match error.kind() {
        io::ErrorKind::NotFound => PathFailure::NotFound,
        io::ErrorKind::PermissionDenied => PathFailure::AccessDenied,
        io::ErrorKind::InvalidInput => PathFailure::InvalidPath,
        io::ErrorKind::AlreadyExists => PathFailure::Conflict,
        _ => PathFailure::IoFailure,
    }
}

fn is_destination_conflict(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists || matches!(error.raw_os_error(), Some(80 | 183))
}

#[derive(Debug)]
struct TemporaryPublication {
    path: PathBuf,
}

impl TemporaryPublication {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TemporaryPublication {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
