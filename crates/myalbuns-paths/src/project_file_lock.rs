use std::{
    fmt::{self, Display, Formatter},
    fs::{File, OpenOptions},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};

use crate::{
    PhysicalIdentityEvidence, ResolvedObject,
    resolve::{compare_file_identity, read_file_handle_to_string},
};

use windows_sys::Win32::{
    Foundation::{ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION, HANDLE},
    Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, LOCKFILE_EXCLUSIVE_LOCK,
        LOCKFILE_FAIL_IMMEDIATELY, LockFileEx, UnlockFileEx,
    },
    System::IO::{OVERLAPPED, OVERLAPPED_0, OVERLAPPED_0_0},
};

/// Exclusive operating-system lock used as the final protection against two
/// editable sessions opening the same physical Project file.
///
/// Identity and focus policy remain the responsibility of the opening flow.
/// This type owns only the native file handle and its lock lifetime.
#[derive(Debug)]
pub struct ProjectFileLock {
    file: File,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProjectFileLockError {
    Conflict,
    Unavailable { reason: String },
}

impl ProjectFileLock {
    /// Attempts an immediate exclusive lock without changing Project bytes.
    /// The lock is released by `Drop` or by Windows when the process exits.
    pub fn try_acquire(path: &Path) -> Result<Self, ProjectFileLockError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(path)
            .map_err(classify_lock_error)?;
        if !file.metadata().map_err(classify_lock_error)?.is_file() {
            return Err(ProjectFileLockError::Unavailable {
                reason: "o alvo não é um arquivo regular".into(),
            });
        }

        let mut overlapped = lock_region();
        let succeeded = unsafe {
            LockFileEx(
                file.as_raw_handle() as HANDLE,
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                0,
                1,
                0,
                &mut overlapped,
            )
        };
        if succeeded == 0 {
            return Err(classify_lock_error(std::io::Error::last_os_error()));
        }
        Ok(Self { file })
    }

    /// Confirms that this authoritative lock belongs to the object already
    /// resolved by the opening policy.
    pub fn compare_physical(&self, resolved: &ResolvedObject) -> PhysicalIdentityEvidence {
        compare_file_identity(&self.file, resolved.file())
    }

    /// Reads the persisted revision through the handle that owns the lock, so
    /// a path replacement cannot redirect the final opening read.
    pub fn read_to_string(&self) -> std::io::Result<String> {
        read_file_handle_to_string(&self.file)
    }
}

impl Display for ProjectFileLockError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => formatter.write_str("o Projeto já possui uma sessão editável"),
            Self::Unavailable { reason } => {
                write!(
                    formatter,
                    "o Bloqueio de abertura está indisponível: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ProjectFileLockError {}

impl Drop for ProjectFileLock {
    fn drop(&mut self) {
        let mut overlapped = lock_region();
        unsafe {
            UnlockFileEx(
                self.file.as_raw_handle() as HANDLE,
                0,
                1,
                0,
                &mut overlapped,
            );
        }
    }
}

fn classify_lock_error(error: std::io::Error) -> ProjectFileLockError {
    match error.raw_os_error().map(|value| value as u32) {
        Some(ERROR_LOCK_VIOLATION) | Some(ERROR_SHARING_VIOLATION) => {
            ProjectFileLockError::Conflict
        }
        _ => ProjectFileLockError::Unavailable {
            reason: error.to_string(),
        },
    }
}

fn lock_region() -> OVERLAPPED {
    // A byte far beyond any supported Project payload avoids interfering with
    // ordinary reads and writes while every participant contends for the same
    // native range.
    OVERLAPPED {
        Anonymous: OVERLAPPED_0 {
            Anonymous: OVERLAPPED_0_0 {
                Offset: 0,
                OffsetHigh: 1_u32 << 30,
            },
        },
        ..OVERLAPPED::default()
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectFileLock;
    use crate::{ExpectedObject, OperationPathContext, PhysicalIdentityEvidence};

    #[test]
    fn acquiring_and_releasing_the_opening_lock_preserves_the_project_file() {
        let directory = tempfile::tempdir().expect("the fixture directory exists");
        let project = directory.path().join("Projeto.myalbum");
        let expected = b"persisted project revision";
        std::fs::write(&project, expected).expect("the Project fixture is writable");

        let opening_lock =
            ProjectFileLock::try_acquire(&project).expect("the first session acquires the lock");
        let mut paths = OperationPathContext::new();
        let resolved = paths
            .resolve_existing(&project, ExpectedObject::RegularFile)
            .expect("the locked Project resolves by handle");
        assert_eq!(
            opening_lock.compare_physical(&resolved),
            PhysicalIdentityEvidence::Same,
            "the authoritative lock belongs to the resolved physical Project"
        );
        assert_eq!(
            opening_lock
                .read_to_string()
                .expect("the persisted revision is read through the locked handle"),
            String::from_utf8(expected.to_vec()).expect("the fixture is UTF-8")
        );
        assert_eq!(
            std::fs::read(&project).expect("the locked Project remains readable"),
            expected
        );
        drop(opening_lock);

        assert_eq!(
            std::fs::read(&project).expect("the released Project remains readable"),
            expected
        );
    }
}
