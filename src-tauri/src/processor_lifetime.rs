use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    time::Duration,
};

use myalbuns_paths::{
    AppPaths, AppPathsError, CachePathPlan, CacheWriterClaimStorage, ProcessInstanceHandle,
    ProcessInstanceId,
};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    Threading::{PROCESS_SET_QUOTA, PROCESS_TERMINATE, WaitForSingleObject},
};

#[cfg(test)]
use std::path::PathBuf;

const CACHE_WRITER_CLAIM_SCHEMA_VERSION: u32 = 1;
#[cfg(test)]
const CACHE_WRITER_CLAIM_FILE: &str = ".processor-writer.v1.json";
#[cfg(test)]
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
    process: ProcessInstanceHandle,
    process_instance: ProcessInstanceId,
    claim_storage: Option<CacheWriterClaimStorage>,
}

/// Pins the validated Cache namespace and the exact claimed Process instance
/// before any potentially blocking wait begins.
///
/// Keeping both handles in one value makes the synchronization boundary
/// explicit: a successor can only wait and clean through the physical
/// namespace that it prepared before a pathname replacement.
struct PreparedCacheWriterQuiescence {
    storage: CacheWriterClaimStorage,
    encoded_claim: Vec<u8>,
    process: Option<ProcessInstanceHandle>,
}

impl PreparedCacheWriterQuiescence {
    fn finish(self) -> io::Result<()> {
        let Self {
            storage,
            encoded_claim,
            process,
        } = self;
        if let Some(process) = process
            && !process.wait_for_exit_timeout(CACHE_WRITER_WAIT_TIMEOUT)?
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the previous Cache writer did not terminate in time",
            ));
        }
        if !storage
            .remove_claim_if_matches(&encoded_claim)
            .map_err(cache_storage_error)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the Cache writer claim changed during synchronization",
            ));
        }
        storage
            .discard_claim_temporaries()
            .map_err(cache_storage_error)?;
        Ok(())
    }
}

impl ProcessorChildLifetime {
    pub(crate) fn attach(process_instance: ProcessInstanceId) -> io::Result<Self> {
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

        // The expected PID + creation time came through the newly spawned
        // child's stdout pipe. Reopening validates that exact instance before
        // Job authority is acquired; a recycled PID therefore cannot assign
        // or terminate the process that happens to occupy the numeric ID.
        let process =
            ProcessInstanceHandle::open(process_instance, PROCESS_SET_QUOTA | PROCESS_TERMINATE)?;
        // SAFETY: both handles are live, owned handles of the required kinds.
        // Windows 8+ forms a nested Job when an outer launcher Job is present.
        if unsafe {
            AssignProcessToJobObject(
                job.as_raw_handle().cast::<c_void>(),
                process.as_raw_handle(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            job: Some(job),
            process,
            process_instance,
            claim_storage: None,
        })
    }

    /// Publishes the exact contained Processor instance before the Host sends
    /// a Cache command. A replacement Host can then wait for the instance's
    /// signaled exit before inspecting or deleting the namespace.
    pub(crate) fn publish_cache_writer_claim(
        &mut self,
        app_paths: &AppPaths,
        paths: &CachePathPlan,
    ) -> io::Result<()> {
        if self.claim_storage.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the Processor already owns a Cache writer claim",
            ));
        }
        let storage = app_paths
            .open_cache_writer_claim_storage(paths)
            .map_err(cache_storage_error)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the Cache writer claim namespace does not exist",
                )
            })?;
        let claim = CacheWriterClaim {
            schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
            process: self.process_instance,
        };
        let encoded = serde_json::to_vec(&claim)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        storage
            .publish_claim(&encoded)
            .map_err(cache_storage_error)?;
        self.claim_storage = Some(storage);
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
        if unsafe { WaitForSingleObject(self.process.as_raw_handle(), 0) } != WAIT_OBJECT_0 {
            return;
        }
        let Some(claim_storage) = self.claim_storage.take() else {
            return;
        };
        let expected = CacheWriterClaim {
            schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
            process: self.process_instance,
        };
        if let Ok(encoded) = serde_json::to_vec(&expected) {
            let _ = claim_storage.remove_claim_if_matches(&encoded);
        }
    }
}

