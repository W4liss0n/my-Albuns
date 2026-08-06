use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Reversible pathname DTO for process and frontend-neutral wire boundaries.
///
/// The serialized value is the platform-native wire representation itself;
/// the pathname is never converted through a Unicode string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePathDto(PathBuf);

impl NativePathDto {
    pub fn from_path(path: &Path) -> Self {
        Self(path.to_path_buf())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl From<PathBuf> for NativePathDto {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<&Path> for NativePathDto {
    fn from(path: &Path) -> Self {
        Self::from_path(path)
    }
}

impl From<NativePathDto> for PathBuf {
    fn from(path: NativePathDto) -> Self {
        path.0
    }
}

impl AsRef<Path> for NativePathDto {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Serialize for NativePathDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for NativePathDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize(deserializer).map(Self)
    }
}

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
