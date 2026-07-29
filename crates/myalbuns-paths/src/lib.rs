//! Descoberta centralizada dos caminhos de dados do aplicativo.

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

/// Namespace temporário usado para não misturar os dados deste desenvolvimento
/// com os dados da versão anterior do MyAlbuns.
///
/// A distribuição final voltará a usar `MyAlbuns` depois que a nova versão
/// estiver concluída.
const TEMPORARY_APP_DIRECTORY_NAME: &str = "MyAlbuns2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppPathsError {
    KnownFoldersUnavailable,
    InvalidProjectNamespace,
    InvalidCacheArtifact,
    CacheArtifactOutsideRoot,
    PathNotRepresentable,
    CacheStorageUnavailable,
    CacheStorageOutsideRoot,
}

impl Display for AppPathsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFoldersUnavailable => {
                formatter.write_str("não foi possível localizar as pastas de dados do usuário")
            }
            Self::InvalidProjectNamespace => {
                formatter.write_str("a Identidade do Projeto não forma um namespace seguro")
            }
            Self::InvalidCacheArtifact => {
                formatter.write_str("a identidade do artefato de Cache é inválida")
            }
            Self::CacheArtifactOutsideRoot => {
                formatter.write_str("o artefato não pertence à raiz autorizada do Cache")
            }
            Self::PathNotRepresentable => {
                formatter.write_str("o caminho do Cache não pode ser representado pelo WebView")
            }
            Self::CacheStorageUnavailable => {
                formatter.write_str("a estrutura de diretórios do Cache está indisponível")
            }
            Self::CacheStorageOutsideRoot => {
                formatter.write_str("a estrutura física do Cache escapou da raiz autorizada")
            }
        }
    }
}

impl Error for AppPathsError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    roaming_root: PathBuf,
    local_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CachePathPlan {
    root: PathBuf,
}

/// Keeps the validated Cache directory chain open while artifacts are written.
///
/// On Windows the handles deny directory replacement, so a reparse point
/// cannot redirect a job after containment has been verified.
#[derive(Debug)]
pub struct PreparedCacheStorage {
    _directories: Vec<DirectoryGuard>,
}

#[derive(Debug)]
struct DirectoryGuard {
    _handle: File,
    logical_path: PathBuf,
    physical_path: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathsError> {
        let known_folders = BaseDirs::new().ok_or(AppPathsError::KnownFoldersUnavailable)?;
        Ok(Self::from_known_folders(
            known_folders.data_dir(),
            known_folders.data_local_dir(),
        ))
    }

    pub fn from_known_folders(roaming_data: &Path, local_data: &Path) -> Self {
        Self {
            roaming_root: roaming_data.join(TEMPORARY_APP_DIRECTORY_NAME),
            local_root: local_data.join(TEMPORARY_APP_DIRECTORY_NAME),
        }
    }

    pub fn roaming_root(&self) -> &Path {
        &self.roaming_root
    }

    pub fn local_root(&self) -> &Path {
        &self.local_root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.roaming_root.join("settings.json")
    }

    pub fn layouts_dir(&self) -> PathBuf {
        self.roaming_root.join("Layouts")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.local_root.join("Cache")
    }

    pub fn project_cache(&self, project_namespace: &str) -> Result<CachePathPlan, AppPathsError> {
        if !valid_project_namespace(project_namespace) {
            return Err(AppPathsError::InvalidProjectNamespace);
        }
        Ok(CachePathPlan {
            root: self.cache_dir().join(project_namespace),
        })
    }

    pub fn cache_asset_url(&self, cache_file: &Path) -> Result<String, AppPathsError> {
        if !cache_file.is_absolute()
            || cache_file.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
            || !cache_file.starts_with(self.cache_dir())
        {
            return Err(AppPathsError::CacheArtifactOutsideRoot);
        }
        let path = cache_file
            .to_str()
            .ok_or(AppPathsError::PathNotRepresentable)?;
        let protocol = if cfg!(any(target_os = "windows", target_os = "android")) {
            "http://asset.localhost/"
        } else {
            "asset://localhost/"
        };
        Ok(format!("{protocol}{}", encode_uri_component(path)))
    }

    pub fn prepare_cache_storage(
        &self,
        plan: &CachePathPlan,
    ) -> Result<PreparedCacheStorage, AppPathsError> {
        if plan.root.parent() != Some(self.cache_dir().as_path()) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        plan.prepare_storage()
    }

    pub fn clear_project_cache(&self, plan: &CachePathPlan) -> Result<bool, AppPathsError> {
        if plan.root.parent() != Some(self.cache_dir().as_path()) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        plan.validate()?;

        let local_data_root = self
            .local_root
            .parent()
            .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
        let local_data = open_directory(local_data_root)?;
        let Some(application) = open_existing_direct_child(&local_data, &self.local_root)? else {
            return Ok(false);
        };
        let Some(cache) = open_existing_direct_child(&application, &self.cache_dir())? else {
            return Ok(false);
        };
        let Some(project) = open_existing_direct_child(&cache, &plan.root)? else {
            return Ok(false);
        };

        clear_project_directory(&project)?;
        drop(project);
        remove_empty_directory(&plan.root)?;
        Ok(true)
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.local_root.join("Recovery")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.local_root.join("State")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.local_root.join("Logs")
    }
}

