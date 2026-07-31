use std::{
    fmt::{self, Debug, Display, Formatter},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use myalbuns_core::{EditableProject, ProjectCore};
use myalbuns_paths::{
    ExpectedObject, OperationPathContext, PhysicalIdentityEvidence, ProjectFileLock,
    ProjectFileLockError, ResolveError, ResolvedObject,
};

type ActiveIdentityComparator = fn(&ResolvedObject, &ResolvedObject) -> PhysicalIdentityEvidence;

#[derive(Clone)]
pub(crate) struct ProjectOpeningGuard {
    inner: Arc<ProjectOpeningGuardInner>,
}

struct ProjectOpeningGuardInner {
    core: ProjectCore,
    state: Mutex<OpeningState>,
    compare_active_identity: ActiveIdentityComparator,
}

#[derive(Default)]
struct OpeningState {
    next_registration_id: u64,
    active: Vec<ActiveProject>,
}

struct ActiveProject {
    registration_id: u64,
    project_id: String,
    focus_target: String,
    resolved: Arc<ResolvedObject>,
}

pub(crate) enum ProjectOpeningOutcome {
    Opened(OpenedProject),
    FocusExisting {
        project_id: String,
        target: String,
    },
    ExternalCopyPending {
        project_id: String,
        existing_target: String,
    },
}

pub(crate) struct OpenedProject {
    session: Option<EditableProject>,
    opening_lock: Option<ProjectFileLock>,
    resolved: Arc<ResolvedObject>,
    registration_id: u64,
    inner: Arc<ProjectOpeningGuardInner>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProjectOpeningError {
    Resolve(ResolveError),
    Read { path: PathBuf, reason: String },
    InvalidProject { reason: String },
    PhysicalIdentityIndeterminate,
    CandidateChangedDuringOpening,
    OpenedElsewhere,
    OpeningLockUnavailable { reason: String },
}

impl ProjectOpeningGuard {
    pub(crate) fn new() -> Self {
        Self::with_active_identity_comparator(ResolvedObject::compare_physical)
    }

    fn with_active_identity_comparator(compare_active_identity: ActiveIdentityComparator) -> Self {
        Self {
            inner: Arc::new(ProjectOpeningGuardInner {
                core: ProjectCore::new(),
                state: Mutex::new(OpeningState::default()),
                compare_active_identity,
            }),
        }
    }

    /// Opens one editable Project or classifies why an existing session must
    /// handle the request instead.
    ///
    /// This method performs blocking filesystem work and therefore belongs on
    /// an opening worker, never on the interface thread.
    pub(crate) fn open_or_focus(
        &self,
        logical_path: &Path,
        focus_target: impl Into<String>,
    ) -> Result<ProjectOpeningOutcome, ProjectOpeningError> {
        let focus_target = focus_target.into();
        let mut path_context = OperationPathContext::new();
        let candidate = path_context
            .resolve_existing(logical_path, ExpectedObject::RegularFile)
            .map_err(ProjectOpeningError::Resolve)?;
        let plan = path_context.freeze();
        let candidate_source = read_project_source(&candidate)?;
        let candidate_project_id = self.validated_project_id(&candidate_source)?;

        // This mutex is deliberately retained until the new registration is
        // complete. It makes comparison, native locking and registration one
        // in-process opening transaction.
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let comparisons = state
            .active
            .iter()
            .map(|active| {
                (
                    active,
                    (self.inner.compare_active_identity)(&candidate, &active.resolved),
                )
            })
            .collect::<Vec<_>>();

        if let Some((active, _)) = comparisons
            .iter()
            .find(|(_, evidence)| *evidence == PhysicalIdentityEvidence::Same)
        {
            return Ok(ProjectOpeningOutcome::FocusExisting {
                project_id: active.project_id.clone(),
                target: active.focus_target.clone(),
            });
        }
        if comparisons
            .iter()
            .any(|(_, evidence)| *evidence == PhysicalIdentityEvidence::Indeterminate)
        {
            return Err(ProjectOpeningError::PhysicalIdentityIndeterminate);
        }
        if let Some((active, _)) = comparisons.iter().find(|(active, evidence)| {
            *evidence == PhysicalIdentityEvidence::Different
                && active.project_id == candidate_project_id
        }) {
            return Ok(ProjectOpeningOutcome::ExternalCopyPending {
                project_id: candidate_project_id,
                existing_target: active.focus_target.clone(),
            });
        }

        let opening_lock = ProjectFileLock::try_acquire(candidate.operational_path())
            .map_err(map_opening_lock_error)?;
        let rechecked = plan
            .resolve_existing(logical_path, ExpectedObject::RegularFile)
            .map_err(ProjectOpeningError::Resolve)?;
        require_same_opening_object(candidate.compare_physical(&rechecked))?;
        require_same_opening_object(opening_lock.compare_physical(&rechecked))?;

        let locked_source =
            opening_lock
                .read_to_string()
                .map_err(|error| ProjectOpeningError::Read {
                    path: logical_path.to_path_buf(),
                    reason: error.to_string(),
                })?;
        let locked_project_id = self.validated_project_id(&locked_source)?;
        if locked_project_id != candidate_project_id {
            return Err(ProjectOpeningError::CandidateChangedDuringOpening);
        }
        let session = self
            .inner
            .core
            .open_editable_session(&locked_source)
            .map_err(|error| ProjectOpeningError::InvalidProject {
                reason: error.to_string(),
            })?;
        let resolved = Arc::new(rechecked);
        let registration_id = state.next_registration_id;
        state.next_registration_id = state.next_registration_id.wrapping_add(1);
        state.active.push(ActiveProject {
            registration_id,
            project_id: locked_project_id,
            focus_target,
            resolved: Arc::clone(&resolved),
        });

        Ok(ProjectOpeningOutcome::Opened(OpenedProject {
            session: Some(session),
            opening_lock: Some(opening_lock),
            resolved,
            registration_id,
            inner: Arc::clone(&self.inner),
        }))
    }

    #[cfg(test)]
    pub(crate) fn active_session_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .len()
    }

    fn validated_project_id(&self, source: &str) -> Result<String, ProjectOpeningError> {
        self.inner
            .core
            .load_persisted_revision(source)
            .map(|revision| revision.render_snapshot().project_id)
            .map_err(|error| ProjectOpeningError::InvalidProject {
                reason: error.to_string(),
            })
    }
}

impl Default for ProjectOpeningGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OpenedProject {
    fn drop(&mut self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Other openings stay serialized until both ownership mechanisms are
        // released and the registry no longer advertises this session.
        drop(self.session.take());
        drop(self.opening_lock.take());
        state
            .active
            .retain(|active| active.registration_id != self.registration_id);
    }
}

impl Debug for OpenedProject {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedProject")
            .field("logical_path", &self.resolved.logical_path())
            .finish_non_exhaustive()
    }
}

