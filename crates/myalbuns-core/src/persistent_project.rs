use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use myalbuns_paths::{ExpectedObject, OperationPathContext, PhysicalIdentityEvidence};
use uuid::Uuid;

use crate::{
    composition::build_render_snapshot,
    model::{
        ComposedOutputUnit, CoreError, EditorProjection, MediaId, ProjectIntent, RenderSnapshot,
    },
    persistent_projection,
    persistent_session::PersistentProjectSession,
    project_document::{InitialProject, MediaRef, ProjectDocument, ProjectRevision},
    project_store::{
        self, CreateStoreError, DocumentFailure, IdentityLeaseError, IdentityLeaseObservation,
        IdentityRegistryLookup, OpenStoreError, PathFailure, ProjectIdentityLease,
        ProjectIdentityRegistry, ProjectLocation, ProjectStore, SaveStoreError, SaveStoreResult,
    },
};

/// Small public seam for productive Project persistence and editable ownership.
///
/// Session, store and identity coordination remain private. Each process
/// configures its live lease and durable identity roots, then creates, opens or
/// loads Projects through this type.
#[derive(Clone, Debug, Default)]
pub struct ProjectCore {
    identity_lease_root: Option<PathBuf>,
    identity_registry_root: Option<PathBuf>,
}

impl ProjectCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_identity_storage_roots(
        mut self,
        identity_lease_root: PathBuf,
        identity_registry_root: PathBuf,
    ) -> Self {
        self.identity_lease_root = Some(identity_lease_root);
        self.identity_registry_root = Some(identity_registry_root);
        self
    }

    fn identity_lease_root(&self) -> Option<&Path> {
        self.identity_lease_root.as_deref()
    }

    fn identity_registry_root(&self) -> Option<&Path> {
        self.identity_registry_root.as_deref()
    }
}

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

#[derive(Clone, Debug)]
pub struct FrozenProjectRendering {
    projection: EditorProjection,
    render_snapshot: RenderSnapshot,
    sources: Vec<MediaRef>,
}

#[derive(Clone, Debug)]
pub struct FrozenSheetRendering {
    render_snapshot: RenderSnapshot,
    output_unit: ComposedOutputUnit,
    sources: Vec<MediaRef>,
}

impl FrozenProjectRendering {
    pub fn projection(&self) -> &EditorProjection {
        &self.projection
    }

    pub fn render_snapshot(&self) -> &RenderSnapshot {
        &self.render_snapshot
    }

    pub fn sources(&self) -> &[MediaRef] {
        &self.sources
    }

    pub fn into_parts(self) -> (EditorProjection, RenderSnapshot, Vec<MediaRef>) {
        (self.projection, self.render_snapshot, self.sources)
    }

    pub fn into_sheet(self, sheet_id: &str) -> Result<FrozenSheetRendering, CoreError> {
        let output_unit = self.render_snapshot.output_unit(sheet_id)?;
        let referenced = output_unit
            .sheet
            .referenced_media_ids()
            .collect::<HashSet<_>>();
        let sources = self
            .sources
            .into_iter()
            .filter(|source| referenced.contains(&MediaId::from_uuid(source.id())))
            .collect::<Vec<_>>();
        if sources.len() != referenced.len() {
            return Err(CoreError::InvalidSnapshot(
                "a composição congelada referencia uma fonte ausente".into(),
            ));
        }
        Ok(FrozenSheetRendering {
            render_snapshot: self.render_snapshot,
            output_unit,
            sources,
        })
    }
}

impl FrozenSheetRendering {
    pub fn render_snapshot(&self) -> &RenderSnapshot {
        &self.render_snapshot
    }

    pub fn output_unit(&self) -> &ComposedOutputUnit {
        &self.output_unit
    }

    pub fn sources(&self) -> &[MediaRef] {
        &self.sources
    }

    pub fn into_parts(self) -> (RenderSnapshot, ComposedOutputUnit, Vec<MediaRef>) {
        (self.render_snapshot, self.output_unit, self.sources)
    }
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

