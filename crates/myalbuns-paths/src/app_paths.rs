use std::path::{Path, PathBuf};

use directories::BaseDirs;
use sha2::{Digest, Sha256};

use crate::{
    AppPathsError, CachePathPlan, PreparedCacheStorage,
    cache::{clear_project_cache, discard_project_cache_temporaries, prepare_cache_storage},
    guarded_fs::{DirectoryGuard, GuardedFsError, ensure_direct_child, open_directory},
};

/// Temporary namespace that keeps this development version's data separate
/// from data belonging to the previous MyAlbuns version.
///
/// The final distribution will return to `MyAlbuns` after the new version is
/// complete.
const TEMPORARY_APP_DIRECTORY_NAME: &str = "MyAlbuns2";

/// Derives an opaque, stable, and safe representation of the Identidade do Projeto
/// for internal directories. The Identidade remains a domain value free from
/// file-name restrictions; only this representation reaches the filesystem.
pub fn project_data_namespace(project_id: &str) -> String {
    opaque_data_key("project", project_id)
}

pub(crate) fn media_cache_key(media_id: &str) -> String {
    opaque_data_key("media", media_id)
}

fn opaque_data_key(kind: &str, identity: &str) -> String {
    let digest = Sha256::digest(identity.as_bytes());
    format!("{kind}-{digest:x}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub(crate) roaming_root: PathBuf,
    pub(crate) local_root: PathBuf,
    temporary_root: PathBuf,
}

/// Keeps the temporary application namespace and preview directory physically
/// contained while an Export preview is planned and published.
#[derive(Debug)]
pub struct PreparedExportPreviewDirectory {
    _temporary_base: DirectoryGuard,
    _application_root: DirectoryGuard,
    preview: DirectoryGuard,
}

impl PreparedExportPreviewDirectory {
    pub fn path(&self) -> &Path {
        &self.preview.logical_path
    }
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathsError> {
        #[cfg(debug_assertions)]
        if let Some(root) = debug_process_gate_root()? {
            return Ok(Self::from_roots(
                &root.join("Roaming"),
                &root.join("Local"),
                &root.join("Temporary"),
            ));
        }

        let known_folders = BaseDirs::new().ok_or(AppPathsError::KnownFoldersUnavailable)?;
        Ok(Self::from_roots(
            known_folders.data_dir(),
            known_folders.data_local_dir(),
            &std::env::temp_dir(),
        ))
    }

    pub fn from_roots(roaming_data: &Path, local_data: &Path, temporary_data: &Path) -> Self {
        Self {
            roaming_root: roaming_data.join(TEMPORARY_APP_DIRECTORY_NAME),
            local_root: local_data.join(TEMPORARY_APP_DIRECTORY_NAME),
            temporary_root: temporary_data.join(TEMPORARY_APP_DIRECTORY_NAME),
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
        if !valid_namespace_component(project_namespace) {
            return Err(AppPathsError::InvalidProjectNamespace);
        }
        Ok(CachePathPlan::from_root(
            self.cache_dir().join(project_namespace),
        ))
    }

    pub fn validate_cache_artifact(&self, cache_file: &Path) -> Result<(), AppPathsError> {
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
        Ok(())
    }

    pub fn prepare_cache_storage(
        &self,
        plan: &CachePathPlan,
    ) -> Result<PreparedCacheStorage, AppPathsError> {
        prepare_cache_storage(self, plan)
    }

    pub fn clear_project_cache(&self, plan: &CachePathPlan) -> Result<bool, AppPathsError> {
        clear_project_cache(self, plan)
    }

    pub fn discard_project_cache_temporaries(
        &self,
        plan: &CachePathPlan,
        process_id: u32,
    ) -> Result<usize, AppPathsError> {
        discard_project_cache_temporaries(self, plan, process_id)
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.local_root.join("Recovery")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.local_root.join("State")
    }

    pub fn recent_projects_file(&self) -> PathBuf {
        self.state_dir().join("recent-projects.json")
    }

    pub fn project_identity_leases_dir(&self) -> PathBuf {
        self.state_dir().join("ProjectIdentityLeases")
    }

    pub fn webview_data_directory(&self, host_namespace: &str) -> Result<PathBuf, AppPathsError> {
        if !valid_namespace_component(host_namespace) {
            return Err(AppPathsError::InvalidStateNamespace);
        }
        Ok(self.state_dir().join("WebView2").join(host_namespace))
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.local_root.join("Logs")
    }

    pub fn prepare_export_preview_directory(
        &self,
    ) -> Result<PreparedExportPreviewDirectory, AppPathsError> {
        let temporary_base_path = self
            .temporary_root
            .parent()
            .ok_or(AppPathsError::ExportStorageUnavailable)?;
        let temporary_base =
            open_directory(temporary_base_path).map_err(export_preview_storage_error)?;
        let application_root = ensure_direct_child(&temporary_base, &self.temporary_root)
            .map_err(export_preview_storage_error)?;
        let preview_path = self.temporary_root.join("ExportPreview");
        let preview = ensure_direct_child(&application_root, &preview_path)
            .map_err(export_preview_storage_error)?;
        Ok(PreparedExportPreviewDirectory {
            _temporary_base: temporary_base,
            _application_root: application_root,
            preview,
        })
    }
}

/// Isolates real-process integration gates from the user's application data.
///
/// This override is intentionally absent from release builds. It configures
/// only internal data roots controlled by the test runner; Project pathnames
/// continue to cross their native DTO boundary unchanged.
#[cfg(debug_assertions)]
fn debug_process_gate_root() -> Result<Option<PathBuf>, AppPathsError> {
    const PROCESS_GATE_ROOT_ENV: &str = "MYALBUNS_PROCESS_GATE_DATA_ROOT";

    let Some(root) = std::env::var_os(PROCESS_GATE_ROOT_ENV).map(PathBuf::from) else {
        return Ok(None);
    };
    if !root.is_absolute() {
        return Err(AppPathsError::KnownFoldersUnavailable);
    }
    Ok(Some(root))
}

fn export_preview_storage_error(error: GuardedFsError) -> AppPathsError {
    match error {
        GuardedFsError::Unavailable => AppPathsError::ExportStorageUnavailable,
        GuardedFsError::OutsideRoot => AppPathsError::ExportStorageOutsideDestination,
    }
}

pub(crate) fn valid_namespace_component(value: &str) -> bool {
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

pub(crate) fn valid_cache_component(value: &str) -> bool {
    valid_namespace_component(value)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
