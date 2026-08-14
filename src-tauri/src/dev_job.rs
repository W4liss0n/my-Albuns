use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};

use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

pub(crate) fn create_job_with_limits(limit_flags: u32) -> io::Result<OwnedHandle> {
    // SAFETY: null name/security creates an unnamed Job. The successful raw
    // handle is immediately owned, and limits remains valid for the call.
    unsafe {
        let handle = CreateJobObjectW(ptr::null(), ptr::null());
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = OwnedHandle::from_raw_handle(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = limit_flags;
        if SetInformationJobObject(
            job.as_raw_handle().cast::<c_void>(),
            JobObjectExtendedLimitInformation,
            ptr::from_ref(&limits).cast_mut().cast::<c_void>(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::c_void, os::windows::io::AsRawHandle, ptr};

    use windows_sys::Win32::System::JobObjects::{
        JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        QueryInformationJobObject,
    };

    use super::create_job_with_limits;

    #[test]
    fn creates_jobs_with_the_callers_exact_limit_policy() {
        for expected in [
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        ] {
            let job = create_job_with_limits(expected).expect("configured Job Object");
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            // SAFETY: job is an owned Job handle, limits is initialized writable
            // storage, and no return-length output is required for this query.
            let queried = unsafe {
                QueryInformationJobObject(
                    job.as_raw_handle().cast::<c_void>(),
                    JobObjectExtendedLimitInformation,
                    ptr::from_mut(&mut limits).cast::<c_void>(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    ptr::null_mut(),
                )
            };
            assert_ne!(queried, 0, "the configured Job limits can be queried");
            assert_eq!(limits.BasicLimitInformation.LimitFlags, expected);
        }
    }
}
