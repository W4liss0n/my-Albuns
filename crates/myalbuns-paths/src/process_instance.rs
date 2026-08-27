#[cfg(windows)]
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{
        ERROR_INVALID_PARAMETER, FILETIME, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::Threading::{
        GetCurrentProcess, GetProcessTimes, INFINITE, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
    },
};

#[cfg(windows)]
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;

/// Stable identity of one operating-system process instance.
///
/// A process ID alone can be reused. The creation FILETIME keeps authorities
/// bound to the exact instance that originally published them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInstanceId {
    process_id: u32,
    creation_time: u64,
}

impl ProcessInstanceId {
    pub fn from_wire(process_id: u32, creation_time: u64) -> Option<Self> {
        (process_id != 0 && creation_time != 0).then_some(Self {
            process_id,
            creation_time,
        })
    }

    #[cfg(windows)]
    pub fn current() -> io::Result<Self> {
        Self::from_process_handle(std::process::id(), unsafe { GetCurrentProcess() })
    }

    #[cfg(windows)]
    pub fn from_process_handle(process_id: u32, process: HANDLE) -> io::Result<Self> {
        let creation_time = query_creation_time(process)?;
        Self::from_wire(process_id, creation_time)
            .ok_or_else(|| io::Error::other("the captured process instance is invalid"))
    }

    pub fn process_id(self) -> u32 {
        self.process_id
    }

    pub fn creation_time_wire(self) -> u64 {
        self.creation_time
    }
}

impl<'de> Deserialize<'de> for ProcessInstanceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct WireProcessInstanceId {
            process_id: u32,
            creation_time: u64,
        }

        let wire = WireProcessInstanceId::deserialize(deserializer)?;
        Self::from_wire(wire.process_id, wire.creation_time)
            .ok_or_else(|| D::Error::custom("the process instance identity is invalid"))
    }
}

/// Owned Windows handle proven to belong to one exact, live process instance.
///
/// The handle keeps the process object alive, so its PID cannot be reassigned
/// while callers inspect windows or coordinate lifecycle through this guard.
#[cfg(windows)]
#[derive(Debug)]
pub struct ProcessInstanceHandle(OwnedHandle);

#[cfg(windows)]
impl ProcessInstanceHandle {
    /// Opens and validates the expected process instance.
    ///
    /// Query and synchronization rights are always requested because exact
    /// identity and liveness are part of this constructor's contract. Callers
    /// may add the native rights required by their concrete operation.
    pub fn open(expected: ProcessInstanceId, additional_access: u32) -> io::Result<Self> {
        Self::open_if_running(expected, additional_access)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "the expected process instance is no longer running",
            )
        })
    }

    /// Opens the exact process instance when it is still running.
    ///
    /// `Ok(None)` is returned only after proving that the PID is absent, now
    /// belongs to another creation time, or already reached its signaled exit
    /// state. Access and query failures remain errors so lifecycle consumers
    /// fail closed instead of mistaking an unobservable process for a dead one.
    pub fn open_if_running(
        expected: ProcessInstanceId,
        additional_access: u32,
    ) -> io::Result<Option<Self>> {
        // SAFETY: ProcessInstanceId guarantees a non-zero PID. The returned
        // non-inheritable handle is immediately wrapped by this owned guard.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | additional_access,
                0,
                expected.process_id(),
            )
        };
        if handle.is_null() {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        // SAFETY: OpenProcess returned a new owned handle which is transferred
        // exactly once to OwnedHandle. Windows kernel handles are process-wide,
        // so this guard can safely move with async work between Host threads.
        let process = Self(unsafe { OwnedHandle::from_raw_handle(handle) });
        let observed =
            ProcessInstanceId::from_process_handle(expected.process_id(), process.as_raw_handle())?;
        if observed != expected {
            return Ok(None);
        }
        if !process.is_running()? {
            return Ok(None);
        }
        Ok(Some(process))
    }

    pub fn as_raw_handle(&self) -> HANDLE {
        self.0.as_raw_handle().cast()
    }

    pub fn is_running(&self) -> io::Result<bool> {
        // SAFETY: self owns a process handle with synchronization rights.
        match unsafe { WaitForSingleObject(self.as_raw_handle(), 0) } {
            WAIT_TIMEOUT => Ok(true),
            WAIT_OBJECT_0 => Ok(false),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            wait => Err(io::Error::other(format!(
                "unexpected result while checking a process instance: {wait}"
            ))),
        }
    }

    pub fn wait_for_exit(&self) -> io::Result<()> {
        // SAFETY: self owns a process handle with synchronization rights.
        match unsafe { WaitForSingleObject(self.as_raw_handle(), INFINITE) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            wait => Err(io::Error::other(format!(
                "unexpected result while waiting for a process instance: {wait}"
            ))),
        }
    }

    /// Waits for a bounded interval and reports whether the exact process
    /// instance reached its terminal signaled state.
    pub fn wait_for_exit_timeout(&self, timeout: Duration) -> io::Result<bool> {
        let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX - 1);
        // SAFETY: self owns a process handle with synchronization rights.
        match unsafe { WaitForSingleObject(self.as_raw_handle(), milliseconds) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            wait => Err(io::Error::other(format!(
                "unexpected result while waiting for a process instance: {wait}"
            ))),
        }
    }
}

#[cfg(windows)]
fn query_creation_time(process: HANDLE) -> io::Result<u64> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all output pointers address initialized FILETIME values for the
    // duration of the call. Callers provide a process handle with query rights.
    let succeeded =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    let value = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    (value != 0)
        .then_some(value)
        .ok_or_else(|| io::Error::other("the process creation time is invalid"))
}
