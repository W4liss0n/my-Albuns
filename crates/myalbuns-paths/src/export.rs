use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use crate::{
    AppPathsError,
    app_paths::valid_cache_component,
    guarded_fs::{
        DirectoryGuard, GuardedFsError, is_direct_physical_child, is_reparse_point, open_directory,
        validate_open_file,
    },
    operation::validate_external_path,
    publish_new_file, replace_existing_file,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportWriteAuthorization {
    CreateOnly,
    ReplaceConfirmed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportPathPlan {
    output_path: PathBuf,
    preparation_directory: PathBuf,
    prepared_output_path: PathBuf,
    authorization: ExportWriteAuthorization,
}

/// Keeps the Export destination and its operation-specific preparation open.
#[derive(Debug)]
pub struct PreparedExportStorage {
    destination: DirectoryGuard,
    preparation: DirectoryGuard,
    plan: ExportPathPlan,
}

impl ExportPathPlan {
    pub fn new(output_path: PathBuf, operation_id: &str) -> Result<Self, AppPathsError> {
        Self::new_authorized(
            output_path,
            operation_id,
            ExportWriteAuthorization::CreateOnly,
        )
    }

    pub fn new_authorized(
        output_path: PathBuf,
        operation_id: &str,
        authorization: ExportWriteAuthorization,
    ) -> Result<Self, AppPathsError> {
        if validate_external_path(&output_path).is_err() || !valid_cache_component(operation_id) {
            return Err(AppPathsError::InvalidExportPath);
        }
        let destination = output_path
            .parent()
            .ok_or(AppPathsError::InvalidExportPath)?;
        let output_name = output_path
            .file_name()
            .ok_or(AppPathsError::InvalidExportPath)?;
        let preparation_directory =
            destination.join(format!(".myalbuns-export-{operation_id}.tmp"));
        let prepared_output_path = preparation_directory.join(output_name);
        Ok(Self {
            output_path,
            preparation_directory,
            prepared_output_path,
            authorization,
        })
    }

    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub fn preparation_directory(&self) -> &Path {
        &self.preparation_directory
    }

    pub fn prepared_output_path(&self) -> &Path {
        &self.prepared_output_path
    }

    pub fn authorization(&self) -> ExportWriteAuthorization {
        self.authorization
    }

    pub fn prepare(&self) -> Result<PreparedExportStorage, AppPathsError> {
        let destination_path = self
            .output_path
            .parent()
            .ok_or(AppPathsError::InvalidExportPath)?;
        let destination = open_directory(destination_path).map_err(export_storage_error)?;
        let preparation =
            create_unique_export_directory(&destination, &self.preparation_directory)?;
        Ok(PreparedExportStorage {
            destination,
            preparation,
            plan: self.clone(),
        })
    }
}

impl PreparedExportStorage {
    pub fn publish(self) -> Result<(), AppPathsError> {
        let validation = (|| {
            let prepared =
                open_export_file(&self.preparation, &self.plan.prepared_output_path, false)?
                    .ok_or(AppPathsError::ExportStorageUnavailable)?;
            drop(prepared);
            let final_exists = if let Some(existing) =
                open_export_file(&self.destination, &self.plan.output_path, true)?
            {
                drop(existing);
                true
            } else {
                false
            };
            if final_exists && self.plan.authorization == ExportWriteAuthorization::CreateOnly {
                return Err(AppPathsError::ExportTargetConflict);
            }
            Ok(final_exists)
        })();
        let final_exists = match validation {
            Ok(final_exists) => final_exists,
            Err(error) => {
                let _ = self.discard();
                return Err(error);
            }
        };

        let publication = match (self.plan.authorization, final_exists) {
            (ExportWriteAuthorization::ReplaceConfirmed, true) => {
                replace_existing_file(&self.plan.prepared_output_path, &self.plan.output_path)
            }
            _ => publish_new_file(&self.plan.prepared_output_path, &self.plan.output_path),
        };
        if publication.is_err() {
            let _ = self.discard();
            return Err(AppPathsError::ExportStorageUnavailable);
        }
        let preparation_directory = self.plan.preparation_directory.clone();
        drop(self);
        // A remaining empty directory is a disposable orphan and does not undo Publicação.
        let _ = fs::remove_dir(preparation_directory);
        Ok(())
    }

    pub fn discard(self) -> Result<bool, AppPathsError> {
        let mut removed = false;
        for entry in fs::read_dir(&self.preparation.logical_path)
            .map_err(|_| AppPathsError::ExportStorageUnavailable)?
        {
            let entry = entry.map_err(|_| AppPathsError::ExportStorageUnavailable)?;
            let path = entry.path();
            if path != self.plan.prepared_output_path {
                return Err(AppPathsError::ExportStorageOutsideDestination);
            }
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| AppPathsError::ExportStorageUnavailable)?;
            if is_reparse_point(&metadata) || !metadata.is_file() {
                return Err(AppPathsError::ExportStorageOutsideDestination);
            }
            fs::remove_file(path).map_err(|_| AppPathsError::ExportStorageUnavailable)?;
            removed = true;
        }
        let preparation_directory = self.plan.preparation_directory.clone();
        drop(self);
        fs::remove_dir(preparation_directory)
            .map_err(|_| AppPathsError::ExportStorageUnavailable)?;
        Ok(removed)
    }
}

fn create_unique_export_directory(
    destination: &DirectoryGuard,
    preparation_path: &Path,
) -> Result<DirectoryGuard, AppPathsError> {
    if preparation_path.parent() != Some(destination.logical_path.as_path()) {
        return Err(AppPathsError::ExportStorageOutsideDestination);
    }
    fs::create_dir(preparation_path).map_err(|_| AppPathsError::ExportStorageUnavailable)?;
    let metadata = fs::symlink_metadata(preparation_path)
        .map_err(|_| AppPathsError::ExportStorageUnavailable)?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(AppPathsError::ExportStorageOutsideDestination);
    }
    let preparation = open_directory(preparation_path).map_err(export_storage_error)?;
    let expected_name = preparation_path
        .file_name()
        .ok_or(AppPathsError::ExportStorageOutsideDestination)?;
    if !is_direct_physical_child(
        &destination.physical_path,
        &preparation.physical_path,
        expected_name,
    ) {
        return Err(AppPathsError::ExportStorageOutsideDestination);
    }
    Ok(preparation)
}

fn open_export_file(
    parent: &DirectoryGuard,
    path: &Path,
    allow_absent: bool,
) -> Result<Option<File>, AppPathsError> {
    if path.parent() != Some(parent.logical_path.as_path()) {
        return Err(AppPathsError::ExportStorageOutsideDestination);
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if allow_absent && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(_) => return Err(AppPathsError::ExportStorageUnavailable),
    };
    if is_reparse_point(&metadata) || !metadata.is_file() {
        return Err(AppPathsError::ExportStorageOutsideDestination);
    }
    let file = File::open(path).map_err(|_| AppPathsError::ExportStorageUnavailable)?;
    validate_open_file(parent, path, &file).map_err(export_storage_error)?;
    Ok(Some(file))
}

fn export_storage_error(error: GuardedFsError) -> AppPathsError {
    match error {
        GuardedFsError::OutsideRoot => AppPathsError::ExportStorageOutsideDestination,
        GuardedFsError::AlreadyExists | GuardedFsError::NotFound | GuardedFsError::Unavailable => {
            AppPathsError::ExportStorageUnavailable
        }
    }
}