impl CachePathPlan {
    pub fn validate(&self) -> Result<(), AppPathsError> {
        let namespace = self
            .root
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(AppPathsError::InvalidProjectNamespace)?;
        let parent_name = self
            .root
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str());
        if !self.root.is_absolute()
            || self.root.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
            || parent_name != Some("Cache")
            || !valid_project_namespace(namespace)
        {
            return Err(AppPathsError::InvalidProjectNamespace);
        }
        Ok(())
    }

    pub fn media_directory(&self) -> PathBuf {
        self.root.join("Media")
    }

    fn prepare_storage(&self) -> Result<PreparedCacheStorage, AppPathsError> {
        self.validate()?;
        let cache_root = self
            .root
            .parent()
            .ok_or(AppPathsError::InvalidProjectNamespace)?;
        let application_root = cache_root
            .parent()
            .ok_or(AppPathsError::InvalidProjectNamespace)?;
        let local_data_root = application_root
            .parent()
            .ok_or(AppPathsError::InvalidProjectNamespace)?;
        let directories = [
            application_root.to_path_buf(),
            cache_root.to_path_buf(),
            self.root.clone(),
            self.media_directory(),
        ];

        let mut guards = Vec::with_capacity(directories.len() + 1);
        guards.push(open_directory(local_data_root)?);
        for directory in directories {
            let parent = guards
                .last()
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            guards.push(ensure_direct_child(parent, &directory)?);
        }
        Ok(PreparedCacheStorage {
            _directories: guards,
        })
    }

    pub fn preview_file(
        &self,
        media_id: &str,
        generation_id: &str,
    ) -> Result<PathBuf, AppPathsError> {
        if !valid_cache_component(media_id) || !valid_cache_component(generation_id) {
            return Err(AppPathsError::InvalidCacheArtifact);
        }
        Ok(self
            .media_directory()
            .join(format!("{media_id}.{generation_id}.jpg")))
    }

    pub fn preview_temporary_file(
        &self,
        media_id: &str,
        generation_id: &str,
        process_id: u32,
    ) -> Result<PathBuf, AppPathsError> {
        if !valid_cache_component(media_id) || !valid_cache_component(generation_id) {
            return Err(AppPathsError::InvalidCacheArtifact);
        }
        Ok(self
            .media_directory()
            .join(format!("{media_id}.{generation_id}.jpg.tmp-{process_id}")))
    }

    pub fn metadata_file(&self) -> PathBuf {
        self.root.join("metadata.json")
    }

    pub fn metadata_temporary_file(&self, process_id: u32) -> PathBuf {
        self.root.join(format!("metadata.json.tmp-{process_id}"))
    }
}

