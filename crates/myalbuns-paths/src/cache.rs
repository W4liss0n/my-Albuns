use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    AppPaths, AppPathsError,
    app_paths::{media_cache_key, valid_cache_component, valid_namespace_component},
    guarded_fs::{
        DirectoryGuard, GuardedFsError, ensure_direct_child, is_reparse_point, open_directory,
        open_existing_direct_child, remove_empty_directory, validate_open_file,
    },
};

impl From<GuardedFsError> for AppPathsError {
    fn from(error: GuardedFsError) -> Self {
        match error {
            GuardedFsError::Unavailable => Self::CacheStorageUnavailable,
            GuardedFsError::OutsideRoot => Self::CacheStorageOutsideRoot,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheArtifactFormat {
    Jpeg,
    Png,
}

impl CacheArtifactFormat {
    const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
        }
    }
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
    directories: Vec<DirectoryGuard>,
}

/// A Cache file that is still being written.
///
/// Dropping this value before publication removes only its temporary file.
pub struct PendingCachePublication<'storage> {
    storage: &'storage PreparedCacheStorage,
    final_path: PathBuf,
    temporary: TemporaryCacheFile,
}

/// A synchronized Cache file that can be promoted to its final name.
///
/// Dropping this value before publication removes only its temporary file.
pub struct SynchronizedCachePublication<'storage> {
    storage: &'storage PreparedCacheStorage,
    final_path: PathBuf,
    temporary: TemporaryCacheFile,
}

struct TemporaryCacheFile {
    path: PathBuf,
    file: Option<File>,
    published: bool,
}

#[derive(Debug)]
struct ExistingProjectCache {
    directories: Vec<DirectoryGuard>,
}

impl ExistingProjectCache {
    fn project(&self) -> &DirectoryGuard {
        self.directories
            .last()
            .expect("an existing project Cache always contains its project directory")
    }
}

impl CachePathPlan {
    pub(crate) fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

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
            || !valid_namespace_component(namespace)
        {
            return Err(AppPathsError::InvalidProjectNamespace);
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
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
            directories: guards,
        })
    }

    pub fn preview_file(
        &self,
        media_id: &str,
        generation_id: &str,
        format: CacheArtifactFormat,
    ) -> Result<PathBuf, AppPathsError> {
        if media_id.trim().is_empty() || !valid_cache_component(generation_id) {
            return Err(AppPathsError::InvalidCacheArtifact);
        }
        let media_key = media_cache_key(media_id);
        Ok(self.media_directory().join(format!(
            "{media_key}.{generation_id}.{}",
            format.extension()
        )))
    }

    pub fn preview_temporary_file(
        &self,
        media_id: &str,
        generation_id: &str,
        format: CacheArtifactFormat,
        process_id: u32,
    ) -> Result<PathBuf, AppPathsError> {
        if media_id.trim().is_empty() || !valid_cache_component(generation_id) {
            return Err(AppPathsError::InvalidCacheArtifact);
        }
        let media_key = media_cache_key(media_id);
        Ok(self.media_directory().join(format!(
            "{media_key}.{generation_id}.{}.tmp-{process_id}",
            format.extension()
        )))
    }

    pub fn metadata_file(&self) -> PathBuf {
        self.root.join("metadata.json")
    }

    pub fn metadata_temporary_file(&self, process_id: u32) -> PathBuf {
        self.root.join(format!("metadata.json.tmp-{process_id}"))
    }
}

impl PreparedCacheStorage {
    pub fn begin_file_publication<'storage>(
        &'storage self,
        temporary_path: &Path,
        final_path: &Path,
    ) -> Result<PendingCachePublication<'storage>, AppPathsError> {
        self.validate_publication_paths(temporary_path, final_path)?;
        let file = self.create_temporary_file(temporary_path)?;
        Ok(PendingCachePublication {
            storage: self,
            final_path: final_path.to_path_buf(),
            temporary: TemporaryCacheFile {
                path: temporary_path.to_path_buf(),
                file: Some(file),
                published: false,
            },
        })
    }

    fn create_temporary_file(&self, path: &Path) -> Result<File, AppPathsError> {
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

    pub fn remove_existing_file(&self, path: &Path) -> Result<bool, AppPathsError> {
        let parent = self
            .directories
            .iter()
            .rev()
            .take(2)
            .find(|directory| path.parent() == Some(directory.logical_path.as_path()))
            .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        let file = File::open(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        validate_open_file(parent, path, &file)?;
        drop(file);
        fs::remove_file(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        Ok(true)
    }

    pub fn remove_unreferenced_generations(
        &self,
        referenced: &HashSet<PathBuf>,
    ) -> Result<usize, AppPathsError> {
        let media_directory = self
            .directories
            .last()
            .ok_or(AppPathsError::CacheStorageUnavailable)?;
        for path in referenced {
            if path.parent() != Some(media_directory.logical_path.as_path()) {
                return Err(AppPathsError::CacheStorageOutsideRoot);
            }
        }

        let mut removed = 0;
        let entries = fs::read_dir(&media_directory.logical_path)
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        for entry in entries {
            let entry = entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
            let path = entry.path();
            if !is_final_generation_name(&entry.file_name()) || referenced.contains(&path) {
                continue;
            }
            removed += usize::from(self.remove_existing_file(&path)?);
        }
        Ok(removed)
    }

    fn replace_file(&self, temporary: &Path, final_path: &Path) -> Result<(), AppPathsError> {
        let (temporary_parent, final_parent) =
            self.validate_publication_paths(temporary, final_path)?;

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
        validate_open_file(final_parent, final_path, &published)?;
        Ok(())
    }

    fn validate_publication_paths(
        &self,
        temporary: &Path,
        final_path: &Path,
    ) -> Result<(&DirectoryGuard, &DirectoryGuard), AppPathsError> {
        if temporary == final_path {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        let temporary_parent = self.parent_for(temporary)?;
        let final_parent = self.parent_for(final_path)?;
        if temporary_parent.logical_path != final_parent.logical_path {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        Ok((temporary_parent, final_parent))
    }

    fn parent_for(&self, path: &Path) -> Result<&DirectoryGuard, AppPathsError> {
        self.directories
            .iter()
            .find(|directory| path.parent() == Some(directory.logical_path.as_path()))
            .ok_or(AppPathsError::CacheStorageOutsideRoot)
    }
}

fn is_final_generation_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let mut parts = name.split('.');
    let (Some(media_key), Some(generation), Some(extension), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Some(digest) = media_key.strip_prefix("media-") else {
        return false;
    };
    digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        && valid_cache_component(generation)
        && matches!(extension, "jpg" | "png")
}

impl Write for PendingCachePublication<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.temporary.file_mut().write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.temporary.file_mut().flush()
    }
}

impl<'storage> PendingCachePublication<'storage> {
    pub fn sync(mut self) -> Result<SynchronizedCachePublication<'storage>, AppPathsError> {
        self.temporary.sync_and_close()?;
        Ok(SynchronizedCachePublication {
            storage: self.storage,
            final_path: self.final_path,
            temporary: self.temporary,
        })
    }
}

impl SynchronizedCachePublication<'_> {
    pub fn publish(mut self) -> Result<(), AppPathsError> {
        self.storage
            .replace_file(&self.temporary.path, &self.final_path)?;
        self.temporary.published = true;
        Ok(())
    }
}

