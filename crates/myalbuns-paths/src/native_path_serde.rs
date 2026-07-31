use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(windows)]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "encoding")]
enum NativePathWire {
    WindowsUtf16 { units: Vec<u16> },
}

#[cfg(unix)]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "encoding")]
enum NativePathWire {
    UnixBytes { bytes: Vec<u8> },
}

pub(crate) fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        NativePathWire::WindowsUtf16 {
            units: path.as_os_str().encode_wide().collect(),
        }
        .serialize(serializer)
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        NativePathWire::UnixBytes {
            bytes: path.as_os_str().as_bytes().to_vec(),
        }
        .serialize(serializer)
    }
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    #[cfg(windows)]
    {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};

        let NativePathWire::WindowsUtf16 { units } = NativePathWire::deserialize(deserializer)?;
        Ok(PathBuf::from(OsString::from_wide(&units)))
    }

    #[cfg(unix)]
    {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let NativePathWire::UnixBytes { bytes } = NativePathWire::deserialize(deserializer)?;
        Ok(PathBuf::from(OsString::from_vec(bytes)))
    }
}
