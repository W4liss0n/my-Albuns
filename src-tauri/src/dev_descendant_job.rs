use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    sync::OnceLock,
};

use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    },
    Threading::GetCurrentProcess,
};

use crate::dev_supervisor_protocol::HOST_LEASE_ENDPOINT_ENV;

static PROCESS_DESCENDANT_JOB: OnceLock<Result<OwnedHandle, String>> = OnceLock::new();
const DESCENDANT_JOB_FAILURE_PROBE_ENV: &str = "MYALBUNS_DEV_DESCENDANT_JOB_FAILURE_PROBE";

/// Installs a process-local nested Job before Tauri creates either WebView.
///
/// The supervisor owns the outer development Job. WebView2 can use a brokered
/// process launch, so each supervised desktop process also establishes an
/// immediate Job. Its explicit breakaway permission is consumed only by the
/// Global when it spawns a Host; WebView2 does not request breakaway. Windows
/// closes the retained handle when the desktop process exits and
/// deterministically terminates that WebView tree.
pub(crate) fn install_if_supervised() -> io::Result<()> {
    if std::env::var_os(HOST_LEASE_ENDPOINT_ENV).is_none() {
        return Ok(());
    }
    if std::env::var(DESCENDANT_JOB_FAILURE_PROBE_ENV).as_deref() == Ok("1") {
        return Err(io::Error::other(
            "a sonda do gate recusou a contenção de descendentes",
        ));
    }
    match PROCESS_DESCENDANT_JOB.get_or_init(install_process_descendant_job) {
        Ok(_) => Ok(()),
        Err(error) => Err(io::Error::other(error.clone())),
    }
}

fn install_process_descendant_job() -> Result<OwnedHandle, String> {
    let job = create_kill_on_close_job().map_err(|error| error.to_string())?;
    // SAFETY: GetCurrentProcess returns the pseudo-handle for this live
    // process; job is an owned handle with ASSIGN_PROCESS access.
    if unsafe {
        AssignProcessToJobObject(job.as_raw_handle().cast::<c_void>(), GetCurrentProcess())
    } == 0
    {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(job)
}

fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    // SAFETY: null name/security creates an unnamed Job; limits remains valid
    // for SetInformationJobObject, and a successful raw handle is owned here.
    unsafe {
        let handle = CreateJobObjectW(ptr::null(), ptr::null());
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = OwnedHandle::from_raw_handle(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
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
