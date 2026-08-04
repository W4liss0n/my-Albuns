use std::{fs, io, path::PathBuf};

#[cfg(windows)]
use myalbuns_paths::{
    ExpectedObject, PhysicalFileIdentity, PhysicalIdentityEvidence, ProjectFileLock,
    ProjectFileLockError, ResolveError,
};
use uuid::Uuid;

use super::{
    DecodeFailure, DocumentFailure, PathFailure, ProjectLocation, decode, encode, map_path_failure,
    windows_publish::{publish_new, replace_existing, sibling_temporary, write_synced_new},
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
    operational_path: PathBuf,
    replaced_project_id: Option<Uuid>,
    #[cfg(windows)]
    replaced_lock: Option<ProjectFileLock>,
}

impl PreparedReplacement {
    pub(crate) fn replaced_project_id(&self) -> Option<Uuid> {
        self.replaced_project_id
    }

    #[cfg(windows)]
    pub(crate) fn publish(mut self) -> Result<ProjectStore, CreateStoreError> {
        let publish_result = if self.replaced_lock.is_some() {
            replace_existing(self.temporary.path(), &self.operational_path)
        } else {
            publish_new(self.temporary.path(), &self.operational_path)
        };
        publish_result.map_err(|error| {
            if is_destination_conflict(&error) {
                CreateStoreError::DestinationConflict
            } else {
                CreateStoreError::Path(map_io_path(error))
            }
        })?;
        self.temporary.mark_published();
        drop(self.replaced_lock.take());
        verify_created(
            &self.location,
            &self.expected_bytes,
            &self.expected_revision,
        )
        .map_err(|_| CreateStoreError::StateIndeterminate)
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
    let operational_path = location
        .operational_path()
        .map_err(CreateStoreError::Path)?;
    let bytes = encode(revision).map_err(map_create_decode_error)?;
    let mut temporary = prepare_temporary(&operational_path, &bytes)?;
    publish_new(temporary.path(), &operational_path).map_err(|error| {
        if is_destination_conflict(&error) {
            CreateStoreError::DestinationConflict
        } else {
            CreateStoreError::Path(map_io_path(error))
        }
    })?;
    temporary.mark_published();
    verify_created(&location, &bytes, revision).map_err(|_| CreateStoreError::StateIndeterminate)
}

#[cfg(windows)]
pub(crate) fn prepare_replacement(
    location: ProjectLocation,
    revision: &ProjectRevision,
) -> Result<PreparedReplacement, CreateStoreError> {
    let operational_path = location
        .operational_path()
        .map_err(CreateStoreError::Path)?;
    let expected_bytes = encode(revision).map_err(map_create_decode_error)?;
    let temporary = prepare_temporary(&operational_path, &expected_bytes)?;

    let resolved = match location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
    {
        Ok(resolved) => Some(resolved),
        Err(ResolveError::NotFound) => None,
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
        operational_path,
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
    operational_path: &std::path::Path,
    bytes: &[u8],
) -> Result<TemporaryPublication, CreateStoreError> {
    let temporary_path = sibling_temporary(operational_path)
        .map_err(|error| CreateStoreError::Path(map_io_path(error)))?;
    let temporary = TemporaryPublication::new(temporary_path);
    write_synced_new(temporary.path(), bytes)
        .map_err(|error| CreateStoreError::Path(map_io_path(error)))?;
    Ok(temporary)
}

#[cfg(windows)]
fn verify_created(
    location: &ProjectLocation,
    expected_bytes: &[u8],
    expected_revision: &ProjectRevision,
) -> Result<ProjectStore, ()> {
    let resolved = location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
        .map_err(|_| ())?;
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
    published: bool,
}

impl TemporaryPublication {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: false,
        }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn mark_published(&mut self) {
        self.published = true;
    }
}

impl Drop for TemporaryPublication {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}