    /// Freezes the public editor/render projections and only the exact linked
    /// originals referenced by that same creative Revision.
    pub fn freeze_rendering(&self) -> FrozenProjectRendering {
        let projection = self.projection();
        let render_snapshot = build_render_snapshot(
            &projection.state.project_id,
            &projection.state.project_name,
            projection.state.revision,
            projection.state.document.dpi,
            &projection.state.album,
        );
        let referenced = render_snapshot
            .composition
            .sheets
            .iter()
            .flat_map(|sheet| sheet.referenced_media_ids())
            .collect::<HashSet<_>>();
        let sources = self
            .project()
            .media()
            .iter()
            .filter(|media| referenced.contains(&MediaId::from_uuid(media.id())))
            .cloned()
            .collect();

        FrozenProjectRendering {
            projection,
            render_snapshot,
            sources,
        }
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
        #[cfg(test)]
        wait_at_identity_authorization_test_barrier(&loaded.project_path);
        authorize_loaded_identity(self, &loaded)?;
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
        #[cfg(test)]
        wait_at_identity_authorization_test_barrier(store.location().project_path());
        if !store.location_still_matches_baseline() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::IdentityIndeterminate);
        }
        if publish_identity_location(self, revision.project_id, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::CreateStateIndeterminate);
        }
        if bind_identity_target(&identity_lease, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::IdentityIndeterminate);
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
        #[cfg(test)]
        wait_at_identity_authorization_test_barrier(opened.store.location().project_path());
        if !opened.store.location_still_matches_baseline() {
            return Err(OpenProjectError::IdentityIndeterminate);
        }
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

fn authorize_loaded_identity(
    core: &ProjectCore,
    loaded: &project_store::LoadedStoredRevision,
) -> Result<(), project_store::LoadProjectError> {
    let Some(lease_root) = core.identity_lease_root() else {
        return Ok(());
    };
    let project_id = loaded.revision.project_id;
    const AUTHORIZATION_WAIT_LIMIT: usize = 400;
    const AUTHORIZATION_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(5);

    for _ in 0..AUTHORIZATION_WAIT_LIMIT {
        match ProjectIdentityLease::observe(lease_root, project_id, loaded.physical_identity) {
            Ok(IdentityLeaseObservation::SamePhysicalTarget) => return Ok(()),
            Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                return Err(
                    project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution,
                );
            }
            Ok(IdentityLeaseObservation::Inactive) => {}
            Ok(IdentityLeaseObservation::Pending) => {
                std::thread::sleep(AUTHORIZATION_RETRY_DELAY);
                continue;
            }
            Err(_) => return Err(project_store::LoadProjectError::IdentityIndeterminate),
        }
        let lease = match ProjectIdentityLease::acquire(lease_root, project_id) {
            Ok(lease) => lease,
            Err(IdentityLeaseError::Conflict) => {
                std::thread::sleep(AUTHORIZATION_RETRY_DELAY);
                continue;
            }
            Err(IdentityLeaseError::Unavailable) => {
                return Err(project_store::LoadProjectError::IdentityIndeterminate);
            }
        };
        let physical_identity = loaded
            .physical_identity
            .ok_or(project_store::LoadProjectError::IdentityIndeterminate)?;
        let current_candidate = loaded
            .root_bindings
            .resolve_existing(&loaded.project_path, ExpectedObject::RegularFile)
            .map_err(|_| project_store::LoadProjectError::IdentityIndeterminate)?;
        if loaded.resolved_object.compare_physical(&current_candidate)
            != PhysicalIdentityEvidence::Same
        {
            lease.discard_unpublished();
            return Err(project_store::LoadProjectError::IdentityIndeterminate);
        }
        if let Err(error) = authorize_identity_candidate(
            core,
            project_id,
            &loaded.project_path,
            loaded.physical_identity,
        ) {
            lease.discard_unpublished();
            return Err(map_load_identity_error(error));
        }
        if lease.bind_target(physical_identity).is_err() {
            lease.discard_unpublished();
            return Err(project_store::LoadProjectError::IdentityIndeterminate);
        }
        return Ok(());
    }
    Err(project_store::LoadProjectError::IdentityIndeterminate)
}

