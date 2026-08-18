use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use crate::guarded_fs::{GuardedFsError, is_direct_physical_child, physical_path_from_file};
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

/// A single, validated file name beneath an existing directory retained by
/// handle for the lifetime of a publication attempt.
#[derive(Debug)]
pub struct PreparedFileDestination {
    logical_path: PathBuf,
    operational_path: PathBuf,
    parent: ResolvedObject,
}

impl PreparedFileDestination {
    pub fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    pub fn operational_path(&self) -> &Path {
        &self.operational_path
    }

    /// Derives a unique staging file beside this validated destination. The
    /// caller owns publication semantics but never constructs a child path.
    pub fn sibling_temporary_path(&self) -> PathBuf {
        self.operational_path.with_file_name(format!(
            ".myalbuns-create-{}.tmp",
            uuid::Uuid::new_v4().hyphenated()
        ))
    }

    /// Resolves an object currently occupying the destination and proves that
    /// its final physical path is a direct child of the retained parent.
    pub fn resolve_existing(&self) -> Result<Option<ResolvedObject>, ResolveError> {
        let child = match resolve_existing_operational(
            &self.logical_path,
            &self.operational_path,
            self.parent.operational_path(),
            ExpectedObject::RegularFile,
        ) {
            Ok(child) => child,
            Err(ResolveError::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        };
        let expected_name = self
            .operational_path
            .file_name()
            .ok_or(ResolveError::InvalidPath)?;
        self.validate_containment(&child, expected_name)?;
        Ok(Some(child))
    }

    pub fn resolve_created(&self) -> Result<ResolvedObject, ResolveError> {
        self.resolve_existing()?.ok_or(ResolveError::NotFound)
    }

    /// Opens an existing regular-file sibling whose path is already in the
    /// operational namespace retained by this destination.
    ///
    /// Unlike `RootBindingPlan::resolve_existing`, this does not reinterpret
    /// the supplied path as logical input. It is intended for staging paths
    /// returned by [`Self::sibling_temporary_path`].
    pub fn resolve_existing_sibling(
        &self,
        operational_path: &Path,
    ) -> Result<ResolvedObject, ResolveError> {
        let operational_parent = self
            .operational_path
            .parent()
            .ok_or(ResolveError::InvalidPath)?;
        if operational_path.parent() != Some(operational_parent) {
            return Err(ResolveError::InvalidPath);
        }
        let expected_name = operational_path
            .file_name()
            .ok_or(ResolveError::InvalidPath)?;
        let logical_path = self.logical_path.with_file_name(expected_name);
        let sibling = resolve_existing_operational(
            &logical_path,
            operational_path,
            self.parent.operational_path(),
            ExpectedObject::RegularFile,
        )?;
        self.validate_containment(&sibling, expected_name)?;
        Ok(sibling)
    }

    fn validate_containment(
        &self,
        child: &ResolvedObject,
        expected_name: &std::ffi::OsStr,
    ) -> Result<(), ResolveError> {
        let parent_path =
            physical_path_from_file(self.parent.file(), self.parent.operational_path())
                .map_err(map_guarded_error)?;
        let child_path = physical_path_from_file(child.file(), child.operational_path())
            .map_err(map_guarded_error)?;
        if !is_direct_physical_child(&parent_path, &child_path, expected_name) {
            return Err(ResolveError::InvalidPath);
        }
        Ok(())
    }
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

    /// Reopens this already-resolved regular file for content reads without
    /// resolving its pathname a second time.
    pub fn reopen_for_read(&self) -> std::io::Result<File> {
        if self.object_type != ExpectedObject::RegularFile {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "o objeto resolvido não é um arquivo regular",
            ));
        }
        reopen_file_for_read(&self.file)
    }

    pub fn compare_physical(&self, other: &Self) -> PhysicalIdentityEvidence {
        compare_file_identity(&self.file, &other.file)
    }

    /// Returns the opaque, operating-system identity of this opened object.
    ///
    /// The value is local evidence only: it is not a pathname, a Project
    /// identity, or a portable identifier for interchange between machines.
    pub fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        file_identity(&self.file)
    }

    /// Reads a resolved regular file through the physical handle already used
    /// for identity, without resolving its pathname a second time.
    pub fn read_to_string(&self) -> std::io::Result<String> {
        let mut readable = self.reopen_for_read()?;
        readable.seek(SeekFrom::Start(0))?;
        let mut source = String::new();
        readable.read_to_string(&mut source)?;
        Ok(source)
    }

    pub fn read_bytes(&self) -> std::io::Result<Vec<u8>> {
        read_file_handle_to_bytes(&self.file)
    }
}

