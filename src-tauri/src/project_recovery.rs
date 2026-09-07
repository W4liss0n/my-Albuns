use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use myalbuns_core::{ProjectIdentityAuthority, RecoveryCheckpoint};
use myalbuns_paths::{AppPaths, project_data_namespace, publish_new_file, replace_existing_file};
use uuid::Uuid;

const CHECKPOINT_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub(crate) struct RecoveryStore {
    app_paths: AppPaths,
}

impl RecoveryStore {
    pub(crate) fn new(app_paths: AppPaths) -> Self {
        Self { app_paths }
    }

    pub(crate) fn checkpoint_path(
        &self,
        authority: &ProjectIdentityAuthority,
    ) -> io::Result<PathBuf> {
        self.app_paths
            .project_recovery_checkpoint(&project_data_namespace(
                &authority.project_id().hyphenated().to_string(),
            ))
            .map_err(io::Error::other)
    }

    pub(crate) fn load(
        &self,
        authority: &ProjectIdentityAuthority,
    ) -> io::Result<Option<RecoveryCheckpoint>> {
        let path = self.checkpoint_path(authority)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        require_regular_file(&metadata)?;
        let checkpoint = RecoveryCheckpoint::from_bytes(&fs::read(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if checkpoint.project_id() != authority.project_id() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "o checkpoint pertence a outra Identidade de Projeto",
            ));
        }
        Ok(Some(checkpoint))
    }

    pub(crate) fn publish(
        &self,
        authority: &ProjectIdentityAuthority,
        checkpoint: &RecoveryCheckpoint,
    ) -> io::Result<()> {
        if checkpoint.project_id() != authority.project_id() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a autoridade não corresponde ao checkpoint de Recuperação",
            ));
        }
        let target = self.checkpoint_path(authority)?;
        let parent = target
            .parent()
            .ok_or_else(|| io::Error::other("o checkpoint não possui diretório pai"))?;
        fs::create_dir_all(parent)?;
        let temporary = TemporaryCheckpoint::new(parent.join(format!(
            ".{}.{}.tmp",
            target
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(|| io::Error::other("o nome do checkpoint é inválido"))?,
            Uuid::new_v4().simple()
        )));
        let bytes = checkpoint
            .to_bytes()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary.path())?;
        file.write_all(&bytes).and_then(|()| file.sync_all())?;
        drop(file);

        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                require_regular_file(&metadata)?;
                replace_existing_file(temporary.path(), &target)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                publish_new_file(temporary.path(), &target)?;
            }
            Err(error) => return Err(error),
        }
        if fs::read(&target)? != bytes {
            return Err(io::Error::other(
                "o checkpoint publicado não corresponde ao estado consolidado",
            ));
        }
        Ok(())
    }

    pub(crate) fn finish(&self, authority: &ProjectIdentityAuthority) -> io::Result<bool> {
        self.finish_path(self.checkpoint_path(authority)?)
    }

    fn finish_path(&self, path: PathBuf) -> io::Result<bool> {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        require_regular_file(&metadata)?;
        fs::remove_file(path)?;
        Ok(true)
    }
}

fn require_regular_file(metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(io::Error::other(
            "o checkpoint de Recuperação não é um arquivo regular",
        ))
    }
}

struct TemporaryCheckpoint {
    path: PathBuf,
}

impl TemporaryCheckpoint {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryCheckpoint {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone)]
pub(crate) struct RecoveryCoordinator {
    store: RecoveryStore,
    delay: Duration,
    state: Arc<Mutex<PendingRecoveryState>>,
    io: Arc<Mutex<()>>,
}

#[derive(Default)]
struct PendingRecoveryState {
    generation: u64,
    pending: Option<PendingCheckpoint>,
}

struct PendingCheckpoint {
    generation: u64,
    authority: ProjectIdentityAuthority,
    checkpoint: RecoveryCheckpoint,
}

impl RecoveryCoordinator {
    pub(crate) fn new(store: RecoveryStore) -> Self {
        Self::with_delay(store, CHECKPOINT_DELAY)
    }

