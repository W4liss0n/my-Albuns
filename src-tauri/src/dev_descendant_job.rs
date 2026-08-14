use std::{
    ffi::c_void,
    io,
    os::windows::io::{AsRawHandle, OwnedHandle},
    sync::OnceLock,
};

use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
    Threading::GetCurrentProcess,
};

use crate::{dev_job::create_job_with_limits, dev_supervisor_protocol::HOST_LEASE_ENDPOINT_ENV};

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
            "the gate probe rejected descendant containment",
        ));
    }
    match PROCESS_DESCENDANT_JOB.get_or_init(install_process_descendant_job) {
        Ok(_) => Ok(()),
        Err(error) => Err(io::Error::other(error.clone())),
    }
}

fn install_process_descendant_job() -> Result<OwnedHandle, String> {
    let job =
        create_job_with_limits(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK)
            .map_err(|error| error.to_string())?;
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