/// Waits for a writer claim left by a dead Host before any authoritative
/// namespace inspection, recovery, or deletion begins.
pub(crate) fn await_cache_writer_quiescence(
    app_paths: &AppPaths,
    paths: &CachePathPlan,
) -> io::Result<()> {
    let Some(prepared) = prepare_cache_writer_quiescence(app_paths, paths)? else {
        return Ok(());
    };
    prepared.finish()
}

fn prepare_cache_writer_quiescence(
    app_paths: &AppPaths,
    paths: &CachePathPlan,
) -> io::Result<Option<PreparedCacheWriterQuiescence>> {
    let Some(storage) = app_paths
        .open_cache_writer_claim_storage(paths)
        .map_err(cache_storage_error)?
    else {
        return Ok(None);
    };
    let encoded = match storage.read_claim().map_err(cache_storage_error)? {
        Some(encoded) => encoded,
        None => {
            storage
                .discard_claim_temporaries()
                .map_err(cache_storage_error)?;
            return Ok(None);
        }
    };
    let claim: CacheWriterClaim = serde_json::from_slice(&encoded)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if claim.schema_version != CACHE_WRITER_CLAIM_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the Cache writer claim schema is incompatible",
        ));
    }
    let process = ProcessInstanceHandle::open_if_running(claim.process, 0)?;
    Ok(Some(PreparedCacheWriterQuiescence {
        storage,
        encoded_claim: encoded,
        process,
    }))
}

#[cfg(test)]
fn cache_writer_claim_path(paths: &CachePathPlan) -> PathBuf {
    paths.root().join(CACHE_WRITER_CLAIM_FILE)
}

