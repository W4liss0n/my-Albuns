use std::{
    ffi::c_void,
    fs,
    io::{self, Write},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    ptr,
    time::Duration,
};

use myalbuns_paths::{CachePathPlan, ProcessInstanceHandle, ProcessInstanceId, publish_new_file};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        WaitForSingleObject,
    },
};

const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const CACHE_WRITER_CLAIM_SCHEMA_VERSION: u32 = 1;
const CACHE_WRITER_CLAIM_FILE: &str = ".processor-writer.v1.json";
const CACHE_WRITER_TEMPORARY_PREFIX: &str = ".processor-writer-";
const CACHE_WRITER_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CacheWriterClaim {
    schema_version: u32,
    process: ProcessInstanceId,
}

/// Owns the Windows Job that contains one dispatched Imaging process.
///
/// The Job handle is created and retained by the Project Host before a request
/// is written to the child. If the Host terminates, Windows closes its handle
/// and requests termination of the contained process. A published exact
/// process-instance claim lets the next namespace owner wait for the
/// asynchronous termination before it inspects or deletes Cache state.
#[derive(Debug)]
pub(crate) struct ProcessorChildLifetime {
    job: Option<OwnedHandle>,
    process: OwnedHandle,
    process_instance: ProcessInstanceId,
    claim_path: Option<PathBuf>,
}

impl ProcessorChildLifetime {
    pub(crate) fn attach(process_id: u32) -> io::Result<Self> {
        // SAFETY: null security/name creates a private non-inheritable Job. A
        // successful raw handle is immediately transferred to OwnedHandle.
        let job = unsafe {
            let handle = CreateJobObjectW(ptr::null(), ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            OwnedHandle::from_raw_handle(handle)
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: job is live and limits points to initialized storage of the
        // exact information-class size for the duration of this call.
        if unsafe {
            SetInformationJobObject(
                job.as_raw_handle().cast::<c_void>(),
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast_mut().cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: process_id came from the freshly spawned CommandChild. The
        // requested rights are exactly those required by AssignProcessToJobObject.
        let process = unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION
                    | PROCESS_SET_QUOTA
                    | PROCESS_SYNCHRONIZE
                    | PROCESS_TERMINATE,
                0,
                process_id,
            );
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            OwnedHandle::from_raw_handle(handle)
        };
        let process_instance = ProcessInstanceId::from_process_handle(
            process_id,
            process.as_raw_handle().cast::<c_void>(),
        )?;
        // SAFETY: both handles are live, owned handles of the required kinds.
        // Windows 8+ forms a nested Job when an outer launcher Job is present.
        if unsafe {
            AssignProcessToJobObject(
                job.as_raw_handle().cast::<c_void>(),
                process.as_raw_handle().cast::<c_void>(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            job: Some(job),
            process,
            process_instance,
            claim_path: None,
        })
    }

    /// Publishes the exact contained Processor instance before the Host sends
    /// a Cache command. A replacement Host can then wait for the instance's
    /// signaled exit before inspecting or deleting the namespace.
    pub(crate) fn publish_cache_writer_claim(&mut self, paths: &CachePathPlan) -> io::Result<()> {
        if self.claim_path.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the Processor already owns a Cache writer claim",
            ));
        }
        ensure_cache_root(paths.root())?;
        let claim_path = cache_writer_claim_path(paths);
        match fs::symlink_metadata(&claim_path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "a Cache writer claim already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let claim = CacheWriterClaim {
            schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
            process: self.process_instance,
        };
        let encoded = serde_json::to_vec(&claim)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let temporary = paths.root().join(format!(
            "{CACHE_WRITER_TEMPORARY_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let prepared = file.write_all(&encoded).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = prepared {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = publish_new_file(&temporary, &claim_path) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        self.claim_path = Some(claim_path);
        Ok(())
    }
}

impl Drop for ProcessorChildLifetime {
    fn drop(&mut self) {
        // Closing the last Job handle requests termination first. The process
        // handle remains open so removal of the claim is conditional on the
        // exact child reaching its signaled terminal state.
        drop(self.job.take());
        // SAFETY: self owns a process handle with synchronization rights.
        if unsafe { WaitForSingleObject(self.process.as_raw_handle().cast::<c_void>(), 0) }
            != WAIT_OBJECT_0
        {
            return;
        }
        let Some(claim_path) = self.claim_path.take() else {
            return;
        };
        let expected = CacheWriterClaim {
            schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
            process: self.process_instance,
        };
        if read_cache_writer_claim(&claim_path).is_ok_and(|claim| claim == expected) {
            let _ = fs::remove_file(claim_path);
        }
    }
}

/// Waits for a writer claim left by a dead Host before any authoritative
/// namespace inspection, recovery, or deletion begins.
pub(crate) fn await_cache_writer_quiescence(paths: &CachePathPlan) -> io::Result<()> {
    match ensure_cache_root(paths.root()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    let claim_path = cache_writer_claim_path(paths);
    let claim = match fs::symlink_metadata(&claim_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(io::Error::other(
                    "the Cache writer claim is not a regular file",
                ));
            }
            read_cache_writer_claim(&claim_path)?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            remove_abandoned_claim_temporaries(paths)?;
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if claim.schema_version != CACHE_WRITER_CLAIM_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the Cache writer claim schema is incompatible",
        ));
    }
    if let Some(process) = ProcessInstanceHandle::open_if_running(claim.process, 0)?
        && !process.wait_for_exit_timeout(CACHE_WRITER_WAIT_TIMEOUT)?
    {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the previous Cache writer did not terminate in time",
        ));
    }
    fs::remove_file(&claim_path)?;
    remove_abandoned_claim_temporaries(paths)
}

