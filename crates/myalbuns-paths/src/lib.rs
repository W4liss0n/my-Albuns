//! Descoberta centralizada dos caminhos de dados do aplicativo.

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    path::{Path, PathBuf},
};

use directories::BaseDirs;

/// Namespace temporário usado para não misturar os dados deste desenvolvimento
/// com os dados da versão anterior do MyAlbuns.
///
/// A distribuição final voltará a usar `MyAlbuns` depois que a nova versão
/// estiver concluída.
const TEMPORARY_APP_DIRECTORY_NAME: &str = "MyAlbuns2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppPathsError {
    KnownFoldersUnavailable,
}

impl Display for AppPathsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::KnownFoldersUnavailable => {
                formatter.write_str("não foi possível localizar as pastas de dados do usuário")
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
