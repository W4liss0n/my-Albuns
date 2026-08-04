use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use uuid::Uuid;

pub(crate) fn sibling_temporary(target: &Path) -> Result<PathBuf, io::Error> {
    let file_name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing file name"))?;
    let mut temporary_name = OsString::from(".");
    temporary_name.push(file_name);
    temporary_name.push(format!(".create-{}.tmp", Uuid::new_v4().hyphenated()));
    Ok(target.with_file_name(temporary_name))
}

pub(crate) fn write_synced_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(windows)]
    use std::os::windows::fs::OpenOptionsExt;
    #[cfg(windows)]
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    let mut writer = options.open(path)?;
    writer.write_all(bytes)?;
    writer.flush()?;
    writer.sync_all()
}

#[cfg(windows)]
pub(crate) fn publish_new(prepared: &Path, target: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let prepared = wide_path(prepared);
    let target = wide_path(target);
    let succeeded = unsafe { MoveFileExW(prepared.as_ptr(), target.as_ptr(), 0) };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn publish_new(prepared: &Path, target: &Path) -> io::Result<()> {
    std::fs::hard_link(prepared, target)?;
    std::fs::remove_file(prepared)
}

#[cfg(windows)]
pub(crate) fn replace_existing(prepared: &Path, target: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let target = wide_path(target);
    let prepared = wide_path(prepared);
    let succeeded = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            prepared.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn replace_existing(prepared: &Path, target: &Path) -> io::Result<()> {
    std::fs::rename(prepared, target)
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