impl PreparedCacheStorage {
    pub fn create_temporary_file(&self, path: &Path) -> Result<File, AppPathsError> {
        let parent = self.parent_for(path)?;
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(AppPathsError::CacheStorageUnavailable);
            }
            Ok(_) => fs::remove_file(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
        }

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        validate_open_file(parent, path, &file)?;
        Ok(file)
    }

    pub fn open_existing_file(&self, path: &Path) -> Result<Option<File>, AppPathsError> {
        let parent = self.parent_for(path)?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        let file = File::open(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        validate_open_file(parent, path, &file)?;
        Ok(Some(file))
    }

    pub fn replace_file(&self, temporary: &Path, final_path: &Path) -> Result<(), AppPathsError> {
        let temporary_parent = self.parent_for(temporary)?;
        let final_parent = self.parent_for(final_path)?;
        if temporary_parent.logical_path != final_parent.logical_path {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }

        let temporary_file =
            File::open(temporary).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        validate_open_file(temporary_parent, temporary, &temporary_file)?;
        drop(temporary_file);

        match fs::symlink_metadata(final_path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(AppPathsError::CacheStorageUnavailable);
            }
            Ok(_) => {
                fs::remove_file(final_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
        }
        fs::rename(temporary, final_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        let published =
            File::open(final_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        validate_open_file(final_parent, final_path, &published)
    }

    fn parent_for(&self, path: &Path) -> Result<&DirectoryGuard, AppPathsError> {
        self._directories
            .iter()
            .find(|directory| path.parent() == Some(directory.logical_path.as_path()))
            .ok_or(AppPathsError::CacheStorageOutsideRoot)
    }
}

fn valid_project_namespace(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || value.ends_with('.')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return false;
    }

    let stem = value.split('.').next().unwrap_or_default();
    !matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn valid_cache_component(value: &str) -> bool {
    valid_project_namespace(value)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn encode_uri_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn ensure_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<DirectoryGuard, AppPathsError> {
    if child_path.parent() != Some(parent.logical_path.as_path()) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    match fs::create_dir(child_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
    }
    let metadata =
        fs::symlink_metadata(child_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }

    let child = open_directory(child_path)?;
    let expected_name = child_path
        .file_name()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    if !is_direct_physical_child(&parent.physical_path, &child.physical_path, expected_name) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(child)
}

fn open_existing_direct_child(
    parent: &DirectoryGuard,
    child_path: &Path,
) -> Result<Option<DirectoryGuard>, AppPathsError> {
    if child_path.parent() != Some(parent.logical_path.as_path()) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    let metadata = match fs::symlink_metadata(child_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
    };
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    let child = open_directory(child_path)?;
    let expected_name = child_path
        .file_name()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    if !is_direct_physical_child(&parent.physical_path, &child.physical_path, expected_name) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(Some(child))
}

fn clear_project_directory(project: &DirectoryGuard) -> Result<(), AppPathsError> {
    for entry in
        fs::read_dir(&project.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let path = entry
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?
            .path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        if is_reparse_point(&metadata) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        if metadata.is_file() {
            fs::remove_file(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
            continue;
        }
        if metadata.is_dir() && path.file_name() == Some(std::ffi::OsStr::new("Media")) {
            let media = open_existing_direct_child(project, &path)?
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            clear_cache_files(&media)?;
            drop(media);
            remove_empty_directory(&path)?;
            continue;
        }
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(())
}

fn clear_cache_files(directory: &DirectoryGuard) -> Result<(), AppPathsError> {
    for entry in
        fs::read_dir(&directory.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let path = entry
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?
            .path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        if is_reparse_point(&metadata) || !metadata.is_file() {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        fs::remove_file(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    }
    Ok(())
}

fn remove_empty_directory(path: &Path) -> Result<(), AppPathsError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    fs::remove_dir(path).map_err(|_| AppPathsError::CacheStorageUnavailable)
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn is_direct_physical_child(parent: &Path, child: &Path, expected_name: &std::ffi::OsStr) -> bool {
    child.parent() == Some(parent)
        && child
            .file_name()
            .is_some_and(|actual_name| same_component(actual_name, expected_name))
}

fn validate_open_file(
    parent: &DirectoryGuard,
    logical_path: &Path,
    file: &File,
) -> Result<(), AppPathsError> {
    if !file
        .metadata()
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?
        .is_file()
    {
        return Err(AppPathsError::CacheStorageUnavailable);
    }
    let physical_path = physical_path_from_file(file, logical_path)?;
    let expected_name = logical_path
        .file_name()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    if !is_direct_physical_child(&parent.physical_path, &physical_path, expected_name) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(())
}

fn same_component(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    match (left.to_str(), right.to_str()) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
}

#[cfg(windows)]
fn open_directory(path: &Path) -> Result<DirectoryGuard, AppPathsError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let handle = OpenOptions::new()
        .read(true)
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    if !handle
        .metadata()
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?
        .is_dir()
    {
        return Err(AppPathsError::CacheStorageUnavailable);
    }

    let physical_path = physical_path_from_file(&handle, path)?;

    Ok(DirectoryGuard {
        _handle: handle,
        logical_path: path.to_path_buf(),
        physical_path,
    })
}

#[cfg(windows)]
fn physical_path_from_file(file: &File, _logical_path: &Path) -> Result<PathBuf, AppPathsError> {
    use std::{
        ffi::OsString,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS},
    };

    let mut buffer = vec![0_u16; 512];
    loop {
        let length = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle() as HANDLE,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if length == 0 {
            return Err(AppPathsError::CacheStorageUnavailable);
        }
        if (length as usize) < buffer.len() {
            return Ok(PathBuf::from(OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
        buffer.resize(length as usize + 1, 0);
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, path::Path};

    use super::is_direct_physical_child;

    #[test]
    fn physical_containment_rejects_a_redirected_child() {
        assert!(is_direct_physical_child(
            Path::new("/local/MyAlbuns2/Cache"),
            Path::new("/local/MyAlbuns2/Cache/project-01"),
            OsStr::new("project-01"),
        ));
        assert!(!is_direct_physical_child(
            Path::new("/local/MyAlbuns2/Cache"),
            Path::new("/redirected/project-01"),
            OsStr::new("project-01"),
        ));
    }
}

#[cfg(not(windows))]
fn open_directory(path: &Path) -> Result<DirectoryGuard, AppPathsError> {
    let handle = File::open(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    if !handle
        .metadata()
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?
        .is_dir()
    {
        return Err(AppPathsError::CacheStorageUnavailable);
    }
    let physical_path =
        fs::canonicalize(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    Ok(DirectoryGuard {
        _handle: handle,
        logical_path: path.to_path_buf(),
        physical_path,
    })
}

#[cfg(not(windows))]
fn physical_path_from_file(_file: &File, logical_path: &Path) -> Result<PathBuf, AppPathsError> {
    fs::canonicalize(logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)
}
