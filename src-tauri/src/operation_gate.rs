use std::{
    fmt, io,
    os::windows::ffi::OsStrExt,
    sync::mpsc::{self, Sender},
    thread::{self, JoinHandle},
};

use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

#[derive(Debug)]
pub(crate) struct OperationGate {
    mutex_name: Vec<u16>,
}

#[derive(Debug)]
pub(crate) struct OperationGrant {
    release: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OperationGateError {
    Conflict,
    Unavailable { reason: String },
}

enum WorkerAcquisition {
    Acquired,
    Conflict,
    Unavailable(String),
}

impl OperationGate {
    pub(crate) fn new(app_paths: &AppPaths) -> Self {
        let mut digest = Sha256::new();
        for unit in app_paths.local_root().as_os_str().encode_wide() {
            digest.update(unit.to_le_bytes());
        }
        let digest = digest.finalize();
        let suffix = digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mutex_name = format!(r"Local\MyAlbuns.OperationGate.v1.{suffix}")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        Self { mutex_name }
    }

    pub(crate) fn try_acquire(&self) -> Result<OperationGrant, OperationGateError> {
        let mutex_name = self.mutex_name.clone();
        let (acquisition_sender, acquisition_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("myalbuns-operation-gate".into())
            .spawn(move || {
                let handle = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr()) };
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
            .map_err(|error| OperationGateError::Unavailable {
                reason: error.to_string(),
            })?;

        match acquisition_receiver.recv() {
            Ok(WorkerAcquisition::Acquired) => Ok(OperationGrant {
                release: Some(release_sender),
                worker: Some(worker),
            }),
            Ok(WorkerAcquisition::Conflict) => {
                let _ = worker.join();
                Err(OperationGateError::Conflict)
            }
            Ok(WorkerAcquisition::Unavailable(reason)) => {
                let _ = worker.join();
                Err(OperationGateError::Unavailable { reason })
            }
            Err(error) => {
                let _ = worker.join();
                Err(OperationGateError::Unavailable {
                    reason: error.to_string(),
                })
            }
        }
    }
}

impl fmt::Display for OperationGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(formatter, "outra operação exclusiva já está em andamento"),
            Self::Unavailable { reason } => {
                write!(formatter, "não foi possível reservar a operação: {reason}")
            }
        }
    }
}

impl std::error::Error for OperationGateError {}

impl Drop for OperationGrant {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::Path,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use myalbuns_paths::AppPaths;
    use tempfile::tempdir;

    use super::{OperationGate, OperationGateError};
    use crate::{
        cache_engine::CacheEngine,
        imaging_processor::ImagingProcessor,
        operation_lease::{OperationLease, OperationLeaseError},
    };

    const OWNER_ROOT_ENV: &str = "MYALBUNS_OPERATION_GATE_OWNER_ROOT";
    const OWNER_READY_ENV: &str = "MYALBUNS_OPERATION_GATE_OWNER_READY";

    fn app_paths(root: &Path) -> AppPaths {
        AppPaths::from_roots(&root.join("roaming"), &root.join("local"), root)
    }

    #[test]
    fn same_process_callers_share_one_grant_without_queue_and_release_on_drop() {
        let root = tempdir().expect("the gate fixture exists");
        let paths = app_paths(root.path());
        let first_gate = OperationGate::new(&paths);
        let second_gate = OperationGate::new(&paths);

        let grant = first_gate
            .try_acquire()
            .expect("the first caller acquires immediately");
        assert!(
            matches!(second_gate.try_acquire(), Err(OperationGateError::Conflict)),
            "a concurrent caller is refused instead of queued"
        );

        drop(grant);
        second_gate
            .try_acquire()
            .expect("the grant is available immediately after its owner releases it");
    }

    #[test]
    fn independent_hosts_share_one_grant_and_recover_after_owner_process_termination() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the process gate fixture exists");
            let ready = root.path().join("owner.ready");
            let paths = app_paths(root.path());
            let mut owner = Command::new(env::current_exe().expect("the test executable is known"))
                .arg("operation_gate::tests::operation_gate_owner_process")
                .args(["--ignored", "--exact", "--nocapture"])
                .env(OWNER_ROOT_ENV, root.path())
                .env(OWNER_READY_ENV, &ready)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("the owner process starts");
            let deadline = Instant::now() + Duration::from_secs(10);
            while !ready.is_file() {
                assert!(
                    owner
                        .try_wait()
                        .expect("the owner state is readable")
                        .is_none(),
                    "the owner exited before acquiring the lease"
                );
                assert!(
                    Instant::now() < deadline,
                    "the owner did not acquire the lease in time"
                );
                thread::sleep(Duration::from_millis(20));
            }

            let challenger_cache = CacheEngine::default();
            let challenger_processor = ImagingProcessor::default();
            assert!(matches!(
                OperationLease::acquire(
                    &OperationGate::new(&paths),
                    &challenger_cache,
                    &challenger_processor,
                )
                .await,
                Err(OperationLeaseError::Gate(OperationGateError::Conflict))
            ));
            owner.kill().expect("the owner process is terminated");
            owner.wait().expect("the terminated owner is reaped");

            let recovered = OperationLease::acquire(
                &OperationGate::new(&paths),
                &challenger_cache,
                &challenger_processor,
            )
            .await
            .expect("the successor acquires the complete lease after owner death");
            drop(recovered);
        });
    }

    #[test]
    #[ignore = "spawned by the real OperationGate process test"]
    fn operation_gate_owner_process() {
        let root = env::var_os(OWNER_ROOT_ENV).expect("the owner root is configured");
        let ready = env::var_os(OWNER_READY_ENV).expect("the owner ready path is configured");
        let root = std::path::PathBuf::from(root);
        let paths = app_paths(&root);
        tauri::async_runtime::block_on(async {
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let _lease = OperationLease::acquire(&OperationGate::new(&paths), &cache, &processor)
                .await
                .expect("the child owns the complete operation lease");
            std::fs::write(ready, b"owned").expect("the child signals lease ownership");
            thread::sleep(Duration::from_secs(120));
        });
    }
}