impl Debug for ProjectOpeningOutcome {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Opened(opened) => formatter.debug_tuple("Opened").field(opened).finish(),
            Self::FocusExisting { project_id, target } => formatter
                .debug_struct("FocusExisting")
                .field("project_id", project_id)
                .field("target", target)
                .finish(),
            Self::ExternalCopyPending {
                project_id,
                existing_target,
            } => formatter
                .debug_struct("ExternalCopyPending")
                .field("project_id", project_id)
                .field("existing_target", existing_target)
                .finish(),
        }
    }
}

impl Display for ProjectOpeningError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve(error) => write!(formatter, "o Projeto não pôde ser resolvido: {error}"),
            Self::Read { path, reason } => write!(
                formatter,
                "o Projeto {} não pôde ser lido: {reason}",
                path.display()
            ),
            Self::InvalidProject { reason } => {
                write!(formatter, "o documento de Projeto é inválido: {reason}")
            }
            Self::PhysicalIdentityIndeterminate => formatter
                .write_str("a identidade física do Projeto não pôde ser determinada com segurança"),
            Self::CandidateChangedDuringOpening => {
                formatter.write_str("o arquivo de Projeto mudou durante a tentativa de abertura")
            }
            Self::OpenedElsewhere => {
                formatter.write_str("o Projeto já está aberto em outra instância")
            }
            Self::OpeningLockUnavailable { reason } => write!(
                formatter,
                "o bloqueio nativo de abertura está indisponível: {reason}"
            ),
        }
    }
}

impl std::error::Error for ProjectOpeningError {}

fn read_project_source(resolved: &ResolvedObject) -> Result<String, ProjectOpeningError> {
    resolved
        .read_to_string()
        .map_err(|error| ProjectOpeningError::Read {
            path: resolved.logical_path().to_path_buf(),
            reason: error.to_string(),
        })
}

fn map_opening_lock_error(error: ProjectFileLockError) -> ProjectOpeningError {
    match error {
        ProjectFileLockError::Conflict => ProjectOpeningError::OpenedElsewhere,
        ProjectFileLockError::Unavailable { reason } => {
            ProjectOpeningError::OpeningLockUnavailable { reason }
        }
    }
}

