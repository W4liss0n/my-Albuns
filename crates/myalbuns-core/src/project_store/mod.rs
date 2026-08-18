mod editable_store;
mod identity_lease;
mod identity_registry;
mod versioned_codec;
mod windows_publish;

use std::path::{Path, PathBuf};

use myalbuns_paths::{
    ExpectedObject, PhysicalFileIdentity, PreparedFileDestination, ResolveError, ResolvedObject,
    RootBindingPlan,
};

use crate::project_document::ProjectRevision;

pub(crate) use editable_store::{
    CreateStoreError, OpenStoreError, OpenedProject, ProjectStore, SaveStoreError, SaveStoreResult,
    create_only, open_editable, prepare_replacement,
};
pub(crate) use identity_lease::{
    IdentityLeaseError, IdentityLeaseObservation, IdentityTargetBinder,
    PendingProjectIdentityLease, ProjectIdentityLease,
};
pub(crate) use identity_registry::{IdentityRegistryLookup, ProjectIdentityRegistry};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentFailure {
    InvalidDocumentType,
    UnsupportedFutureSchema { version: u32 },
    UnsupportedLegacySchema { version: u32 },
    InvalidProjectDocument,
    InvalidProjectState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathFailure {
    NotFound,
    Unavailable,
    AccessDenied,
    InvalidPath,
    UnexpectedObjectType,
    Conflict,
    IoFailure,
}

#[derive(Clone, Debug)]
pub struct ProjectLocation {
    project_path: PathBuf,
    root_bindings: RootBindingPlan,
}

impl ProjectLocation {
    pub fn new(project_path: PathBuf, root_bindings: RootBindingPlan) -> Self {
        Self {
            project_path,
            root_bindings,
        }
    }

    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    pub(crate) fn root_bindings(&self) -> &RootBindingPlan {
        &self.root_bindings
    }

    pub(crate) fn prepare_file_destination(&self) -> Result<PreparedFileDestination, PathFailure> {
        self.root_bindings
            .prepare_file_destination(&self.project_path)
            .map_err(map_path_failure)
    }
}

#[derive(Clone, Debug)]
pub struct LoadProjectRequest {
    location: ProjectLocation,
}

impl LoadProjectRequest {
    pub fn new(location: ProjectLocation) -> Self {
        Self { location }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadProjectError {
    Path(PathFailure),
    Document(DocumentFailure),
    ExternalCopyRequiresInteractiveResolution,
    IdentityIndeterminate,
}

pub(crate) struct LoadedStoredRevision {
    pub(crate) revision: ProjectRevision,
    pub(crate) physical_identity: Option<PhysicalFileIdentity>,
    pub(crate) project_path: PathBuf,
    pub(crate) root_bindings: RootBindingPlan,
    pub(crate) resolved_object: ResolvedObject,
}

pub(crate) struct DecodedStoredRevision {
    pub(crate) revision: ProjectRevision,
    pub(crate) requires_schema_upgrade: bool,
}

#[derive(Debug)]
pub(crate) enum DecodeFailure {
    Path(PathFailure),
    Document(DocumentFailure),
}

pub(crate) fn load(request: LoadProjectRequest) -> Result<LoadedStoredRevision, LoadProjectError> {
    read(&request.location).map_err(map_load_error)
}

pub(crate) fn read(location: &ProjectLocation) -> Result<LoadedStoredRevision, DecodeFailure> {
    let resolved = location
        .root_bindings
        .resolve_existing(&location.project_path, ExpectedObject::RegularFile)
        .map_err(|error| DecodeFailure::Path(map_path_failure(error)))?;
    let physical_identity = resolved.physical_identity();
    let source = resolved.read_to_string().map_err(map_read_error)?;
    versioned_codec::decode(source.as_bytes()).map(|decoded| LoadedStoredRevision {
        revision: decoded.revision,
        physical_identity,
        project_path: location.project_path.clone(),
        root_bindings: location.root_bindings.clone(),
        resolved_object: resolved,
    })
}

pub(crate) fn decode(bytes: &[u8]) -> Result<ProjectRevision, DecodeFailure> {
    decode_with_metadata(bytes).map(|decoded| decoded.revision)
}

pub(crate) fn decode_with_metadata(bytes: &[u8]) -> Result<DecodedStoredRevision, DecodeFailure> {
    versioned_codec::decode(bytes).map(|decoded| DecodedStoredRevision {
        revision: decoded.revision,
        requires_schema_upgrade: decoded.source_schema_version < versioned_codec::SCHEMA_VERSION_V2,
    })
}

fn encode(revision: &ProjectRevision) -> Result<Vec<u8>, DecodeFailure> {
    versioned_codec::encode(revision)
}

pub(crate) fn map_path_failure(error: ResolveError) -> PathFailure {
    match error {
        ResolveError::InvalidPath
        | ResolveError::UnsupportedNamespace
        | ResolveError::UnboundRoot => PathFailure::InvalidPath,
        ResolveError::NotFound => PathFailure::NotFound,
        ResolveError::AccessDenied => PathFailure::AccessDenied,
        ResolveError::Unavailable => PathFailure::Unavailable,
        ResolveError::UnexpectedObjectType { .. } => PathFailure::UnexpectedObjectType,
        ResolveError::IoFailure => PathFailure::IoFailure,
    }
}

pub(crate) fn map_read_error(error: std::io::Error) -> DecodeFailure {
    if error.kind() == std::io::ErrorKind::InvalidData {
        DecodeFailure::Document(DocumentFailure::InvalidProjectDocument)
    } else {
        DecodeFailure::Path(PathFailure::IoFailure)
    }
}

pub(crate) fn map_load_error(error: DecodeFailure) -> LoadProjectError {
    match error {
        DecodeFailure::Path(error) => LoadProjectError::Path(error),
        DecodeFailure::Document(error) => LoadProjectError::Document(error),
    }
}