pub(crate) fn compare_file_identity(left: &File, right: &File) -> PhysicalIdentityEvidence {
    match (file_identity(left), file_identity(right)) {
        (Some(left), Some(right)) if left == right => PhysicalIdentityEvidence::Same,
        (Some(_), Some(_)) => PhysicalIdentityEvidence::Different,
        _ => PhysicalIdentityEvidence::Indeterminate,
    }
}

pub(crate) fn read_file_handle_to_string(file: &File) -> std::io::Result<String> {
    let mut readable = reopen_file_for_read(file)?;
    readable.seek(SeekFrom::Start(0))?;
    let mut source = String::new();
    readable.read_to_string(&mut source)?;
    Ok(source)
}

pub(crate) fn read_file_handle_to_bytes(file: &File) -> std::io::Result<Vec<u8>> {
    let mut readable = reopen_file_for_read(file)?;
    readable.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    readable.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(windows)]
fn reopen_file_for_read(file: &File) -> std::io::Result<File> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, ReOpenFile},
    };

    let handle = unsafe {
        ReOpenFile(
            file.as_raw_handle() as HANDLE,
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

#[cfg(not(windows))]
fn reopen_file_for_read(file: &File) -> std::io::Result<File> {
    file.try_clone()
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
        resolve_existing_operational(logical_path, &operational_path, operational_root, expected)
    }

    /// Prepares one new regular-file destination beneath an existing parent.
    /// The parent handle remains retained so the final object can be checked
    /// physically after publication instead of trusting pathname text.
    pub fn prepare_file_destination(
        &self,
        logical_path: &Path,
    ) -> Result<PreparedFileDestination, ResolveError> {
        let operational_path = self.resolve(logical_path).map_err(resolve_plan_error)?;
        let logical_parent = logical_path.parent().ok_or(ResolveError::InvalidPath)?;
        let operational_parent = operational_path.parent().ok_or(ResolveError::InvalidPath)?;
        let operational_root = self
            .operational_root_for(logical_path)
            .map_err(resolve_plan_error)?;
        let parent = resolve_existing_operational(
            logical_parent,
            operational_parent,
            operational_root,
            ExpectedObject::Directory,
        )?;
        Ok(PreparedFileDestination {
            logical_path: logical_path.to_path_buf(),
            operational_path,
            parent,
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

fn resolve_existing_operational(
    logical_path: &Path,
    operational_path: &Path,
    operational_root: &Path,
    expected: ExpectedObject,
) -> Result<ResolvedObject, ResolveError> {
    let file = open_object(operational_path)
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
        operational_path: operational_path.to_path_buf(),
        object_type: actual,
        file,
    })
}

fn map_guarded_error(error: GuardedFsError) -> ResolveError {
    match error {
        GuardedFsError::OutsideRoot => ResolveError::InvalidPath,
        GuardedFsError::Unavailable => ResolveError::IoFailure,
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
pub struct PhysicalFileIdentity {
    volume: u64,
    file_id: WindowsFileId,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsFileId {
    Extended([u8; 16]),
    Legacy([u8; 8]),
}

#[cfg(windows)]
impl PhysicalFileIdentity {
    const EXTENDED_TOKEN_PREFIX: &'static str = "windows-file-id-v1:";
    const LEGACY_TOKEN_PREFIX: &'static str = "windows-file-index-v1:";

    /// Serializes this identity for local process-coordination metadata.
    pub fn to_local_token(self) -> String {
        let prefix = match self.file_id {
            WindowsFileId::Extended(_) => Self::EXTENDED_TOKEN_PREFIX,
            WindowsFileId::Legacy(_) => Self::LEGACY_TOKEN_PREFIX,
        };
        let mut token = format!("{prefix}{:016x}:", self.volume);
        match self.file_id {
            WindowsFileId::Extended(file_id) => append_hex_bytes(&mut token, &file_id),
            WindowsFileId::Legacy(file_id) => append_hex_bytes(&mut token, &file_id),
        }
        token
    }

    /// Restores local process-coordination evidence written by this platform.
    pub fn from_local_token(token: &str) -> Option<Self> {
        let (payload, extended) = token
            .strip_prefix(Self::EXTENDED_TOKEN_PREFIX)
            .map(|payload| (payload, true))
            .or_else(|| {
                token
                    .strip_prefix(Self::LEGACY_TOKEN_PREFIX)
                    .map(|payload| (payload, false))
            })?;
        let (volume, file_id) = payload.split_once(':')?;
        let expected_file_id_length = if extended { 32 } else { 16 };
        if volume.len() != 16 || file_id.len() != expected_file_id_length {
            return None;
        }
        let volume = u64::from_str_radix(volume, 16).ok()?;
        let file_id = if extended {
            let mut identifier = [0_u8; 16];
            decode_hex_bytes(file_id, &mut identifier)?;
            WindowsFileId::Extended(identifier)
        } else {
            let mut identifier = [0_u8; 8];
            decode_hex_bytes(file_id, &mut identifier)?;
            WindowsFileId::Legacy(identifier)
        };
        Some(Self { volume, file_id })
    }
}

#[cfg(windows)]
fn append_hex_bytes(destination: &mut String, source: &[u8]) {
    for byte in source {
        use std::fmt::Write as _;
        write!(destination, "{byte:02x}").expect("writing to a String cannot fail");
    }
}

#[cfg(windows)]
fn decode_hex_bytes(source: &str, destination: &mut [u8]) -> Option<()> {
    for (index, byte) in destination.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&source[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(())
}

#[cfg(windows)]
pub(crate) fn file_identity(file: &File) -> Option<PhysicalFileIdentity> {
    use std::{ffi::c_void, mem::size_of, os::windows::io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle,
            GetFileInformationByHandleEx,
        },
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
    if succeeded != 0 {
        return Some(PhysicalFileIdentity {
            volume: identity.VolumeSerialNumber,
            file_id: WindowsFileId::Extended(identity.FileId.Identifier),
        });
    }

    let mut legacy = BY_HANDLE_FILE_INFORMATION::default();
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut legacy) };
    (succeeded != 0).then_some(PhysicalFileIdentity {
        volume: u64::from(legacy.dwVolumeSerialNumber),
        file_id: WindowsFileId::Legacy(
            (u64::from(legacy.nFileIndexHigh) << 32 | u64::from(legacy.nFileIndexLow))
                .to_be_bytes(),
        ),
    })
}

#[cfg(all(test, windows))]
mod windows_identity_tests {
    use super::{PhysicalFileIdentity, WindowsFileId};

    #[test]
    fn extended_and_legacy_file_ids_round_trip_without_sharing_a_token_shape() {
        let extended = PhysicalFileIdentity {
            volume: 7,
            file_id: WindowsFileId::Extended([3; 16]),
        };
        let legacy = PhysicalFileIdentity {
            volume: 7,
            file_id: WindowsFileId::Legacy([3; 8]),
        };

        for identity in [extended, legacy] {
            let token = identity.to_local_token();
            assert_eq!(
                PhysicalFileIdentity::from_local_token(&token),
                Some(identity)
            );
        }
        assert_ne!(extended.to_local_token(), legacy.to_local_token());
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl PhysicalFileIdentity {
    const TOKEN_PREFIX: &'static str = "unix-file-id-v1:";

    pub fn to_local_token(self) -> String {
        format!(
            "{}{:016x}:{:016x}",
            Self::TOKEN_PREFIX,
            self.device,
            self.inode
        )
    }

    pub fn from_local_token(token: &str) -> Option<Self> {
        let payload = token.strip_prefix(Self::TOKEN_PREFIX)?;
        let (device, inode) = payload.split_once(':')?;
        if device.len() != 16 || inode.len() != 16 {
            return None;
        }
        Some(Self {
            device: u64::from_str_radix(device, 16).ok()?,
            inode: u64::from_str_radix(inode, 16).ok()?,
        })
    }
}

#[cfg(unix)]
pub(crate) fn file_identity(file: &File) -> Option<PhysicalFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata().ok()?;
    Some(PhysicalFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