fn require_same_opening_object(
    evidence: PhysicalIdentityEvidence,
) -> Result<(), ProjectOpeningError> {
    match evidence {
        PhysicalIdentityEvidence::Same => Ok(()),
        PhysicalIdentityEvidence::Different => {
            Err(ProjectOpeningError::CandidateChangedDuringOpening)
        }
        PhysicalIdentityEvidence::Indeterminate => {
            Err(ProjectOpeningError::PhysicalIdentityIndeterminate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectOpeningError, ProjectOpeningGuard, ProjectOpeningOutcome};
    use crate::sample_project::SampleProject;
    use myalbuns_paths::{PhysicalIdentityEvidence, ProjectFileLock, ProjectFileLockError};

    #[cfg(windows)]
    const LOCAL_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_LOCAL_ROOT";
    #[cfg(windows)]
    const UNC_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_UNC_ROOT";
    #[cfg(windows)]
    const DRIVE_ENV: &str = "MYALBUNS_PATH_GATE_DRIVE";

    #[test]
    fn a_physical_alias_focuses_the_existing_session_without_opening_another_one() {
        let fixture = project_fixture(SampleProject::Horizon);
        let alias = fixture.directory.path().join("Atalho-fisico.myalbum");
        std::fs::hard_link(&fixture.project, &alias)
            .expect("the fixture supports a physical alias");
        let guard = ProjectOpeningGuard::new();

        let opened = expect_opened(guard.open_or_focus(&fixture.project, "window-a"));
        let outcome = guard
            .open_or_focus(&alias, "window-b")
            .expect("the alias is classified");

        assert!(matches!(
            outcome,
            ProjectOpeningOutcome::FocusExisting {
                ref project_id,
                ref target,
            } if project_id == SampleProject::Horizon.project_id() && target == "window-a"
        ));
        assert_eq!(guard.active_session_count(), 1);
        drop(opened);
        assert_eq!(guard.active_session_count(), 0);
    }

    #[test]
    fn a_physical_copy_with_the_same_persisted_identity_stays_pending() {
        let fixture = project_fixture(SampleProject::Horizon);
        let copy = fixture.directory.path().join("Copia.myalbum");
        std::fs::copy(&fixture.project, &copy).expect("the fixture copy is writable");
        let guard = ProjectOpeningGuard::new();

        let opened = expect_opened(guard.open_or_focus(&fixture.project, "window-a"));
        let outcome = guard
            .open_or_focus(&copy, "window-copy")
            .expect("the copy is classified");

        assert!(matches!(
            outcome,
            ProjectOpeningOutcome::ExternalCopyPending {
                ref project_id,
                ref existing_target,
            } if project_id == SampleProject::Horizon.project_id()
                && existing_target == "window-a"
        ));
        assert_eq!(guard.active_session_count(), 1);
        drop(opened);
    }

    #[test]
    fn indeterminate_active_identity_fails_closed_without_mutating_the_registry() {
        let fixture = project_fixture(SampleProject::Horizon);
        let other = fixture.directory.path().join("Outro.myalbum");
        std::fs::write(
            &other,
            SampleProject::Aurora
                .persisted_source(2)
                .expect("the second Project fixture is valid"),
        )
        .expect("the second Project fixture is writable");
        let guard = ProjectOpeningGuard::with_active_identity_comparator(|_, _| {
            PhysicalIdentityEvidence::Indeterminate
        });
        let opened = expect_opened(guard.open_or_focus(&fixture.project, "window-a"));

        let error = guard
            .open_or_focus(&other, "window-b")
            .expect_err("ambiguous identity cannot create a session");

        assert_eq!(error, ProjectOpeningError::PhysicalIdentityIndeterminate);
        assert_eq!(guard.active_session_count(), 1);
        drop(opened);
        assert_eq!(guard.active_session_count(), 0);
    }

    #[test]
    fn the_native_lock_conflict_fails_closed_and_is_released_with_the_session() {
        let fixture = project_fixture(SampleProject::Horizon);
        let external_lock = ProjectFileLock::try_acquire(&fixture.project)
            .expect("the external owner acquires the Project lock");
        let guard = ProjectOpeningGuard::new();

        let error = guard
            .open_or_focus(&fixture.project, "window-a")
            .expect_err("the opening lock is authoritative");
        assert_eq!(error, ProjectOpeningError::OpenedElsewhere);
        assert_eq!(guard.active_session_count(), 0);

        drop(external_lock);
        let opened = expect_opened(guard.open_or_focus(&fixture.project, "window-a"));
        assert_eq!(
            ProjectFileLock::try_acquire(&fixture.project)
                .expect_err("the editable session retains its opening lock"),
            ProjectFileLockError::Conflict
        );
        drop(opened);

        let recovered = ProjectFileLock::try_acquire(&fixture.project)
            .expect("dropping the session releases its opening lock");
        drop(recovered);
        assert_eq!(guard.active_session_count(), 0);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "executed by scripts/Test-WindowsPathGate.ps1 with a real SMB alias"]
    fn real_mapped_drive_and_unc_alias_focus_one_session() {
        use std::{env, path::PathBuf, process::Command};

        let local_root = PathBuf::from(
            env::var_os(LOCAL_ROOT_ENV).expect("the local Windows gate root is configured"),
        );
        let unc_root = PathBuf::from(
            env::var_os(UNC_ROOT_ENV).expect("the UNC Windows gate root is configured"),
        );
        let drive = env::var(DRIVE_ENV).expect("the mapped drive is configured");
        assert!(drive.len() == 2 && drive.ends_with(':'));
        let local_fixture = local_root.join("opening-guardian");
        let unc_fixture = unc_root.join("opening-guardian");
        std::fs::create_dir_all(&local_fixture).expect("the opening fixture is materialized");
        let local_project = local_fixture.join("Projeto.myalbum");
        std::fs::write(
            &local_project,
            SampleProject::Horizon
                .persisted_source(2)
                .expect("the real Project fixture serializes"),
        )
        .expect("the real Project fixture is writable");

        let mapping = TestDriveMapping::new(&drive);
        mapping.map_to(&unc_fixture);
        let logical_project = PathBuf::from(format!(r"{drive}\Projeto.myalbum"));
        let unc_project = unc_fixture.join("Projeto.myalbum");
        let guard = ProjectOpeningGuard::new();

        let opened = expect_opened(guard.open_or_focus(&logical_project, "window-a"));
        let outcome = guard
            .open_or_focus(&unc_project, "window-b")
            .expect("the UNC alias is classified");

        assert!(matches!(
            outcome,
            ProjectOpeningOutcome::FocusExisting {
                ref project_id,
                ref target,
            } if project_id == SampleProject::Horizon.project_id() && target == "window-a"
        ));
        assert_eq!(guard.active_session_count(), 1);
        assert_eq!(
            ProjectFileLock::try_acquire(&unc_project)
                .expect_err("the existing session retains the authoritative file lock"),
            ProjectFileLockError::Conflict
        );
        drop(opened);
        assert_eq!(guard.active_session_count(), 0);
        drop(
            ProjectFileLock::try_acquire(&unc_project)
                .expect("closing the unique session releases the physical alias lock"),
        );
        drop(mapping);

        struct TestDriveMapping {
            drive: String,
        }

        impl TestDriveMapping {
            fn new(drive: &str) -> Self {
                let mapping = Self {
                    drive: drive.to_owned(),
                };
                mapping.unmap();
                mapping
            }

            fn map_to(&self, target: &std::path::Path) {
                self.unmap();
                let status = Command::new("net")
                    .arg("use")
                    .arg(&self.drive)
                    .arg(target)
                    .arg("/persistent:no")
                    .status()
                    .expect("net use starts");
                assert!(status.success(), "the SMB fixture maps to the drive");
            }

            fn unmap(&self) {
                let _ = Command::new("net")
                    .arg("use")
                    .arg(&self.drive)
                    .arg("/delete")
                    .arg("/y")
                    .output();
            }
        }

        impl Drop for TestDriveMapping {
            fn drop(&mut self) {
                self.unmap();
            }
        }
    }

    fn expect_opened(
        outcome: Result<ProjectOpeningOutcome, ProjectOpeningError>,
    ) -> super::OpenedProject {
        match outcome.expect("the Project opens") {
            ProjectOpeningOutcome::Opened(opened) => opened,
            _ => panic!("the first physical Project must create the editable session"),
        }
    }

    struct ProjectFixture {
        directory: tempfile::TempDir,
        project: std::path::PathBuf,
    }

    fn project_fixture(sample: SampleProject) -> ProjectFixture {
        let directory = tempfile::tempdir().expect("the fixture directory exists");
        let project = directory.path().join("Projeto.myalbum");
        std::fs::write(
            &project,
            sample
                .persisted_source(2)
                .expect("the Project fixture is valid"),
        )
        .expect("the Project fixture is writable");
        ProjectFixture { directory, project }
    }
}