impl TemporaryCacheFile {
    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("a pending Cache publication always owns its temporary file")
    }

    fn sync_and_close(&mut self) -> Result<(), AppPathsError> {
        self.file_mut()
            .sync_all()
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        drop(self.file.take());
        Ok(())
    }
}

impl Drop for TemporaryCacheFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn prepare_cache_storage(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<PreparedCacheStorage, AppPathsError> {
    if plan.root.parent() != Some(app_paths.cache_dir().as_path()) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    plan.prepare_storage()
}

pub(crate) fn clear_project_cache(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<bool, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, plan)? else {
        return Ok(false);
    };

    clear_project_directory(project_cache.project())?;
    drop(project_cache);
    remove_empty_directory(&plan.root)?;
    Ok(true)
}

pub(crate) fn discard_project_cache_temporaries(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
    process_id: u32,
) -> Result<usize, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, plan)? else {
        return Ok(0);
    };

    let project = project_cache.project();
    let mut removed = discard_matching_files(project, |name| {
        is_metadata_temporary_name_for(name, process_id)
    })?;
    if let Some(media) = open_existing_direct_child(project, &plan.media_directory())? {
        removed += discard_matching_files(&media, |name| {
            is_preview_temporary_name_for(name, process_id)
        })?;
    }
    Ok(removed)
}

fn open_existing_project_cache(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<Option<ExistingProjectCache>, AppPathsError> {
    if plan.root.parent() != Some(app_paths.cache_dir().as_path()) {
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    plan.validate()?;

    let local_data_root = app_paths
        .local_root
        .parent()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    let mut directories = vec![open_directory(local_data_root)?];
    for path in [&app_paths.local_root, &app_paths.cache_dir(), &plan.root] {
        let parent = directories
            .last()
            .ok_or(AppPathsError::CacheStorageUnavailable)?;
        let Some(directory) = open_existing_direct_child(parent, path)? else {
            return Ok(None);
        };
        directories.push(directory);
    }
    Ok(Some(ExistingProjectCache { directories }))
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

fn discard_matching_files<F>(
    directory: &DirectoryGuard,
    is_temporary: F,
) -> Result<usize, AppPathsError>
where
    F: Fn(&std::ffi::OsStr) -> bool,
{
    let mut removed = 0;
    for entry in
        fs::read_dir(&directory.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let entry = entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        if is_reparse_point(&metadata) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        if metadata.is_file() && is_temporary(&entry.file_name()) {
            fs::remove_file(path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn is_metadata_temporary_name_for(name: &std::ffi::OsStr, process_id: u32) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix("metadata.json.tmp-"))
        .and_then(|value| value.parse::<u32>().ok())
        == Some(process_id)
}

fn is_preview_temporary_name_for(name: &std::ffi::OsStr, expected_process_id: u32) -> bool {
    let Some((artifact, process_id)) = name.to_str().and_then(|name| name.rsplit_once(".tmp-"))
    else {
        return false;
    };
    let Some(components) = [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png]
        .into_iter()
        .find_map(|format| artifact.strip_suffix(&format!(".{}", format.extension())))
    else {
        return false;
    };
    let mut components = components.split('.');
    matches!(
        (components.next(), components.next(), components.next()),
        (Some(media_id), Some(generation_id), None)
            if valid_cache_component(media_id)
                && valid_cache_component(generation_id)
                && process_id.parse::<u32>().ok() == Some(expected_process_id)
    )
}
