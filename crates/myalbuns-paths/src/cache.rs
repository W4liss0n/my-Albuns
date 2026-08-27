use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    AppPaths, AppPathsError,
    app_paths::{media_cache_key, valid_cache_component, valid_namespace_component},
    guarded_fs::{
        DirectoryGuard, GuardedFsError, create_new_deletable_file, delete_open_file,
        ensure_direct_child, is_reparse_point, open_deletable_file, open_directory,
        open_existing_direct_child, open_readable_file, remove_empty_directory, rename_open_file,
    },
};

const CACHE_WRITER_CLAIM_FILE: &str = ".processor-writer.v1.json";
const CACHE_WRITER_TEMPORARY_PREFIX: &str = ".processor-writer-";
const CACHE_WRITER_CLAIM_MAX_BYTES: usize = 1024;

impl From<GuardedFsError> for AppPathsError {
    fn from(error: GuardedFsError) -> Self {
        match error {
            GuardedFsError::AlreadyExists
            | GuardedFsError::NotFound
            | GuardedFsError::Unavailable => Self::CacheStorageUnavailable,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheNamespaceUsage {
    paths: CachePathPlan,
    bytes: u64,
}

impl CacheNamespaceUsage {
    pub fn paths(&self) -> &CachePathPlan {
        &self.paths
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Keeps the validated Cache directory chain open while artifacts are written.
///
/// Open handles bind the verified physical chain, and each artifact is opened,
/// renamed, or removed as one component relative to its parent handle. A later
/// reparse-point replacement therefore cannot redirect the job outside it.
#[derive(Debug)]
pub struct PreparedCacheStorage {
    directories: Vec<DirectoryGuard>,
}

/// Keeps the validated Project Cache directory chain open while a Host
/// publishes, waits for, or removes the exact Processor writer claim.
///
/// The open directory handles bind every claim operation to the validated
/// physical namespace. A later pathname replacement therefore cannot redirect
/// publication or deletion through a junction.
#[derive(Debug)]
pub struct CacheWriterClaimStorage {
    project_cache: ExistingProjectCache,
}

/// A Cache file that is still being written.
///
/// Dropping this value before publication removes only its temporary file.
pub struct PendingCachePublication<'storage> {
    storage: &'storage PreparedCacheStorage,
    final_path: PathBuf,
    temporary: TemporaryCacheFile<'storage>,
}

/// A synchronized Cache file that can be promoted to its final name.
///
/// Dropping this value before publication removes only its temporary file.
pub struct SynchronizedCachePublication<'storage> {
    storage: &'storage PreparedCacheStorage,
    final_path: PathBuf,
    temporary: TemporaryCacheFile<'storage>,
}

struct TemporaryCacheFile<'storage> {
    parent: &'storage DirectoryGuard,
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

    fn cache_parent(&self) -> &DirectoryGuard {
        self.directories
            .get(self.directories.len().saturating_sub(2))
            .expect("an existing project Cache always contains its Cache parent")
    }
}

impl CacheWriterClaimStorage {
    fn project(&self) -> &DirectoryGuard {
        self.project_cache.project()
    }

    fn claim_path(&self) -> PathBuf {
        self.project().logical_path.join(CACHE_WRITER_CLAIM_FILE)
    }

    /// Atomically publishes one opaque, bounded claim inside the guarded
    /// namespace. Existing claims are never replaced.
    pub fn publish_claim(&self, bytes: &[u8]) -> Result<(), AppPathsError> {
        validate_writer_claim_bytes(bytes)?;
        let temporary = self.project().logical_path.join(format!(
            "{CACHE_WRITER_TEMPORARY_PREFIX}{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let mut file = create_new_deletable_file(self.project(), &temporary)?;
        let prepared = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| AppPathsError::CacheStorageUnavailable);
        if let Err(error) = prepared {
            let _ = delete_open_file(self.project(), &temporary, &file);
            drop(file);
            return Err(error);
        }
        if let Err(error) = rename_open_file(
            self.project(),
            &temporary,
            &file,
            std::ffi::OsStr::new(CACHE_WRITER_CLAIM_FILE),
        ) {
            let _ = delete_open_file(self.project(), &temporary, &file);
            drop(file);
            return Err(error.into());
        }
        drop(file);
        Ok(())
    }

    /// Reads the claim through an open file whose physical parent is the
    /// guarded Project namespace.
    pub fn read_claim(&self) -> Result<Option<Vec<u8>>, AppPathsError> {
        let path = self.claim_path();
        let mut file = match open_readable_file(self.project(), &path) {
            Ok(file) => file,
            Err(GuardedFsError::NotFound) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        read_writer_claim_file(&mut file).map(Some)
    }

    /// Removes the claim only when the bytes observed through the guarded
    /// namespace still match the expected exact Process instance.
    pub fn remove_claim_if_matches(&self, expected: &[u8]) -> Result<bool, AppPathsError> {
        validate_writer_claim_bytes(expected)?;
        let path = self.claim_path();
        let mut file = match open_deletable_file(self.project(), &path) {
            Ok(file) => file,
            Err(GuardedFsError::NotFound) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let observed = read_writer_claim_file(&mut file)?;
        if observed != expected {
            return Ok(false);
        }
        delete_open_file(self.project(), &path, &file)?;
        drop(file);
        Ok(true)
    }

    /// Discards only well-formed writer-claim temporaries after the exact
    /// previous writer has been proven quiescent.
    pub fn discard_claim_temporaries(&self) -> Result<usize, AppPathsError> {
        let mut removed = 0_usize;
        for entry in fs::read_dir(&self.project().logical_path)
            .map_err(|_| AppPathsError::CacheStorageUnavailable)?
        {
            let entry = entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !name.starts_with(CACHE_WRITER_TEMPORARY_PREFIX) || !name.ends_with(".tmp") {
                continue;
            }
            let path = entry.path();
            let file = match open_deletable_file(self.project(), &path) {
                Ok(file) => file,
                Err(GuardedFsError::NotFound) => continue,
                Err(error) => return Err(error.into()),
            };
            delete_open_file(self.project(), &path, &file)?;
            drop(file);
            removed += 1;
        }
        Ok(removed)
    }
}

fn validate_writer_claim_bytes(bytes: &[u8]) -> Result<(), AppPathsError> {
    if bytes.is_empty() || bytes.len() > CACHE_WRITER_CLAIM_MAX_BYTES {
        return Err(AppPathsError::CacheStorageUnavailable);
    }
    Ok(())
}

fn read_writer_claim_file(file: &mut File) -> Result<Vec<u8>, AppPathsError> {
    let size = file
        .metadata()
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?
        .len();
    let mut bytes = Vec::with_capacity(
        usize::try_from(size)
            .unwrap_or(CACHE_WRITER_CLAIM_MAX_BYTES.saturating_add(1))
            .min(CACHE_WRITER_CLAIM_MAX_BYTES.saturating_add(1)),
    );
    file.take((CACHE_WRITER_CLAIM_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
    validate_writer_claim_bytes(&bytes)?;
    Ok(bytes)
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
        let (parent, file) = self.create_temporary_file(temporary_path)?;
        Ok(PendingCachePublication {
            storage: self,
            final_path: final_path.to_path_buf(),
            temporary: TemporaryCacheFile {
                parent,
                path: temporary_path.to_path_buf(),
                file: Some(file),
                published: false,
            },
        })
    }

    fn create_temporary_file(&self, path: &Path) -> Result<(&DirectoryGuard, File), AppPathsError> {
        let parent = self.parent_for(path)?;
        match open_deletable_file(parent, path) {
            Ok(existing) => delete_open_file(parent, path, &existing)?,
            Err(GuardedFsError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let file = create_new_deletable_file(parent, path)?;
        Ok((parent, file))
    }

    pub fn open_existing_file(&self, path: &Path) -> Result<Option<File>, AppPathsError> {
        let parent = self.parent_for(path)?;
        match open_readable_file(parent, path) {
            Ok(file) => Ok(Some(file)),
            Err(GuardedFsError::NotFound) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remove_existing_file(&self, path: &Path) -> Result<bool, AppPathsError> {
        let parent = self
            .directories
            .iter()
            .rev()
            .take(2)
            .find(|directory| path.parent() == Some(directory.logical_path.as_path()))
            .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
        let file = match open_deletable_file(parent, path) {
            Ok(file) => file,
            Err(GuardedFsError::NotFound) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        delete_open_file(parent, path, &file)?;
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

    fn replace_file(
        &self,
        temporary: &Path,
        temporary_file: &File,
        final_path: &Path,
    ) -> Result<(), AppPathsError> {
        let (temporary_parent, final_parent) =
            self.validate_publication_paths(temporary, final_path)?;
        debug_assert_eq!(temporary_parent.logical_path, final_parent.logical_path);
        match open_deletable_file(final_parent, final_path) {
            Ok(existing) => delete_open_file(final_parent, final_path, &existing)?,
            Err(GuardedFsError::NotFound) => {}
            Err(error) => return Err(error.into()),
        }
        let target_name = final_path
            .file_name()
            .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
        rename_open_file(temporary_parent, temporary, temporary_file, target_name)?;
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
        self.temporary.sync()?;
        Ok(SynchronizedCachePublication {
            storage: self.storage,
            final_path: self.final_path,
            temporary: self.temporary,
        })
    }
}

impl SynchronizedCachePublication<'_> {
    pub fn publish(mut self) -> Result<(), AppPathsError> {
        self.storage.replace_file(
            &self.temporary.path,
            self.temporary.file(),
            &self.final_path,
        )?;
        self.temporary.published = true;
        Ok(())
    }
}

impl TemporaryCacheFile<'_> {
    fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("a Cache publication always owns its exact temporary handle")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("a pending Cache publication always owns its temporary file")
    }

    fn sync(&mut self) -> Result<(), AppPathsError> {
        self.file_mut()
            .sync_all()
            .map_err(|_| AppPathsError::CacheStorageUnavailable)
    }
}

impl Drop for TemporaryCacheFile<'_> {
    fn drop(&mut self) {
        if !self.published
            && let Some(file) = self.file.as_ref()
        {
            let _ = delete_open_file(self.parent, &self.path, file);
        }
        drop(self.file.take());
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

pub(crate) fn open_cache_writer_claim_storage(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<Option<CacheWriterClaimStorage>, AppPathsError> {
    open_existing_project_cache(app_paths, plan).map(|project_cache| {
        project_cache.map(|project_cache| CacheWriterClaimStorage { project_cache })
    })
}

pub(crate) fn list_cache_namespaces(
    app_paths: &AppPaths,
) -> Result<Vec<CachePathPlan>, AppPathsError> {
    let local_data_root = app_paths
        .local_root
        .parent()
        .ok_or(AppPathsError::CacheStorageOutsideRoot)?;
    let local_data = open_directory(local_data_root)?;
    let Some(application) = open_existing_direct_child(&local_data, &app_paths.local_root)? else {
        return Ok(Vec::new());
    };
    let Some(cache) = open_existing_direct_child(&application, &app_paths.cache_dir())? else {
        return Ok(Vec::new());
    };
    let mut namespaces = Vec::new();
    for entry in
        fs::read_dir(&cache.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let entry = entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        let Some(namespace) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        };
        if is_reparse_point(&metadata)
            || !metadata.is_dir()
            || !valid_namespace_component(&namespace)
        {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        let Some(project) = open_existing_direct_child(&cache, &path)? else {
            continue;
        };
        drop(project);
        namespaces.push(CachePathPlan::from_root(path));
    }
    namespaces.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(namespaces)
}

pub(crate) fn inspect_cache_namespace(
    app_paths: &AppPaths,
    paths: &CachePathPlan,
) -> Result<Option<CacheNamespaceUsage>, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, paths)? else {
        return Ok(None);
    };
    Ok(Some(CacheNamespaceUsage {
        bytes: measure_project_cache(project_cache.project(), paths)?,
        paths: paths.clone(),
    }))
}

pub(crate) fn snapshot_active_cache_namespace(
    app_paths: &AppPaths,
    paths: &CachePathPlan,
) -> Result<Option<CacheNamespaceUsage>, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, paths)? else {
        return Ok(None);
    };
    Ok(Some(CacheNamespaceUsage {
        bytes: measure_active_project_cache(project_cache.project(), paths)?,
        paths: paths.clone(),
    }))
}

fn measure_project_cache(
    project: &DirectoryGuard,
    paths: &CachePathPlan,
) -> Result<u64, AppPathsError> {
    let mut bytes = 0_u64;
    for entry in
        fs::read_dir(&project.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let entry = entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| AppPathsError::CacheStorageUnavailable)?;
        if is_reparse_point(&metadata) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        if metadata.is_file() {
            bytes = bytes
                .checked_add(measure_open_file(project, &path)?)
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            continue;
        }
        if metadata.is_dir() && path == paths.media_directory() {
            let media = open_existing_direct_child(project, &path)?
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            for media_entry in fs::read_dir(&media.logical_path)
                .map_err(|_| AppPathsError::CacheStorageUnavailable)?
            {
                let media_entry =
                    media_entry.map_err(|_| AppPathsError::CacheStorageUnavailable)?;
                let media_path = media_entry.path();
                let media_metadata = fs::symlink_metadata(&media_path)
                    .map_err(|_| AppPathsError::CacheStorageUnavailable)?;
                if is_reparse_point(&media_metadata) || !media_metadata.is_file() {
                    return Err(AppPathsError::CacheStorageOutsideRoot);
                }
                bytes = bytes
                    .checked_add(measure_open_file(&media, &media_path)?)
                    .ok_or(AppPathsError::CacheStorageUnavailable)?;
            }
            continue;
        }
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(bytes)
}

fn measure_open_file(parent: &DirectoryGuard, path: &Path) -> Result<u64, AppPathsError> {
    let file = open_readable_file(parent, path)?;
    file.metadata()
        .map(|metadata| metadata.len())
        .map_err(|_| AppPathsError::CacheStorageUnavailable)
}

/// Captures occupied bytes without opening files that the active Processor may
/// hold exclusively. A file that disappears between enumeration and metadata
/// capture is ordinary publication churn and is omitted from this snapshot.
/// Reparse points and unexpected entry types still fail closed.
fn measure_active_project_cache(
    project: &DirectoryGuard,
    paths: &CachePathPlan,
) -> Result<u64, AppPathsError> {
    let mut bytes = 0_u64;
    for entry in
        fs::read_dir(&project.logical_path).map_err(|_| AppPathsError::CacheStorageUnavailable)?
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
        };
        let path = entry.path();
        let Some(metadata) = transient_entry_metadata(&entry)? else {
            continue;
        };
        if is_reparse_point(&metadata) {
            return Err(AppPathsError::CacheStorageOutsideRoot);
        }
        if metadata.is_file() {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            continue;
        }
        if metadata.is_dir() && path == paths.media_directory() {
            let Some(media) = open_existing_direct_child(project, &path)? else {
                continue;
            };
            for media_entry in fs::read_dir(&media.logical_path)
                .map_err(|_| AppPathsError::CacheStorageUnavailable)?
            {
                let media_entry = match media_entry {
                    Ok(entry) => entry,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                    Err(_) => return Err(AppPathsError::CacheStorageUnavailable),
                };
                let Some(metadata) = transient_entry_metadata(&media_entry)? else {
                    continue;
                };
                if is_reparse_point(&metadata) || !metadata.is_file() {
                    return Err(AppPathsError::CacheStorageOutsideRoot);
                }
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or(AppPathsError::CacheStorageUnavailable)?;
            }
            continue;
        }
        return Err(AppPathsError::CacheStorageOutsideRoot);
    }
    Ok(bytes)
}

fn transient_entry_metadata(entry: &fs::DirEntry) -> Result<Option<fs::Metadata>, AppPathsError> {
    // On Windows DirEntry returns the WIN32_FIND_DATA captured by read_dir and
    // performs no second open. That keeps active, share-denying files measurable.
    match entry.metadata() {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(AppPathsError::CacheStorageUnavailable),
    }
}

pub(crate) fn clear_project_cache(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<bool, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, plan)? else {
        return Ok(false);
    };

    clear_project_directory(project_cache.project())?;
    remove_empty_directory(project_cache.cache_parent(), &plan.root)?;
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

/// Discards every well-formed Cache temporary in a namespace whose caller has
/// already acquired the exclusive Project-namespace reservation.
///
/// The reservation is the authority: the Host acquires it before starting any
/// new Processor, and Processor lifetime is contained by that Host. Therefore
/// no matching temporary can still belong to a live writer at this point.
pub(crate) fn discard_abandoned_project_cache_temporaries(
    app_paths: &AppPaths,
    plan: &CachePathPlan,
) -> Result<usize, AppPathsError> {
    let Some(project_cache) = open_existing_project_cache(app_paths, plan)? else {
        return Ok(0);
    };

    let project = project_cache.project();
    let mut removed = discard_matching_files(project, |name| {
        metadata_temporary_process_id(name).is_some()
    })?;
    if let Some(media) = open_existing_direct_child(project, &plan.media_directory())? {
        removed +=
            discard_matching_files(&media, |name| preview_temporary_process_id(name).is_some())?;
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
            remove_guarded_file(project, &path)?;
            continue;
        }
        if metadata.is_dir() && path.file_name() == Some(std::ffi::OsStr::new("Media")) {
            let media = open_existing_direct_child(project, &path)?
                .ok_or(AppPathsError::CacheStorageUnavailable)?;
            clear_cache_files(&media)?;
            remove_empty_directory(project, &path)?;
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
        remove_guarded_file(directory, &path)?;
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
        if metadata.is_file()
            && is_temporary(&entry.file_name())
            && remove_guarded_file_if_present(directory, &path)?
        {
            removed += 1;
        }
    }
    Ok(removed)
}

fn remove_guarded_file(directory: &DirectoryGuard, path: &Path) -> Result<(), AppPathsError> {
    if remove_guarded_file_if_present(directory, path)? {
        Ok(())
    } else {
        Err(AppPathsError::CacheStorageUnavailable)
    }
}

fn remove_guarded_file_if_present(
    directory: &DirectoryGuard,
    path: &Path,
) -> Result<bool, AppPathsError> {
    let file = match open_deletable_file(directory, path) {
        Ok(file) => file,
        Err(GuardedFsError::NotFound) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    delete_open_file(directory, path, &file)?;
    Ok(true)
}

fn is_metadata_temporary_name_for(name: &std::ffi::OsStr, process_id: u32) -> bool {
    metadata_temporary_process_id(name) == Some(process_id)
}

fn metadata_temporary_process_id(name: &std::ffi::OsStr) -> Option<u32> {
    name.to_str()
        .and_then(|name| name.strip_prefix("metadata.json.tmp-"))
        .and_then(|value| value.parse::<u32>().ok())
}

fn is_preview_temporary_name_for(name: &std::ffi::OsStr, expected_process_id: u32) -> bool {
    preview_temporary_process_id(name) == Some(expected_process_id)
}

fn preview_temporary_process_id(name: &std::ffi::OsStr) -> Option<u32> {
    let (artifact, process_id) = name.to_str().and_then(|name| name.rsplit_once(".tmp-"))?;
    let components = [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png]
        .into_iter()
        .find_map(|format| artifact.strip_suffix(&format!(".{}", format.extension())))?;
    let mut components = components.split('.');
    matches!(
        (components.next(), components.next(), components.next()),
        (Some(media_id), Some(generation_id), None)
            if valid_cache_component(media_id)
                && valid_cache_component(generation_id)
    )
    .then(|| process_id.parse::<u32>().ok())
    .flatten()
}

#[cfg(all(test, windows))]
mod windows_mutation_tests {
    use std::{path::Path, process::Command};

    use super::{
        AppPaths, clear_cache_files, open_existing_direct_child, open_existing_project_cache,
        remove_empty_directory,
    };

    #[test]
    fn recursive_cleanup_mutates_only_the_held_directory_after_a_junction_swap() {
        let root = tempfile::tempdir().expect("temporary guarded cleanup root");
        let external = tempfile::tempdir().expect("external cleanup target");
        let paths = AppPaths::from_roots(root.path(), root.path());
        let plan = paths
            .project_cache("project-guarded-cleanup")
            .expect("the Cache plan is valid");
        drop(
            paths
                .prepare_cache_storage(&plan)
                .expect("the physical Cache chain exists"),
        );
        let internal = plan.media_directory().join("same-name.bin");
        std::fs::write(&internal, b"internal Cache file").expect("the internal Cache file exists");
        let external_file = external.path().join("same-name.bin");
        std::fs::write(&external_file, b"external sentinel").expect("the external sentinel exists");
        let project_cache = open_existing_project_cache(&paths, &plan)
            .expect("the Cache chain is inspectable")
            .expect("the Project Cache exists");
        let media = open_existing_direct_child(project_cache.project(), &plan.media_directory())
            .expect("the Media directory is inspectable")
            .expect("the Media directory exists");
        let displaced = replace_directory_with_junction(&plan.media_directory(), external.path());

        clear_cache_files(&media).expect("cleanup stays relative to the held Media handle");
        assert_eq!(
            std::fs::read(&external_file).expect("the external sentinel survives"),
            b"external sentinel"
        );
        assert!(!displaced.join("same-name.bin").exists());
        assert_eq!(
            remove_empty_directory(project_cache.project(), &plan.media_directory()).unwrap_err(),
            crate::guarded_fs::GuardedFsError::OutsideRoot,
            "directory removal refuses the newly visible junction"
        );

        drop(media);
        drop(project_cache);
        restore_replaced_directory(&plan.media_directory(), &displaced);
        assert!(
            paths
                .clear_project_cache(&plan)
                .expect("the restored empty Cache is removable")
        );
    }

    fn replace_directory_with_junction(directory: &Path, external: &Path) -> std::path::PathBuf {
        let displaced = directory.with_file_name(format!(
            "{}-displaced-{}",
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("the directory has one UTF-8 component"),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::rename(directory, &displaced).expect("the physical directory is displaced");
        let output = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(directory)
            .arg(external)
            .output()
            .expect("the junction command starts");
        assert!(
            output.status.success(),
            "the junction is created: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        displaced
    }

    fn restore_replaced_directory(directory: &Path, displaced: &Path) {
        std::fs::remove_dir(directory).expect("the injected junction is removed");
        std::fs::rename(displaced, directory).expect("the physical directory is restored");
    }
}
