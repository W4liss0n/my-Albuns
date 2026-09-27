use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use myalbuns_paths::{
    NativePathDto, project_data_namespace, publish_new_file, replace_existing_file,
    validate_external_path,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const IDENTITY_RECORD_SCHEMA_VERSION: u32 = 1;
const LEGACY_PENDING_DIRECTORY: &str = "legacy-pending";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IdentityRegistryLookup {
    Missing,
    Location(PathBuf),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityRegistryError {
    Corrupt,
    Unavailable,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectIdentityRegistry {
    root: PathBuf,
}

impl ProjectIdentityRegistry {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn lookup(
        &self,
        project_id: Uuid,
    ) -> Result<IdentityRegistryLookup, IdentityRegistryError> {
        let target = self.record_path(project_id);
        let bytes = match fs::read(target) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(IdentityRegistryLookup::Missing);
            }
            Err(_) => return Err(IdentityRegistryError::Unavailable),
        };
        let record: ProjectIdentityRecord =
            serde_json::from_slice(&bytes).map_err(|_| IdentityRegistryError::Corrupt)?;
        if record.schema_version != IDENTITY_RECORD_SCHEMA_VERSION
            || record.project_id != project_id.hyphenated().to_string()
            || validate_external_path(record.location.as_path()).is_err()
        {
            return Err(IdentityRegistryError::Corrupt);
        }
        Ok(IdentityRegistryLookup::Location(
            record.location.into_path_buf(),
        ))
    }

    pub(crate) fn publish(
        &self,
        project_id: Uuid,
        location: &Path,
    ) -> Result<(), IdentityRegistryError> {
        validate_external_path(location).map_err(|_| IdentityRegistryError::Corrupt)?;
        fs::create_dir_all(&self.root).map_err(|_| IdentityRegistryError::Unavailable)?;
        let record = ProjectIdentityRecord {
            schema_version: IDENTITY_RECORD_SCHEMA_VERSION,
            project_id: project_id.hyphenated().to_string(),
            location: NativePathDto::from_path(location),
        };
        let mut bytes =
            serde_json::to_vec_pretty(&record).map_err(|_| IdentityRegistryError::Unavailable)?;
        bytes.push(b'\n');

        let target = self.record_path(project_id);
        let temporary = TemporaryRecord::new(self.root.join(format!(
            ".{}.{}.tmp",
            project_data_namespace(&project_id.hyphenated().to_string()),
            Uuid::new_v4().hyphenated()
        )));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary.path())
            .map_err(|_| IdentityRegistryError::Unavailable)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| IdentityRegistryError::Unavailable)?;
        drop(file);

        match fs::metadata(&target) {
            Ok(metadata) if metadata.is_file() => {
                replace_existing_file(temporary.path(), &target)
                    .map_err(|_| IdentityRegistryError::Unavailable)?;
            }
            Ok(_) => return Err(IdentityRegistryError::Corrupt),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                publish_new_file(temporary.path(), &target)
                    .map_err(|_| IdentityRegistryError::Unavailable)?;
            }
            Err(_) => return Err(IdentityRegistryError::Unavailable),
        }
        if fs::read(&target).map_err(|_| IdentityRegistryError::Unavailable)? != bytes {
            return Err(IdentityRegistryError::Unavailable);
        }
        Ok(())
    }

    /// The Identity an old myAlbuns Project uses until its first save. It is
    /// created once per file content and location, so a Recovery written
    /// before a crash finds the same Identity when the file is reopened.
    pub(crate) fn legacy_pending_identity(&self, key: &str) -> Result<Uuid, IdentityRegistryError> {
        let directory = self.root.join(LEGACY_PENDING_DIRECTORY);
        fs::create_dir_all(&directory).map_err(|_| IdentityRegistryError::Unavailable)?;
        let path = directory.join(format!("{key}.id"));
        let candidate = Uuid::new_v4();
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(candidate.hyphenated().to_string().as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|_| {
                        let _ = fs::remove_file(&path);
                        IdentityRegistryError::Unavailable
                    })?;
                Ok(candidate)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let text =
                    fs::read_to_string(&path).map_err(|_| IdentityRegistryError::Unavailable)?;
                Uuid::parse_str(text.trim())
                    .ok()
                    .filter(|id| {
                        id.get_version_num() == 4 && id.hyphenated().to_string() == text.trim()
                    })
                    .ok_or(IdentityRegistryError::Corrupt)
            }
            Err(_) => Err(IdentityRegistryError::Unavailable),
        }
    }

    /// The pending Identity of an old Project, without creating one.
    pub(crate) fn find_legacy_pending_identity(&self, key: &str) -> Option<Uuid> {
        let path = self
            .root
            .join(LEGACY_PENDING_DIRECTORY)
            .join(format!("{key}.id"));
        let text = fs::read_to_string(path).ok()?;
        Uuid::parse_str(text.trim())
            .ok()
            .filter(|id| id.get_version_num() == 4 && id.hyphenated().to_string() == text.trim())
    }

    /// Forgets the pending Identity once the file carries it.
    pub(crate) fn forget_legacy_pending_identity(&self, key: &str) {
        let _ = fs::remove_file(
            self.root
                .join(LEGACY_PENDING_DIRECTORY)
                .join(format!("{key}.id")),
        );
    }

    fn record_path(&self, project_id: Uuid) -> PathBuf {
        self.root.join(format!(
            "{}.json",
            project_data_namespace(&project_id.hyphenated().to_string())
        ))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectIdentityRecord {
    schema_version: u32,
    project_id: String,
    location: NativePathDto,
}

struct TemporaryRecord {
    path: PathBuf,
}

impl TemporaryRecord {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryRecord {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
