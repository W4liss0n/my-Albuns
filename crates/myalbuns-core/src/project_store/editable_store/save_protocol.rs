use std::{io, path::Path};

use super::{ProjectStore, TemporaryPublication, map_io_path};
use crate::{
    project_document::ProjectRevision,
    project_store::{
        IdentityTargetBinder, PathFailure, PendingProjectIdentityLease, ProjectIdentityLease,
        ProjectLocation, decode, encode, map_path_failure,
        versioned_codec::rewrite_project_id,
        windows_publish::{replace_existing, write_synced_new},
    },
};
#[cfg(windows)]
use myalbuns_paths::{
    PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination, ProjectFileLock,
    ProjectTransitionBarrier, ProjectTransitionBarrierError, ResolveError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SaveStoreError {
    PersistedBaselineConflict,
    Path(PathFailure),
}

pub(crate) enum SaveStoreResult {
    Saved(SaveReceipt),
    NotSaved(SaveStoreError),
    StateIndeterminate,
}

pub(crate) struct SaveReceipt {
    candidate: Box<ProjectRevision>,
}

impl SaveReceipt {
    pub(crate) fn candidate(&self) -> &ProjectRevision {
        &self.candidate
    }
}

#[derive(Debug)]
pub(super) struct PersistedBaseline {
    #[cfg(windows)]
    lock: ProjectFileLock,
    bytes: Vec<u8>,
}

impl PersistedBaseline {
    #[cfg(windows)]
    pub(super) fn new(lock: ProjectFileLock, bytes: Vec<u8>) -> Self {
        Self { lock, bytes }
    }

    #[cfg(windows)]
    pub(super) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self.lock.physical_identity()
    }

    #[cfg(windows)]
    pub(super) fn matches(&self, resolved: &myalbuns_paths::ResolvedObject) -> bool {
        self.compare_physical(resolved) == myalbuns_paths::PhysicalIdentityEvidence::Same
    }

    #[cfg(windows)]
    pub(super) fn compare_physical(
        &self,
        resolved: &myalbuns_paths::ResolvedObject,
    ) -> myalbuns_paths::PhysicalIdentityEvidence {
        self.lock.compare_physical(resolved)
    }
}

#[cfg(windows)]
pub(super) fn save(
    store: &mut ProjectStore,
    candidate: ProjectRevision,
    identity_lease: &ProjectIdentityLease,
) -> SaveStoreResult {
    let Some(baseline) = store.baseline.take() else {
        return SaveStoreResult::StateIndeterminate;
    };
    save_candidate(
        &store.location,
        &store.transition_root,
        baseline,
        candidate,
        identity_lease,
    )
    .install_into(store)
}

#[cfg(windows)]
pub(super) fn rewrite_identity(
    store: &mut ProjectStore,
    project_id: uuid::Uuid,
    identity_lease: &PendingProjectIdentityLease,
) -> SaveStoreResult {
    let Some(baseline) = store.baseline.take() else {
        return SaveStoreResult::StateIndeterminate;
    };
    let (candidate_bytes, candidate) = match rewrite_project_id(&baseline.bytes, project_id) {
        Ok(candidate) => candidate,
        Err(_) => {
            store.baseline = Some(baseline);
            return SaveStoreResult::NotSaved(SaveStoreError::Path(PathFailure::IoFailure));
        }
    };
    save_candidate_with_bytes(
        &store.location,
        &store.transition_root,
        baseline,
        candidate,
        candidate_bytes,
        identity_lease,
    )
    .install_into(store)
}

#[cfg(not(windows))]
pub(super) fn save(
    _store: &mut ProjectStore,
    _candidate: ProjectRevision,
    _identity_lease: &ProjectIdentityLease,
) -> SaveStoreResult {
    SaveStoreResult::NotSaved(SaveStoreError::Path(PathFailure::IoFailure))
}

#[cfg(not(windows))]
pub(super) fn rewrite_identity(
    _store: &mut ProjectStore,
    _project_id: uuid::Uuid,
    _identity_lease: &PendingProjectIdentityLease,
) -> SaveStoreResult {
    SaveStoreResult::NotSaved(SaveStoreError::Path(PathFailure::IoFailure))
}

#[cfg(windows)]
enum SaveCandidateResult {
    Saved {
        baseline: PersistedBaseline,
        receipt: SaveReceipt,
    },
    NotSaved {
        baseline: PersistedBaseline,
        error: SaveStoreError,
    },
    StateIndeterminate,
}

