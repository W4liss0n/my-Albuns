#[cfg(windows)]
use std::io;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
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
pub struct ProcessInstanceHandle(HANDLE);

#[cfg(windows)]
impl ProcessInstanceHandle {
    /// Opens and validates the expected process instance.
    ///
    /// Query and synchronization rights are always requested because exact
    /// identity and liveness are part of this constructor's contract. Callers
    /// may add the native rights required by their concrete operation.
    pub fn open(expected: ProcessInstanceId, additional_access: u32) -> io::Result<Self> {
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
            return Err(io::Error::last_os_error());
        }
        let process = Self(handle);
        let observed = ProcessInstanceId::from_process_handle(expected.process_id(), process.0)?;
        if observed != expected {
            return Err(io::Error::other(
                "the PID belongs to another process instance",
            ));
        }
        if !process.is_running()? {
            return Err(io::Error::other(
                "the process instance exited before handle validation",
            ));
        }
        Ok(process)
    }

    pub fn as_raw_handle(&self) -> HANDLE {
        self.0
    }

    pub fn is_running(&self) -> io::Result<bool> {
        // SAFETY: self owns a process handle with synchronization rights.
        match unsafe { WaitForSingleObject(self.0, 0) } {
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
        match unsafe { WaitForSingleObject(self.0, INFINITE) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            wait => Err(io::Error::other(format!(
                "unexpected result while waiting for a process instance: {wait}"
            ))),
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessInstanceHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: this guard exclusively owns the process handle.
            unsafe { CloseHandle(self.0) };
            self.0 = std::ptr::null_mut();
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
