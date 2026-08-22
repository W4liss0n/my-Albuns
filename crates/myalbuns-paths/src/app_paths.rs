use std::path::{Path, PathBuf};

use directories::BaseDirs;
use sha2::{Digest, Sha256};

use crate::{
    AppPathsError, CachePathPlan, CacheWriterClaimStorage, PreparedCacheStorage,
    cache::{
        CacheNamespaceUsage, clear_project_cache, discard_abandoned_project_cache_temporaries,
        discard_project_cache_temporaries, inspect_cache_namespace, list_cache_namespaces,
        open_cache_writer_claim_storage, prepare_cache_storage, snapshot_active_cache_namespace,
    },
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
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppPathsError> {
        #[cfg(debug_assertions)]
        if let Some(root) = debug_process_gate_root()? {
            return Ok(Self::from_roots(&root.join("Roaming"), &root.join("Local")));
        }

        let known_folders = BaseDirs::new().ok_or(AppPathsError::KnownFoldersUnavailable)?;
        Ok(Self::from_roots(
            known_folders.data_dir(),
            known_folders.data_local_dir(),
        ))
    }

    pub fn from_roots(roaming_data: &Path, local_data: &Path) -> Self {
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

    pub fn open_cache_writer_claim_storage(
        &self,
        plan: &CachePathPlan,
    ) -> Result<Option<CacheWriterClaimStorage>, AppPathsError> {
        open_cache_writer_claim_storage(self, plan)
    }

    pub fn clear_project_cache(&self, plan: &CachePathPlan) -> Result<bool, AppPathsError> {
        clear_project_cache(self, plan)
    }

    pub fn list_cache_namespaces(&self) -> Result<Vec<CachePathPlan>, AppPathsError> {
        list_cache_namespaces(self)
    }

    pub fn inspect_cache_namespace(
        &self,
        plan: &CachePathPlan,
    ) -> Result<Option<CacheNamespaceUsage>, AppPathsError> {
        inspect_cache_namespace(self, plan)
    }

    /// Returns a non-authoritative occupied-byte snapshot for a namespace whose
    /// external owner is still active. This value is display-only: callers must
    /// acquire exclusive ownership and use `inspect_cache_namespace` before it
    /// can authorize release or deletion.
    pub fn snapshot_active_cache_namespace(
        &self,
        plan: &CachePathPlan,
    ) -> Result<Option<CacheNamespaceUsage>, AppPathsError> {
        snapshot_active_cache_namespace(self, plan)
    }

    pub fn discard_project_cache_temporaries(
        &self,
        plan: &CachePathPlan,
        process_id: u32,
    ) -> Result<usize, AppPathsError> {
        discard_project_cache_temporaries(self, plan, process_id)
    }

    pub fn discard_abandoned_project_cache_temporaries(
        &self,
        plan: &CachePathPlan,
    ) -> Result<usize, AppPathsError> {
        discard_abandoned_project_cache_temporaries(self, plan)
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.local_root.join("Recovery")
    }

    pub fn project_recovery_checkpoint(
        &self,
        project_namespace: &str,
    ) -> Result<PathBuf, AppPathsError> {
        if !valid_namespace_component(project_namespace) {
            return Err(AppPathsError::InvalidStateNamespace);
        }
        Ok(self
            .recovery_dir()
            .join("Projects")
            .join(format!("{project_namespace}.json")))
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

    pub fn project_identities_dir(&self) -> PathBuf {
        self.state_dir().join("ProjectIdentities")
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
