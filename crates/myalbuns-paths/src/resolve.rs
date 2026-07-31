use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use crate::{AppPathsError, OperationPathContext, RootBindingPlan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedObject {
    RegularFile,
    Directory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalIdentityEvidence {
    Same,
    Different,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveError {
    InvalidPath,
    UnsupportedNamespace,
    UnboundRoot,
    NotFound,
    AccessDenied,
    Unavailable,
    UnexpectedObjectType { expected: ExpectedObject },
    IoFailure,
}

impl Display for ResolveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => formatter.write_str("o caminho é inválido"),
            Self::UnsupportedNamespace => {
                formatter.write_str("o namespace do caminho não é aceito")
            }
            Self::UnboundRoot => formatter.write_str("a raiz não pertence ao plano da operação"),
            Self::NotFound => formatter.write_str("o objeto não foi encontrado"),
            Self::AccessDenied => formatter.write_str("o acesso ao objeto foi negado"),
            Self::Unavailable => formatter.write_str("o objeto está temporariamente indisponível"),
            Self::UnexpectedObjectType { expected } => {
                write!(formatter, "o objeto não é do tipo esperado: {expected:?}")
            }
            Self::IoFailure => formatter.write_str("a operação de entrada e saída falhou"),
        }
    }
}

impl Error for ResolveError {}

#[derive(Debug)]
pub struct ResolvedObject {
    logical_path: PathBuf,
    operational_path: PathBuf,
    object_type: ExpectedObject,
    file: File,
}

impl ResolvedObject {
    pub fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    pub fn operational_path(&self) -> &Path {
        &self.operational_path
    }

    pub fn object_type(&self) -> ExpectedObject {
        self.object_type
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn compare_physical(&self, other: &Self) -> PhysicalIdentityEvidence {
        match (file_identity(&self.file), file_identity(&other.file)) {
            (Some(left), Some(right)) if left == right => PhysicalIdentityEvidence::Same,
            (Some(_), Some(_)) => PhysicalIdentityEvidence::Different,
            _ => PhysicalIdentityEvidence::Indeterminate,
        }
    }
}

impl RootBindingPlan {
    /// Opens an existing object through this immutable operation plan.
    ///
    /// This function is blocking and must be dispatched away from an interface
    /// thread when the captured binding can reach a network resource.
    pub fn resolve_existing(
        &self,
        logical_path: &Path,
        expected: ExpectedObject,
    ) -> Result<ResolvedObject, ResolveError> {
        let operational_path = self.resolve(logical_path).map_err(resolve_plan_error)?;
        let operational_root = self
            .operational_root_for(logical_path)
            .map_err(resolve_plan_error)?;
        let file = open_object(&operational_path)
            .map_err(|error| classify_open_error(error, operational_root))?;
        let metadata = file.metadata().map_err(classify_io_error)?;
        let actual = if metadata.is_file() {
            ExpectedObject::RegularFile
        } else if metadata.is_dir() {
            ExpectedObject::Directory
        } else {
            return Err(ResolveError::UnexpectedObjectType { expected });
        };
        if actual != expected {
            return Err(ResolveError::UnexpectedObjectType { expected });
        }
        validate_disk_handle(&file, expected)?;

        Ok(ResolvedObject {
            logical_path: logical_path.to_path_buf(),
            operational_path,
            object_type: actual,
            file,
        })
    }

