use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuardedFsError {
    AlreadyExists,
    NotFound,
    Unavailable,
    OutsideRoot,
}

#[derive(Debug)]
pub(crate) struct DirectoryGuard {
    pub(crate) containment_handle: File,
    pub(crate) logical_path: PathBuf,
    pub(crate) physical_path: PathBuf,
}

#[cfg(not(windows))]
pub(crate) fn ensure_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<DirectoryGuard, GuardedFsError> {
    validate_child_location(parent, child_path)?;
    match fs::create_dir(child_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(GuardedFsError::Unavailable),
    }
    let metadata = fs::symlink_metadata(child_path).map_err(|_| GuardedFsError::Unavailable)?;
    open_validated_direct_child(parent, child_path, &metadata)
}

#[cfg(not(windows))]
pub(crate) fn open_existing_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<Option<DirectoryGuard>, GuardedFsError> {
    validate_child_location(parent, child_path)?;
    let metadata = match fs::symlink_metadata(child_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(GuardedFsError::Unavailable),
    };
    open_validated_direct_child(parent, child_path, &metadata).map(Some)
}

#[cfg(windows)]
pub(crate) fn ensure_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<DirectoryGuard, GuardedFsError> {
    open_relative_directory(parent, child_path, relative_file::FILE_OPEN_IF)
}

