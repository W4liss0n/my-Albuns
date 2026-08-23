use std::{fs, io, path::PathBuf};

use myalbuns_paths::{AppPaths, project_data_namespace};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct ProjectRecoveryCheckpoints {
    app_paths: AppPaths,
}

impl ProjectRecoveryCheckpoints {
    pub(crate) fn new(app_paths: AppPaths) -> Self {
        Self { app_paths }
    }

    pub(crate) fn checkpoint_path(&self, project_id: Uuid) -> io::Result<PathBuf> {
        self.app_paths
            .project_recovery_checkpoint(&project_data_namespace(
                &project_id.hyphenated().to_string(),
            ))
            .map_err(io::Error::other)
    }

    /// Completes only the identity-transition seam owned by Save As.
    /// Creation and periodic publication remain in the Recovery feature.
    pub(crate) fn finish_previous_checkpoint(&self, project_id: Uuid) -> io::Result<bool> {
        let checkpoint = self.checkpoint_path(project_id)?;
        let metadata = match fs::symlink_metadata(&checkpoint) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "o checkpoint de Recuperação não é um arquivo regular",
            ));
        }
        fs::remove_file(checkpoint)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectRecoveryCheckpoints;
    use myalbuns_paths::AppPaths;
    use uuid::Uuid;

    fn checkpoints(root: &std::path::Path) -> ProjectRecoveryCheckpoints {
        ProjectRecoveryCheckpoints::new(AppPaths::from_roots(
            &root.join("roaming"),
            &root.join("local"),
        ))
    }

    #[test]
    fn successful_identity_transition_finishes_only_the_previous_checkpoint() {
        let root = tempfile::tempdir().expect("temporary Recovery fixture");
        let checkpoints = checkpoints(root.path());
        let previous_id = Uuid::new_v4();
        let next_id = Uuid::new_v4();
        let previous = checkpoints
            .checkpoint_path(previous_id)
            .expect("the previous checkpoint path is valid");
        let next = checkpoints
            .checkpoint_path(next_id)
            .expect("the next checkpoint path is valid");
        std::fs::create_dir_all(previous.parent().expect("the checkpoint has a parent"))
            .expect("the Recovery directory exists");
        std::fs::write(&previous, b"previous recovery").expect("the previous checkpoint exists");
        std::fs::write(&next, b"next recovery").expect("the next checkpoint exists");

        assert!(
            checkpoints
                .finish_previous_checkpoint(previous_id)
                .expect("the previous checkpoint is finished")
        );
        assert!(!previous.exists());
        assert_eq!(
            std::fs::read(next).expect("the unrelated checkpoint remains"),
            b"next recovery"
        );
    }

    #[test]
    fn a_non_file_checkpoint_fails_closed_and_is_preserved() {
        let root = tempfile::tempdir().expect("temporary Recovery failure fixture");
        let checkpoints = checkpoints(root.path());
        let previous_id = Uuid::new_v4();
        let previous = checkpoints
            .checkpoint_path(previous_id)
            .expect("the previous checkpoint path is valid");
        std::fs::create_dir_all(&previous).expect("a conflicting directory occupies the path");

        assert!(checkpoints.finish_previous_checkpoint(previous_id).is_err());
        assert!(previous.is_dir());
    }
}
