use std::path::Path;

/// Identity of a locally mounted storage volume. An unknown/remote volume never
/// authorizes reclaiming an unrelated local cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageVolume(String);

impl StorageVolume {
    pub fn containing(path: &Path) -> Option<Self> {
        if !path.is_absolute() {
            return None;
        }
        volume_name(path).map(Self)
    }
}

#[cfg(windows)]
fn volume_name(path: &Path) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    };
    let input = crate::wide_api_path(path);
    let mut mount = vec![0_u16; 32_768];
    let mut volume = [0_u16; 50];
    // SAFETY: input is terminated UTF-16; both output buffers remain allocated
    // for their declared lengths. These APIs follow local junctions/mounts.
    if unsafe { GetVolumePathNameW(input.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) } == 0
        || unsafe {
            GetVolumeNameForVolumeMountPointW(
                mount.as_ptr(),
                volume.as_mut_ptr(),
                volume.len() as u32,
            )
        } == 0
    {
        return None;
    }
    let end = volume.iter().position(|unit| *unit == 0)?;
    String::from_utf16(&volume[..end])
        .ok()
        .map(|name| name.to_ascii_lowercase())
}

#[cfg(not(windows))]
fn volume_name(_path: &Path) -> Option<String> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn missing_output_uses_its_existing_parent_volume() {
        let root = std::env::temp_dir();
        let volume = StorageVolume::containing(&root).expect("local temporary volume");
        assert_eq!(
            Some(volume),
            StorageVolume::containing(&root.join("missing-album/output.jpg"))
        );
        assert_eq!(
            None,
            StorageVolume::containing(Path::new("relative/output.jpg"))
        );
    }
}
