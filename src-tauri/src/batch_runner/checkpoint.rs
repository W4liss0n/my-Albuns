use super::*;
use crate::ipc_contract::BatchRecoverySummary;
use myalbuns_paths::NativePathDto;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub(super) enum SaveFailure {
    StorageFull,
    Other(String),
}

impl From<SaveFailure> for String {
    fn from(error: SaveFailure) -> Self {
        match error {
            SaveFailure::StorageFull => "Não há espaço para registrar a retomada do lote.".into(),
            SaveFailure::Other(message) => message,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct InterruptedPreparation {
    pub request_id: String,
    pub path: NativePathDto,
}

impl InterruptedPreparation {
    pub(super) fn discard(&self, paths: &RootBindingPlan) -> Result<(), String> {
        let resolved = paths
            .resolve(self.path.as_path())
            .map_err(|error| error.to_string())?;
        let parent = resolved
            .parent()
            .ok_or("A preparação interrompida está inválida.")?;
        let plan =
            myalbuns_paths::ExportPathPlan::new(parent.join("recovery.bin"), &self.request_id)
                .map_err(|error| error.to_string())?;
        if plan.preparation_directory() != resolved {
            return Err("A preparação interrompida está inválida.".into());
        }
        plan.discard_abandoned_preparation()
            .map_err(|error| error.to_string())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    version: u32,
    id: String,
    source: NativePathDto,
    destination: Option<NativePathDto>,
    format: ExportFormat,
    mode: ExportMode,
    items: Vec<CheckpointItem>,
    current: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckpointItem {
    id: String,
    path: NativePathDto,
    status: BatchItemStatus,
    problems: Vec<BatchProblem>,
    preparation: Option<InterruptedPreparation>,
}

fn checkpoint_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| "A identificação do lote é inválida.")?;
    if parsed.to_string() != id {
        return Err("A identificação do lote é inválida.".into());
    }
    Ok(root.join(format!("{id}.json")))
}

fn read_checkpoint(root: &Path, id: &str) -> Result<Checkpoint, String> {
    let path = checkpoint_path(root, id)?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("O checkpoint do lote está indisponível.".into());
    }
    let value: Checkpoint =
        serde_json::from_slice(&std::fs::read(path).map_err(|error| error.to_string())?)
            .map_err(|_| "O checkpoint do lote está inválido.")?;
    if value.version != 1 || value.id != id || value.items.is_empty() {
        return Err("O checkpoint do lote está inválido.".into());
    }
    myalbuns_paths::validate_external_path(value.source.as_path())
        .map_err(|error| error.to_string())?;
    if let Some(destination) = &value.destination {
        myalbuns_paths::validate_external_path(destination.as_path())
            .map_err(|error| error.to_string())?;
    }
    let mut ids = HashSet::new();
    for item in &value.items {
        if uuid::Uuid::parse_str(&item.id).is_err()
            || !ids.insert(&item.id)
            || !item.path.as_path().starts_with(value.source.as_path())
            || item
                .path
                .as_path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("myalbuns"))
        {
            return Err("O checkpoint contém um item inválido.".into());
        }
    }
    if let Some(current) = &value.current
        && !ids.contains(current)
    {
        return Err("O item interrompido não pertence ao lote.".into());
    }
    for item in &value.items {
        let Some(current) = &item.preparation else {
            continue;
        };
        let preparation = current.path.as_path();
        let parent = preparation
            .parent()
            .ok_or("A preparação interrompida está inválida.")?;
        let expected =
            myalbuns_paths::ExportPathPlan::new(parent.join("probe.png"), &current.request_id)
                .map_err(|error| error.to_string())?;
        if expected.preparation_directory() != preparation {
            return Err("A preparação interrompida está inválida.".into());
        }
    }
    Ok(value)
}

impl BatchRunner {
    pub(crate) fn resume(root: &Path, id: &str, core: ProjectCore) -> Result<Self, String> {
        let saved = read_checkpoint(root, id)?;
        let configuration = BatchConfiguration {
            source: saved.source.into_path_buf(),
            destination: saved.destination.map(NativePathDto::into_path_buf),
            format: saved.format,
            mode: saved.mode,
        };
        configuration.format.validate()?;
        let mut paths = OperationPathContext::new();
        paths
            .capture(&configuration.source)
            .map_err(|error| error.to_string())?;
        if let Some(destination) = &configuration.destination {
            paths
                .capture(destination)
                .map_err(|error| error.to_string())?;
        }
        let items = saved
            .items
            .into_iter()
            .map(|item| {
                let path = item.path.into_path_buf();
                let name = project_name_from_path(&path);
                let parent = path
                    .parent()
                    .expect("validated checkpoint items have a parent");
                let destination = configuration
                    .destination
                    .as_ref()
                    .map_or_else(
                        || parent.to_path_buf(),
                        |destination| {
                            destination.join(
                                parent
                                    .strip_prefix(&configuration.source)
                                    .expect("validated source ancestry"),
                            )
                        },
                    )
                    .join(&name);
                BatchItem {
                    id: item.id.clone(),
                    path,
                    name,
                    destination,
                    status: if saved.current.as_deref() == Some(&item.id) {
                        BatchItemStatus::Pending
                    } else {
                        item.status
                    },
                    problems: item.problems,
                    digest: None,
                    revision: None,
                    has_conflicts: false,
                    relinks: ItemRelinks::default(),
                    preparation: item.preparation,
                }
            })
            .collect();
        let mut batch = Self {
            id: saved.id,
            configuration,
            core,
            checkpoint_root: root.to_path_buf(),
            paths,
            items,
            phase: BatchPhase::Prepared,
            current: None,
        };
        batch.recheck();
        Ok(batch)
    }

