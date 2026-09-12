pub(crate) mod commands;
mod windows;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use myalbuns_core::MediaKind;
use myalbuns_paths::{
    AppPaths, ExpectedObject, NativePathDto, OperationPathContext, PhysicalFileIdentity,
    ResolveError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ipc_contract::{
        PhotoshopCommandError, PhotoshopErrorCode, PhotoshopInstallation, PhotoshopStatus,
    },
    local_store_io::{CrossProcessStoreGuard, store_mutex_name, write_atomically},
    media_runtime::MediaBinding,
};

const SCHEMA_VERSION: u16 = 1;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

impl PhotoshopCommandError {
    pub(crate) fn new(code: PhotoshopErrorCode) -> Self {
        let message = match code {
            PhotoshopErrorCode::InstallationUnavailable => {
                "O Photoshop selecionado não está disponível. Abra Configurações para escolher ou localizar outra instalação."
            }
            PhotoshopErrorCode::InvalidInstallation => {
                "O arquivo escolhido não é uma instalação compatível do Adobe Photoshop."
            }
            PhotoshopErrorCode::OriginalAbsent => {
                "O arquivo original da Foto está ausente. Religue a Foto antes de abri-la no Photoshop."
            }
            PhotoshopErrorCode::OriginalUnavailable => {
                "Não foi possível acessar o original da Foto. Verifique o acesso e tente novamente."
            }
            PhotoshopErrorCode::InvalidContext => {
                "Selecione exatamente uma Foto ou um Frame preenchido para abrir no Photoshop."
            }
            PhotoshopErrorCode::LaunchFailed => {
                "Não foi possível iniciar o Photoshop selecionado. Abra Configurações para escolher ou localizar outra instalação."
            }
            PhotoshopErrorCode::StoreUnavailable => {
                "Não foi possível salvar a preferência do Photoshop. Tente novamente."
            }
            PhotoshopErrorCode::DialogUnavailable => {
                "Não foi possível abrir a seleção do executável do Photoshop."
            }
        };
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct ExecutableInfo {
    name: String,
    version: [u16; 4],
}

trait PhotoshopPlatform: Send + Sync {
    fn candidates(&self) -> Vec<PathBuf>;
    fn inspect(&self, executable: &Path) -> Option<ExecutableInfo>;
    fn launch(&self, executable: &Path, original: &Path) -> std::io::Result<()>;
}

struct Installation {
    path: PathBuf,
    identity: Option<PhysicalFileIdentity>,
    info: ExecutableInfo,
    id: String,
}

impl Installation {
    fn projection(&self) -> PhotoshopInstallation {
        PhotoshopInstallation {
            id: self.id.clone(),
            name: self.info.name.clone(),
            version: self
                .info
                .version
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join("."),
            path: self.path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhotoshopPreference {
    schema_version: u16,
    revision: u64,
    selected: Option<NativePathDto>,
}

/// Concrete StateStore owner for the machine's Photoshop preference.
/// Discovery and launch stay behind this boundary; no creative state is stored here.
pub(crate) struct PhotoshopStateStore {
    access: Mutex<()>,
    file: PathBuf,
    write_mutex_name: Vec<u16>,
    platform: Arc<dyn PhotoshopPlatform>,
}

impl PhotoshopStateStore {
    pub(crate) fn new(paths: &AppPaths) -> Self {
        Self {
            access: Mutex::new(()),
            file: paths.photoshop_file(),
            write_mutex_name: store_mutex_name("Photoshop", paths.local_root()),
            platform: Arc::new(windows::WindowsPhotoshop),
        }
    }

    pub(crate) fn status(&self) -> Result<PhotoshopStatus, PhotoshopCommandError> {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _writer =
            CrossProcessStoreGuard::acquire(&self.write_mutex_name, "PhotoshopStateStore")
                .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?;
        let mut preference = self.load()?;
        let installations = self.discover(&preference);
        let selected = preference
            .selected
            .as_ref()
            .and_then(|path| {
                installations
                    .iter()
                    .find(|installation| installation.path == path.as_path())
            })
            .or_else(|| installations.first());
        if let Some(selected) = selected
            && preference.selected.as_ref().map(NativePathDto::as_path)
                != Some(selected.path.as_path())
        {
            self.publish_selection(&mut preference, selected.path.clone())?;
        }
        Ok(PhotoshopStatus {
            revision: preference.revision,
            selected_installation_id: selected.map(|installation| installation.id.clone()),
            installations: installations.iter().map(Installation::projection).collect(),
        })
    }

    pub(crate) fn select(&self, id: &str) -> Result<PhotoshopStatus, PhotoshopCommandError> {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _writer =
            CrossProcessStoreGuard::acquire(&self.write_mutex_name, "PhotoshopStateStore")
                .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?;
        let mut preference = self.load()?;
        let installations = self.discover(&preference);
        let selected = installations
            .iter()
            .find(|installation| installation.id == id)
            .ok_or_else(|| {
                PhotoshopCommandError::new(PhotoshopErrorCode::InstallationUnavailable)
            })?;
        self.publish_selection(&mut preference, selected.path.clone())?;
        Ok(PhotoshopStatus {
            revision: preference.revision,
            selected_installation_id: Some(selected.id.clone()),
            installations: installations.iter().map(Installation::projection).collect(),
        })
    }

    pub(crate) fn choose(&self, path: PathBuf) -> Result<PhotoshopStatus, PhotoshopCommandError> {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selected = self
            .inspect(&path)
            .ok_or_else(|| PhotoshopCommandError::new(PhotoshopErrorCode::InvalidInstallation))?;
        let _writer =
            CrossProcessStoreGuard::acquire(&self.write_mutex_name, "PhotoshopStateStore")
                .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?;
        let mut preference = self.load()?;
        self.publish_selection(&mut preference, selected.path.clone())?;
        let installations = self.discover(&preference);
        Ok(PhotoshopStatus {
            revision: preference.revision,
            selected_installation_id: Some(selected.id),
            installations: installations.iter().map(Installation::projection).collect(),
        })
    }

    pub(crate) fn open_original(
        &self,
        binding: &MediaBinding,
    ) -> Result<(), PhotoshopCommandError> {
        if binding.kind != MediaKind::Photo {
            return Err(PhotoshopCommandError::new(
                PhotoshopErrorCode::InvalidContext,
            ));
        }
        // Opening never substitutes another installation if the previously selected one vanished.
        let preference = self.load()?;
        let executable = preference.selected.ok_or_else(|| {
            PhotoshopCommandError::new(PhotoshopErrorCode::InstallationUnavailable)
        })?;
        let mut context = OperationPathContext::new();
        let resolved_executable = context
            .resolve_existing(executable.as_path(), ExpectedObject::RegularFile)
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::InstallationUnavailable))?;
        if self
            .platform
            .inspect(resolved_executable.operational_path())
            .is_none()
        {
            return Err(PhotoshopCommandError::new(
                PhotoshopErrorCode::InstallationUnavailable,
            ));
        }
        let original = context
            .resolve_existing(&binding.logical_path, ExpectedObject::RegularFile)
            .map_err(|error| {
                PhotoshopCommandError::new(match error {
                    ResolveError::NotFound => PhotoshopErrorCode::OriginalAbsent,
                    _ => PhotoshopErrorCode::OriginalUnavailable,
                })
            })?;
        let _readable_original = original
            .reopen_for_read()
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::OriginalUnavailable))?;
        // The native launcher receives only these two resolved paths, never a Cache representation.
        self.platform
            .launch(
                resolved_executable.operational_path(),
                original.operational_path(),
            )
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::LaunchFailed))
    }

    fn inspect(&self, path: &Path) -> Option<Installation> {
        let mut context = OperationPathContext::new();
        let resolved = context
            .resolve_existing(path, ExpectedObject::RegularFile)
            .ok()?;
        let info = self.platform.inspect(resolved.operational_path())?;
        let encoded = serde_json::to_vec(&NativePathDto::from(path.to_path_buf())).ok()?;
        let id = format!("{:x}", Sha256::digest(encoded));
        Some(Installation {
            path: path.to_path_buf(),
            identity: resolved.physical_identity(),
            info,
            id,
        })
    }

    fn discover(&self, preference: &PhotoshopPreference) -> Vec<Installation> {
        // Put the explicit preference first so an alias of it does not replace its identity in the UI.
        let candidates = preference
            .selected
            .iter()
            .map(|path| path.as_path().to_path_buf())
            .chain(self.platform.candidates());
        let mut installations: Vec<Installation> = Vec::new();
        for path in candidates {
            if installations.iter().any(|entry| entry.path == path) {
                continue;
            }
            let Some(entry) = self.inspect(&path) else {
                continue;
            };
            if installations
                .iter()
                .any(|existing| existing.identity.is_some() && existing.identity == entry.identity)
            {
                continue;
            }
            installations.push(entry);
        }
        installations.sort_by(|left, right| {
            right
                .info
                .version
                .cmp(&left.info.version)
                .then_with(|| left.path.cmp(&right.path))
        });
        installations
    }

    fn load(&self) -> Result<PhotoshopPreference, PhotoshopCommandError> {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PhotoshopPreference::default());
            }
            Err(_) => {
                return Err(PhotoshopCommandError::new(
                    PhotoshopErrorCode::StoreUnavailable,
                ));
            }
        };
        Ok(serde_json::from_slice::<PhotoshopPreference>(&bytes)
            .ok()
            .filter(|preference| {
                preference.schema_version == SCHEMA_VERSION && preference.revision <= MAX_REVISION
            })
            .unwrap_or_default())
    }

    fn publish_selection(
        &self,
        preference: &mut PhotoshopPreference,
        path: PathBuf,
    ) -> Result<(), PhotoshopCommandError> {
        if preference.selected.as_ref().map(NativePathDto::as_path) == Some(path.as_path()) {
            return Ok(());
        }
        preference.schema_version = SCHEMA_VERSION;
        preference.revision = preference
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_REVISION)
            .ok_or_else(|| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?;
        preference.selected = Some(path.into());
        let bytes = serde_json::to_vec_pretty(preference)
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))?;
        write_atomically(&self.file, &bytes, "photoshop.json")
            .map_err(|_| PhotoshopCommandError::new(PhotoshopErrorCode::StoreUnavailable))
    }
}

#[cfg(test)]
mod tests;
