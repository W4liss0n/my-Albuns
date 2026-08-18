use std::{fmt, os::windows::ffi::OsStrExt};

use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};

use crate::named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant};

#[derive(Debug)]
pub(crate) struct OperationGate {
    mutex: NamedMutex,
}

#[derive(Debug)]
pub(crate) struct OperationGrant {
    _grant: NamedMutexGrant,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OperationGateError {
    Conflict,
    Unavailable { reason: String },
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
        let mutex_name = format!(r"Local\MyAlbuns.OperationGate.v1.{suffix}");
        Self {
            mutex: NamedMutex::new(mutex_name, "myalbuns-operation-gate"),
        }
    }

    pub(crate) fn try_acquire(&self) -> Result<OperationGrant, OperationGateError> {
        self.mutex
            .try_acquire()
            .map(|grant| OperationGrant { _grant: grant })
            .map_err(|error| match error {
                NamedMutexError::Conflict => OperationGateError::Conflict,
                NamedMutexError::Unavailable(reason) => OperationGateError::Unavailable { reason },
            })
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
        AppPaths::from_roots(&root.join("roaming"), &root.join("local"))
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