    /// Ends recovery explicitly; published outputs and project files are never touched.
    pub(crate) fn abandon(mut self, cleanup_preparation: bool) -> Result<(), String> {
        for item in &self.items {
            if cleanup_preparation && let Some(preparation) = &item.preparation {
                let result = self
                    .paths
                    .capture(preparation.path.as_path())
                    .map_err(|error| error.to_string())
                    .and_then(|_| preparation.discard(&self.paths.current_plan()));
                if let Err(error) = result {
                    tracing::warn!(target: "myalbuns.desktop", event = "batch_preparation_cleanup_deferred", error = %error);
                }
            }
        }
        self.remove_checkpoint()
    }

    pub(super) fn checkpoint_or_pause(&mut self) -> Result<(), String> {
        match self.save_checkpoint() {
            Ok(()) => Ok(()),
            Err(SaveFailure::StorageFull) => {
                // Keep the live runner even if the full volume cannot record
                // its new state. The last atomic checkpoint remains intact.
                self.phase = BatchPhase::StorageFull;
                Ok(())
            }
            Err(error) if self.phase == BatchPhase::StorageFull => {
                tracing::warn!(target: "myalbuns.desktop", event = "batch_checkpoint_deferred", error = %String::from(error));
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn save_checkpoint(&self) -> Result<(), SaveFailure> {
        let value = Checkpoint {
            version: 1,
            id: self.id.clone(),
            source: self.configuration.source.clone().into(),
            destination: self.configuration.destination.clone().map(Into::into),
            format: self.configuration.format.clone(),
            mode: self.configuration.mode,
            current: self.current.clone(),
            items: self
                .items
                .iter()
                .map(|item| CheckpointItem {
                    id: item.id.clone(),
                    path: item.path.clone().into(),
                    status: item.status,
                    problems: item.problems.clone(),
                    preparation: item.preparation.clone(),
                })
                .collect(),
        };
        let bytes =
            serde_json::to_vec(&value).map_err(|error| SaveFailure::Other(error.to_string()))?;
        crate::local_store_io::write_atomically(
            &checkpoint_path(&self.checkpoint_root, &self.id).map_err(SaveFailure::Other)?,
            &bytes,
            "batch-checkpoint",
        )
        .map_err(|error| {
            if myalbuns_paths::AppPathsError::is_storage_full(&error) {
                SaveFailure::StorageFull
            } else {
                SaveFailure::Other(format!(
                    "Não foi possível registrar a retomada do lote: {error}"
                ))
            }
        })
    }

    pub(super) fn remove_checkpoint(&self) -> Result<(), String> {
        match std::fs::remove_file(checkpoint_path(&self.checkpoint_root, &self.id)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "Não foi possível encerrar o registro do lote: {error}"
            )),
        }
    }

    pub(crate) fn recoveries(root: &Path) -> Result<Vec<BatchRecoverySummary>, String> {
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(error) => return Err(error.to_string()),
        };
        let mut result = vec![];
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let value = read_checkpoint(root, id)?;
            result.push(BatchRecoverySummary {
                id: value.id,
                source_folder: value.source.as_path().to_string_lossy().into_owned(),
                total: value.items.len() as u32,
                remaining: value
                    .items
                    .iter()
                    .filter(|item| {
                        !matches!(
                            item.status,
                            BatchItemStatus::Completed | BatchItemStatus::Ignored
                        ) || value
                            .current
                            .as_ref()
                            .is_some_and(|current| *current == item.id)
                    })
                    .count() as u32,
            });
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    }
}
