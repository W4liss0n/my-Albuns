use std::path::Path;

use uuid::Uuid;

use crate::{
    persistent_session::PersistentProjectSession,
    project::ProjectCore,
    project_document::{InitialProject, ProjectDocument, ProjectRevision},
    project_store::{
        self, CreateStoreError, DocumentFailure, IdentityLeaseError, IdentityLeaseObservation,
        OpenStoreError, PathFailure, ProjectIdentityLease, ProjectLocation, ProjectStore,
    },
};

#[derive(Debug)]
pub struct LoadedProjectRevision {
    revision: ProjectRevision,
}

impl LoadedProjectRevision {
    pub fn project_id(&self) -> Uuid {
        self.revision.project_id
    }

    pub fn revision(&self) -> u64 {
        self.revision.revision
    }

    pub fn project(&self) -> &ProjectDocument {
        &self.revision.project
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateAuthorization {
    CreateOnly,
    ReplaceConfirmed,
}

#[derive(Clone, Debug)]
pub struct CreateProjectRequest {
    location: ProjectLocation,
    initial_project: InitialProject,
    authorization: CreateAuthorization,
}

impl CreateProjectRequest {
    pub fn new(
        location: ProjectLocation,
        initial_project: InitialProject,
        authorization: CreateAuthorization,
    ) -> Self {
        Self {
            location,
            initial_project,
            authorization,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenProjectRequest {
    location: ProjectLocation,
}

impl OpenProjectRequest {
    pub fn new(location: ProjectLocation) -> Self {
        Self { location }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateProjectError {
    InvalidInitialProject,
    Path(PathFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    CreateStateIndeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenProjectError {
    Path(PathFailure),
    Document(DocumentFailure),
    ProjectInUse,
    ExternalCopyRequiresInteractiveResolution,
    IdentityIndeterminate,
}

#[derive(Debug)]
pub struct EditableProject {
    session: PersistentProjectSession,
    store: ProjectStore,
    _identity_lease: ProjectIdentityLease,
}

impl EditableProject {
    pub fn project_id(&self) -> Uuid {
        self.session.project_id()
    }

    pub fn revision(&self) -> u64 {
        self.session.revision()
    }

    pub fn saved_revision(&self) -> u64 {
        self.session.saved_revision()
    }

    pub fn project(&self) -> &ProjectDocument {
        self.session.project()
    }

    pub fn project_path(&self) -> &Path {
        self.store.location().project_path()
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.session.has_unsaved_changes()
    }

    pub fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.can_redo()
    }
}

impl ProjectCore {
    pub fn load_persisted_revision(
        &self,
        request: project_store::LoadProjectRequest,
    ) -> Result<LoadedProjectRevision, project_store::LoadProjectError> {
        let loaded = project_store::load(request)?;
        if let Some(lease_root) = self.identity_lease_root() {
            match ProjectIdentityLease::observe(
                lease_root,
                loaded.revision.project_id,
                loaded.physical_identity,
            ) {
                Ok(IdentityLeaseObservation::Inactive)
                | Ok(IdentityLeaseObservation::SamePhysicalTarget) => {}
                Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                    return Err(
                        project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution,
                    );
                }
                Err(_) => {
                    return Err(project_store::LoadProjectError::IdentityIndeterminate);
                }
            }
        }
        Ok(LoadedProjectRevision {
            revision: loaded.revision,
        })
    }

    pub fn create_editable(
        &self,
        request: CreateProjectRequest,
    ) -> Result<EditableProject, CreateProjectError> {
        let lease_root = self
            .identity_lease_root()
            .ok_or(CreateProjectError::Path(PathFailure::IoFailure))?;
        let revision =
            ProjectRevision::new(Uuid::new_v4(), 0, request.initial_project.into_project());
        let identity_lease = ProjectIdentityLease::acquire(lease_root, revision.project_id)
            .map_err(map_new_identity_lease_error)?;

        let store = match request.authorization {
            CreateAuthorization::CreateOnly => {
                project_store::create_only(request.location, &revision)
                    .map_err(map_create_store_error)?
            }
            CreateAuthorization::ReplaceConfirmed => {
                let prepared = project_store::prepare_replacement(request.location, &revision)
                    .map_err(map_create_store_error)?;
                let _replaced_identity_lease = prepared
                    .replaced_project_id()
                    .map(|project_id| ProjectIdentityLease::acquire(lease_root, project_id))
                    .transpose()
                    .map_err(map_replaced_identity_lease_error)?;
                prepared.publish().map_err(map_create_store_error)?
            }
        };
        bind_identity_target(&identity_lease, &store)
            .map_err(|_| CreateProjectError::IdentityIndeterminate)?;

        Ok(EditableProject {
            session: PersistentProjectSession::from_persisted(revision),
            store,
            _identity_lease: identity_lease,
        })
    }

    pub fn open_editable(
        &self,
        request: OpenProjectRequest,
    ) -> Result<EditableProject, OpenProjectError> {
        let lease_root = self
            .identity_lease_root()
            .ok_or(OpenProjectError::Path(PathFailure::IoFailure))?;
        let opened =
            project_store::open_editable(request.location).map_err(map_open_store_error)?;
        let identity_lease = ProjectIdentityLease::acquire(lease_root, opened.revision.project_id)
            .map_err(map_open_identity_lease_error)?;
        bind_identity_target(&identity_lease, &opened.store)
            .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
        Ok(EditableProject {
            session: PersistentProjectSession::from_persisted(opened.revision),
            store: opened.store,
            _identity_lease: identity_lease,
        })
    }
}

fn bind_identity_target(
    lease: &ProjectIdentityLease,
    store: &ProjectStore,
) -> Result<(), IdentityLeaseError> {
    let physical_identity = store
        .physical_identity()
        .ok_or(IdentityLeaseError::Unavailable)?;
    lease.bind_target(physical_identity)
}

fn map_create_store_error(error: CreateStoreError) -> CreateProjectError {
    match error {
        CreateStoreError::Path(error) => CreateProjectError::Path(error),
        CreateStoreError::Document(_) => CreateProjectError::InvalidInitialProject,
        CreateStoreError::DestinationConflict => CreateProjectError::DestinationConflict,
        CreateStoreError::ProjectInUse => CreateProjectError::ProjectInUse,
        CreateStoreError::IdentityIndeterminate => CreateProjectError::IdentityIndeterminate,
        CreateStoreError::StateIndeterminate => CreateProjectError::CreateStateIndeterminate,
    }
}

fn map_open_store_error(error: OpenStoreError) -> OpenProjectError {
    match error {
        OpenStoreError::Path(error) => OpenProjectError::Path(error),
        OpenStoreError::Document(error) => OpenProjectError::Document(error),
        OpenStoreError::ProjectInUse => OpenProjectError::ProjectInUse,
        OpenStoreError::IdentityIndeterminate => OpenProjectError::IdentityIndeterminate,
    }
}

fn map_new_identity_lease_error(error: IdentityLeaseError) -> CreateProjectError {
    match error {
        IdentityLeaseError::Conflict => CreateProjectError::IdentityIndeterminate,
        IdentityLeaseError::Unavailable => CreateProjectError::Path(PathFailure::IoFailure),
    }
}

fn map_replaced_identity_lease_error(error: IdentityLeaseError) -> CreateProjectError {
    match error {
        IdentityLeaseError::Conflict => CreateProjectError::ProjectInUse,
        IdentityLeaseError::Unavailable => CreateProjectError::Path(PathFailure::IoFailure),
    }
}

fn map_open_identity_lease_error(error: IdentityLeaseError) -> OpenProjectError {
    match error {
        IdentityLeaseError::Conflict => OpenProjectError::ExternalCopyRequiresInteractiveResolution,
        IdentityLeaseError::Unavailable => OpenProjectError::Path(PathFailure::IoFailure),
    }
}