#[cfg(windows)]
pub(crate) fn open_existing_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<Option<DirectoryGuard>, GuardedFsError> {
    match open_relative_directory(parent, child_path, relative_file::FILE_OPEN) {
        Ok(directory) => Ok(Some(directory)),
        Err(GuardedFsError::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}

fn validate_child_location(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<(), GuardedFsError> {
    if child_path.parent() != Some(parent.logical_path.as_path()) {
        return Err(GuardedFsError::OutsideRoot);
    }
    Ok(())
}

#[cfg(not(windows))]
fn open_validated_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
    metadata: &fs::Metadata,
) -> Result<DirectoryGuard, GuardedFsError> {
    if is_reparse_point(metadata) || !metadata.is_dir() {
        return Err(GuardedFsError::OutsideRoot);
    }
    let child = open_directory(child_path)?;
    let expected_name = child_path.file_name().ok_or(GuardedFsError::OutsideRoot)?;
    if !is_direct_physical_child(&parent.physical_path, &child.physical_path, expected_name) {
        return Err(GuardedFsError::OutsideRoot);
    }
    Ok(child)
}

#[cfg(not(windows))]
pub(crate) fn remove_empty_directory(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<(), GuardedFsError> {
    validate_child_location(parent, path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| GuardedFsError::Unavailable)?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(GuardedFsError::OutsideRoot);
    }
    fs::remove_dir(path).map_err(|_| GuardedFsError::Unavailable)
}

#[cfg(windows)]
pub(crate) fn remove_empty_directory(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<(), GuardedFsError> {
    let directory = relative_file::open_directory(
        parent,
        path,
        windows_sys::Win32::Storage::FileSystem::DELETE,
        relative_file::FILE_OPEN,
    )?;
    mark_open_file_for_deletion(&directory.containment_handle)
}

#[cfg(windows)]
pub(crate) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
pub(crate) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

pub(crate) fn is_direct_physical_child(
    parent: &Path,
    child: &Path,
    expected_name: &std::ffi::OsStr,
) -> bool {
    child.parent() == Some(parent)
        && child
            .file_name()
            .is_some_and(|actual_name| same_component(actual_name, expected_name))
}

pub(crate) fn validate_open_file(
    parent: &DirectoryGuard,
    logical_path: &Path,
    file: &File,
) -> Result<(), GuardedFsError> {
    if !parent
        .containment_handle
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_dir()
    {
        return Err(GuardedFsError::OutsideRoot);
    }
    if !file
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_file()
    {
        return Err(GuardedFsError::Unavailable);
    }
    let physical_path = physical_path_from_file(file, logical_path)?;
    let expected_name = logical_path
        .file_name()
        .ok_or(GuardedFsError::OutsideRoot)?;
    if !is_direct_physical_child(&parent.physical_path, &physical_path, expected_name) {
        return Err(GuardedFsError::OutsideRoot);
    }
    Ok(())
}

#[cfg(windows)]
mod relative_file {
    use std::{
        ffi::OsStr,
        fs::File,
        os::windows::{ffi::OsStrExt, io::FromRawHandle},
        path::{Path, PathBuf},
    };

    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT,
                FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
            },
        },
        Win32::{
            Foundation::{
                HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, STATUS_NO_SUCH_FILE,
                STATUS_OBJECT_NAME_COLLISION, STATUS_OBJECT_NAME_NOT_FOUND,
                STATUS_OBJECT_PATH_NOT_FOUND, UNICODE_STRING,
            },
            Storage::FileSystem::{
                FILE_ACCESS_RIGHTS, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
                FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
                SYNCHRONIZE,
            },
            System::IO::IO_STATUS_BLOCK,
        },
    };

    use super::{DirectoryGuard, GuardedFsError, is_reparse_point, validate_child_location};

    pub(super) use windows_sys::Wdk::Storage::FileSystem::{FILE_CREATE, FILE_OPEN, FILE_OPEN_IF};

    pub(super) fn open_regular(
        parent: &DirectoryGuard,
        path: &Path,
        desired_access: FILE_ACCESS_RIGHTS,
        disposition: u32,
    ) -> Result<File, GuardedFsError> {
        let file = open_relative(
            parent,
            path,
            desired_access | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            disposition,
            FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
        )?;
        let metadata = file.metadata().map_err(|_| GuardedFsError::Unavailable)?;
        if is_reparse_point(&metadata) || !metadata.is_file() {
            return Err(GuardedFsError::OutsideRoot);
        }
        Ok(file)
    }

    pub(super) fn open_directory(
        parent: &DirectoryGuard,
        path: &Path,
        desired_access: FILE_ACCESS_RIGHTS,
        disposition: u32,
    ) -> Result<DirectoryGuard, GuardedFsError> {
        let handle = open_relative(
            parent,
            path,
            desired_access | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            disposition,
            FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
        )?;
        let metadata = handle.metadata().map_err(|_| GuardedFsError::Unavailable)?;
        if is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(GuardedFsError::OutsideRoot);
        }
        let physical_path = super::physical_path_from_file(&handle, path)?;
        Ok(DirectoryGuard {
            containment_handle: handle,
            logical_path: path.to_path_buf(),
            physical_path,
        })
    }

    fn open_relative(
        parent: &DirectoryGuard,
        path: &Path,
        desired_access: FILE_ACCESS_RIGHTS,
        disposition: u32,
        options: u32,
    ) -> Result<File, GuardedFsError> {
        validate_child_location(parent, path)?;
        let name = path.file_name().ok_or(GuardedFsError::OutsideRoot)?;
        validate_one_component(name)?;
        let mut encoded = name.encode_wide().collect::<Vec<_>>();
        if encoded.is_empty() || encoded.contains(&0) {
            return Err(GuardedFsError::OutsideRoot);
        }
        let length = encoded
            .len()
            .checked_mul(std::mem::size_of::<u16>())
            .and_then(|bytes| u16::try_from(bytes).ok())
            .ok_or(GuardedFsError::OutsideRoot)?;
        let name = UNICODE_STRING {
            Length: length,
            MaximumLength: length,
            Buffer: encoded.as_mut_ptr(),
        };
        let attributes = OBJECT_ATTRIBUTES {
            Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
                .map_err(|_| GuardedFsError::Unavailable)?,
            RootDirectory: parent.containment_handle.as_raw_handle().cast(),
            ObjectName: std::ptr::from_ref(&name),
            Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut status_block = IO_STATUS_BLOCK::default();
        let mut handle: HANDLE = std::ptr::null_mut();
        // SAFETY: every pointer references a live, initialized buffer for the
        // duration of the call. RootDirectory is owned by `parent`, the object
        // name is one relative component, and the returned handle is adopted
        // exactly once by `File` on success.
        let status = unsafe {
            NtCreateFile(
                &mut handle,
                desired_access,
                &attributes,
                &mut status_block,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL as FILE_FLAGS_AND_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                disposition,
                options,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 {
            if status == STATUS_OBJECT_NAME_COLLISION {
                return Err(GuardedFsError::AlreadyExists);
            }
            return Err(
                if matches!(
                    status,
                    STATUS_NO_SUCH_FILE
                        | STATUS_OBJECT_NAME_NOT_FOUND
                        | STATUS_OBJECT_PATH_NOT_FOUND
                ) {
                    GuardedFsError::NotFound
                } else {
                    GuardedFsError::Unavailable
                },
            );
        }
        if handle.is_null() {
            return Err(GuardedFsError::Unavailable);
        }
        // SAFETY: NtCreateFile returned one owned HANDLE on success.
        Ok(unsafe { File::from_raw_handle(handle.cast()) })
    }

    fn validate_one_component(name: &OsStr) -> Result<(), GuardedFsError> {
        let path = PathBuf::from(name);
        if path.components().count() != 1 {
            return Err(GuardedFsError::OutsideRoot);
        }
        Ok(())
    }

    use std::os::windows::io::AsRawHandle;
}

#[cfg(windows)]
fn open_relative_directory(
    parent: &DirectoryGuard,
    path: &Path,
    disposition: u32,
) -> Result<DirectoryGuard, GuardedFsError> {
    relative_file::open_directory(parent, path, 0, disposition)
}

#[cfg(windows)]
pub(crate) fn create_new_deletable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    use windows_sys::Win32::{Foundation::GENERIC_WRITE, Storage::FileSystem::DELETE};

    relative_file::open_regular(
        parent,
        path,
        GENERIC_WRITE | DELETE,
        relative_file::FILE_CREATE,
    )
}

#[cfg(not(windows))]
pub(crate) fn create_new_deletable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    validate_child_location(parent, path)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| GuardedFsError::Unavailable)?;
    validate_open_file(parent, path, &file)?;
    Ok(file)
}

