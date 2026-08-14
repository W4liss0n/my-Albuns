use std::io;

use windows_sys::Win32::{
    Foundation::{FILETIME, HANDLE},
    System::Threading::GetProcessTimes,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct HostProcessInstanceId {
    process_id: u32,
    creation_time: ProcessCreationTime,
}

impl HostProcessInstanceId {
    pub(crate) fn from_process_handle(process_id: u32, process: HANDLE) -> io::Result<Self> {
        let creation_time = query_creation_time(process)?;
        Self::from_wire(process_id, creation_time.to_wire())
            .ok_or_else(|| io::Error::other("the captured Host process instance is invalid"))
    }

    pub(crate) fn from_wire(process_id: u32, creation_time: u64) -> Option<Self> {
        (process_id != 0)
            .then(|| ProcessCreationTime::from_wire(creation_time))
            .flatten()
            .map(|creation_time| Self {
                process_id,
                creation_time,
            })
    }

    pub(crate) fn process_id(self) -> u32 {
        self.process_id
    }

    pub(crate) fn creation_time_wire(self) -> u64 {
        self.creation_time.to_wire()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ProcessCreationTime(u64);

impl ProcessCreationTime {
    fn from_wire(value: u64) -> Option<Self> {
        (value != 0).then_some(Self(value))
    }

    fn to_wire(self) -> u64 {
        self.0
    }
}

fn query_creation_time(process: HANDLE) -> io::Result<ProcessCreationTime> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all output pointers address initialized FILETIME values for the
    // duration of the call; callers provide a live process handle with query
    // rights (the CreateProcess child handle has full access).
    let succeeded =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    let value = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    ProcessCreationTime::from_wire(value)
        .ok_or_else(|| io::Error::other("the Host process creation time is invalid"))
}
