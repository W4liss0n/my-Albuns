use std::path::Path;

use myalbuns_paths::OperationPathContext;
use uuid::Uuid;

use crate::{
    composition::build_render_snapshot,
    model::{CoreError, EditorProjection, ProjectIntent, RenderSnapshot},
    persistent_projection,
    persistent_session::PersistentProjectSession,
    project::ProjectCore,
    project_document::{InitialProject, ProjectDocument, ProjectRevision},
    project_store::{
        self, CreateStoreError, DocumentFailure, IdentityLeaseError, IdentityLeaseObservation,
        IdentityRegistryLookup, OpenStoreError, PathFailure, ProjectIdentityLease,
        ProjectIdentityRegistry, ProjectLocation, ProjectStore, SaveStoreError, SaveStoreResult,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveProjectOutcome {
    Saved { revision: u64 },
    AlreadyCurrent { revision: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveProjectError {
    StaleRevision { expected: u64, current: u64 },
    PersistedBaselineConflict,
    Path(PathFailure),
    SaveStateIndeterminate,
}

/// Opaque proof that one editable Project passed the identity-opening barrier.
///
/// Read-only loads never produce this value. Local state keyed by Project
/// identity must require it instead of accepting an ID parsed from a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIdentityAuthority {
    project_id: Uuid,
}

impl ProjectIdentityAuthority {
    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    fn authorized(project_id: Uuid) -> Self {
        Self { project_id }
    }
}

#[derive(Debug)]
pub struct EditableProject {
    session: PersistentProjectSession,
    store: ProjectStore,
    identity_lease: ProjectIdentityLease,
    identity_authority: ProjectIdentityAuthority,
    session_valid: bool,
}

impl EditableProject {
    pub fn project_id(&self) -> Uuid {
        self.session.project_id()
    }

    pub fn identity_authority(&self) -> &ProjectIdentityAuthority {
        &self.identity_authority
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
        self.session_valid && self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session_valid && self.session.can_redo()
    }

    /// Initial read-only editor view of the productive v1 document.
    ///
    /// Creative mutations are added by the following vertical slices; this
    /// projection intentionally contains no content from the temporary demo.
    pub fn projection(&self) -> EditorProjection {
        let project_name = self
            .project_path()
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Projeto".into());
        persistent_projection::editor_projection(
            self.project_id(),
            self.revision(),
            self.saved_revision(),
            self.can_undo(),
            self.can_redo(),
            &project_name,
            self.project(),
        )
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        let projection = self.projection();
        build_render_snapshot(
            &projection.state.project_id,
            &projection.state.project_name,
            projection.state.revision,
            projection.state.document.dpi,
            &projection.state.album,
        )
    }

    pub fn apply(&mut self, intent: ProjectIntent) -> Result<EditorProjection, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        self.session.apply(intent)?;
        Ok(self.projection())
    }

    pub fn undo(&mut self) -> Option<EditorProjection> {
        if !self.session_valid {
            return None;
        }
        self.session.undo()?;
        Some(self.projection())
    }

    pub fn redo(&mut self) -> Option<EditorProjection> {
        if !self.session_valid {
            return None;
        }
        self.session.redo()?;
        Some(self.projection())
    }

    pub fn save(&mut self, expected_revision: u64) -> Result<SaveProjectOutcome, SaveProjectError> {
        if !self.session_valid {
            return Err(SaveProjectError::SaveStateIndeterminate);
        }
        let current = self.revision();
        if expected_revision != current {
            return Err(SaveProjectError::StaleRevision {
                expected: expected_revision,
                current,
            });
        }
        if !self.session.requires_save() {
            return Ok(SaveProjectOutcome::AlreadyCurrent { revision: current });
        }
        let candidate = self.session.current_revision();
        match self.store.save(candidate, &self.identity_lease) {
            SaveStoreResult::Saved(receipt) => {
                if self.session.confirm_saved(receipt.candidate()).is_err() {
                    self.invalidate_session();
                    return Err(SaveProjectError::SaveStateIndeterminate);
                }
                Ok(SaveProjectOutcome::Saved {
                    revision: receipt.candidate().revision,
                })
            }
            SaveStoreResult::NotSaved(SaveStoreError::PersistedBaselineConflict) => {
                Err(SaveProjectError::PersistedBaselineConflict)
            }
            SaveStoreResult::NotSaved(SaveStoreError::Path(error)) => {
                Err(SaveProjectError::Path(error))
            }
            SaveStoreResult::StateIndeterminate => {
                self.invalidate_session();
                Err(SaveProjectError::SaveStateIndeterminate)
            }
        }
    }

    fn invalidate_session(&mut self) {
        self.session_valid = false;
        self.store.invalidate();
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
        let CreateProjectRequest {
            location,
            initial_project,
            authorization,
        } = request;
        let project = initial_project
            .into_project()
            .map_err(|_| CreateProjectError::InvalidInitialProject)?;
        let lease_root = self
            .identity_lease_root()
            .ok_or(CreateProjectError::Path(PathFailure::IoFailure))?;
        let revision = ProjectRevision::new(Uuid::new_v4(), 0, project);
        let identity_lease = ProjectIdentityLease::acquire(lease_root, revision.project_id)
            .map_err(map_new_identity_lease_error)?;

        let store_result = match authorization {
            CreateAuthorization::CreateOnly => {
                project_store::create_only(location, &revision).map_err(map_create_store_error)
            }
            CreateAuthorization::ReplaceConfirmed => {
                project_store::prepare_replacement(location, &revision)
                    .map_err(map_create_store_error)
                    .and_then(|prepared| {
                        let _replaced_identity_lease = prepared
                            .replaced_project_id()
                            .map(|project_id| ProjectIdentityLease::acquire(lease_root, project_id))
                            .transpose()
                            .map_err(map_replaced_identity_lease_error)?;
                        prepared.publish().map_err(map_create_store_error)
                    })
            }
        };
        let store = match store_result {
            Ok(store) => store,
            Err(error) => {
                identity_lease.discard_unpublished();
                return Err(error);
            }
        };
        if bind_identity_target(&identity_lease, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::IdentityIndeterminate);
        }
        if publish_identity_location(self, revision.project_id, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::CreateStateIndeterminate);
        }

        let identity_authority = ProjectIdentityAuthority::authorized(identity_lease.project_id());
        Ok(EditableProject {
            session: PersistentProjectSession::from_persisted(revision, false),
            store,
            identity_lease,
            identity_authority,
            session_valid: true,
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
        authorize_opened_identity(self, opened.revision.project_id, &opened.store)?;
        bind_identity_target(&identity_lease, &opened.store)
            .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
        let identity_authority = ProjectIdentityAuthority::authorized(identity_lease.project_id());
        Ok(EditableProject {
            session: PersistentProjectSession::from_persisted(
                opened.revision,
                opened.requires_schema_upgrade,
            ),
            store: opened.store,
            identity_lease,
            identity_authority,
            session_valid: true,
        })
    }
}

fn identity_registry(core: &ProjectCore) -> Result<ProjectIdentityRegistry, ()> {
    core.identity_registry_root()
        .map(|root| ProjectIdentityRegistry::new(root.to_path_buf()))
        .ok_or(())
}

fn publish_identity_location(
    core: &ProjectCore,
    project_id: Uuid,
    store: &ProjectStore,
) -> Result<(), ()> {
    identity_registry(core)?
        .publish(project_id, store.location().project_path())
        .map_err(|_| ())
}

fn authorize_opened_identity(
    core: &ProjectCore,
    project_id: Uuid,
    store: &ProjectStore,
) -> Result<(), OpenProjectError> {
    let registry = identity_registry(core).map_err(|()| OpenProjectError::IdentityIndeterminate)?;
    let previous_location = match registry
        .lookup(project_id)
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?
    {
        IdentityRegistryLookup::Missing => {
            return registry
                .publish(project_id, store.location().project_path())
                .map_err(|_| OpenProjectError::IdentityIndeterminate);
        }
        IdentityRegistryLookup::Location(location) => location,
    };

    let mut context = OperationPathContext::new();
    context
        .capture(&previous_location)
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
    let previous = project_store::read(&ProjectLocation::new(previous_location, context.freeze()));
    match previous {
        Ok(previous) if previous.revision.project_id == project_id => {
            match (previous.physical_identity, store.physical_identity()) {
                (Some(previous), Some(candidate)) => {
                    if previous == candidate {
                        Ok(())
                    } else {
                        Err(OpenProjectError::ExternalCopyRequiresInteractiveResolution)
                    }
                }
                _ => Err(OpenProjectError::IdentityIndeterminate),
            }
        }
        Ok(_) => Err(OpenProjectError::IdentityIndeterminate),
        Err(project_store::DecodeFailure::Path(PathFailure::NotFound)) => {
            Err(OpenProjectError::IdentityIndeterminate)
        }
        Err(_) => Err(OpenProjectError::IdentityIndeterminate),
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

#[cfg(test)]
mod save_tests {
    use myalbuns_paths::OperationPathContext;

    use super::{CreateAuthorization, CreateProjectRequest, OpenProjectRequest, SaveProjectError};
    use crate::{CoreError, InitialProject, ProjectCore, ProjectIntent, ProjectLocation};

    fn location(path: &std::path::Path) -> ProjectLocation {
        let mut context = OperationPathContext::new();
        context
            .capture(path)
            .expect("the test Project root is captured");
        ProjectLocation::new(path.to_path_buf(), context.freeze())
    }

    #[test]
    fn projection_and_render_snapshot_derive_the_non_ascii_project_name_from_its_native_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let project_path = directory.path().join("Casamento da Júlia.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            directory.path().join("leases"),
            directory.path().join("identities"),
        );
        let project = core
            .create_editable(CreateProjectRequest::new(
                location(&project_path),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the named Project opens");

        assert_eq!(project.project_path(), project_path);
        assert_eq!(
            project.projection().state.project_name,
            "Casamento da Júlia"
        );
        assert_eq!(project.render_snapshot().project_name, "Casamento da Júlia");

        drop(project);
        let reopened = core
            .open_editable(OpenProjectRequest::new(location(&project_path)))
            .expect("the named Project reopens");
        assert_eq!(
            reopened.projection().state.project_name,
            "Casamento da Júlia"
        );
    }

    #[test]
    fn an_indeterminate_post_publication_state_is_not_confirmed_and_requires_reopening() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let project_path = directory.path().join("Estado inconclusivo.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            directory.path().join("leases"),
            directory.path().join("identities"),
        );
        let mut project = core
            .create_editable(CreateProjectRequest::new(
                location(&project_path),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the productive Project opens");
        project
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the visible revision advances");
        crate::project_store::inject_post_publication_indeterminate_for_current_thread();

        assert_eq!(
            project
                .save(1)
                .expect_err("an unverified publication never confirms the candidate"),
            SaveProjectError::SaveStateIndeterminate
        );
        let unconfirmed = project.projection();
        assert_eq!(unconfirmed.state.revision, 1);
        assert_eq!(unconfirmed.state.saved_revision, 0);
        assert!(unconfirmed.state.dirty);
        assert!(!unconfirmed.state.can_undo);
        assert!(!unconfirmed.state.can_redo);
        assert_eq!(
            project
                .apply(ProjectIntent::SetDpi { dpi: 600 })
                .expect_err("an invalidated editable Session rejects creative mutations"),
            CoreError::EditableSessionInvalidated
        );
        assert!(project.undo().is_none());
        assert!(project.redo().is_none());
        assert_eq!(project.projection(), unconfirmed);
        assert_eq!(
            project
                .save(1)
                .expect_err("the invalidated Store cannot be retried"),
            SaveProjectError::SaveStateIndeterminate
        );

        crate::project_store::release_post_publication_indeterminate_for_current_thread();
        drop(project);
        let reopened = core
            .open_editable(OpenProjectRequest::new(location(&project_path)))
            .expect("reopening establishes which complete revision reached the pathname");
        assert_eq!(reopened.revision(), 1);
        assert_eq!(reopened.saved_revision(), 1);
        assert_eq!(reopened.project().document().dpi(), 240);
        assert!(!reopened.has_unsaved_changes());
        assert!(!reopened.can_undo());
        assert!(!reopened.can_redo());
    }
}
