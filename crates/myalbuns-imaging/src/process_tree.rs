#[cfg(windows)]
pub(crate) fn install_kill_on_parent_exit() -> Result<(), String> {
    use std::{ffi::c_void, io, mem::size_of, ptr};

    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::GetCurrentProcess,
        },
    };

    // SAFETY: every pointer passed to Win32 is either null or points to a live,
    // correctly sized structure for the duration of the call.
    unsafe {
        let job = CreateJobObjectW(ptr::null(), ptr::null());
        if job.is_null() {
            return Err(format!(
                "não foi possível criar o Job Object do Processador: {}",
                io::Error::last_os_error()
            ));
        }

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            ptr::from_ref(&limits).cast::<c_void>(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(format!(
                "não foi possível configurar o Job Object do Processador: {error}"
            ));
        }

        if AssignProcessToJobObject(job, GetCurrentProcess()) == 0 {
            let error = io::Error::last_os_error();
            CloseHandle(job);
            return Err(format!(
                "não foi possível associar o Processador ao Job Object: {error}"
            ));
        }

        // The sidecar intentionally owns this sole non-inheritable handle until
        // process teardown. Windows then closes it and atomically terminates every
        // descendant that inherited membership in the job.
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn install_kill_on_parent_exit() -> Result<(), String> {
    Ok(())
}
