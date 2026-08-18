use std::{
    io,
    os::windows::ffi::OsStrExt,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

#[derive(Debug)]
pub(crate) struct NamedMutex {
    name: Vec<u16>,
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
    pub(crate) fn new(name: impl AsRef<std::ffi::OsStr>, worker_name: &'static str) -> Self {
        let name = name
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        assert_eq!(
            name.iter().filter(|unit| **unit == 0).count(),
            1,
            "a named mutex cannot contain an embedded NUL"
        );
        Self { name, worker_name }
    }

    pub(crate) fn try_acquire(&self) -> Result<NamedMutexGrant, NamedMutexError> {
        let name = self.name.clone();
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