    pub fn compare_existing(
        &self,
        left: &Path,
        right: &Path,
        expected: ExpectedObject,
    ) -> PhysicalIdentityEvidence {
        let Ok(left) = self.resolve_existing(left, expected) else {
            return PhysicalIdentityEvidence::Indeterminate;
        };
        let Ok(right) = self.resolve_existing(right, expected) else {
            return PhysicalIdentityEvidence::Indeterminate;
        };
        left.compare_physical(&right)
    }
}

impl OperationPathContext {
    /// Captures the current platform binding and opens the object for the
    /// operation owner. This function is blocking for the same reason as
    /// `RootBindingPlan::resolve_existing`.
    pub fn resolve_existing(
        &mut self,
        logical_path: &Path,
        expected: ExpectedObject,
    ) -> Result<ResolvedObject, ResolveError> {
        self.capture(logical_path).map_err(resolve_plan_error)?;
        self.current_plan().resolve_existing(logical_path, expected)
    }
}

fn resolve_plan_error(error: AppPathsError) -> ResolveError {
    match error {
        AppPathsError::InvalidOperationPath => ResolveError::InvalidPath,
        AppPathsError::UnsupportedOperationNamespace => ResolveError::UnsupportedNamespace,
        AppPathsError::PathRootNotBound => ResolveError::UnboundRoot,
        AppPathsError::OperationPathAccessDenied => ResolveError::AccessDenied,
        AppPathsError::OperationPathUnavailable => ResolveError::Unavailable,
        AppPathsError::OperationPathIoFailure => ResolveError::IoFailure,
        _ => ResolveError::IoFailure,
    }
}

fn classify_open_error(error: std::io::Error, operational_root: &Path) -> ResolveError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return match std::fs::metadata(operational_root) {
            Ok(metadata) if metadata.is_dir() => ResolveError::NotFound,
            Ok(_) => ResolveError::UnexpectedObjectType {
                expected: ExpectedObject::Directory,
            },
            Err(root_error) => match classify_io_error(root_error) {
                ResolveError::AccessDenied => ResolveError::AccessDenied,
                _ => ResolveError::Unavailable,
            },
        };
    }
    classify_io_error(error)
}

fn classify_io_error(error: std::io::Error) -> ResolveError {
    match error.kind() {
        std::io::ErrorKind::NotFound => ResolveError::NotFound,
        std::io::ErrorKind::PermissionDenied => ResolveError::AccessDenied,
        std::io::ErrorKind::InvalidInput => ResolveError::InvalidPath,
        std::io::ErrorKind::TimedOut
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::ConnectionRefused
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::NotConnected
        | std::io::ErrorKind::NetworkDown
        | std::io::ErrorKind::NetworkUnreachable => ResolveError::Unavailable,
        _ if is_windows_unavailable_error(error.raw_os_error()) => ResolveError::Unavailable,
        _ => ResolveError::IoFailure,
    }
}

#[cfg(windows)]
fn is_windows_unavailable_error(raw_error: Option<i32>) -> bool {
    matches!(
        raw_error,
        Some(53 | 64 | 67 | 1201 | 1203 | 1222 | 1231 | 2250)
    )
}

#[cfg(not(windows))]
fn is_windows_unavailable_error(_raw_error: Option<i32>) -> bool {
    false
}

#[cfg(windows)]
fn open_object(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_object(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
fn validate_disk_handle(file: &File, expected: ExpectedObject) -> Result<(), ResolveError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_TYPE_DISK, GetFileType},
    };

    let file_type = unsafe { GetFileType(file.as_raw_handle() as HANDLE) };
    if file_type == FILE_TYPE_DISK {
        Ok(())
    } else {
        Err(ResolveError::UnexpectedObjectType { expected })
    }
}

#[cfg(not(windows))]
fn validate_disk_handle(_file: &File, _expected: ExpectedObject) -> Result<(), ResolveError> {
    Ok(())
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalFileIdentity {
    volume: u64,
    file_id: [u8; 16],
}

#[cfg(windows)]
fn file_identity(file: &File) -> Option<PhysicalFileIdentity> {
    use std::{ffi::c_void, mem::size_of, os::windows::io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
    };

    let mut identity = FILE_ID_INFO::default();
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as HANDLE,
            FileIdInfo,
            (&mut identity as *mut FILE_ID_INFO).cast::<c_void>(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    (succeeded != 0).then_some(PhysicalFileIdentity {
        volume: identity.VolumeSerialNumber,
        file_id: identity.FileId.Identifier,
    })
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn file_identity(file: &File) -> Option<PhysicalFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata().ok()?;
    Some(PhysicalFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