#[cfg(test)]
struct IdentityAuthorizationTestBarrier {
    id: Uuid,
    reached: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
static IDENTITY_AUTHORIZATION_TEST_BARRIER: std::sync::OnceLock<
    std::sync::Mutex<
        std::collections::HashMap<std::path::PathBuf, IdentityAuthorizationTestBarrier>,
    >,
> = std::sync::OnceLock::new();

#[cfg(test)]
fn wait_at_identity_authorization_test_barrier(project_path: &Path) {
    let barrier = {
        let mut installed = IDENTITY_AUTHORIZATION_TEST_BARRIER
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        installed.remove(project_path)
    };
    let Some(barrier) = barrier else { return };
    barrier
        .reached
        .send(())
        .expect("the identity authorization test observes the loaded handle");
    barrier
        .release
        .recv()
        .expect("the identity authorization test releases classification");
}

#[cfg(test)]
fn install_identity_authorization_test_barrier(
    project_path: std::path::PathBuf,
    reached: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
) -> Uuid {
    let id = Uuid::new_v4();
    let previous = IDENTITY_AUTHORIZATION_TEST_BARRIER
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            project_path.clone(),
            IdentityAuthorizationTestBarrier {
                id,
                reached,
                release,
            },
        );
    assert!(
        previous.is_none(),
        "one test barrier owns each Project pathname"
    );
    id
}