#[cfg(windows)]
impl SaveCandidateResult {
    fn install_into(self, store: &mut ProjectStore) -> SaveStoreResult {
        match self {
            Self::Saved { baseline, receipt } => {
                store.baseline = Some(baseline);
                SaveStoreResult::Saved(receipt)
            }
            Self::NotSaved { baseline, error } => {
                store.baseline = Some(baseline);
                SaveStoreResult::NotSaved(error)
            }
            Self::StateIndeterminate => SaveStoreResult::StateIndeterminate,
        }
    }
}

#[cfg(windows)]
fn save_candidate(
    location: &ProjectLocation,
    transition_root: &Path,
    baseline: PersistedBaseline,
    candidate: ProjectRevision,
    identity_lease: &dyn IdentityTargetBinder,
) -> SaveCandidateResult {
    let candidate_bytes = match encode(&candidate) {
        Ok(bytes) => bytes,
        Err(_) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(PathFailure::IoFailure),
            };
        }
    };
    save_candidate_with_bytes(
        location,
        transition_root,
        baseline,
        candidate,
        candidate_bytes,
        identity_lease,
    )
}

#[cfg(windows)]
fn save_candidate_with_bytes(
    location: &ProjectLocation,
    transition_root: &Path,
    baseline: PersistedBaseline,
    candidate: ProjectRevision,
    candidate_bytes: Vec<u8>,
    identity_lease: &dyn IdentityTargetBinder,
) -> SaveCandidateResult {
    let destination = match location.prepare_file_destination() {
        Ok(destination) => destination,
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(error),
            };
        }
    };
    let temporary = TemporaryPublication::new(destination.sibling_temporary_path());
    if let Err(error) = write_synced_new(temporary.path(), &candidate_bytes) {
        return SaveCandidateResult::NotSaved {
            baseline,
            error: SaveStoreError::Path(map_io_path(error)),
        };
    }
    let project_id = candidate.project_id.hyphenated().to_string();
    let _barrier = match ProjectTransitionBarrier::try_acquire(transition_root, &project_id) {
        Ok(barrier) => barrier,
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(map_barrier_error(error)),
            };
        }
    };
    let old_object = match destination.resolve_existing() {
        Ok(Some(object)) => object,
        Ok(None) | Err(ResolveError::UnexpectedObjectType { .. }) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::PersistedBaselineConflict,
            };
        }
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(map_path_failure(error)),
            };
        }
    };
    match baseline.lock.compare_physical(&old_object) {
        PhysicalIdentityEvidence::Same => {}
        PhysicalIdentityEvidence::Different => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::PersistedBaselineConflict,
            };
        }
        PhysicalIdentityEvidence::Indeterminate => {
            return SaveCandidateResult::StateIndeterminate;
        }
    }
    match baseline.lock.read_bytes() {
        Ok(bytes) if bytes == baseline.bytes => {}
        Ok(_) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::PersistedBaselineConflict,
            };
        }
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(map_io_path(error)),
            };
        }
    }
    let candidate_object = match destination.resolve_existing_sibling(temporary.path()) {
        Ok(object) => object,
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(map_path_failure(error)),
            };
        }
    };
    match candidate_object.read_bytes() {
        Ok(bytes) if bytes == candidate_bytes => {}
        Ok(_) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(PathFailure::Conflict),
            };
        }
        Err(error) => {
            return SaveCandidateResult::NotSaved {
                baseline,
                error: SaveStoreError::Path(map_io_path(error)),
            };
        }
    }

    let PersistedBaseline {
        lock: old_lock,
        bytes: baseline_bytes,
    } = baseline;
    drop(old_lock);
    match replace_existing(temporary.path(), destination.operational_path()) {
        Ok(()) => verify_saved_candidate(
            &destination,
            &candidate_object,
            candidate_bytes,
            candidate,
            identity_lease,
        ),
        Err(error) => reconcile_save_error(
            &destination,
            &old_object,
            &candidate_object,
            baseline_bytes,
            candidate_bytes,
            candidate,
            identity_lease,
            error,
        ),
    }
}

