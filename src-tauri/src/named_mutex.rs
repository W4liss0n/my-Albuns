use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

#[derive(Clone, Debug)]
pub(crate) struct NamedMutex {
    name: Result<Vec<u16>, String>,
    worker_name: &'static str,
}

#[derive(Debug)]
pub(crate) struct NamedMutexGrant {
    release: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NamedMutexError {
    Conflict,
    Unavailable(String),
}

enum WorkerAcquisition {
    Acquired,
    Conflict,
    Unavailable(String),
}

impl NamedMutex {
    pub(crate) fn scoped(
        app_paths: &AppPaths,
        kind: &str,
        scope: &str,
        worker_name: &'static str,
    ) -> Self {
        let name = scoped_name(app_paths.local_root(), kind, scope).map(|name| {
            std::ffi::OsStr::new(&name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
        });
        Self { name, worker_name }
    }

    pub(crate) fn try_acquire(&self) -> Result<NamedMutexGrant, NamedMutexError> {
        let name = self
            .name
            .as_ref()
            .map_err(|reason| NamedMutexError::Unavailable(reason.clone()))?
            .clone();
        let (acquisition_sender, acquisition_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name(self.worker_name.into())
            .spawn(move || {
                let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
                if handle.is_null() {
                    let _ = acquisition_sender.send(WorkerAcquisition::Unavailable(
                        io::Error::last_os_error().to_string(),
                    ));
                    return;
                }
                let wait_result = unsafe { WaitForSingleObject(handle, 0) };
                let acquisition = match wait_result {
                    WAIT_OBJECT_0 | WAIT_ABANDONED => WorkerAcquisition::Acquired,
                    WAIT_TIMEOUT => WorkerAcquisition::Conflict,
                    WAIT_FAILED => {
                        WorkerAcquisition::Unavailable(io::Error::last_os_error().to_string())
                    }
                    other => WorkerAcquisition::Unavailable(format!(
                        "resultado inesperado do mutex: {other}"
                    )),
                };
                let owns_mutex = matches!(acquisition, WorkerAcquisition::Acquired);
                let acquisition_was_delivered = acquisition_sender.send(acquisition).is_ok();
                if owns_mutex {
                    if acquisition_was_delivered {
                        let _ = release_receiver.recv();
                    }
                    unsafe {
                        ReleaseMutex(handle);
                    }
                }
                unsafe {
                    CloseHandle(handle);
                }
            })
            .map_err(|error| NamedMutexError::Unavailable(error.to_string()))?;

        match acquisition_receiver.recv() {
            Ok(WorkerAcquisition::Acquired) => Ok(NamedMutexGrant {
                release: Some(release_sender),
                worker: Some(worker),
            }),
            Ok(WorkerAcquisition::Conflict) => {
                let _ = worker.join();
                Err(NamedMutexError::Conflict)
            }
            Ok(WorkerAcquisition::Unavailable(reason)) => {
                let _ = worker.join();
                Err(NamedMutexError::Unavailable(reason))
            }
            Err(error) => {
                let _ = worker.join();
                Err(NamedMutexError::Unavailable(error.to_string()))
            }
        }
    }
}

fn scoped_name(local_root: &Path, kind: &str, scope: &str) -> Result<String, String> {
    let mut digest = Sha256::new();
    for unit in windows_filesystem_case(local_root)? {
        digest.update(unit.to_le_bytes());
    }
    if !scope.is_empty() {
        digest.update([0]);
        for byte in scope.bytes() {
            digest.update([byte.to_ascii_lowercase()]);
        }
    }
    let digest = digest.finalize();
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(r"Local\MyAlbuns.{kind}.v1.{suffix}"))
}

fn windows_filesystem_case(path: &Path) -> Result<Vec<u16>, String> {
    use windows_sys::Win32::{
        Foundation::GetLastError,
        Globalization::{LCMAP_UPPERCASE, LCMapStringEx, LOCALE_NAME_INVARIANT},
    };

    let source = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let source_len = i32::try_from(source.len())
        .map_err(|_| "a raiz local excede o limite de case mapping do Windows".to_string())?;
    // SAFETY: source is a live UTF-16 buffer of source_len units. A null
    // destination with length zero asks Windows for the exact required size.
    let required = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if required == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        return Err(format!(
            "não foi possível normalizar a raiz local para o mutex (Windows {code})"
        ));
    }
    let mut mapped = vec![0_u16; required as usize];
    // SAFETY: mapped has the exact capacity returned by the preceding call;
    // all remaining pointers and lengths follow the same documented contract.
    let written = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            mapped.as_mut_ptr(),
            required,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if written == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        return Err(format!(
            "não foi possível materializar a raiz local normalizada (Windows {code})"
        ));
    }
    mapped.truncate(written as usize);
    Ok(mapped)
}

impl Drop for NamedMutexGrant {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
