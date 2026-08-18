use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use myalbuns_paths::{
    ExpectedObject, OperationPathContext, PhysicalIdentityEvidence, ProcessInstanceId,
};
use uuid::Uuid;

use crate::{
    model::{
        ComposedOutputUnit, CoreError, EditorProjection, MediaId, ProjectIntent, RelinkMedia,
        RenderSnapshot, RenderSnapshotMetadata, RenderSnapshotRef,
    },
    persistent_projection,
    persistent_session::PersistentProjectSession,
    project_document::{InitialProject, MediaRef, ProjectDocument, ProjectRevision},
    project_store::{
        self, CreateStoreError, DocumentFailure, IdentityLeaseError, IdentityLeaseObservation,
        IdentityRegistryLookup, OpenStoreError, PathFailure, PendingProjectIdentityLease,
        ProjectIdentityLease, ProjectIdentityRegistry, ProjectLocation, ProjectStore,
        SaveStoreError, SaveStoreResult,
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

#[derive(Debug)]
pub enum OpenProjectError {
    Path(PathFailure),
    Document(DocumentFailure),
    ProjectInUse,
    FocusExisting {
        project_id: Uuid,
        owner_process: ProcessInstanceId,
    },
    ExternalCopyRequiresInteractiveResolution,
    ExternalCopyNotWritable(Box<ExternalCopySource>),
    IdentityIndeterminate,
}

impl PartialEq for OpenProjectError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Path(left), Self::Path(right)) => left == right,
            (Self::Document(left), Self::Document(right)) => left == right,
            (Self::ProjectInUse, Self::ProjectInUse)
            | (
                Self::ExternalCopyRequiresInteractiveResolution,
                Self::ExternalCopyRequiresInteractiveResolution,
            )
            | (Self::ExternalCopyNotWritable(_), Self::ExternalCopyNotWritable(_))
            | (Self::IdentityIndeterminate, Self::IdentityIndeterminate) => true,
            (
                Self::FocusExisting {
                    project_id: left_id,
                    owner_process: left_process,
                },
                Self::FocusExisting {
                    project_id: right_id,
                    owner_process: right_process,
                },
            ) => left_id == right_id && left_process == right_process,
            _ => false,
        }
    }
}

impl Eq for OpenProjectError {}

#[derive(Debug)]
pub struct ExternalCopySource {
    revision: ProjectRevision,
    store: ProjectStore,
}

#[derive(Debug)]
pub struct SaveCopyAsRequest {
    source: ExternalCopySource,
    destination: ProjectLocation,
    authorization: CreateAuthorization,
}

