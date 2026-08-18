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
        (Some(left), Some(right)) => left.compare(right),
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
    Extended(ExtendedFileId),
    Legacy(LegacyFileId),
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExtendedFileId([u8; 16]);

#[cfg(windows)]
impl ExtendedFileId {
    fn new(identifier: [u8; 16]) -> Option<Self> {
        (identifier != [0; 16] && identifier != [u8::MAX; 16]).then_some(Self(identifier))
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LegacyFileId {
    guarantee: LegacyFileIdGuarantee,
    identifier: [u8; 8],
}

#[cfg(windows)]
impl LegacyFileId {
    fn new(guarantee: LegacyFileIdGuarantee, identifier: [u8; 8]) -> Option<Self> {
        (identifier != [0; 8] && identifier != [u8::MAX; 8]).then_some(Self {
            guarantee,
            identifier,
        })
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LegacyFileIdGuarantee {
    Ntfs,
    Udfs,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WindowsFileSystem {
    Ntfs,
    Udfs,
    Refs,
    Cdfs,
    Other,
}

#[cfg(windows)]
impl WindowsFileSystem {
    fn from_api_name(name: &[u16]) -> Self {
        const NTFS: &[u16] = &[0x004e, 0x0054, 0x0046, 0x0053];
        const UDFS: &[u16] = &[0x0055, 0x0044, 0x0046];
        const REFS: &[u16] = &[0x0052, 0x0065, 0x0046, 0x0053];
        const CDFS: &[u16] = &[0x0043, 0x0044, 0x0046, 0x0053];

        if name == NTFS {
            Self::Ntfs
        } else if name == UDFS {
            Self::Udfs
        } else if name == REFS {
            Self::Refs
        } else if name == CDFS {
            Self::Cdfs
        } else {
            Self::Other
        }
    }
}

#[cfg(windows)]
fn legacy_identity_from_observation(
    volume: u64,
    identifier: [u8; 8],
    file_system: WindowsFileSystem,
) -> Option<PhysicalFileIdentity> {
    let guarantee = match file_system {
        WindowsFileSystem::Ntfs => LegacyFileIdGuarantee::Ntfs,
        WindowsFileSystem::Udfs => LegacyFileIdGuarantee::Udfs,
        WindowsFileSystem::Refs | WindowsFileSystem::Cdfs | WindowsFileSystem::Other => {
            return None;
        }
    };
    Some(PhysicalFileIdentity {
        volume,
        file_id: WindowsFileId::Legacy(LegacyFileId::new(guarantee, identifier)?),
    })
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtendedFileIdQueryFailure {
    Unsupported,
    Unexpected,
}

#[cfg(windows)]
impl ExtendedFileIdQueryFailure {
    fn from_observation(error: u32, file_system: WindowsFileSystem) -> Self {
        use windows_sys::Win32::Foundation::{
            ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED,
        };

        match error {
            ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED => Self::Unsupported,
            ERROR_INVALID_PARAMETER if file_system == WindowsFileSystem::Udfs => Self::Unsupported,
            _ => Self::Unexpected,
        }
    }
}

#[cfg(windows)]
impl PhysicalFileIdentity {
    const EXTENDED_TOKEN_PREFIX: &'static str = "windows-file-id-v1:";
    const NTFS_LEGACY_TOKEN_PREFIX: &'static str = "windows-ntfs-file-index-v1:";
    const UDFS_LEGACY_TOKEN_PREFIX: &'static str = "windows-udfs-file-index-v1:";

    /// Serializes this identity for local process-coordination metadata.
    pub fn to_local_token(self) -> String {
        let prefix = match self.file_id {
            WindowsFileId::Extended(_) => Self::EXTENDED_TOKEN_PREFIX,
            WindowsFileId::Legacy(LegacyFileId {
                guarantee: LegacyFileIdGuarantee::Ntfs,
                ..
            }) => Self::NTFS_LEGACY_TOKEN_PREFIX,
            WindowsFileId::Legacy(LegacyFileId {
                guarantee: LegacyFileIdGuarantee::Udfs,
                ..
            }) => Self::UDFS_LEGACY_TOKEN_PREFIX,
        };
        let mut token = format!("{prefix}{:016x}:", self.volume);
        match self.file_id {
            WindowsFileId::Extended(file_id) => append_hex_bytes(&mut token, &file_id.0),
            WindowsFileId::Legacy(file_id) => append_hex_bytes(&mut token, &file_id.identifier),
        }
        token
    }

    /// Restores local process-coordination evidence written by this platform.
    pub fn from_local_token(token: &str) -> Option<Self> {
        let (payload, kind) = token
            .strip_prefix(Self::EXTENDED_TOKEN_PREFIX)
            .map(|payload| (payload, LocalTokenKind::Extended))
            .or_else(|| {
                token
                    .strip_prefix(Self::NTFS_LEGACY_TOKEN_PREFIX)
                    .map(|payload| (payload, LocalTokenKind::Legacy(LegacyFileIdGuarantee::Ntfs)))
            })
            .or_else(|| {
                token
                    .strip_prefix(Self::UDFS_LEGACY_TOKEN_PREFIX)
                    .map(|payload| (payload, LocalTokenKind::Legacy(LegacyFileIdGuarantee::Udfs)))
            })?;
        let (volume, file_id) = payload.split_once(':')?;
        let expected_file_id_length = match kind {
            LocalTokenKind::Extended => 32,
            LocalTokenKind::Legacy(_) => 16,
        };
        if volume.len() != 16 || file_id.len() != expected_file_id_length {
            return None;
        }
        let volume = u64::from_str_radix(volume, 16).ok()?;
        let file_id = match kind {
            LocalTokenKind::Extended => {
                let mut identifier = [0_u8; 16];
                decode_hex_bytes(file_id, &mut identifier)?;
                WindowsFileId::Extended(ExtendedFileId::new(identifier)?)
            }
            LocalTokenKind::Legacy(guarantee) => {
                let mut identifier = [0_u8; 8];
                decode_hex_bytes(file_id, &mut identifier)?;
                WindowsFileId::Legacy(LegacyFileId::new(guarantee, identifier)?)
            }
        };
        Some(Self { volume, file_id })
    }

    /// Compares only compatible Windows file-ID domains. Mixed extended and
    /// legacy identifiers on one volume remain inconclusive.
    pub fn compare(self, other: Self) -> PhysicalIdentityEvidence {
        if self.volume != other.volume {
            return PhysicalIdentityEvidence::Different;
        }
        let same_file_id = match (self.file_id, other.file_id) {
            (WindowsFileId::Extended(left), WindowsFileId::Extended(right)) => Some(left == right),
            (WindowsFileId::Legacy(left), WindowsFileId::Legacy(right))
                if left.guarantee == right.guarantee =>
            {
                Some(left.identifier == right.identifier)
            }
            (WindowsFileId::Legacy(_), WindowsFileId::Legacy(_)) => None,
            (WindowsFileId::Extended(_), WindowsFileId::Legacy(_))
            | (WindowsFileId::Legacy(_), WindowsFileId::Extended(_)) => None,
        };
        match same_file_id {
            Some(true) => PhysicalIdentityEvidence::Same,
            Some(false) => PhysicalIdentityEvidence::Different,
            None => PhysicalIdentityEvidence::Indeterminate,
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum LocalTokenKind {
    Extended,
    Legacy(LegacyFileIdGuarantee),
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
        Foundation::{GetLastError, HANDLE},
        Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
    };

    let handle = file.as_raw_handle() as HANDLE;
    let mut identity = FILE_ID_INFO::default();
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&mut identity as *mut FILE_ID_INFO).cast::<c_void>(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if succeeded != 0 {
        if let Some(file_id) = ExtendedFileId::new(identity.FileId.Identifier) {
            return Some(PhysicalFileIdentity {
                volume: identity.VolumeSerialNumber,
                file_id: WindowsFileId::Extended(file_id),
            });
        }
        if identity.FileId.Identifier != [0; 16] {
            return None;
        }
        let file_system = query_windows_file_system(handle)?;
        return (file_system == WindowsFileSystem::Udfs)
            .then(|| query_guaranteed_legacy_identity(handle, file_system))
            .flatten();
    }
    let extended_error = unsafe { GetLastError() };
    let file_system = query_windows_file_system(handle)?;
    if ExtendedFileIdQueryFailure::from_observation(extended_error, file_system)
        != ExtendedFileIdQueryFailure::Unsupported
    {
        return None;
    }

    query_guaranteed_legacy_identity(handle, file_system)
}

#[cfg(windows)]
fn query_guaranteed_legacy_identity(
    handle: windows_sys::Win32::Foundation::HANDLE,
    file_system: WindowsFileSystem,
) -> Option<PhysicalFileIdentity> {
    let mut legacy = windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION::default();
    let succeeded = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(handle, &mut legacy)
    };
    if succeeded == 0 {
        return None;
    }
    let identifier =
        (u64::from(legacy.nFileIndexHigh) << 32 | u64::from(legacy.nFileIndexLow)).to_be_bytes();
    legacy_identity_from_observation(
        u64::from(legacy.dwVolumeSerialNumber),
        identifier,
        file_system,
    )
}

#[cfg(windows)]
fn query_windows_file_system(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Option<WindowsFileSystem> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationByHandleW;

    let mut name = [0_u16; 32];
    let succeeded = unsafe {
        GetVolumeInformationByHandleW(
            handle,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if succeeded == 0 {
        return None;
    }
    let length = name
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(name.len());
    Some(WindowsFileSystem::from_api_name(&name[..length]))
}

#[cfg(all(test, windows))]
mod windows_identity_tests {
    use super::{
        ExtendedFileId, ExtendedFileIdQueryFailure, LegacyFileId, LegacyFileIdGuarantee,
        PhysicalFileIdentity, PhysicalIdentityEvidence, WindowsFileId, WindowsFileSystem,
        legacy_identity_from_observation,
    };
    use windows_sys::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED,
    };

    fn extended_id(identifier: [u8; 16]) -> WindowsFileId {
        WindowsFileId::Extended(ExtendedFileId::new(identifier).expect("authoritative fixture"))
    }

    fn legacy_id(identifier: [u8; 8]) -> WindowsFileId {
        WindowsFileId::Legacy(
            LegacyFileId::new(LegacyFileIdGuarantee::Ntfs, identifier)
                .expect("authoritative fixture"),
        )
    }

    #[test]
    fn extended_and_legacy_file_ids_round_trip_without_sharing_a_token_shape() {
        let extended = PhysicalFileIdentity {
            volume: 7,
            file_id: extended_id([3; 16]),
        };
        let legacy = PhysicalFileIdentity {
            volume: 7,
            file_id: legacy_id([3; 8]),
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

    #[test]
    fn an_all_zero_extended_file_id_is_never_authoritative() {
        assert_eq!(
            PhysicalFileIdentity::from_local_token(
                "windows-file-id-v1:0000000000000007:00000000000000000000000000000000",
            ),
            None
        );
    }

    #[test]
    fn an_all_ones_extended_file_id_is_never_authoritative() {
        assert_eq!(
            PhysicalFileIdentity::from_local_token(
                "windows-file-id-v1:0000000000000007:ffffffffffffffffffffffffffffffff",
            ),
            None
        );
    }

    #[test]
    fn a_provenance_free_legacy_file_id_cannot_authorize_same() {
        let token = "windows-file-index-v1:0000000000000007:0303030303030303";
        let evidence = match (
            PhysicalFileIdentity::from_local_token(token),
            PhysicalFileIdentity::from_local_token(token),
        ) {
            (Some(left), Some(right)) => left.compare(right),
            _ => PhysicalIdentityEvidence::Indeterminate,
        };

        assert_eq!(evidence, PhysicalIdentityEvidence::Indeterminate);
    }

    #[test]
    fn equal_legacy_ids_on_refs_or_an_unknown_filesystem_are_indeterminate() {
        for file_system in [WindowsFileSystem::Refs, WindowsFileSystem::Other] {
            let left = legacy_identity_from_observation(7, [3; 8], file_system);
            let right = legacy_identity_from_observation(7, [3; 8], file_system);
            let evidence = match (left, right) {
                (Some(left), Some(right)) => left.compare(right),
                _ => PhysicalIdentityEvidence::Indeterminate,
            };

            assert_eq!(evidence, PhysicalIdentityEvidence::Indeterminate);
        }
    }

    #[test]
    fn an_unexpected_extended_file_id_error_never_falls_back_to_legacy_identity() {
        for (error, file_system) in [
            (ERROR_ACCESS_DENIED, WindowsFileSystem::Ntfs),
            (ERROR_INVALID_PARAMETER, WindowsFileSystem::Ntfs),
            (ERROR_INVALID_PARAMETER, WindowsFileSystem::Refs),
            (ERROR_INVALID_PARAMETER, WindowsFileSystem::Other),
        ] {
            assert_eq!(
                ExtendedFileIdQueryFailure::from_observation(error, file_system),
                ExtendedFileIdQueryFailure::Unexpected
            );
        }
    }

    #[test]
    fn unsupported_extended_queries_can_use_a_guaranteed_ntfs_legacy_id() {
        for error in [ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED] {
            assert_eq!(
                ExtendedFileIdQueryFailure::from_observation(error, WindowsFileSystem::Ntfs,),
                ExtendedFileIdQueryFailure::Unsupported
            );
            let left = legacy_identity_from_observation(7, [3; 8], WindowsFileSystem::Ntfs)
                .expect("NTFS guarantees the non-sentinel legacy identifier");
            let right = legacy_identity_from_observation(7, [3; 8], WindowsFileSystem::Ntfs)
                .expect("the same guaranteed observation remains authoritative");

            assert_eq!(left.compare(right), PhysicalIdentityEvidence::Same);
        }
    }

    #[test]
    fn udfs_can_treat_invalid_parameter_as_an_unsupported_extended_query() {
        assert_eq!(
            ExtendedFileIdQueryFailure::from_observation(
                ERROR_INVALID_PARAMETER,
                WindowsFileSystem::Udfs,
            ),
            ExtendedFileIdQueryFailure::Unsupported
        );
        let left = legacy_identity_from_observation(7, [3; 8], WindowsFileSystem::Udfs)
            .expect("UDF has no extended IDs and guarantees its legacy ID");
        let right = legacy_identity_from_observation(7, [3; 8], WindowsFileSystem::Udfs)
            .expect("the same UDF observation remains authoritative");

        assert_eq!(left.compare(right), PhysicalIdentityEvidence::Same);
    }

    #[test]
    fn filesystem_names_are_classified_exactly_before_granting_legacy_authority() {
        assert_eq!(
            WindowsFileSystem::from_api_name(&"NTFS".encode_utf16().collect::<Vec<_>>()),
            WindowsFileSystem::Ntfs
        );
        assert_eq!(
            WindowsFileSystem::from_api_name(&"ReFS".encode_utf16().collect::<Vec<_>>()),
            WindowsFileSystem::Refs
        );
        assert_eq!(
            WindowsFileSystem::from_api_name(&"NTFS-compatible".encode_utf16().collect::<Vec<_>>()),
            WindowsFileSystem::Other
        );
    }

    #[test]
    fn physical_identity_comparison_is_closed_across_file_id_domains() {
        let extended = PhysicalFileIdentity {
            volume: 7,
            file_id: extended_id([3; 16]),
        };
        let other_extended = PhysicalFileIdentity {
            volume: 7,
            file_id: extended_id([4; 16]),
        };
        let legacy = PhysicalFileIdentity {
            volume: 7,
            file_id: legacy_id([3; 8]),
        };
        let other_volume = PhysicalFileIdentity {
            volume: 8,
            file_id: legacy_id([3; 8]),
        };

        assert_eq!(extended.compare(extended), PhysicalIdentityEvidence::Same);
        assert_eq!(
            extended.compare(other_extended),
            PhysicalIdentityEvidence::Different
        );
        assert_eq!(
            legacy.compare(other_volume),
            PhysicalIdentityEvidence::Different
        );
        assert_eq!(
            extended.compare(legacy),
            PhysicalIdentityEvidence::Indeterminate
        );
        assert_eq!(
            legacy.compare(extended),
            PhysicalIdentityEvidence::Indeterminate
        );
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

    /// Compares the complete Unix device/inode identity pair.
    pub fn compare(self, other: Self) -> PhysicalIdentityEvidence {
        if self == other {
            PhysicalIdentityEvidence::Same
        } else {
            PhysicalIdentityEvidence::Different
        }
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
