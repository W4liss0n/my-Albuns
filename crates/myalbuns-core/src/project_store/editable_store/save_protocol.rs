use std::io;

#[cfg(test)]
use std::cell::Cell;
#[cfg(all(test, windows))]
use std::{cell::RefCell, fs, os::windows::fs::OpenOptionsExt};

#[cfg(windows)]
use myalbuns_paths::{
    PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination, ProjectFileLock,
    ProjectTransitionBarrier, ProjectTransitionBarrierError, ResolveError,
};
#[cfg(all(test, windows))]
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};

use super::{ProjectStore, TemporaryPublication, map_io_path};
use crate::{
    project_document::ProjectRevision,
    project_store::{
        PathFailure, ProjectIdentityLease, ProjectLocation, decode, encode, map_path_failure,
        windows_publish::{replace_existing, write_synced_new},
    },
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
    candidate: ProjectRevision,
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
}

#[cfg(test)]
thread_local! {
    static INJECT_POST_PUBLICATION_VERIFICATION_BLOCK: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn inject_post_publication_indeterminate_for_current_thread() {
    INJECT_POST_PUBLICATION_VERIFICATION_BLOCK.with(|injected| injected.set(true));
}

#[cfg(all(test, windows))]
thread_local! {
    static POST_PUBLICATION_VERIFICATION_BLOCKER: RefCell<Option<fs::File>> =
        const { RefCell::new(None) };
}

#[cfg(all(test, windows))]
pub(crate) fn release_post_publication_indeterminate_for_current_thread() {
    POST_PUBLICATION_VERIFICATION_BLOCKER.with(|blocker| drop(blocker.borrow_mut().take()));
}

#[cfg(all(test, not(windows)))]
pub(crate) fn release_post_publication_indeterminate_for_current_thread() {}

#[cfg(all(test, windows))]
fn install_post_publication_verification_blocker_if_requested(path: &std::path::Path) {
    let requested =
        INJECT_POST_PUBLICATION_VERIFICATION_BLOCK.with(|injected| injected.replace(false));
    if !requested {
        return;
    }

    let Ok(blocker) = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
    else {
        return;
    };
    POST_PUBLICATION_VERIFICATION_BLOCKER.with(|held| *held.borrow_mut() = Some(blocker));
}

#[cfg(all(not(test), windows))]
fn install_post_publication_verification_blocker_if_requested(_path: &std::path::Path) {}

#[cfg(windows)]
pub(super) fn save(
    store: &mut ProjectStore,
    candidate: ProjectRevision,
    identity_lease: &ProjectIdentityLease,
) -> SaveStoreResult {
    let Some(baseline) = store.baseline.take() else {
        return SaveStoreResult::StateIndeterminate;
    };
    save_candidate(&store.location, baseline, candidate, identity_lease).install_into(store)
}

#[cfg(not(windows))]
pub(super) fn save(
    _store: &mut ProjectStore,
    _candidate: ProjectRevision,
    _identity_lease: &ProjectIdentityLease,
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
    baseline: PersistedBaseline,
    candidate: ProjectRevision,
    identity_lease: &ProjectIdentityLease,
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
    let _barrier = match ProjectTransitionBarrier::try_acquire(
        destination.operational_path(),
        &candidate.project_id.hyphenated().to_string(),
    ) {
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
        Ok(()) => {
            install_post_publication_verification_blocker_if_requested(
                destination.operational_path(),
            );
            verify_saved_candidate(
                &destination,
                &candidate_object,
                candidate_bytes,
                candidate,
                identity_lease,
            )
        }
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
    identity_lease: &ProjectIdentityLease,
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
        receipt: SaveReceipt { candidate },
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
    identity_lease: &ProjectIdentityLease,
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