impl SaveCopyAsRequest {
    pub fn new(
        source: ExternalCopySource,
        destination: ProjectLocation,
        authorization: CreateAuthorization,
    ) -> Self {
        Self {
            source,
            destination,
            authorization,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveCopyAsError {
    Path(PathFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    SaveCopyStateIndeterminate,
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

    pub fn render_snapshot(&self) -> RenderSnapshotRef<'_> {
        RenderSnapshotRef::from_resolved(
            RenderSnapshotMetadata::from(&self.projection.state),
            &self.projection.composition,
        )
    }

    pub fn sources(&self) -> &[MediaRef] {
        &self.sources
    }

    pub fn into_sheet(self, sheet_id: &str) -> Result<FrozenSheetRendering, CoreError> {
        let render_snapshot = RenderSnapshot::from_resolved(
            RenderSnapshotMetadata::from(&self.projection.state),
            self.projection.composition,
        );
        let output_unit = render_snapshot.output_unit(sheet_id)?;
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
            render_snapshot,
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
        RenderSnapshot::from_resolved(
            RenderSnapshotMetadata::from(&projection.state),
            projection.composition,
        )
    }

    /// Freezes one resolved editor projection and only the exact linked
    /// originals referenced by its CompositionPlan at that creative Revision.
    pub fn freeze_rendering(&self) -> FrozenProjectRendering {
        let projection = self.projection();
        let referenced = projection
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

    pub fn relink_media(&mut self, command: RelinkMedia) -> Result<EditorProjection, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        self.session.relink_media(command)?;
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
                project_store::create_only(location, &revision, lease_root)
                    .map_err(map_create_store_error)
            }
            CreateAuthorization::ReplaceConfirmed => {
                project_store::prepare_replacement(location, &revision, lease_root)
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
        if !store.location_still_matches_baseline() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::IdentityIndeterminate);
        }
        if publish_identity_location(self, revision.project_id, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(CreateProjectError::CreateStateIndeterminate);
        }
        let identity_lease = bind_identity_target(identity_lease, &store)
            .map_err(|_| CreateProjectError::IdentityIndeterminate)?;

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
        let opened = match project_store::open_editable(request.location, lease_root) {
            Ok(opened) => opened,
            Err(OpenStoreError::ProjectInUse {
                project_id,
                physical_identity,
            }) => {
                return Err(map_active_identity_observation(
                    lease_root,
                    project_id,
                    physical_identity,
                ));
            }
            Err(error) => return Err(map_open_store_error(error)),
        };
        let identity_lease =
            match ProjectIdentityLease::acquire(lease_root, opened.revision.project_id) {
                Ok(lease) => lease,
                Err(IdentityLeaseError::Conflict) => {
                    return match ProjectIdentityLease::observe(
                        lease_root,
                        opened.revision.project_id,
                        opened.store.physical_identity(),
                    ) {
                        Ok(IdentityLeaseObservation::SamePhysicalTarget { owner_process }) => {
                            Err(OpenProjectError::FocusExisting {
                                project_id: opened.revision.project_id,
                                owner_process,
                            })
                        }
                        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                            promote_external_copy(self, opened, None)
                        }
                        Ok(
                            IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending,
                        ) => Err(OpenProjectError::ProjectInUse),
                        Err(_) => Err(OpenProjectError::IdentityIndeterminate),
                    };
                }
                Err(IdentityLeaseError::Unavailable) => {
                    return Err(OpenProjectError::Path(PathFailure::IoFailure));
                }
            };
        if !opened.store.location_still_matches_baseline() {
            return Err(OpenProjectError::IdentityIndeterminate);
        }
        match authorize_identity_candidate(
            self,
            opened.revision.project_id,
            opened.store.location().project_path(),
            IdentityCandidateTarget::Editable(&opened.store),
        ) {
            Ok(()) => {}
            Err(IdentityCandidateError::ExternalCopy) => {
                return promote_external_copy(self, opened, Some(identity_lease));
            }
            Err(IdentityCandidateError::Indeterminate) => {
                return Err(OpenProjectError::IdentityIndeterminate);
            }
        }
        let identity_lease = bind_identity_target(identity_lease, &opened.store)
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
    match ProjectIdentityLease::observe(lease_root, project_id, loaded.physical_identity) {
        Ok(IdentityLeaseObservation::SamePhysicalTarget { .. }) => return Ok(()),
        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
            return Err(project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution);
        }
        Ok(IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending) => {}
        Err(_) => {
            return Err(project_store::LoadProjectError::IdentityIndeterminate);
        }
    }
    let lease = match ProjectIdentityLease::acquire(lease_root, project_id) {
        Ok(lease) => lease,
        Err(IdentityLeaseError::Conflict) => {
            return match ProjectIdentityLease::observe(
                lease_root,
                project_id,
                loaded.physical_identity,
            ) {
                Ok(IdentityLeaseObservation::SamePhysicalTarget { .. }) => Ok(()),
                Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                    Err(project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution)
                }
                _ => Err(project_store::LoadProjectError::IdentityIndeterminate),
            };
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
    if loaded.resolved_object.compare_physical(&current_candidate) != PhysicalIdentityEvidence::Same
    {
        lease.discard_unpublished();
        return Err(project_store::LoadProjectError::IdentityIndeterminate);
    }
    if let Err(error) = authorize_identity_candidate(
        core,
        project_id,
        &loaded.project_path,
        IdentityCandidateTarget::Loaded(&loaded.resolved_object),
    ) {
        lease.discard_unpublished();
        return Err(map_load_identity_error(error));
    }
    lease
        .bind_target(physical_identity)
        .and_then(|_| lease.into_published().map(|_| ()))
        .map_err(|_| project_store::LoadProjectError::IdentityIndeterminate)?;
    Ok(())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityCandidateError {
    ExternalCopy,
    Indeterminate,
}

fn authorize_identity_candidate(
    core: &ProjectCore,
    project_id: Uuid,
    candidate_location: &Path,
    candidate_target: IdentityCandidateTarget<'_>,
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
            match candidate_target.compare(&previous.resolved_object) {
                PhysicalIdentityEvidence::Same => Ok(()),
                PhysicalIdentityEvidence::Different => Err(IdentityCandidateError::ExternalCopy),
                PhysicalIdentityEvidence::Indeterminate => {
                    Err(IdentityCandidateError::Indeterminate)
                }
            }
        }
        Ok(_) => Err(IdentityCandidateError::Indeterminate),
        Err(project_store::DecodeFailure::Path(PathFailure::NotFound)) => registry
            .publish(project_id, candidate_location)
            .map_err(|_| IdentityCandidateError::Indeterminate),
        Err(_) => Err(IdentityCandidateError::Indeterminate),
    }
}

