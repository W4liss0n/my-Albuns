use std::{io, path::Path};

#[cfg(windows)]
pub fn publish_new_file(prepared: &Path, target: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let prepared = crate::wide_api_path(prepared);
    let target = crate::wide_api_path(target);
    // No replacement flag: a concurrently created target remains protected.
    let succeeded = unsafe { MoveFileExW(prepared.as_ptr(), target.as_ptr(), 0) };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn publish_new_file(prepared: &Path, target: &Path) -> io::Result<()> {
    std::fs::hard_link(prepared, target)?;
    std::fs::remove_file(prepared)
}

#[cfg(windows)]
pub fn replace_existing_file(prepared: &Path, target: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let target = crate::wide_api_path(target);
    let prepared = crate::wide_api_path(prepared);
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
pub fn replace_existing_file(prepared: &Path, target: &Path) -> io::Result<()> {
    std::fs::rename(prepared, target)
}
