use std::{
    ffi::{OsStr, OsString, c_void},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr,
};

use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    },
    System::{
        Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
            KEY_WOW64_64KEY, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
            RegGetValueW, RegOpenKeyExW,
        },
        Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW},
    },
};

use super::{ExecutableInfo, PhotoshopPlatform};

pub(super) struct WindowsPhotoshop;

impl PhotoshopPlatform for WindowsPhotoshop {
    fn candidates(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                if let Some(key) = RegistryKey::open(
                    hive,
                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\Photoshop.exe",
                    view,
                ) && let Some(path) = key.string("")
                {
                    paths.push(PathBuf::from(path));
                }
                if let Some(key) = RegistryKey::open(hive, r"SOFTWARE\Adobe\Photoshop", view) {
                    for subkey in key.subkeys() {
                        if let Some(version) = RegistryKey::open(key.0, &subkey, view)
                            && let Some(path) = version.string("ApplicationPath")
                        {
                            paths.push(PathBuf::from(path).join("Photoshop.exe"));
                        }
                    }
                }
            }
        }
        // A missing/stale registration must not hide a regular installation.
        // Only inspect Adobe's direct installation directories, never the user's disk recursively.
        for root in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(root)
                && let Ok(entries) = std::fs::read_dir(PathBuf::from(root).join("Adobe"))
            {
                for entry in entries.flatten() {
                    if entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("Adobe Photoshop")
                    {
                        paths.push(entry.path().join("Photoshop.exe"));
                    }
                }
            }
        }
        paths
    }

    fn inspect(&self, executable: &Path) -> Option<ExecutableInfo> {
        if !executable
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return None;
        }
        let path = wide(executable.as_os_str());
        let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), ptr::null_mut()) };
        if size == 0 || size > 16 * 1024 * 1024 {
            return None;
        }
        // u32 allocation keeps VS_FIXEDFILEINFO and language identifiers aligned.
        let mut data = vec![0u32; (size as usize).div_ceil(4)];
        if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
            return None;
        }
        let info = VersionInfo(&data);
        let (fixed, fixed_size) = info.query("\\")?;
        if fixed_size < size_of::<VS_FIXEDFILEINFO>() as u32 {
            return None;
        }
        let fixed = unsafe { ptr::read_unaligned(fixed.cast::<VS_FIXEDFILEINFO>()) };
        if fixed.dwSignature != 0xfeef04bd {
            return None;
        }
        let (translations, size) = info.query(r"\VarFileInfo\Translation")?;
        let translations =
            unsafe { std::slice::from_raw_parts(translations.cast::<u16>(), size as usize / 2) };
        for translation in translations.chunks_exact(2) {
            let prefix = format!(
                r"\StringFileInfo\{:04x}{:04x}\",
                translation[0], translation[1]
            );
            let Some(name) = info.string(&(prefix.clone() + "ProductName")) else {
                continue;
            };
            let Some(company) = info.string(&(prefix.clone() + "CompanyName")) else {
                continue;
            };
            let Some(filename) = info.string(&(prefix + "OriginalFilename")) else {
                continue;
            };
            if name.starts_with("Adobe Photoshop")
                && company.starts_with("Adobe")
                && filename.eq_ignore_ascii_case("Photoshop.exe")
            {
                let version = [
                    (fixed.dwProductVersionMS >> 16) as u16,
                    fixed.dwProductVersionMS as u16,
                    (fixed.dwProductVersionLS >> 16) as u16,
                    fixed.dwProductVersionLS as u16,
                ];
                if version[0] > 0 {
                    return Some(ExecutableInfo { name, version });
                }
            }
        }
        None
    }

    fn launch(&self, executable: &Path, original: &Path) -> std::io::Result<()> {
        // The application is external: it must outlive MyAlbuns and its development Job.
        // Windows closes the Child handles on drop without terminating the application.
        let child = Command::new(executable)
            .arg(original)
            .current_dir(executable.parent().unwrap_or(executable))
            .creation_flags(CREATE_BREAKAWAY_FROM_JOB | CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        drop(child);
        Ok(())
    }
}

struct RegistryKey(HKEY);

impl RegistryKey {
    fn open(hive: HKEY, name: &str, view: u32) -> Option<Self> {
        let name = wide(OsStr::new(name));
        let mut key = ptr::null_mut();
        (unsafe { RegOpenKeyExW(hive, name.as_ptr(), 0, KEY_READ | view, &mut key) }
            == ERROR_SUCCESS)
            .then_some(Self(key))
    }

    fn string(&self, name: &str) -> Option<OsString> {
        let name = wide(OsStr::new(name));
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
        let mut size = 0;
        if unsafe {
            RegGetValueW(
                self.0,
                ptr::null(),
                name.as_ptr(),
                flags,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
            )
        } != ERROR_SUCCESS
            || size == 0
            || size > 128 * 1024
        {
            return None;
        }
        let mut value = vec![0u16; (size as usize).div_ceil(2)];
        if unsafe {
            RegGetValueW(
                self.0,
                ptr::null(),
                name.as_ptr(),
                flags,
                ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut size,
            )
        } != ERROR_SUCCESS
        {
            return None;
        }
        let length = value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len());
        Some(OsString::from_wide(&value[..length]))
    }

    fn subkeys(&self) -> Vec<String> {
        let mut names = Vec::new();
        for index in 0..1024 {
            let mut name = [0u16; 256];
            let mut size = name.len() as u32;
            if unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    name.as_mut_ptr(),
                    &mut size,
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            } != ERROR_SUCCESS
            {
                break;
            }
            if let Ok(name) = String::from_utf16(&name[..size as usize]) {
                names.push(name);
            }
        }
        names
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

struct VersionInfo<'a>(&'a [u32]);

impl VersionInfo<'_> {
    fn query(&self, subkey: &str) -> Option<(*mut c_void, u32)> {
        let subkey = wide(OsStr::new(subkey));
        let mut result = ptr::null_mut();
        let mut size = 0;
        if unsafe {
            VerQueryValueW(
                self.0.as_ptr().cast(),
                subkey.as_ptr(),
                &mut result,
                &mut size,
            )
        } == 0
            || result.is_null()
        {
            return None;
        }
        Some((result, size))
    }

    fn string(&self, subkey: &str) -> Option<String> {
        let (value, size) = self.query(subkey)?;
        if size == 0 {
            return None;
        }
        let units = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), size as usize) };
        let length = units
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units.len());
        String::from_utf16(&units[..length]).ok()
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