enum IdentityCandidateTarget<'a> {
    Loaded(&'a myalbuns_paths::ResolvedObject),
    Editable(&'a ProjectStore),
}

impl IdentityCandidateTarget<'_> {
    fn compare(&self, previous: &myalbuns_paths::ResolvedObject) -> PhysicalIdentityEvidence {
        match self {
            Self::Loaded(candidate) => previous.compare_physical(candidate),
            Self::Editable(candidate) => candidate.compare_physical(previous),
        }
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
    lease: PendingProjectIdentityLease,
    store: &ProjectStore,
) -> Result<ProjectIdentityLease, IdentityLeaseError> {
    let physical_identity = store
        .physical_identity()
        .ok_or(IdentityLeaseError::Unavailable)?;
    lease.bind_target(physical_identity)?;
    lease.into_published()
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
        OpenStoreError::ProjectInUse { .. } => OpenProjectError::ProjectInUse,
        OpenStoreError::IdentityIndeterminate => OpenProjectError::IdentityIndeterminate,
    }
}

fn map_save_copy_store_error(error: CreateStoreError) -> SaveCopyAsError {
    match error {
        CreateStoreError::Path(error) => SaveCopyAsError::Path(error),
        CreateStoreError::Document(_) | CreateStoreError::IdentityIndeterminate => {
            SaveCopyAsError::IdentityIndeterminate
        }
        CreateStoreError::DestinationConflict => SaveCopyAsError::DestinationConflict,
        CreateStoreError::ProjectInUse => SaveCopyAsError::ProjectInUse,
        CreateStoreError::StateIndeterminate => SaveCopyAsError::SaveCopyStateIndeterminate,
    }
}

fn map_save_copy_identity_lease_error(error: IdentityLeaseError) -> SaveCopyAsError {
    match error {
        IdentityLeaseError::Conflict => SaveCopyAsError::IdentityIndeterminate,
        IdentityLeaseError::Unavailable => SaveCopyAsError::Path(PathFailure::IoFailure),
    }
}

fn map_save_copy_replaced_lease_error(error: IdentityLeaseError) -> SaveCopyAsError {
    match error {
        IdentityLeaseError::Conflict => SaveCopyAsError::ProjectInUse,
        IdentityLeaseError::Unavailable => SaveCopyAsError::Path(PathFailure::IoFailure),
    }
}

