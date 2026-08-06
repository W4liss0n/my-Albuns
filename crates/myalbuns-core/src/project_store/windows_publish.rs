use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

pub(crate) fn write_synced_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(windows)]
    use std::os::windows::fs::OpenOptionsExt;
    #[cfg(windows)]
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    let mut writer = options.open(path)?;
    writer.write_all(bytes)?;
    writer.flush()?;
    writer.sync_all()
}

pub(crate) fn publish_new(prepared: &Path, target: &Path) -> io::Result<()> {
    myalbuns_paths::publish_new_file(prepared, target)
}

pub(crate) fn replace_existing(prepared: &Path, target: &Path) -> io::Result<()> {
    myalbuns_paths::replace_existing_file(prepared, target)
}