#[cfg(windows)]
pub(crate) fn open_deletable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    use windows_sys::Win32::{Foundation::GENERIC_READ, Storage::FileSystem::DELETE};

    relative_file::open_regular(
        parent,
        path,
        GENERIC_READ | DELETE,
        relative_file::FILE_OPEN,
    )
}

#[cfg(windows)]
pub(crate) fn open_readable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    use windows_sys::Win32::Foundation::GENERIC_READ;

    relative_file::open_regular(parent, path, GENERIC_READ, relative_file::FILE_OPEN)
}

#[cfg(not(windows))]
pub(crate) fn open_deletable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    validate_child_location(parent, path)?;
    let file = File::open(path).map_err(|_| GuardedFsError::Unavailable)?;
    validate_open_file(parent, path, &file)?;
    Ok(file)
}

#[cfg(not(windows))]
pub(crate) fn open_readable_file(
    parent: &DirectoryGuard,
    path: &Path,
) -> Result<File, GuardedFsError> {
    validate_child_location(parent, path)?;
    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GuardedFsError::NotFound
        } else {
            GuardedFsError::Unavailable
        }
    })?;
    validate_open_file(parent, path, &file)?;
    Ok(file)
}

#[cfg(windows)]
pub(crate) fn delete_open_file(
    parent: &DirectoryGuard,
    path: &Path,
    file: &File,
) -> Result<(), GuardedFsError> {
    validate_held_file_capability(parent, path, file)?;
    mark_open_file_for_deletion(file)
}

#[cfg(windows)]
fn validate_held_file_capability(
    parent: &DirectoryGuard,
    logical_path: &Path,
    file: &File,
) -> Result<(), GuardedFsError> {
    validate_child_location(parent, logical_path)?;
    if !parent
        .containment_handle
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_dir()
    {
        return Err(GuardedFsError::OutsideRoot);
    }
    if !file
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_file()
    {
        return Err(GuardedFsError::Unavailable);
    }
    Ok(())
}

#[cfg(windows)]
fn mark_open_file_for_deletion(file: &File) -> Result<(), GuardedFsError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: file is a live handle opened with DELETE access and disposition
    // points to the documented buffer for FileDispositionInfo.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle().cast(),
            FileDispositionInfo,
            std::ptr::from_ref(&disposition).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(GuardedFsError::Unavailable);
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn delete_open_file(
    parent: &DirectoryGuard,
    path: &Path,
    file: &File,
) -> Result<(), GuardedFsError> {
    validate_open_file(parent, path, file)?;
    fs::remove_file(path).map_err(|_| GuardedFsError::Unavailable)
}

