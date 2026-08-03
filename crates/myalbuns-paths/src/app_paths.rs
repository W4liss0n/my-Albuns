use std::path::{Path, PathBuf};

use directories::BaseDirs;
use sha2::{Digest, Sha256};

use crate::{
    AppPathsError, CachePathPlan, PreparedCacheStorage,
    cache::{clear_project_cache, discard_project_cache_temporaries, prepare_cache_storage},
};

/// Namespace temporário usado para não misturar os dados deste desenvolvimento
/// com os dados da versão anterior do MyAlbuns.
///
/// A distribuição final voltará a usar `MyAlbuns` depois que a nova versão
/// estiver concluída.
const TEMPORARY_APP_DIRECTORY_NAME: &str = "MyAlbuns2";

/// Deriva uma representação opaca, estável e segura da Identidade do Projeto
/// para diretórios internos. A Identidade permanece um valor de domínio livre
/// de restrições de nome de arquivo; somente esta representação chega ao disco.
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
            temporary_root: std::env::temp_dir().join(TEMPORARY_APP_DIRECTORY_NAME),
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

    pub fn webview_data_directory(&self, host_namespace: &str) -> Result<PathBuf, AppPathsError> {
        if !valid_namespace_component(host_namespace) {
            return Err(AppPathsError::InvalidStateNamespace);
        }
        Ok(self.state_dir().join("WebView2").join(host_namespace))
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.local_root.join("Logs")
    }

    pub fn prepare_export_preview_directory(&self) -> Result<PathBuf, AppPathsError> {
        let directory = self.temporary_root.join("ExportPreview");
        std::fs::create_dir_all(&directory).map_err(|_| AppPathsError::ExportStorageUnavailable)?;
        if !directory.is_dir() {
            return Err(AppPathsError::ExportStorageUnavailable);
        }
        Ok(directory)
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