#[cfg(test)]
fn clear_identity_authorization_test_barrier(id: Uuid) {
    let mut installed = IDENTITY_AUTHORIZATION_TEST_BARRIER
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    installed.retain(|_, barrier| barrier.id != id);
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
    authorize_identity_candidate(
        core,
        project_id,
        store.location().project_path(),
        store.physical_identity(),
    )
    .map_err(map_open_identity_candidate_error)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityCandidateError {
    ExternalCopy,
    Indeterminate,
}

fn authorize_identity_candidate(
    core: &ProjectCore,
    project_id: Uuid,
    candidate_location: &Path,
    candidate_physical_identity: Option<myalbuns_paths::PhysicalFileIdentity>,
) -> Result<(), IdentityCandidateError> {
    let registry = identity_registry(core).map_err(|()| IdentityCandidateError::Indeterminate)?;
    let previous_location = match registry
        .lookup(project_id)
        .map_err(|_| IdentityCandidateError::Indeterminate)?
    {
        IdentityRegistryLookup::Missing => {
            return registry
                .publish(project_id, candidate_location)
                .map_err(|_| IdentityCandidateError::Indeterminate);
        }
        IdentityRegistryLookup::Location(location) => location,
    };

    let mut context = OperationPathContext::new();
    context
        .capture(&previous_location)
        .map_err(|_| IdentityCandidateError::Indeterminate)?;
    let previous = project_store::read(&ProjectLocation::new(previous_location, context.freeze()));
    match previous {
        Ok(previous) if previous.revision.project_id == project_id => {
            match (previous.physical_identity, candidate_physical_identity) {
                (Some(previous), Some(candidate)) => {
                    if previous == candidate {
                        Ok(())
                    } else {
                        Err(IdentityCandidateError::ExternalCopy)
                    }
                }
                _ => Err(IdentityCandidateError::Indeterminate),
            }
        }
        Ok(_) => Err(IdentityCandidateError::Indeterminate),
        Err(project_store::DecodeFailure::Path(PathFailure::NotFound)) => {
            Err(IdentityCandidateError::Indeterminate)
        }
        Err(_) => Err(IdentityCandidateError::Indeterminate),
    }
}

fn map_open_identity_candidate_error(error: IdentityCandidateError) -> OpenProjectError {
    match error {
        IdentityCandidateError::ExternalCopy => {
            OpenProjectError::ExternalCopyRequiresInteractiveResolution
        }
        IdentityCandidateError::Indeterminate => OpenProjectError::IdentityIndeterminate,
    }
}

fn map_load_identity_error(error: IdentityCandidateError) -> project_store::LoadProjectError {
    match error {
        IdentityCandidateError::ExternalCopy => {
            project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution
        }
        IdentityCandidateError::Indeterminate => {
            project_store::LoadProjectError::IdentityIndeterminate
        }
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

    use super::{
        CreateAuthorization, CreateProjectError, CreateProjectRequest, OpenProjectError,
        OpenProjectRequest, SaveProjectError, clear_identity_authorization_test_barrier,
        install_identity_authorization_test_barrier,
    };
    use crate::{
        CoreError, InitialProject, LoadProjectError, LoadProjectRequest, ProjectCore,
        ProjectIntent, ProjectLocation,
    };

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

    #[test]
    fn read_only_first_observation_rejects_a_path_replaced_after_its_handle_was_read() {
        let directory = tempfile::tempdir().expect("temporary identity race directory");
        let project_path = directory.path().join("Projeto observado.myalbuns");
        let displaced_path = directory.path().join("Projeto anterior.myalbuns");
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
            .expect("the first physical Project is created");
        drop(project);
        std::fs::remove_dir_all(directory.path().join("identities"))
            .expect("the fixture resets only its durable identity evidence");
        let replacement_bytes = std::fs::read(&project_path).expect("the Project bytes are read");
        let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let barrier_id = install_identity_authorization_test_barrier(
            project_path.clone(),
            reached_tx,
            release_rx,
        );
        let load_core = core.clone();
        let load_path = project_path.clone();
        let attempt = std::thread::spawn(move || {
            load_core.load_persisted_revision(LoadProjectRequest::new(location(&load_path)))
        });
        reached_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the first object was read before identity publication");
        std::fs::rename(&project_path, &displaced_path)
            .expect("the opened object is displaced atomically");
        std::fs::write(&project_path, replacement_bytes)
            .expect("a different physical object occupies the pathname");
        release_tx
            .send(())
            .expect("identity classification resumes");

        assert_eq!(
            attempt
                .join()
                .expect("the read-only attempt does not panic")
                .expect_err("the pathname no longer identifies the decoded object"),
            LoadProjectError::IdentityIndeterminate
        );
        clear_identity_authorization_test_barrier(barrier_id);
    }

    #[test]
    fn editable_open_rejects_a_path_replaced_after_its_baseline_was_locked() {
        let directory = tempfile::tempdir().expect("temporary open race directory");
        let project_path = directory.path().join("Projeto aberto.myalbuns");
        let displaced_path = directory.path().join("Projeto aberto anterior.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            directory.path().join("leases"),
            directory.path().join("identities"),
        );
        let created = core
            .create_editable(CreateProjectRequest::new(
                location(&project_path),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable fixture is created");
        drop(created);
        let replacement_bytes = std::fs::read(&project_path).expect("the Project bytes are read");
        let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let barrier_id = install_identity_authorization_test_barrier(
            project_path.clone(),
            reached_tx,
            release_rx,
        );
        let open_core = core.clone();
        let open_path = project_path.clone();
        let attempt = std::thread::spawn(move || {
            open_core.open_editable(OpenProjectRequest::new(location(&open_path)))
        });
        reached_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the editable baseline is locked before identity publication");
        std::fs::rename(&project_path, &displaced_path)
            .expect("the locked object is displaced atomically");
        std::fs::write(&project_path, replacement_bytes)
            .expect("a different physical object occupies the pathname");
        release_tx.send(()).expect("editable authorization resumes");

        assert_eq!(
            attempt
                .join()
                .expect("the editable attempt does not panic")
                .expect_err("the pathname no longer identifies the locked baseline"),
            OpenProjectError::IdentityIndeterminate
        );
        clear_identity_authorization_test_barrier(barrier_id);
    }

    #[test]
    fn editable_create_rejects_a_path_replaced_before_identity_publication() {
        let directory = tempfile::tempdir().expect("temporary create race directory");
        let project_path = directory.path().join("Projeto criado.myalbuns");
        let displaced_path = directory.path().join("Projeto criado anterior.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            directory.path().join("leases"),
            directory.path().join("identities"),
        );
        let (reached_tx, reached_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
        let barrier_id = install_identity_authorization_test_barrier(
            project_path.clone(),
            reached_tx,
            release_rx,
        );
        let create_core = core.clone();
        let create_path = project_path.clone();
        let attempt = std::thread::spawn(move || {
            create_core.create_editable(CreateProjectRequest::new(
                location(&create_path),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
        });
        reached_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the created baseline is locked before identity publication");
        let replacement_bytes = std::fs::read(&project_path).expect("the created bytes are read");
        std::fs::rename(&project_path, &displaced_path)
            .expect("the created object is displaced atomically");
        std::fs::write(&project_path, replacement_bytes)
            .expect("a different physical object occupies the pathname");
        release_tx.send(()).expect("creation authorization resumes");

        assert_eq!(
            attempt
                .join()
                .expect("the creation attempt does not panic")
                .expect_err("the pathname no longer identifies the created baseline"),
            CreateProjectError::IdentityIndeterminate
        );
        clear_identity_authorization_test_barrier(barrier_id);
    }
}