fn cache_writer_claim_path(paths: &CachePathPlan) -> PathBuf {
    paths.root().join(CACHE_WRITER_CLAIM_FILE)
}

fn ensure_cache_root(root: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(
            "the Cache writer claim root is not a regular directory",
        ));
    }
    Ok(())
}

fn read_cache_writer_claim(path: &Path) -> io::Result<CacheWriterClaim> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() || bytes.len() > 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the Cache writer claim has an invalid size",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn remove_abandoned_claim_temporaries(paths: &CachePathPlan) -> io::Result<()> {
    for entry in fs::read_dir(paths.root())? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(CACHE_WRITER_TEMPORARY_PREFIX) || !name.ends_with(".tmp") {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other(
                "a Cache writer claim temporary is not a regular file",
            ));
        }
        fs::remove_file(entry.path())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };

    use myalbuns_paths::{AppPaths, ProcessInstanceId};

    use super::{
        CACHE_WRITER_CLAIM_SCHEMA_VERSION, CacheWriterClaim, ProcessorChildLifetime,
        await_cache_writer_quiescence, cache_writer_claim_path,
    };

    const HOST_READY_ENV: &str = "MYALBUNS_PROCESSOR_LIFETIME_HOST_READY";
    const WORKER_SPAWNED_ENV: &str = "MYALBUNS_PROCESSOR_LIFETIME_WORKER_SPAWNED";
    const WORKER_ACTIVE_ENV: &str = "MYALBUNS_PROCESSOR_LIFETIME_WORKER_ACTIVE";
    const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;

    fn wait_for_file(path: &std::path::Path, child: &mut std::process::Child, label: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !path.is_file() {
            assert!(
                child
                    .try_wait()
                    .expect("the child process state is readable")
                    .is_none(),
                "{label} exited before publishing readiness"
            );
            assert!(Instant::now() < deadline, "{label} readiness timed out");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn cache_fixture(root: &std::path::Path) -> myalbuns_paths::CachePathPlan {
        let roaming = root.join("roaming");
        let local = root.join("local");
        std::fs::create_dir_all(&roaming).expect("the roaming fixture root exists");
        std::fs::create_dir_all(&local).expect("the local fixture root exists");
        let app_paths = AppPaths::from_roots(&roaming, &local);
        let paths = app_paths
            .project_cache("processor-claim-fixture")
            .expect("the Cache namespace is valid");
        drop(
            app_paths
                .prepare_cache_storage(&paths)
                .expect("the Cache storage is prepared"),
        );
        paths
    }

    #[test]
    fn recycled_pid_claim_is_removed_only_after_exact_instance_mismatch() {
        let root = tempfile::tempdir().expect("the stale claim fixture exists");
        let paths = cache_fixture(root.path());
        let current = ProcessInstanceId::current().expect("the test process has an identity");
        let stale = ProcessInstanceId::from_wire(
            current.process_id(),
            current
                .creation_time_wire()
                .checked_add(1)
                .expect("the FILETIME is not maximal"),
        )
        .expect("the mismatched process identity is structurally valid");
        let claim_path = cache_writer_claim_path(&paths);
        std::fs::write(
            &claim_path,
            serde_json::to_vec(&CacheWriterClaim {
                schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
                process: stale,
            })
            .expect("the stale claim serializes"),
        )
        .expect("the stale claim is writable");

        await_cache_writer_quiescence(&paths)
            .expect("a PID reused by another exact instance is already quiescent");

        assert!(!claim_path.exists());
    }

    #[test]
    fn corrupted_cache_writer_claim_fails_closed_and_is_preserved() {
        let root = tempfile::tempdir().expect("the corrupt claim fixture exists");
        let paths = cache_fixture(root.path());
        let claim_path = cache_writer_claim_path(&paths);
        std::fs::write(&claim_path, b"{").expect("the corrupt claim is writable");

        let error = await_cache_writer_quiescence(&paths)
            .expect_err("an unprovable writer identity must block namespace recovery");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(claim_path.is_file());
    }

    #[test]
    fn terminating_the_host_closes_its_job_and_terminates_the_active_processor() {
        let root = tempfile::tempdir().expect("the processor lifetime fixture exists");
        let host_ready = root.path().join("host.ready");
        let mut host = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("processor_lifetime::tests::processor_lifetime_host_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(HOST_READY_ENV, &host_ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the independent Host process starts");
        wait_for_file(&host_ready, &mut host, "the Host");
        let processor_id = std::fs::read_to_string(&host_ready)
            .expect("the Host readiness is readable")
            .trim()
            .parse::<u32>()
            .expect("the Host reports a Processor PID");

        // SAFETY: the PID came from the ready Host and the returned handle is
        // closed below after the wait assertions.
        let processor = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, processor_id) };
        assert!(!processor.is_null(), "the active Processor can be observed");
        // SAFETY: processor is a live synchronization handle.
        assert_eq!(unsafe { WaitForSingleObject(processor, 0) }, WAIT_TIMEOUT);

        host.kill().expect("the Host is terminated abruptly");
        host.wait().expect("the terminated Host is reaped");
        // SAFETY: closing the Host-owned Job must signal the Processor handle.
        assert_eq!(
            unsafe { WaitForSingleObject(processor, 10_000) },
            WAIT_OBJECT_0,
            "the Processor cannot outlive the Host that owned its Cache namespace"
        );
        // SAFETY: processor is owned by this test and no longer used.
        unsafe { CloseHandle(processor) };
    }

    #[test]
    #[ignore = "spawned by the real Host-death processor lifetime test"]
    fn processor_lifetime_host_process() {
        let host_ready = env::var_os(HOST_READY_ENV).expect("the Host ready path is configured");
        let root = std::path::PathBuf::from(&host_ready)
            .parent()
            .expect("the ready path has a parent")
            .to_path_buf();
        let worker_spawned = root.join("worker.spawned");
        let worker_active = root.join("worker.active");
        let mut worker = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("processor_lifetime::tests::processor_lifetime_worker_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(WORKER_SPAWNED_ENV, &worker_spawned)
            .env(WORKER_ACTIVE_ENV, &worker_active)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the Processor fixture starts");
        wait_for_file(&worker_spawned, &mut worker, "the Processor");
        let _lifetime = ProcessorChildLifetime::attach(worker.id())
            .expect("the Host contains its Processor before dispatch");
        worker
            .stdin
            .as_mut()
            .expect("the Processor stdin is available")
            .write_all(b"dispatch\n")
            .expect("the Host dispatches work after containment");
        wait_for_file(&worker_active, &mut worker, "the active Processor");
        std::fs::write(host_ready, worker.id().to_string())
            .expect("the Host publishes its contained Processor PID");
        thread::sleep(Duration::from_secs(120));
    }

    #[test]
    #[ignore = "spawned by the real Host-death processor lifetime test"]
    fn processor_lifetime_worker_process() {
        let spawned =
            env::var_os(WORKER_SPAWNED_ENV).expect("the Processor spawned path is configured");
        let active =
            env::var_os(WORKER_ACTIVE_ENV).expect("the Processor active path is configured");
        std::fs::write(spawned, b"spawned").expect("the Processor signals its blocked state");
        let mut dispatch = String::new();
        BufReader::new(std::io::stdin())
            .read_line(&mut dispatch)
            .expect("the Processor receives dispatch");
        assert_eq!(dispatch, "dispatch\n");
        std::fs::write(active, b"active").expect("the Processor signals active work");
        thread::sleep(Duration::from_secs(120));
    }
}
