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
