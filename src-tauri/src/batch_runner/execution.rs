use super::*;
use crate::{
    export_pipeline::{
        ExportExecutionControl, ExportFailureStage, ExportProgressStage, ExportProgressUnits,
    },
    imaging_processor::{ImagingTransport, InvocationContext},
    ipc_contract::BatchExportProgress,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
enum PreparationFailure {
    Problems(Vec<BatchProblem>),
    StorageFull,
}

impl From<Vec<BatchProblem>> for PreparationFailure {
    fn from(problems: Vec<BatchProblem>) -> Self {
        Self::Problems(problems)
    }
}

#[derive(Default)]
pub(crate) struct BatchCancellation {
    requested: AtomicBool,
    current: Mutex<Option<Arc<ExportExecutionControl>>>,
}

impl BatchCancellation {
    pub(crate) fn request(&self) {
        self.requested.store(true, Ordering::Release);
        if let Some(current) = &*self.current.lock().expect("batch control is available") {
            current.request_cancel();
        }
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    fn begin_item(&self) -> Arc<ExportExecutionControl> {
        let item = Arc::new(ExportExecutionControl::default());
        *self.current.lock().expect("batch control is available") = Some(item.clone());
        if self.is_requested() {
            item.request_cancel();
        }
        item
    }
}

impl BatchRunner {
    pub(crate) fn ignore(&mut self, id: &str) -> Result<(), String> {
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or("O item não pertence ao lote.")?;
        if item.status == BatchItemStatus::Completed {
            return Err("O item já foi concluído.".into());
        }
        item.status = BatchItemStatus::Ignored;
        item.relinks = ItemRelinks::default();
        Ok(())
    }

    /// The caller owns one OperationLease and one transport for this entire run.
    pub(crate) async fn run<T: ImagingTransport>(
        mut self,
        transport: &mut T,
        cancellation: &BatchCancellation,
        policy: ExportConflictPolicy,
        progress: &(impl Fn(BatchExportProgress) + Send + Sync),
    ) -> Result<Self, String> {
        let view = self.view();
        if !view.can_continue {
            return Err("Corrija ou ignore os Projetos com problemas antes de continuar.".into());
        }
        if view.has_conflicts && policy == ExportConflictPolicy::Ask {
            return Err("Confirme como tratar a exportação existente antes de continuar.".into());
        }
        let bindings = std::mem::take(&mut self.paths).freeze();
        self.phase = BatchPhase::Running;
        self = tauri::async_runtime::spawn_blocking(move || {
            self.checkpoint_or_pause()?;
            Ok::<_, String>(self)
        })
        .await
        .map_err(|error| error.to_string())??;
        let total = self.items.len() as u32;
        for index in 0..self.items.len() {
            if self.phase == BatchPhase::StorageFull {
                break;
            }
            if cancellation.is_requested() {
                self.phase = BatchPhase::Interrupted;
                break;
            }
            if self.items[index].status != BatchItemStatus::Pending {
                // A recovered item can be ignored without being rendered again.
                // Its retained preparation still belongs to this attempt and is
                // removed only while the caller owns the processor reservation.
                let roots = bindings.clone();
                self = tauri::async_runtime::spawn_blocking(move || {
                    if let Some(preparation) = &self.items[index].preparation {
                        match preparation.discard(&roots) {
                            Ok(()) => self.items[index].preparation = None,
                            Err(error) => {
                                self.items[index].status = BatchItemStatus::Failed;
                                self.items[index].problems =
                                    vec![problem(BatchProblemKind::Unavailable, error)];
                            }
                        }
                        self.checkpoint_or_pause()?;
                    }
                    Ok::<_, String>(self)
                })
                .await
                .map_err(|error| error.to_string())??;
                continue;
            }
            let roots = bindings.clone();
            let (returned, planned) = tauri::async_runtime::spawn_blocking(move || {
                let planned = self.prepare_item(index, &roots, policy);
                (self, planned)
            })
            .await
            .map_err(|error| error.to_string())?;
            self = returned;
            let mut plan = match planned {
                Ok(plan) => plan,
                Err(PreparationFailure::StorageFull) => {
                    self.phase = BatchPhase::StorageFull;
                    self.current = Some(self.items[index].id.clone());
                    break;
                }
                Err(PreparationFailure::Problems(problems)) => {
                    self.items[index].status = BatchItemStatus::Failed;
                    self.items[index].problems = problems;
                    self = self.persist().await?;
                    continue;
                }
            };
            if policy == ExportConflictPolicy::Skip {
                let (returned_plan, has_outputs) =
                    tauri::async_runtime::spawn_blocking(move || {
                        let has_outputs = plan.skip_existing_outputs();
                        (plan, has_outputs)
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                plan = returned_plan;
                match has_outputs {
                    Ok(true) => {}
                    Ok(false) => {
                        self.items[index].status = BatchItemStatus::Ignored;
                        self.items[index].problems = vec![problem(
                            BatchProblemKind::Failed,
                            "Arquivos existentes mantidos.",
                        )];
                        self = self.persist().await?;
                        continue;
                    }
                    Err(error) => {
                        self.items[index].status = BatchItemStatus::Failed;
                        self.items[index].problems =
                            vec![problem(BatchProblemKind::Failed, error.message)];
                        self = self.persist().await?;
                        continue;
                    }
                }
            }
            let request_id = plan.request_id().to_owned();
            self.current = Some(self.items[index].id.clone());
            self.items[index].preparation = Some(checkpoint::InterruptedPreparation {
                request_id: request_id.clone(),
                path: bindings
                    .resolve(plan.preparation_directory())
                    .map_err(|error| error.to_string())?
                    .into(),
            });
            self = self.persist().await?;
            if self.phase == BatchPhase::StorageFull {
                break;
            }
            let item_control = cancellation.begin_item();
            let completed = self
                .items
                .iter()
                .filter(|item| item.status != BatchItemStatus::Pending)
                .count() as u32;
            let last_percent = Mutex::new((f64::from(completed) / f64::from(total)) * 100.0);
            progress(BatchExportProgress {
                completed,
                total,
                percent: *last_percent.lock().unwrap(),
            });
            let result = export_pipeline::execute_album(
                transport,
                plan,
                &bindings,
                &item_control,
                &|update| {
                    let fraction = match update.units {
                        ExportProgressUnits::Measured {
                            completed_units,
                            total_units,
                        } if total_units > 0 => f64::from(completed_units) / f64::from(total_units),
                        _ => 0.0,
                    };
                    let item_fraction = match update.stage {
                        ExportProgressStage::Preparing => 0.0,
                        ExportProgressStage::LoadingSources => 0.1 * fraction,
                        ExportProgressStage::Composing | ExportProgressStage::EncodingOutput => {
                            0.1 + 0.65 * fraction
                        }
                        ExportProgressStage::Verifying => 0.75 + 0.1 * fraction,
                        ExportProgressStage::Publishing => 0.85 + 0.14 * fraction,
                        ExportProgressStage::Completed => 0.99,
                    };
                    let mut last = last_percent.lock().expect("batch progress is available");
                    *last =
                        last.max((f64::from(completed) + item_fraction) / f64::from(total) * 100.0);
                    progress(BatchExportProgress {
                        completed,
                        total,
                        percent: *last,
                    });
                },
                &InvocationContext::new(&request_id, None::<String>),
            )
            .await;
            *cancellation
                .current
                .lock()
                .expect("batch control is available") = None;
            if cancellation.is_requested() {
                // Publication is atomic only per file. A cancelled current item
                // remains pending even if its final promotion already finished.
                self.phase = BatchPhase::Interrupted;
                self.items[index].problems = vec![problem(
                    BatchProblemKind::Failed,
                    "Exportação interrompida. Este Projeto será refeito ao retomar.",
                )];
                break;
            }
            match result {
                Ok(_) => {
                    self.items[index].status = BatchItemStatus::Completed;
                    self.items[index].preparation = None;
                }
                Err(error) => {
                    if error.is_storage_full() {
                        self.phase = BatchPhase::StorageFull;
                        break;
                    }
                    if matches!(error.stage, ExportFailureStage::Processor(crate::imaging_processor::InvocationFailureStage::TerminationUnconfirmed)) {
                        self.phase = BatchPhase::Interrupted;
                        self.items[index].problems = vec![problem(BatchProblemKind::Failed, error.message)];
                        break;
                    }
                    self.items[index].status = BatchItemStatus::Failed;
                    let message = match error.stage {
                        ExportFailureStage::Publish {
                            promoted_outputs,
                            total_outputs,
                        } => format!(
                            "{} Foram publicados {promoted_outputs} de {total_outputs} arquivos. A pasta pode conter saídas anteriores e novas; exporte este Projeto inteiro novamente.",
                            error.message
                        ),
                        _ => error.message,
                    };
                    self.items[index].problems = vec![problem(BatchProblemKind::Failed, message)];
                }
            }
            self.current = None;
            self = self.persist().await?;
            let completed = self
                .items
                .iter()
                .filter(|item| item.status != BatchItemStatus::Pending)
                .count() as u32;
            progress(BatchExportProgress {
                completed,
                total,
                percent: f64::from(completed) / f64::from(total) * 100.0,
            });
        }
        if !matches!(
            self.phase,
            BatchPhase::Interrupted | BatchPhase::StorageFull
        ) {
            self.phase = BatchPhase::Finished;
        }
        if self.phase != BatchPhase::StorageFull {
            for item in &mut self.items {
                item.relinks = ItemRelinks::default();
            }
        }
        tauri::async_runtime::spawn_blocking(move || {
            if self.items.iter().all(|item| {
                matches!(
                    item.status,
                    BatchItemStatus::Completed | BatchItemStatus::Ignored
                )
            }) && self.phase == BatchPhase::Finished
            {
                self.remove_checkpoint()?;
            } else {
                self.checkpoint_or_pause()?;
            }
            Ok(self)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    async fn persist(mut self) -> Result<Self, String> {
        tauri::async_runtime::spawn_blocking(move || {
            self.checkpoint_or_pause()?;
            Ok(self)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    fn prepare_item(
        &mut self,
        index: usize,
        paths: &RootBindingPlan,
        policy: ExportConflictPolicy,
    ) -> Result<AlbumExportPlan, PreparationFailure> {
        if let Some(preparation) = &self.items[index].preparation {
            preparation
                .discard(paths)
                .map_err(|error| vec![problem(BatchProblemKind::Unavailable, error)])?;
            self.items[index].preparation = None;
        }
        let item = &self.items[index];
        let first = load_in_plan(&self.core, paths, item).map_err(|error| vec![error])?;
        let request_id = format!("batch-{}", uuid::Uuid::new_v4());
        // Every item is opened and validated again. A changed document cannot
        // inherit the preflight conflict authorization or source assumptions.
        let checked = inspect_and_plan(
            &first,
            item,
            &self.configuration,
            paths,
            policy,
            &request_id,
        );
        if item.digest.as_deref() != Some(first.content_sha256()) {
            return Err(vec![problem(
                BatchProblemKind::Changed,
                "O Projeto mudou depois da verificação. Verifique novamente antes de exportar.",
            )]
            .into());
        }
        checked?;
        let current = load_in_plan(&self.core, paths, item).map_err(|error| vec![error])?;
        if current.content_sha256() != first.content_sha256() {
            return Err(vec![problem(
                BatchProblemKind::Changed,
                "O Projeto mudou durante a verificação. Tente novamente.",
            )]
            .into());
        }
        let plan = inspect_and_plan(
            &current,
            item,
            &self.configuration,
            paths,
            policy,
            &request_id,
        )?;
        let destination = paths
            .resolve(&item.destination)
            .map_err(|error| vec![problem(BatchProblemKind::Unavailable, error.to_string())])?;
        create_destination(&destination).map_err(|error| {
            if myalbuns_paths::AppPathsError::is_storage_full(&error) {
                return PreparationFailure::StorageFull;
            }
            PreparationFailure::Problems(vec![problem(
                BatchProblemKind::Failed,
                format!("Não foi possível preparar o destino: {error}"),
            )])
        })?;
        Ok(plan)
    }
}

fn create_destination(path: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    myalbuns_paths::test_support::create(path)?;
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_full_while_creating_the_destination_is_a_pause_not_an_item_problem() {
        let root = tempfile::tempdir().unwrap();
        let (core, _editor) = crate::batch_runner::tests::fixture(root.path(), "source/A.myalbuns");
        let mut batch = BatchRunner::discover(
            BatchConfiguration {
                source: root.path().join("source"),
                destination: None,
                format: ExportFormat::Png,
                mode: ExportMode::Sheet,
            },
            core,
            root.path().join("checkpoints"),
        )
        .unwrap();
        let roots = batch.paths.current_plan();
        let destination = batch.items[0].destination.clone();
        let fault = myalbuns_paths::test_support::DiskFull::on_create(&destination);
        assert!(matches!(
            batch.prepare_item(0, &roots, ExportConflictPolicy::Ask),
            Err(PreparationFailure::StorageFull)
        ));
        assert_eq!(fault.failure_count(), 1);
        assert!(!destination.exists());
        drop(fault);
        assert!(
            batch
                .prepare_item(0, &roots, ExportConflictPolicy::Ask)
                .is_ok()
        );
    }
}