#[cfg(windows)]
fn verify_saved_candidate(
    destination: &PreparedFileDestination,
    candidate_object: &myalbuns_paths::ResolvedObject,
    candidate_bytes: Vec<u8>,
    candidate: ProjectRevision,
    identity_lease: &dyn IdentityTargetBinder,
) -> SaveCandidateResult {
    let Ok(current) = destination.resolve_created() else {
        return SaveCandidateResult::StateIndeterminate;
    };
    let Ok(lock) = ProjectFileLock::try_acquire(current.operational_path()) else {
        return SaveCandidateResult::StateIndeterminate;
    };
    if lock.compare_physical(&current) != PhysicalIdentityEvidence::Same
        || lock.compare_physical(candidate_object) != PhysicalIdentityEvidence::Same
    {
        return SaveCandidateResult::StateIndeterminate;
    }
    let Ok(locked_bytes) = lock.read_bytes() else {
        return SaveCandidateResult::StateIndeterminate;
    };
    if locked_bytes != candidate_bytes
        || current.read_bytes().ok().as_deref() != Some(candidate_bytes.as_slice())
        || decode(&locked_bytes).ok().as_ref() != Some(&candidate)
    {
        return SaveCandidateResult::StateIndeterminate;
    }
    let Some(physical_identity) = lock.physical_identity() else {
        return SaveCandidateResult::StateIndeterminate;
    };
    if identity_lease.bind_target(physical_identity).is_err() {
        return SaveCandidateResult::StateIndeterminate;
    }
    SaveCandidateResult::Saved {
        baseline: PersistedBaseline::new(lock, candidate_bytes),
        receipt: SaveReceipt {
            candidate: Box::new(candidate),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconciledBaselineRead {
    Exact,
    Diverged,
    Indeterminate,
}

fn classify_reconciled_baseline_read(
    read: io::Result<Vec<u8>>,
    baseline_bytes: &[u8],
) -> ReconciledBaselineRead {
    match read {
        Ok(bytes) if bytes == baseline_bytes => ReconciledBaselineRead::Exact,
        Ok(_) => ReconciledBaselineRead::Diverged,
        Err(_) => ReconciledBaselineRead::Indeterminate,
    }
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn reconcile_save_error(
    destination: &PreparedFileDestination,
    old_object: &myalbuns_paths::ResolvedObject,
    candidate_object: &myalbuns_paths::ResolvedObject,
    baseline_bytes: Vec<u8>,
    candidate_bytes: Vec<u8>,
    candidate: ProjectRevision,
    identity_lease: &dyn IdentityTargetBinder,
    publication_error: io::Error,
) -> SaveCandidateResult {
    let Ok(Some(current)) = destination.resolve_existing() else {
        return SaveCandidateResult::StateIndeterminate;
    };
    if current.compare_physical(candidate_object) == PhysicalIdentityEvidence::Same {
        return verify_saved_candidate(
            destination,
            candidate_object,
            candidate_bytes,
            candidate,
            identity_lease,
        );
    }
    if current.compare_physical(old_object) != PhysicalIdentityEvidence::Same {
        return SaveCandidateResult::StateIndeterminate;
    }
    let Ok(lock) = ProjectFileLock::try_acquire(current.operational_path()) else {
        return SaveCandidateResult::StateIndeterminate;
    };
    if lock.compare_physical(old_object) != PhysicalIdentityEvidence::Same {
        return SaveCandidateResult::StateIndeterminate;
    }
    let error = match classify_reconciled_baseline_read(lock.read_bytes(), &baseline_bytes) {
        ReconciledBaselineRead::Exact => SaveStoreError::Path(map_io_path(publication_error)),
        ReconciledBaselineRead::Diverged => SaveStoreError::PersistedBaselineConflict,
        ReconciledBaselineRead::Indeterminate => {
            return SaveCandidateResult::StateIndeterminate;
        }
    };
    SaveCandidateResult::NotSaved {
        baseline: PersistedBaseline::new(lock, baseline_bytes),
        error,
    }
}

#[cfg(windows)]
fn map_barrier_error(error: ProjectTransitionBarrierError) -> PathFailure {
    match error {
        ProjectTransitionBarrierError::Conflict => PathFailure::Conflict,
        ProjectTransitionBarrierError::Unavailable => PathFailure::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{ReconciledBaselineRead, classify_reconciled_baseline_read};

    #[test]
    fn reconciliation_distinguishes_exact_divergent_and_unreadable_baselines() {
        let baseline = b"persisted baseline";

        assert_eq!(
            classify_reconciled_baseline_read(Ok(baseline.to_vec()), baseline),
            ReconciledBaselineRead::Exact
        );
        assert_eq!(
            classify_reconciled_baseline_read(Ok(b"external bytes".to_vec()), baseline),
            ReconciledBaselineRead::Diverged
        );
        assert_eq!(
            classify_reconciled_baseline_read(Err(io::Error::other("unreadable")), baseline),
            ReconciledBaselineRead::Indeterminate
        );
    }
}