fn cache_storage_error(error: AppPathsError) -> io::Error {
    io::Error::other(error)
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::c_void,
        io::{BufRead, BufReader, Write},
        os::windows::io::AsRawHandle,
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
        CACHE_WRITER_CLAIM_FILE, CACHE_WRITER_CLAIM_SCHEMA_VERSION, CACHE_WRITER_TEMPORARY_PREFIX,
        CacheWriterClaim, ProcessorChildLifetime, await_cache_writer_quiescence,
        cache_writer_claim_path, prepare_cache_writer_quiescence,
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

    fn cache_fixture(root: &std::path::Path) -> (AppPaths, myalbuns_paths::CachePathPlan) {
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
        (app_paths, paths)
    }

    fn child_identity(child: &std::process::Child) -> ProcessInstanceId {
        ProcessInstanceId::from_process_handle(child.id(), child.as_raw_handle().cast::<c_void>())
            .expect("the exact child identity is observable through its causal handle")
    }

    #[cfg(windows)]
    fn create_directory_junction(source: &std::path::Path, destination: &std::path::Path) {
        let output = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(destination)
            .arg(source)
            .output()
            .expect("the Windows junction command starts");
        assert!(
            output.status.success(),
            "the Windows junction is created: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn attach_rejects_a_recycled_pid_identity_without_containing_or_killing_the_observed_process() {
        let root = tempfile::tempdir().expect("the worker fixture exists");
        let mut worker = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("processor_lifetime::tests::processor_lifetime_worker_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(WORKER_SPAWNED_ENV, root.path().join("spawned"))
            .env(WORKER_ACTIVE_ENV, root.path().join("active"))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the unrelated observed process starts");
        let observed = child_identity(&worker);
        let recycled = ProcessInstanceId::from_wire(
            observed.process_id(),
            observed
                .creation_time_wire()
                .checked_add(1)
                .expect("the creation FILETIME is not maximal"),
        )
        .expect("the recycled-PID fixture is structurally valid");

        ProcessorChildLifetime::attach(recycled)
            .expect_err("a divergent creation time cannot acquire Job authority");
        assert!(
            worker
                .try_wait()
                .expect("the worker state is readable")
                .is_none(),
            "the process currently occupying the PID was neither contained nor killed"
        );
        worker
            .kill()
            .expect("the unrelated process is stopped by its test owner");
        worker.wait().expect("the unrelated process is reaped");
    }

    #[test]
    fn recycled_pid_claim_is_removed_only_after_exact_instance_mismatch() {
        let root = tempfile::tempdir().expect("the stale claim fixture exists");
        let (app_paths, paths) = cache_fixture(root.path());
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

        await_cache_writer_quiescence(&app_paths, &paths)
            .expect("a PID reused by another exact instance is already quiescent");

        assert!(!claim_path.exists());
    }

    #[test]
    fn corrupted_cache_writer_claim_fails_closed_and_is_preserved() {
        let root = tempfile::tempdir().expect("the corrupt claim fixture exists");
        let (app_paths, paths) = cache_fixture(root.path());
        let claim_path = cache_writer_claim_path(&paths);
        std::fs::write(&claim_path, b"{").expect("the corrupt claim is writable");

        let error = await_cache_writer_quiescence(&app_paths, &paths)
            .expect_err("an unprovable writer identity must block namespace recovery");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(claim_path.is_file());
    }

    #[cfg(windows)]
    #[test]
    fn writer_wait_rejects_namespace_link_replacement_and_preserves_external_claim_files() {
        let root = tempfile::tempdir().expect("the guarded claim fixture exists");
        let (app_paths, paths) = cache_fixture(root.path());
        let claim_path = cache_writer_claim_path(&paths);
        let worker_spawned = root.path().join("guarded-worker.spawned");
        let worker_active = root.path().join("guarded-worker.active");
        let mut worker = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("processor_lifetime::tests::processor_lifetime_worker_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(WORKER_SPAWNED_ENV, &worker_spawned)
            .env(WORKER_ACTIVE_ENV, &worker_active)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the exact writer fixture starts");
        wait_for_file(&worker_spawned, &mut worker, "the exact writer");
        let claim = CacheWriterClaim {
            schema_version: CACHE_WRITER_CLAIM_SCHEMA_VERSION,
            process: child_identity(&worker),
        };
        let encoded = serde_json::to_vec(&claim).expect("the writer claim serializes");
        std::fs::write(&claim_path, &encoded).expect("the writer claim is published");

        let external = tempfile::tempdir().expect("the external target exists");
        let external_claim = external.path().join(CACHE_WRITER_CLAIM_FILE);
        let external_temporary = external
            .path()
            .join(format!("{CACHE_WRITER_TEMPORARY_PREFIX}external.tmp"));
        std::fs::write(&external_claim, &encoded).expect("the external claim sentinel exists");
        std::fs::write(&external_temporary, b"external temporary sentinel")
            .expect("the external temporary sentinel exists");

        let prepared = prepare_cache_writer_quiescence(&app_paths, &paths)
            .expect("the guarded writer wait is prepared")
            .expect("the live exact writer requires synchronization");
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let synchronization = thread::spawn(move || {
            started_sender
                .send(())
                .expect("the prepared waiter start is observed");
            prepared.finish()
        });
        started_receiver
            .recv()
            .expect("the prepared writer waiter has started");
        assert!(
            worker
                .try_wait()
                .expect("the exact writer state is readable")
                .is_none(),
            "the exact writer remains alive before the namespace replacement"
        );
        assert!(
            !synchronization.is_finished(),
            "the prepared waiter remains blocked on the exact live writer"
        );

        let displaced = paths.root().with_file_name("processor-claim-displaced");
        std::fs::rename(paths.root(), &displaced)
            .expect("the logical namespace can move during the writer wait");
        create_directory_junction(external.path(), paths.root());

        worker.kill().expect("the exact writer is released");
        worker.wait().expect("the exact writer is reaped");
        let synchronization_error = synchronization
            .join()
            .expect("the guarded synchronization does not panic")
            .expect_err("the guarded writer wait rejects the redirected namespace");
        std::fs::remove_dir(paths.root()).expect("the injected junction is removed");
        std::fs::rename(&displaced, paths.root()).expect("the original namespace is restored");

        assert_eq!(synchronization_error.kind(), std::io::ErrorKind::Other);
        assert_eq!(
            std::fs::read(&external_claim).expect("the external claim survives"),
            encoded
        );
        assert_eq!(
            std::fs::read(&external_temporary).expect("the external temporary survives"),
            b"external temporary sentinel"
        );
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
        let _lifetime = ProcessorChildLifetime::attach(child_identity(&worker))
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
