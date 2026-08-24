use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(windows)]
use {
    sha2::{Digest, Sha256},
    std::os::windows::ffi::OsStrExt,
    windows_sys::Win32::{
        Foundation::{
            CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};

pub(crate) fn write_atomically(target: &Path, bytes: &[u8], fallback_name: &str) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "preference path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temporary = sibling_temporary(target, fallback_name);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn sibling_temporary(target: &Path, fallback_name: &str) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback_name);
    target.with_file_name(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ))
}

#[cfg(windows)]
pub(crate) fn preference_mutex_name(namespace: &str, root: &Path) -> Vec<u16> {
    let mut digest = Sha256::new();
    for unit in root.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    let suffix = digest.finalize()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(r"Local\MyAlbuns.{namespace}.v1.{suffix}")
        .encode_utf16()
        .chain(Some(0))
        .collect()
}

#[cfg(windows)]
pub(crate) struct CrossProcessPreferenceGuard(HANDLE);

#[cfg(windows)]
impl CrossProcessPreferenceGuard {
    pub(crate) fn acquire(name: &[u16], store_name: &str) -> io::Result<Self> {
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        match unsafe { WaitForSingleObject(handle, 5_000) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
            WAIT_TIMEOUT => {
                unsafe { CloseHandle(handle) };
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("timed out waiting for the {store_name} writer"),
                ))
            }
            WAIT_FAILED => {
                let error = io::Error::last_os_error();
                unsafe { CloseHandle(handle) };
                Err(error)
            }
            other => {
                unsafe { CloseHandle(handle) };
                Err(io::Error::other(format!(
                    "unexpected {store_name} mutex result: {other}",
                )))
            }
        }
    }
}

#[cfg(windows)]
impl Drop for CrossProcessPreferenceGuard {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let succeeded = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temporary, target)
}
