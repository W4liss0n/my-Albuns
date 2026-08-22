use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use myalbuns_paths::{
    ExpectedObject, PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination,
    ProjectFileLock, ProjectFileLockError, ProjectTransitionBarrier, ProjectTransitionBarrierError,
    ResolveError, ResolvedObject,
};
use uuid::Uuid;

use super::{
    DecodeFailure, DocumentFailure, PathFailure, PendingProjectIdentityLease, ProjectIdentityLease,
    ProjectLocation, decode, decode_with_metadata, encode, map_path_failure,
    windows_publish::{publish_new, replace_existing, write_synced_new},
};
use crate::project_document::ProjectRevision;

mod save_protocol;

use save_protocol::PersistedBaseline;
pub(crate) use save_protocol::{SaveStoreError, SaveStoreResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenStoreError {
    Path(PathFailure),
    Document(DocumentFailure),
    ProjectInUse {
        project_id: Uuid,
        physical_identity: Option<PhysicalFileIdentity>,
    },
    IdentityIndeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CreateStoreError {
    Path(PathFailure),
    Document(DocumentFailure),
    SameTarget,
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    StateIndeterminate,
}

#[derive(Debug)]
pub(crate) struct ProjectStore {
    location: ProjectLocation,
    transition_root: PathBuf,
    baseline: Option<PersistedBaseline>,
}

impl ProjectStore {
    pub(crate) fn location(&self) -> &ProjectLocation {
        &self.location
    }

    #[cfg(windows)]
    pub(crate) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self.baseline.as_ref()?.physical_identity()
    }

    #[cfg(windows)]
    pub(crate) fn compare_physical(&self, resolved: &ResolvedObject) -> PhysicalIdentityEvidence {
        self.baseline
            .as_ref()
            .map(|baseline| baseline.compare_physical(resolved))
            .unwrap_or(PhysicalIdentityEvidence::Indeterminate)
    }

    #[cfg(windows)]
    pub(crate) fn location_still_matches_baseline(&self) -> bool {
        let Some(baseline) = self.baseline.as_ref() else {
            return false;
        };
        self.location
            .root_bindings()
            .resolve_existing(self.location.project_path(), ExpectedObject::RegularFile)
            .is_ok_and(|resolved| baseline.matches(&resolved))
    }

    #[cfg(not(windows))]
    pub(crate) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        None
    }

    #[cfg(not(windows))]
    pub(crate) fn location_still_matches_baseline(&self) -> bool {
        false
    }

    #[cfg(windows)]
    fn from_verified(
        location: ProjectLocation,
        transition_root: &Path,
        lock: ProjectFileLock,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            location,
            transition_root: transition_root.to_path_buf(),
            baseline: Some(PersistedBaseline::new(lock, bytes)),
        }
    }

    pub(crate) fn save(
        &mut self,
        candidate: ProjectRevision,
        identity_lease: &ProjectIdentityLease,
    ) -> SaveStoreResult {
        save_protocol::save(self, candidate, identity_lease)
    }

    pub(crate) fn rewrite_identity(
        &mut self,
        project_id: Uuid,
        identity_lease: &PendingProjectIdentityLease,
    ) -> SaveStoreResult {
        save_protocol::rewrite_identity(self, project_id, identity_lease)
    }

    pub(crate) fn invalidate(&mut self) {
        self.baseline = None;
    }
}

pub(crate) struct OpenedProject {
    pub(crate) revision: ProjectRevision,
    pub(crate) requires_schema_upgrade: bool,
    pub(crate) store: ProjectStore,
}

