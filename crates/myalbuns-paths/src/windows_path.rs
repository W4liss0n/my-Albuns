#[cfg(windows)]
use std::{
    ffi::{OsString, c_void},
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    slice,
};

#[cfg(windows)]
use crate::{AppPathsError, PathRootKind};

/// Encodes an absolute disk/UNC pathname for Win32 file APIs without the
/// legacy `MAX_PATH` interpretation. Logical and persisted spellings remain
/// unchanged; this representation exists only at the native call boundary.
#[cfg(windows)]
pub fn wide_api_path(path: &Path) -> Vec<u16> {
    use std::path::{Component, Prefix};

    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let prefix = match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => return units.into_iter().chain(Some(0)).collect(),
    };
    let mut result = match prefix {
        Prefix::Disk(_) => r"\\?\".encode_utf16().collect::<Vec<_>>(),
        Prefix::UNC(_, _) => {
            let mut value = r"\\?\UNC\".encode_utf16().collect::<Vec<_>>();
            value.extend(
                units
                    .iter()
                    .skip(2)
                    .copied()
                    .map(normalize_windows_separator),
            );
            value.push(0);
            return value;
        }
        Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(_, _) => {
            return units.into_iter().chain(Some(0)).collect();
        }
        Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
            return units.into_iter().chain(Some(0)).collect();
        }
    };
    result.extend(units.into_iter().map(normalize_windows_separator));
    result.push(0);
    result
}

#[cfg(windows)]
fn normalize_windows_separator(unit: u16) -> u16 {
    if unit == u16::from(b'/') {
        u16::from(b'\\')
    } else {
        unit
    }
}

#[cfg(windows)]
pub(crate) fn current_operational_base(
    logical_root: &Path,
    kind: PathRootKind,
) -> Result<Option<PathBuf>, AppPathsError> {
    use windows_sys::Win32::{
        Foundation::{
            ERROR_ACCESS_DENIED, ERROR_BAD_NET_NAME, ERROR_BAD_NETPATH, ERROR_CONNECTION_UNAVAIL,
            ERROR_MORE_DATA, ERROR_NETWORK_UNREACHABLE, ERROR_NO_NET_OR_BAD_PATH, ERROR_NO_NETWORK,
            ERROR_NOT_CONNECTED, NO_ERROR,
        },
        NetworkManagement::WNet::{
            UNIVERSAL_NAME_INFO_LEVEL, UNIVERSAL_NAME_INFOW, WNetGetUniversalNameW,
        },
    };

    let query_root = match kind {
        PathRootKind::Disk => logical_root.to_path_buf(),
        PathRootKind::VerbatimDisk => verbatim_disk_as_dos_root(logical_root)?,
        _ => return Ok(None),
    };
    let mut local_path = query_root.as_os_str().encode_wide().collect::<Vec<_>>();
    local_path.push(0);
    let mut byte_count = 1024_u32;
    loop {
        let word_count = (byte_count as usize).div_ceil(size_of::<usize>());
        let mut aligned_buffer = vec![0_usize; word_count];
        let mut available_bytes = (aligned_buffer.len() * size_of::<usize>()) as u32;
        let status = unsafe {
            WNetGetUniversalNameW(
                local_path.as_ptr(),
                UNIVERSAL_NAME_INFO_LEVEL,
                aligned_buffer.as_mut_ptr().cast::<c_void>(),
                &mut available_bytes,
            )
        };
        if status == ERROR_MORE_DATA {
            if available_bytes <= byte_count {
                return Err(AppPathsError::OperationPathIoFailure);
            }
            byte_count = available_bytes;
            continue;
        }
        if status == ERROR_NOT_CONNECTED {
            return Ok(None);
        }
        if status == ERROR_ACCESS_DENIED {
            return Err(AppPathsError::OperationPathAccessDenied);
        }
        if matches!(
            status,
            ERROR_BAD_NET_NAME
                | ERROR_BAD_NETPATH
                | ERROR_CONNECTION_UNAVAIL
                | ERROR_NETWORK_UNREACHABLE
                | ERROR_NO_NETWORK
                | ERROR_NO_NET_OR_BAD_PATH
        ) {
            return Err(AppPathsError::OperationPathUnavailable);
        }
        if status != NO_ERROR {
            return Err(AppPathsError::OperationPathIoFailure);
        }

        let buffer_start = aligned_buffer.as_ptr() as usize;
        let buffer_end = buffer_start + aligned_buffer.len() * size_of::<usize>();
        let info = unsafe { &*aligned_buffer.as_ptr().cast::<UNIVERSAL_NAME_INFOW>() };
        let path_start = info.lpUniversalName as usize;
        if path_start < buffer_start
            || path_start >= buffer_end
            || !(path_start - buffer_start).is_multiple_of(size_of::<u16>())
        {
            return Err(AppPathsError::OperationPathIoFailure);
        }
        let maximum_units = (buffer_end - path_start) / size_of::<u16>();
        let units =
            unsafe { slice::from_raw_parts(info.lpUniversalName.cast_const(), maximum_units) };
        let Some(length) = units.iter().position(|unit| *unit == 0) else {
            return Err(AppPathsError::OperationPathIoFailure);
        };
        return Ok(Some(PathBuf::from(OsString::from_wide(&units[..length]))));
    }
}

#[cfg(windows)]
fn verbatim_disk_as_dos_root(path: &Path) -> Result<PathBuf, AppPathsError> {
    use std::path::{Component, Prefix};

    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err(AppPathsError::InvalidOperationPath);
    };
    let Prefix::VerbatimDisk(letter) = prefix.kind() else {
        return Err(AppPathsError::InvalidOperationPath);
    };
    let units = [u16::from(letter), u16::from(b':'), u16::from(b'\\')];
    Ok(PathBuf::from(OsString::from_wide(&units)))
}