    pub(crate) fn with_delay(store: RecoveryStore, delay: Duration) -> Self {
        Self {
            store,
            delay,
            state: Arc::new(Mutex::new(PendingRecoveryState::default())),
            io: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) fn load(
        &self,
        authority: &ProjectIdentityAuthority,
    ) -> io::Result<Option<RecoveryCheckpoint>> {
        self.store.load(authority)
    }

    pub(crate) fn schedule(
        &self,
        authority: ProjectIdentityAuthority,
        checkpoint: RecoveryCheckpoint,
    ) -> io::Result<()> {
        if checkpoint.project_id() != authority.project_id() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a autoridade não corresponde ao checkpoint de Recuperação",
            ));
        }
        let generation = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("o agendador de Recuperação ficou indisponível"))?;
            state.generation = state
                .generation
                .checked_add(1)
                .ok_or_else(|| io::Error::other("o agendador de Recuperação se esgotou"))?;
            let generation = state.generation;
            state.pending = Some(PendingCheckpoint {
                generation,
                authority,
                checkpoint,
            });
            generation
        };
        let coordinator = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(coordinator.delay).await;
            let worker = coordinator.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                worker.publish_if_current(generation)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::error!(
                    target: "myalbuns.desktop",
                    error = %error,
                    event = "project_recovery_checkpoint_publish_failed",
                ),
                Err(error) => tracing::error!(
                    target: "myalbuns.desktop",
                    error = %error,
                    event = "project_recovery_checkpoint_worker_failed",
                ),
            }
        });
        Ok(())
    }

    pub(crate) fn finish(&self, authority: &ProjectIdentityAuthority) -> io::Result<bool> {
        let _io = self
            .io
            .lock()
            .map_err(|_| io::Error::other("o armazenamento de Recuperação ficou indisponível"))?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("o agendador de Recuperação ficou indisponível"))?;
        let next_generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("o agendador de Recuperação se esgotou"))?;
        let removed = self.store.finish(authority)?;
        state.generation = next_generation;
        state.pending = None;
        Ok(removed)
    }

    fn publish_if_current(&self, generation: u64) -> io::Result<()> {
        let _io = self
            .io
            .lock()
            .map_err(|_| io::Error::other("o armazenamento de Recuperação ficou indisponível"))?;
        let pending = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| io::Error::other("o agendador de Recuperação ficou indisponível"))?;
            if state
                .pending
                .as_ref()
                .is_none_or(|pending| pending.generation != generation)
            {
                return Ok(());
            }
            state.pending.take()
        };
        let Some(pending) = pending else {
            return Ok(());
        };
        self.store.publish(&pending.authority, &pending.checkpoint)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, EditableProject, InitialProject, ProjectCore,
        ProjectIdentityAuthority, ProjectIntent, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext, project_data_namespace};

    use super::{RecoveryCoordinator, RecoveryStore};

    struct ProjectFixture {
        project: EditableProject,
        path: PathBuf,
    }

    fn create_project(root: &Path, name: &str) -> ProjectFixture {
        let path = root.join(format!("{name}.myalbuns"));
        let mut context = OperationPathContext::new();
        context
            .capture(&path)
            .expect("the Project root is captured");
        let mut project = ProjectCore::new()
            .with_identity_storage_roots(root.join("leases"), root.join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(path.clone(), context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable Project is created");
        project
            .apply(ProjectIntent::SetDpi { dpi: 360 })
            .expect("one completed creative action is captured");
        ProjectFixture { project, path }
    }

    fn store(root: &Path) -> RecoveryStore {
        RecoveryStore::new(AppPaths::from_roots(
            &root.join("roaming"),
            &root.join("local"),
        ))
    }

    fn authority(project: &EditableProject) -> ProjectIdentityAuthority {
        project.identity_authority().clone()
    }

    #[test]
    fn publishes_under_the_cache_authority_key_and_loads_only_that_identity() {
        let root = tempfile::tempdir().expect("temporary Recovery fixture");
        let first = create_project(root.path(), "Primeiro");
        let second = create_project(root.path(), "Segundo");
        let store = store(root.path());
        let first_authority = authority(&first.project);
        let second_authority = authority(&second.project);
        let first_checkpoint = first
            .project
            .recovery_checkpoint()
            .expect("the first checkpoint is consolidated");
        let second_checkpoint = second
            .project
            .recovery_checkpoint()
            .expect("the second checkpoint is consolidated");

        store
            .publish(&first_authority, &first_checkpoint)
            .expect("the first checkpoint is published");
        store
            .publish(&second_authority, &second_checkpoint)
            .expect("the second checkpoint is published");

        let first_path = store
            .checkpoint_path(&first_authority)
            .expect("the authorized checkpoint path is valid");
        let expected_file_name = format!(
            "{}.json",
            project_data_namespace(&first_authority.project_id().hyphenated().to_string())
        );
        assert_eq!(
            first_path.file_name().and_then(|name| name.to_str()),
            Some(expected_file_name.as_str())
        );
        assert!(
            !first_path
                .to_string_lossy()
                .contains(&first.path.to_string_lossy()[..])
        );
        assert!(
            !first_path
                .to_string_lossy()
                .contains(&first_authority.project_id().hyphenated().to_string())
        );
        assert_eq!(
            store
                .load(&first_authority)
                .expect("the first checkpoint is readable")
                .expect("the first checkpoint exists")
                .to_bytes()
                .expect("the first checkpoint serializes"),
            first_checkpoint
                .to_bytes()
                .expect("the expected checkpoint serializes")
        );
        assert_eq!(
            store
                .load(&second_authority)
                .expect("the second checkpoint is readable")
                .expect("the second checkpoint exists")
                .to_bytes()
                .expect("the second checkpoint serializes"),
            second_checkpoint
                .to_bytes()
                .expect("the expected checkpoint serializes")
        );
    }

    #[test]
    fn an_interrupted_sibling_write_never_replaces_the_last_complete_checkpoint() {
        let root = tempfile::tempdir().expect("temporary interrupted Recovery fixture");
        let project = create_project(root.path(), "Projeto");
        let authority = authority(&project.project);
        let checkpoint = project
            .project
            .recovery_checkpoint()
            .expect("the checkpoint is consolidated");
        let store = store(root.path());
        store
            .publish(&authority, &checkpoint)
            .expect("the complete checkpoint is published");
        let final_path = store
            .checkpoint_path(&authority)
            .expect("the checkpoint path is valid");
        let interrupted = final_path.with_extension("json.interrupted.tmp");
        std::fs::write(&interrupted, b"{\"schemaVersion\":")
            .expect("the interrupted sibling remains partial");

        let loaded = store
            .load(&authority)
            .expect("the complete checkpoint remains readable")
            .expect("the complete checkpoint remains present");

        assert_eq!(
            loaded.to_bytes().expect("the loaded checkpoint serializes"),
            checkpoint
                .to_bytes()
                .expect("the prior checkpoint serializes")
        );
        assert!(interrupted.is_file());
    }

    #[test]
    fn finishing_one_authorized_checkpoint_preserves_other_projects() {
        let root = tempfile::tempdir().expect("temporary Recovery cleanup fixture");
        let first = create_project(root.path(), "Primeiro");
        let second = create_project(root.path(), "Segundo");
        let first_authority = authority(&first.project);
        let second_authority = authority(&second.project);
        let store = store(root.path());
        store
            .publish(
                &first_authority,
                &first
                    .project
                    .recovery_checkpoint()
                    .expect("the first checkpoint is consolidated"),
            )
            .expect("the first checkpoint is published");
        store
            .publish(
                &second_authority,
                &second
                    .project
                    .recovery_checkpoint()
                    .expect("the second checkpoint is consolidated"),
            )
            .expect("the second checkpoint is published");

        assert!(
            store
                .finish(&first_authority)
                .expect("the first checkpoint is finished")
        );
        assert!(
            store
                .load(&first_authority)
                .expect("the first namespace remains readable")
                .is_none()
        );
        assert!(
            store
                .load(&second_authority)
                .expect("the second namespace remains readable")
                .is_some()
        );
    }

    #[test]
    fn a_non_file_checkpoint_fails_closed_and_is_preserved() {
        let root = tempfile::tempdir().expect("temporary Recovery failure fixture");
        let project = create_project(root.path(), "Projeto");
        let authority = authority(&project.project);
        let store = store(root.path());
        let checkpoint = store
            .checkpoint_path(&authority)
            .expect("the checkpoint path is valid");
        std::fs::create_dir_all(&checkpoint).expect("a directory occupies the checkpoint path");

        assert!(store.load(&authority).is_err());
        assert!(store.finish(&authority).is_err());
        assert!(checkpoint.is_dir());
    }

    #[test]
    fn nearby_completed_actions_publish_only_the_latest_consolidated_state() {
        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().expect("temporary debounced Recovery fixture");
            let mut project = create_project(root.path(), "Projeto").project;
            let authority = authority(&project);
            let store = store(root.path());
            let coordinator =
                RecoveryCoordinator::with_delay(store.clone(), Duration::from_millis(60));
            coordinator
                .schedule(
                    authority.clone(),
                    project
                        .recovery_checkpoint()
                        .expect("the first action is consolidated"),
                )
                .expect("the first action is scheduled");
            project
                .apply(ProjectIntent::SetDpi { dpi: 420 })
                .expect("the nearby action is completed");
            coordinator
                .schedule(
                    authority.clone(),
                    project
                        .recovery_checkpoint()
                        .expect("the latest action is consolidated"),
                )
                .expect("the latest action replaces the pending publication");

            tokio::time::sleep(Duration::from_millis(20)).await;
            assert!(
                store
                    .load(&authority)
                    .expect("the namespace is readable before the delay")
                    .is_none()
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
            let bytes = store
                .load(&authority)
                .expect("the checkpoint is readable after the delay")
                .expect("the checkpoint is published after the delay")
                .to_bytes()
                .expect("the checkpoint serializes");
            let document: serde_json::Value =
                serde_json::from_slice(&bytes).expect("the checkpoint is valid JSON");
            assert_eq!(document["creativeState"]["project"]["document"]["dpi"], 420);
        });
    }

    #[test]
    fn finishing_a_checkpoint_cancels_a_pending_publication() {
        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().expect("temporary cancelled Recovery fixture");
            let project = create_project(root.path(), "Projeto").project;
            let authority = authority(&project);
            let store = store(root.path());
            let coordinator =
                RecoveryCoordinator::with_delay(store.clone(), Duration::from_millis(50));
            coordinator
                .schedule(
                    authority.clone(),
                    project
                        .recovery_checkpoint()
                        .expect("the completed action is consolidated"),
                )
                .expect("the action is scheduled");

            assert!(
                !coordinator
                    .finish(&authority)
                    .expect("nothing was published yet")
            );
            tokio::time::sleep(Duration::from_millis(100)).await;

            assert!(
                store
                    .load(&authority)
                    .expect("the namespace remains readable")
                    .is_none()
            );
        });
    }

    #[test]
    fn failed_finish_preserves_the_pending_publication() {
        tauri::async_runtime::block_on(async {
            let root = tempfile::tempdir().expect("temporary failed Recovery finish fixture");
            let project = create_project(root.path(), "Projeto").project;
            let authority = authority(&project);
            let store = store(root.path());
            let coordinator =
                RecoveryCoordinator::with_delay(store.clone(), Duration::from_secs(3600));
            coordinator
                .schedule(
                    authority.clone(),
                    project
                        .recovery_checkpoint()
                        .expect("the completed action is consolidated"),
                )
                .expect("the action is scheduled");
            let generation = coordinator.state.lock().unwrap().generation;
            let checkpoint = store
                .checkpoint_path(&authority)
                .expect("the checkpoint path is valid");
            std::fs::create_dir_all(&checkpoint)
                .expect("a directory temporarily blocks checkpoint removal");

            assert!(coordinator.finish(&authority).is_err());
            std::fs::remove_dir(&checkpoint).expect("the local obstruction is released");
            // Drive the real scheduled worker after the failed finish; debounce timing
            // is covered separately, and must not race this obstruction fixture.
            coordinator
                .publish_if_current(generation)
                .expect("the preserved pending checkpoint can still be published");

            assert!(
                store
                    .load(&authority)
                    .expect("the namespace is readable after the obstruction")
                    .is_some(),
                "a failed terminal operation must not discard the pending checkpoint"
            );
        });
    }
}