#[derive(Debug)]
pub(crate) struct PreparedReplacement {
    location: ProjectLocation,
    transition_root: PathBuf,
    expected_revision: ProjectRevision,
    expected_bytes: Vec<u8>,
    temporary: TemporaryPublication,
    destination: PreparedFileDestination,
    replaced_project_id: Option<Uuid>,
    #[cfg(windows)]
    forbidden_target: Option<PhysicalFileIdentity>,
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
                &self.transition_root,
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
            if let Some(current) = self
                .destination
                .resolve_existing()
                .map_err(|error| CreateStoreError::Path(map_path_failure(error)))?
            {
                reject_forbidden_target(&current, self.forbidden_target)?;
            }
            return Err(CreateStoreError::DestinationConflict);
        }
        if let Ok(store) = verify_created(
            &self.location,
            &self.transition_root,
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
        reject_forbidden_target(&current, self.forbidden_target)?;
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
pub(crate) fn open_editable(
    location: ProjectLocation,
    transition_root: &Path,
) -> Result<OpenedProject, OpenStoreError> {
    let initial = location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
        .map_err(|error| OpenStoreError::Path(map_path_failure(error)))?;
    let initial_bytes = initial
        .read_bytes()
        .map_err(|error| OpenStoreError::Path(map_io_path(error)))?;
    let initial_revision = decode(&initial_bytes).map_err(map_open_decode_error)?;
    let _barrier = match ProjectTransitionBarrier::try_acquire(
        transition_root,
        &initial_revision.project_id.hyphenated().to_string(),
    ) {
        Ok(barrier) => barrier,
        Err(ProjectTransitionBarrierError::Conflict) => {
            return Err(classify_project_in_use_for_current_target(
                &location,
                &initial,
                initial_revision.project_id,
            ));
        }
        Err(ProjectTransitionBarrierError::Unavailable) => {
            return Err(OpenStoreError::Path(PathFailure::Unavailable));
        }
    };
    let resolved = location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
        .map_err(|error| OpenStoreError::Path(map_path_failure(error)))?;
    if initial.compare_physical(&resolved) != PhysicalIdentityEvidence::Same {
        return Err(OpenStoreError::IdentityIndeterminate);
    }
    let lock = match ProjectFileLock::try_acquire(resolved.operational_path()) {
        Ok(lock) => lock,
        Err(ProjectFileLockError::Conflict) => {
            return Err(classify_project_in_use_for_current_target(
                &location,
                &initial,
                initial_revision.project_id,
            ));
        }
        Err(ProjectFileLockError::Unavailable { .. }) => {
            return Err(OpenStoreError::Path(PathFailure::IoFailure));
        }
    };
    if lock.compare_physical(&resolved) != PhysicalIdentityEvidence::Same {
        return Err(OpenStoreError::IdentityIndeterminate);
    }
    let bytes = lock
        .read_bytes()
        .map_err(|error| OpenStoreError::Path(map_io_path(error)))?;
    let decoded = decode_with_metadata(&bytes).map_err(map_open_decode_error)?;
    if decoded.revision.project_id != initial_revision.project_id {
        return Err(OpenStoreError::IdentityIndeterminate);
    }
    Ok(OpenedProject {
        revision: decoded.revision,
        requires_schema_upgrade: decoded.requires_schema_upgrade,
        store: ProjectStore::from_verified(location, transition_root, lock, bytes),
    })
}

#[cfg(windows)]
fn classify_project_in_use_for_current_target(
    location: &ProjectLocation,
    expected: &ResolvedObject,
    project_id: Uuid,
) -> OpenStoreError {
    let Ok(current) = location
        .root_bindings()
        .resolve_existing(location.project_path(), ExpectedObject::RegularFile)
    else {
        return OpenStoreError::IdentityIndeterminate;
    };
    match expected.compare_physical(&current) {
        PhysicalIdentityEvidence::Same => OpenStoreError::ProjectInUse {
            project_id,
            physical_identity: expected.physical_identity(),
        },
        PhysicalIdentityEvidence::Different | PhysicalIdentityEvidence::Indeterminate => {
            OpenStoreError::IdentityIndeterminate
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn open_editable(
    _location: ProjectLocation,
    _transition_root: &Path,
) -> Result<OpenedProject, OpenStoreError> {
    Err(OpenStoreError::Path(PathFailure::IoFailure))
}

pub(crate) fn create_only(
    location: ProjectLocation,
    revision: &ProjectRevision,
    transition_root: &Path,
) -> Result<ProjectStore, CreateStoreError> {
    create_only_inner(location, revision, transition_root, None)
}

#[cfg(windows)]
pub(crate) fn create_only_excluding(
    location: ProjectLocation,
    revision: &ProjectRevision,
    transition_root: &Path,
    forbidden_target: PhysicalFileIdentity,
) -> Result<ProjectStore, CreateStoreError> {
    create_only_inner(location, revision, transition_root, Some(forbidden_target))
}

fn create_only_inner(
    location: ProjectLocation,
    revision: &ProjectRevision,
    transition_root: &Path,
    forbidden_target: Option<PhysicalFileIdentity>,
) -> Result<ProjectStore, CreateStoreError> {
    let destination = location
        .prepare_file_destination()
        .map_err(CreateStoreError::Path)?;
    let bytes = encode(revision).map_err(map_create_decode_error)?;
    let temporary = prepare_temporary(&destination, &bytes)?;
    match publish_new(temporary.path(), destination.operational_path()) {
        Ok(()) => verify_created(&location, transition_root, &destination, &bytes, revision)
            .map_err(|_| CreateStoreError::StateIndeterminate),
        Err(error) => {
            if is_destination_conflict(&error) {
                if let Some(forbidden_target) = forbidden_target {
                    let current = destination
                        .resolve_existing()
                        .map_err(|error| CreateStoreError::Path(map_path_failure(error)))?
                        .ok_or(CreateStoreError::StateIndeterminate)?;
                    reject_forbidden_target(&current, Some(forbidden_target))?;
                }
            }
            reconcile_create_only_error(
                error,
                &location,
                transition_root,
                &destination,
                &bytes,
                revision,
            )
        }
    }
}

#[cfg(windows)]
fn reconcile_create_only_error(
    error: io::Error,
    location: &ProjectLocation,
    transition_root: &Path,
    destination: &PreparedFileDestination,
    expected_bytes: &[u8],
    expected_revision: &ProjectRevision,
) -> Result<ProjectStore, CreateStoreError> {
    if is_destination_conflict(&error) {
        return Err(CreateStoreError::DestinationConflict);
    }
    if let Ok(store) = verify_created(
        location,
        transition_root,
        destination,
        expected_bytes,
        expected_revision,
    ) {
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
    _transition_root: &Path,
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
    transition_root: &Path,
) -> Result<PreparedReplacement, CreateStoreError> {
    prepare_replacement_inner(location, revision, transition_root, None)
}

#[cfg(windows)]
pub(crate) fn prepare_replacement_excluding(
    location: ProjectLocation,
    revision: &ProjectRevision,
    transition_root: &Path,
    forbidden_target: PhysicalFileIdentity,
) -> Result<PreparedReplacement, CreateStoreError> {
    prepare_replacement_inner(location, revision, transition_root, Some(forbidden_target))
}

#[cfg(windows)]
fn prepare_replacement_inner(
    location: ProjectLocation,
    revision: &ProjectRevision,
    transition_root: &Path,
    forbidden_target: Option<PhysicalFileIdentity>,
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
        reject_forbidden_target(&resolved, forbidden_target)?;
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
        transition_root: transition_root.to_path_buf(),
        expected_revision: revision.clone(),
        expected_bytes,
        temporary,
        destination,
        replaced_project_id,
        forbidden_target,
        replaced_lock,
    })
}

#[cfg(windows)]
fn reject_forbidden_target(
    target: &ResolvedObject,
    forbidden_target: Option<PhysicalFileIdentity>,
) -> Result<(), CreateStoreError> {
    let Some(forbidden_target) = forbidden_target else {
        return Ok(());
    };
    match target.physical_identity() {
        Some(identity) if identity == forbidden_target => Err(CreateStoreError::SameTarget),
        Some(_) => Ok(()),
        None => Err(CreateStoreError::IdentityIndeterminate),
    }
}

#[cfg(not(windows))]
pub(crate) fn prepare_replacement(
    _location: ProjectLocation,
    _revision: &ProjectRevision,
    _transition_root: &Path,
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
    transition_root: &Path,
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
    Ok(ProjectStore::from_verified(
        location.clone(),
        transition_root,
        lock,
        bytes,
    ))
}

#[cfg(not(windows))]
fn verify_created(
    _location: &ProjectLocation,
    _transition_root: &Path,
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
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            PathFailure::AccessDenied
        }
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

#[cfg(all(test, windows))]
mod tests {
    use std::{io, path::Path};

    use myalbuns_paths::{ExpectedObject, OperationPathContext};
    use uuid::Uuid;
    use windows_sys::Win32::Foundation::ERROR_WRITE_PROTECT;

    use super::{
        OpenStoreError, ProjectLocation, classify_project_in_use_for_current_target, map_io_path,
    };
    use crate::project_store::PathFailure;

    fn project_location(path: &Path) -> ProjectLocation {
        let mut paths = OperationPathContext::new();
        paths
            .capture(path)
            .expect("the public path seam captures the Project root");
        ProjectLocation::new(path.to_path_buf(), paths.freeze())
    }

    #[test]
    fn write_protected_media_is_an_access_denied_path() {
        let error = io::Error::from_raw_os_error(ERROR_WRITE_PROTECT as i32);

        assert_eq!(error.kind(), io::ErrorKind::ReadOnlyFilesystem);
        assert_eq!(map_io_path(error), PathFailure::AccessDenied);
    }

    #[test]
    fn project_in_use_is_forwarded_only_while_the_path_still_names_the_same_file() {
        let fixture = tempfile::tempdir().expect("temporary Project fixture");
        let project_path = fixture.path().join("Projeto.myalbuns");
        let retired_path = fixture.path().join("Projeto anterior.myalbuns");
        let replacement_path = fixture.path().join("Outro Projeto.myalbuns");
        std::fs::write(&project_path, b"first physical Project")
            .expect("the first physical Project exists");
        std::fs::write(&replacement_path, b"replacement physical Project")
            .expect("the replacement physical Project exists");
        let location = project_location(&project_path);
        let expected = location
            .root_bindings()
            .resolve_existing(&project_path, ExpectedObject::RegularFile)
            .expect("the first physical Project is retained by handle");
        let project_id = Uuid::new_v4();

        assert_eq!(
            classify_project_in_use_for_current_target(&location, &expected, project_id),
            OpenStoreError::ProjectInUse {
                project_id,
                physical_identity: expected.physical_identity(),
            },
            "Same is the only state that may be forwarded for focus"
        );

        std::fs::rename(&project_path, &retired_path)
            .expect("the retained first Project leaves the pathname");
        std::fs::rename(&replacement_path, &project_path)
            .expect("another physical Project takes the pathname");

        assert_eq!(
            classify_project_in_use_for_current_target(&location, &expected, project_id),
            OpenStoreError::IdentityIndeterminate,
            "Different must fail closed instead of focusing the previous physical Project"
        );
    }
}