#[cfg(windows)]
pub(crate) fn rename_open_file(
    parent: &DirectoryGuard,
    source_path: &Path,
    file: &File,
    target_name: &std::ffi::OsStr,
) -> Result<(), GuardedFsError> {
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::{
        Wdk::Storage::FileSystem::{
            FILE_RENAME_INFORMATION, FileRenameInformation, NtSetInformationFile,
        },
        Win32::System::IO::IO_STATUS_BLOCK,
    };

    validate_held_file_capability(parent, source_path, file)?;
    if Path::new(target_name).components().count() != 1 {
        return Err(GuardedFsError::OutsideRoot);
    }
    let target: Vec<u16> = target_name.encode_wide().collect();
    let target_bytes = target
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(GuardedFsError::Unavailable)?;
    let buffer_bytes = std::mem::size_of::<FILE_RENAME_INFORMATION>()
        .checked_add(target_bytes as usize)
        .ok_or(GuardedFsError::Unavailable)?;
    let mut buffer = vec![0_u64; buffer_bytes.div_ceil(std::mem::size_of::<u64>())];
    let rename = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    // SAFETY: buffer is aligned and sized for the fixed header plus every
    // encoded target-name byte. RootDirectory remains live for the call.
    unsafe {
        (*rename).Anonymous.ReplaceIfExists = false;
        (*rename).RootDirectory = parent.containment_handle.as_raw_handle().cast();
        (*rename).FileNameLength = target_bytes;
        std::ptr::copy_nonoverlapping(
            target.as_ptr(),
            std::ptr::addr_of_mut!((*rename).FileName).cast::<u16>(),
            target.len(),
        );
    }
    let mut io_status = IO_STATUS_BLOCK::default();
    // SAFETY: rename addresses the initialized buffer described above, the
    // source handle was opened with DELETE access, and io_status is writable
    // for the documented user-mode NtSetInformationFile call.
    let status = unsafe {
        NtSetInformationFile(
            file.as_raw_handle().cast(),
            &mut io_status,
            rename.cast(),
            u32::try_from(buffer_bytes).map_err(|_| GuardedFsError::Unavailable)?,
            FileRenameInformation,
        )
    };
    if status < 0 {
        if status == windows_sys::Win32::Foundation::STATUS_OBJECT_NAME_COLLISION {
            return Err(GuardedFsError::AlreadyExists);
        }
        return Err(GuardedFsError::Unavailable);
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn rename_open_file(
    parent: &DirectoryGuard,
    source_path: &Path,
    file: &File,
    target_name: &std::ffi::OsStr,
) -> Result<(), GuardedFsError> {
    validate_open_file(parent, source_path, file)?;
    if Path::new(target_name).components().count() != 1 {
        return Err(GuardedFsError::OutsideRoot);
    }
    if parent.logical_path.join(target_name).exists() {
        return Err(GuardedFsError::AlreadyExists);
    }
    fs::rename(source_path, parent.logical_path.join(target_name))
        .map_err(|_| GuardedFsError::Unavailable)
}

fn same_component(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
}

#[cfg(windows)]
pub(crate) fn open_directory(path: &Path) -> Result<DirectoryGuard, GuardedFsError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let handle = OpenOptions::new()
        .read(true)
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|_| GuardedFsError::Unavailable)?;
    if !handle
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_dir()
    {
        return Err(GuardedFsError::Unavailable);
    }

    let physical_path = physical_path_from_file(&handle, path)?;

    Ok(DirectoryGuard {
        containment_handle: handle,
        logical_path: path.to_path_buf(),
        physical_path,
    })
}

#[cfg(windows)]
pub(crate) fn physical_path_from_file(
    file: &File,
    _logical_path: &Path,
) -> Result<PathBuf, GuardedFsError> {
    use std::{
        ffi::OsString,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS},
    };

    let mut buffer = vec![0_u16; 512];
    loop {
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as HANDLE,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if length == 0 {
            return Err(GuardedFsError::Unavailable);
        }
        if (length as usize) < buffer.len() {
            return Ok(PathBuf::from(OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
        buffer.resize(length as usize + 1, 0);
    }
}

#[cfg(not(windows))]
pub(crate) fn open_directory(path: &Path) -> Result<DirectoryGuard, GuardedFsError> {
    let handle = File::open(path).map_err(|_| GuardedFsError::Unavailable)?;
    if !handle
        .metadata()
        .map_err(|_| GuardedFsError::Unavailable)?
        .is_dir()
    {
        return Err(GuardedFsError::Unavailable);
    }
    let physical_path = fs::canonicalize(path).map_err(|_| GuardedFsError::Unavailable)?;
    Ok(DirectoryGuard {
        containment_handle: handle,
        logical_path: path.to_path_buf(),
        physical_path,
    })
}

#[cfg(not(windows))]
pub(crate) fn physical_path_from_file(
    _file: &File,
    logical_path: &Path,
) -> Result<PathBuf, GuardedFsError> {
    fs::canonicalize(logical_path).map_err(|_| GuardedFsError::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, path::Path};

    use super::is_direct_physical_child;

    #[test]
    fn physical_containment_rejects_a_redirected_child() {
        assert!(is_direct_physical_child(
            Path::new("/local/MyAlbuns2/Cache"),
            Path::new("/local/MyAlbuns2/Cache/project-01"),
            OsStr::new("project-01"),
        ));
        assert!(!is_direct_physical_child(
            Path::new("/local/MyAlbuns2/Cache"),
            Path::new("/redirected/project-01"),
            OsStr::new("project-01"),
        ));
    }
}
