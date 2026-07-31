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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationMode {
    NormalExport,
    BatchExclusive,
    CacheMaintenance,
}

impl OperationMode {
    pub(crate) const ALL: [Self; 3] = [
        Self::NormalExport,
        Self::BatchExclusive,
        Self::CacheMaintenance,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NormalExport => "normal_export",
            Self::BatchExclusive => "batch_exclusive",
            Self::CacheMaintenance => "cache_maintenance",
        }
    }
}

#[derive(Debug)]
pub(crate) struct OperationGate {
    mutex_name: Vec<u16>,
}

#[derive(Debug)]
pub(crate) struct OperationGrant {
    mode: OperationMode,
    release: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OperationGateError {
    Conflict {
        requested: OperationMode,
    },
    Unavailable {
        requested: OperationMode,
        reason: String,
    },
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

    pub(crate) fn try_acquire(
        &self,
        mode: OperationMode,
    ) -> Result<OperationGrant, OperationGateError> {
        debug_assert!(OperationMode::ALL.contains(&mode));
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
                requested: mode,
                reason: error.to_string(),
            })?;

        match acquisition_receiver.recv() {
            Ok(WorkerAcquisition::Acquired) => Ok(OperationGrant {
                mode,
                release: Some(release_sender),
                worker: Some(worker),
            }),
            Ok(WorkerAcquisition::Conflict) => {
                let _ = worker.join();
                Err(OperationGateError::Conflict { requested: mode })
            }
            Ok(WorkerAcquisition::Unavailable(reason)) => {
                let _ = worker.join();
                Err(OperationGateError::Unavailable {
                    requested: mode,
                    reason,
                })
            }
            Err(error) => {
                let _ = worker.join();
                Err(OperationGateError::Unavailable {
                    requested: mode,
                    reason: error.to_string(),
                })
            }
        }
    }
}

impl fmt::Display for OperationGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { requested } => write!(
                formatter,
                "outra operação exclusiva já está em andamento ({})",
                requested.as_str()
            ),
            Self::Unavailable { requested, reason } => write!(
                formatter,
                "não foi possível reservar a operação {}: {reason}",
                requested.as_str()
            ),
        }
    }
}

impl std::error::Error for OperationGateError {}

impl OperationGrant {
    pub(crate) const fn mode(&self) -> OperationMode {
        self.mode
    }
}

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
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use myalbuns_paths::AppPaths;
    use tempfile::tempdir;

    use super::{OperationGate, OperationGateError, OperationMode};
    use crate::{
        cache_engine::CacheEngine,
        imaging_processor::ImagingProcessor,
        operation_lease::{OperationLease, OperationLeaseError},
    };

    const OWNER_ROOT_ENV: &str = "MYALBUNS_OPERATION_GATE_OWNER_ROOT";
    const OWNER_READY_ENV: &str = "MYALBUNS_OPERATION_GATE_OWNER_READY";

    #[test]
    fn same_host_window_callers_share_one_grant_without_queue_and_release_on_drop() {
        let root = tempdir().expect("the gate fixture exists");
        let paths =
            AppPaths::from_known_folders(&root.path().join("roaming"), &root.path().join("local"));
        let first_gate = OperationGate::new(&paths);
        let second_gate = OperationGate::new(&paths);

        let grant = first_gate
            .try_acquire(OperationMode::NormalExport)
            .expect("the first caller acquires immediately");
        assert!(
            matches!(
                second_gate.try_acquire(OperationMode::NormalExport),
                Err(OperationGateError::Conflict {
                    requested: OperationMode::NormalExport,
                })
            ),
            "a concurrent caller is refused instead of queued"
        );

        drop(grant);
        second_gate
            .try_acquire(OperationMode::NormalExport)
            .expect("the grant is available immediately after its owner releases it");
    }

    #[test]
    fn every_operation_mode_uses_the_same_small_global_exclusion_boundary() {
        for owner_mode in OperationMode::ALL {
            for requested_mode in OperationMode::ALL {
                let root = tempdir().expect("the mode-matrix fixture exists");
                let paths = AppPaths::from_known_folders(
                    &root.path().join("roaming"),
                    &root.path().join("local"),
                );
                let owner_gate = OperationGate::new(&paths);
                let challenger_gate = OperationGate::new(&paths);
                let owner = owner_gate
                    .try_acquire(owner_mode)
                    .expect("the matrix owner acquires its operation mode");

                assert!(
                    matches!(
                        challenger_gate.try_acquire(requested_mode),
                        Err(OperationGateError::Conflict { requested })
                            if requested == requested_mode
                    ),
                    "{owner_mode:?} must conflict with {requested_mode:?}"
                );

                drop(owner);
                challenger_gate
                    .try_acquire(requested_mode)
                    .expect("the requested mode becomes available after release");
            }
        }
    }

    #[test]
    #[ignore = "spawns and terminates a real owner process; executed by Test-OperationGate.ps1"]
    fn independent_hosts_share_one_grant_and_recover_after_owner_process_termination() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the process gate fixture exists");
            let ready = root.path().join("owner.ready");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
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
                    OperationMode::BatchExclusive,
                )
                .await,
                Err(OperationLeaseError::Gate(OperationGateError::Conflict {
                    requested: OperationMode::BatchExclusive,
                }))
            ));
            owner.kill().expect("the owner process is terminated");
            owner.wait().expect("the terminated owner is reaped");

            let recovered = OperationLease::acquire(
                &OperationGate::new(&paths),
                &challenger_cache,
                &challenger_processor,
                OperationMode::NormalExport,
            )
            .await
            .expect("the successor acquires the complete lease after owner death");
            assert_eq!(recovered.mode(), OperationMode::NormalExport);
        });
    }

    #[test]
    #[ignore = "spawned by the real OperationGate process test"]
    fn operation_gate_owner_process() {
        let root = env::var_os(OWNER_ROOT_ENV).expect("the owner root is configured");
        let ready = env::var_os(OWNER_READY_ENV).expect("the owner ready path is configured");
        let root = std::path::PathBuf::from(root);
        let paths = AppPaths::from_known_folders(&root.join("roaming"), &root.join("local"));
        tauri::async_runtime::block_on(async {
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let _lease = OperationLease::acquire(
                &OperationGate::new(&paths),
                &cache,
                &processor,
                OperationMode::NormalExport,
            )
            .await
            .expect("the child owns the complete operation lease");
            std::fs::write(ready, b"owned").expect("the child signals lease ownership");
            thread::sleep(Duration::from_secs(120));
        });
    }
}