impl ProjectCore {
    pub fn save_copy_as(
        &self,
        request: SaveCopyAsRequest,
    ) -> Result<EditableProject, SaveCopyAsError> {
        let SaveCopyAsRequest {
            source,
            destination,
            authorization,
        } = request;
        if !source.store.location_still_matches_baseline() {
            return Err(SaveCopyAsError::IdentityIndeterminate);
        }
        let lease_root = self
            .identity_lease_root()
            .ok_or(SaveCopyAsError::Path(PathFailure::IoFailure))?;
        let project_id = Uuid::new_v4();
        let identity_lease = ProjectIdentityLease::acquire(lease_root, project_id)
            .map_err(map_save_copy_identity_lease_error)?;
        let revision = ProjectRevision::new(
            project_id,
            source.revision.revision,
            source.revision.project.clone(),
        );
        let store_result = match authorization {
            CreateAuthorization::CreateOnly => {
                project_store::create_only(destination, &revision, lease_root)
                    .map_err(map_save_copy_store_error)
            }
            CreateAuthorization::ReplaceConfirmed => {
                project_store::prepare_replacement(destination, &revision, lease_root)
                    .map_err(map_save_copy_store_error)
                    .and_then(|prepared| {
                        let _replaced_identity_lease = prepared
                            .replaced_project_id()
                            .map(|replaced_id| {
                                ProjectIdentityLease::acquire(lease_root, replaced_id)
                            })
                            .transpose()
                            .map_err(map_save_copy_replaced_lease_error)?;
                        prepared.publish().map_err(map_save_copy_store_error)
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
        if !store.location_still_matches_baseline() {
            identity_lease.discard_unpublished();
            return Err(SaveCopyAsError::IdentityIndeterminate);
        }
        if publish_identity_location(self, project_id, &store).is_err() {
            identity_lease.discard_unpublished();
            return Err(SaveCopyAsError::SaveCopyStateIndeterminate);
        }
        let identity_lease = bind_identity_target(identity_lease, &store)
            .map_err(|_| SaveCopyAsError::SaveCopyStateIndeterminate)?;
        let identity_authority = ProjectIdentityAuthority::authorized(project_id);
        Ok(EditableProject {
            session: PersistentProjectSession::from_persisted(revision, false),
            store,
            identity_lease,
            identity_authority,
            session_valid: true,
        })
    }
}

fn promote_external_copy(
    core: &ProjectCore,
    mut opened: project_store::OpenedProject,
    source_identity_guard: Option<PendingProjectIdentityLease>,
) -> Result<EditableProject, OpenProjectError> {
    let lease_root = core
        .identity_lease_root()
        .ok_or(OpenProjectError::IdentityIndeterminate)?;
    let project_id = Uuid::new_v4();
    let identity_lease = ProjectIdentityLease::acquire(lease_root, project_id)
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
    let revision = match opened.store.rewrite_identity(project_id, &identity_lease) {
        SaveStoreResult::Saved(receipt) => receipt.candidate().clone(),
        SaveStoreResult::NotSaved(SaveStoreError::Path(PathFailure::AccessDenied)) => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::ExternalCopyNotWritable(Box::new(
                external_copy_source(opened),
            )));
        }
        SaveStoreResult::NotSaved(SaveStoreError::Path(error)) => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::Path(error));
        }
        SaveStoreResult::NotSaved(SaveStoreError::PersistedBaselineConflict)
        | SaveStoreResult::StateIndeterminate => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::IdentityIndeterminate);
        }
    };
    if publish_identity_location(core, project_id, &opened.store).is_err() {
        identity_lease.discard_unpublished();
        return Err(OpenProjectError::IdentityIndeterminate);
    }
    let identity_lease = identity_lease
        .into_published()
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
    let identity_authority = ProjectIdentityAuthority::authorized(project_id);
    // The source's repeated Identidade remains reserved until the promoted
    // file, lease, registry and authority have all reached their terminal.
    drop(source_identity_guard);
    Ok(EditableProject {
        session: PersistentProjectSession::from_persisted(revision, opened.requires_schema_upgrade),
        store: opened.store,
        identity_lease,
        identity_authority,
        session_valid: true,
    })
}

fn external_copy_source(opened: project_store::OpenedProject) -> ExternalCopySource {
    ExternalCopySource {
        revision: opened.revision,
        store: opened.store,
    }
}

fn map_active_identity_observation(
    lease_root: &Path,
    project_id: Uuid,
    physical_identity: Option<myalbuns_paths::PhysicalFileIdentity>,
) -> OpenProjectError {
    match ProjectIdentityLease::observe(lease_root, project_id, physical_identity) {
        Ok(IdentityLeaseObservation::SamePhysicalTarget { owner_process }) => {
            OpenProjectError::FocusExisting {
                project_id,
                owner_process,
            }
        }
        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => OpenProjectError::ProjectInUse,
        Ok(IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending) => {
            OpenProjectError::ProjectInUse
        }
        Err(_) => OpenProjectError::IdentityIndeterminate,
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

#[cfg(test)]
mod save_tests {
    use myalbuns_paths::OperationPathContext;

    use super::{CreateAuthorization, CreateProjectRequest, OpenProjectRequest};
    use crate::{InitialProject, ProjectCore, ProjectLocation};

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
}
