use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};

use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
};

/// Owns the Windows Job that contains one dispatched Imaging process.
///
/// The Job handle is created and retained by the Project Host before a request
/// is written to the child. If the Host terminates, Windows closes its handle
/// and terminates the contained process before that process can keep writing to
/// a Cache namespace whose Host reservation has already been released.
#[derive(Debug)]
pub(crate) struct ProcessorChildLifetime {
    _job: OwnedHandle,
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
            let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id);
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            OwnedHandle::from_raw_handle(handle)
        };
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
        Ok(Self { _job: job })
    }
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

    use super::ProcessorChildLifetime;

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
