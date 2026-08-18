#[cfg(windows)]
use std::io;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{FILETIME, HANDLE},
    System::Threading::{GetCurrentProcess, GetProcessTimes},
};

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
