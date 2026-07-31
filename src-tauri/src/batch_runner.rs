use std::{collections::HashSet, fmt, path::Path};

use myalbuns_core::LoadedProjectRevision;
use myalbuns_paths::RootBindingPlan;
use tauri::AppHandle;

use crate::{
    cache_engine::CacheEngine,
    export_pipeline::{
        self, ExportExecutionControl, ExportFailure, ExportFailureStage, ExportPlan,
        ExportProgress, PublishedExport,
    },
    imaging_processor::{
        ImagingProcessor, ImagingTransport, InvocationContext, TauriImagingTransport,
    },
    logging::LoggingState,
    operation_gate::OperationGate,
    operation_gate::OperationMode,
    operation_lease::{OperationLease, OperationLeaseError},
};

#[derive(Debug)]
pub(crate) struct BatchItem {
    item_id: String,
    outputs: Vec<ExportPlan>,
}

#[derive(Debug)]
pub(crate) struct BatchPlan {
    items: Vec<BatchItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BatchEvent {
    Started {
        total_items: u32,
    },
    ItemStarted {
        item_index: u32,
        total_items: u32,
        item_id: String,
    },
    ItemProgress {
        item_index: u32,
        total_items: u32,
        item_id: String,
        progress: ExportProgress,
    },
    ItemCompleted {
        item_index: u32,
        total_items: u32,
        item_id: String,
        output_count: u32,
    },
    Completed {
        total_items: u32,
        output_count: u32,
    },
}

#[derive(Debug)]
pub(crate) struct BatchRunResult {
    pub(crate) completed_items: u32,
    pub(crate) published_outputs: Vec<PublishedExport>,
}

#[derive(Debug)]
pub(crate) enum BatchRunFailure {
    Lease(OperationLeaseError),
    InvalidPlan(String),
    Item {
        completed_items: u32,
        item_index: u32,
        item_id: String,
        failure: ExportFailure,
    },
}

pub(crate) struct BatchRunner;

impl BatchItem {
    pub(crate) fn from_persisted_revision(
        item_id: impl Into<String>,
        revision: LoadedProjectRevision,
        output_options: Vec<export_pipeline::ExportOptions>,
    ) -> Result<Self, String> {
        let item_id = item_id.into();
        if item_id.trim().is_empty() {
            return Err("A identidade do item do lote está vazia.".into());
        }
        if output_options.is_empty() {
            return Err("Cada item do lote precisa conter ao menos uma saída.".into());
        }
        let snapshot = revision.render_snapshot();
        let outputs = output_options
            .into_iter()
            .map(|options| export_pipeline::plan(snapshot.clone(), options))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|failure| failure.message)?;
        Ok(Self { item_id, outputs })
    }
}

impl BatchPlan {
    pub(crate) fn new(items: Vec<BatchItem>) -> Result<Self, String> {
        if items.is_empty() {
            return Err("A Exportação em lote precisa conter ao menos um item.".into());
        }
        u32::try_from(items.len())
            .map_err(|_| "A Exportação em lote excede a quantidade de itens suportada.")?;
        let mut request_ids = HashSet::new();
        for request_id in items
            .iter()
            .flat_map(|item| item.outputs.iter().map(ExportPlan::request_id))
        {
            if !request_ids.insert(request_id) {
                return Err("As saídas do lote precisam ter correlações distintas.".into());
            }
        }
        Ok(Self { items })
    }

    pub(crate) fn required_paths(&self) -> Vec<&Path> {
        self.items
            .iter()
            .flat_map(|item| item.outputs.iter().flat_map(ExportPlan::required_paths))
            .collect()
    }
}

impl BatchRunner {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run(
        app: &AppHandle,
        logging: &LoggingState,
        gate: &OperationGate,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
        plan: BatchPlan,
        root_bindings: &RootBindingPlan,
        on_event: &(dyn Fn(BatchEvent) + Send + Sync),
    ) -> Result<BatchRunResult, BatchRunFailure> {
        let lease = OperationLease::acquire(gate, cache, processor, OperationMode::BatchExclusive)
            .await
            .map_err(BatchRunFailure::Lease)?;
        let mut transport = TauriImagingTransport::new(app, logging, lease.processor_reservation());
        Self::execute(&lease, &mut transport, plan, root_bindings, on_event).await
    }

    pub(crate) async fn execute<T: ImagingTransport>(
        lease: &OperationLease,
        transport: &mut T,
        plan: BatchPlan,
        root_bindings: &RootBindingPlan,
        on_event: &(dyn Fn(BatchEvent) + Send + Sync),
    ) -> Result<BatchRunResult, BatchRunFailure> {
        if lease.mode() != OperationMode::BatchExclusive {
            return Err(BatchRunFailure::InvalidPlan(
                "O BatchRunner exige uma concessão BatchExclusive.".into(),
            ));
        }
        let total_items = u32::try_from(plan.items.len())
            .expect("BatchPlan validates its item count before execution");
        on_event(BatchEvent::Started { total_items });
        let mut published_outputs = Vec::new();

        for (item_index, item) in plan.items.into_iter().enumerate() {
            let item_index = u32::try_from(item_index)
                .expect("BatchPlan validates its item count before execution");
            let BatchItem { item_id, outputs } = item;
            on_event(BatchEvent::ItemStarted {
                item_index,
                total_items,
                item_id: item_id.clone(),
            });
            let output_count = u32::try_from(outputs.len())
                .expect("ExportPipeline validates its grouped output count");
            let exports = outputs
                .into_iter()
                .map(|output| {
                    let context =
                        InvocationContext::new(output.request_id(), Some(output.project_id()));
                    (output, context)
                })
                .collect();
            let control = ExportExecutionControl::default();
            let progress = |progress| {
                on_event(BatchEvent::ItemProgress {
                    item_index,
                    total_items,
                    item_id: item_id.clone(),
                    progress,
                });
            };
            let mut item_outputs = export_pipeline::execute_group(
                transport,
                exports,
                root_bindings,
                &control,
                &progress,
            )
            .await
            .map_err(|failure| BatchRunFailure::Item {
                completed_items: item_index,
                item_index,
                item_id: item_id.clone(),
                failure,
            })?;
            published_outputs.append(&mut item_outputs);
            on_event(BatchEvent::ItemCompleted {
                item_index,
                total_items,
                item_id,
                output_count,
            });
        }

        let output_count = u32::try_from(published_outputs.len())
            .expect("validated grouped outputs fit the progress contract");
        on_event(BatchEvent::Completed {
            total_items,
            output_count,
        });
        Ok(BatchRunResult {
            completed_items: total_items,
            published_outputs,
        })
    }
}

impl BatchRunFailure {
    pub(crate) fn completed_items(&self) -> u32 {
        match self {
            Self::Lease(_) => 0,
            Self::InvalidPlan(_) => 0,
            Self::Item {
                completed_items, ..
            } => *completed_items,
        }
    }

    pub(crate) fn item_index(&self) -> Option<u32> {
        match self {
            Self::Lease(_) => None,
            Self::InvalidPlan(_) => None,
            Self::Item { item_index, .. } => Some(*item_index),
        }
    }

    pub(crate) fn item_id(&self) -> Option<&str> {
        match self {
            Self::Lease(_) => None,
            Self::InvalidPlan(_) => None,
            Self::Item { item_id, .. } => Some(item_id),
        }
    }

    pub(crate) fn export_stage(&self) -> Option<ExportFailureStage> {
        match self {
            Self::Lease(_) => None,
            Self::InvalidPlan(_) => None,
            Self::Item { failure, .. } => Some(failure.stage),
        }
    }
}

impl fmt::Display for BatchRunFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lease(error) => error.fmt(formatter),
            Self::InvalidPlan(message) => formatter.write_str(message),
            Self::Item {
                item_index,
                item_id,
                failure,
                ..
            } => write!(
                formatter,
                "O item {item_id} do lote (índice {item_index}) falhou: {}",
                failure.message
            ),
        }
    }
}

impl std::error::Error for BatchRunFailure {}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use myalbuns_core::ProjectCore;
    use myalbuns_imaging_protocol::{ImagingCommand, ImagingResponse, RenderCompletion};
    use myalbuns_paths::{AppPaths, OperationPathContext, RootBindingPlan};
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{BatchEvent, BatchItem, BatchPlan, BatchRunner};
    use crate::{
        cache_engine::CacheEngine,
        export_pipeline::ExportOptions,
        imaging_processor::{
            ImagingOperation, ImagingProcessor, ImagingTransport, InvocationControl,
            InvocationFailure, InvocationFuture,
        },
        operation_gate::{OperationGate, OperationMode},
        operation_lease::OperationLease,
        sample_project::SampleProject,
    };

    struct RecordingTransport {
        payloads: VecDeque<Vec<u8>>,
        requests: Vec<String>,
    }

    impl ImagingTransport for RecordingTransport {
        fn invoke<'a>(
            &'a mut self,
            command: &'a ImagingCommand,
            _context: &'a crate::imaging_processor::InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
            _control: InvocationControl<'a>,
        ) -> InvocationFuture<'a> {
            let ImagingCommand::Render(request) = command else {
                panic!("the BatchRunner invokes only Render commands");
            };
            assert_eq!(operation, ImagingOperation::Export);
            assert_eq!(attempt, 1);
            let payload = self
                .payloads
                .pop_front()
                .expect("one payload exists for each batch output");
            std::fs::write(&request.prepared_output_path, &payload)
                .expect("the processor boundary writes the prepared output");
            self.requests.push(request.request_id.clone());
            let completion = RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: payload.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&payload)),
            };
            let response = ImagingResponse::completed(&request.request_id, completion);
            Box::pin(async move { Ok::<_, InvocationFailure>(response) })
        }
    }

    fn item(sample: SampleProject, request_id: &str, output_path: std::path::PathBuf) -> BatchItem {
        let source = sample
            .persisted_source(2)
            .expect("the persisted batch fixture is valid");
        let revision = ProjectCore::new()
            .load_persisted_revision(&source)
            .expect("the batch reads a persisted revision without opening a session");
        let snapshot = revision.render_snapshot();
        let sheet_id = snapshot.composition.sheets[0].sheet_id.clone();
        BatchItem::from_persisted_revision(
            request_id,
            revision,
            vec![ExportOptions::new(
                request_id,
                output_path,
                sheet_id,
                25,
                None,
            )],
        )
        .expect("the persisted revision produces one batch output")
    }

    fn bindings(plan: &BatchPlan) -> RootBindingPlan {
        let mut context = OperationPathContext::new();
        for path in plan.required_paths() {
            context
                .capture(path)
                .expect("the batch captures every required root before acquiring the lease");
        }
        context.freeze()
    }

    #[test]
    fn two_items_execute_serially_under_one_continuous_batch_lease() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the batch fixture root exists");
            let destination = root.path().join("destination");
            std::fs::create_dir(&destination).expect("the batch destination exists");
            let first_output = destination.join("Horizonte_001.png");
            let second_output = destination.join("Aurora_001.png");
            let plan = BatchPlan::new(vec![
                item(SampleProject::Horizon, "batch-item-1", first_output.clone()),
                item(SampleProject::Aurora, "batch-item-2", second_output.clone()),
            ])
            .expect("the batch contains two items");
            let bindings = bindings(&plan);
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let lease =
                OperationLease::acquire(&gate, &cache, &processor, OperationMode::BatchExclusive)
                    .await
                    .expect("the batch acquires its only lease");
            let mut transport = RecordingTransport {
                payloads: VecDeque::from([
                    b"first batch output".to_vec(),
                    b"second batch output".to_vec(),
                ]),
                requests: Vec::new(),
            };
            let events = Mutex::new(Vec::new());
            let observe = |event: BatchEvent| {
                if matches!(event, BatchEvent::ItemCompleted { item_index: 0, .. }) {
                    assert!(
                        OperationLease::begin(&gate, OperationMode::NormalExport).is_err(),
                        "NormalExport remains blocked between batch items"
                    );
                }
                events
                    .lock()
                    .expect("the batch event collector remains available")
                    .push(event);
            };

            let result = BatchRunner::execute(&lease, &mut transport, plan, &bindings, &observe)
                .await
                .expect("both batch items complete");

            assert_eq!(result.completed_items, 2);
            assert_eq!(result.published_outputs.len(), 2);
            assert_eq!(transport.requests, ["batch-item-1", "batch-item-2"]);
            assert_eq!(
                std::fs::read(&first_output).expect("the first output is readable"),
                b"first batch output"
            );
            assert_eq!(
                std::fs::read(&second_output).expect("the second output is readable"),
                b"second batch output"
            );
            let lifecycle_events = events
                .lock()
                .expect("the batch event collector remains available")
                .iter()
                .filter(|event| !matches!(event, BatchEvent::ItemProgress { .. }))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(
                lifecycle_events,
                vec![
                    BatchEvent::Started { total_items: 2 },
                    BatchEvent::ItemStarted {
                        item_index: 0,
                        total_items: 2,
                        item_id: "batch-item-1".into(),
                    },
                    BatchEvent::ItemCompleted {
                        item_index: 0,
                        total_items: 2,
                        item_id: "batch-item-1".into(),
                        output_count: 1,
                    },
                    BatchEvent::ItemStarted {
                        item_index: 1,
                        total_items: 2,
                        item_id: "batch-item-2".into(),
                    },
                    BatchEvent::ItemCompleted {
                        item_index: 1,
                        total_items: 2,
                        item_id: "batch-item-2".into(),
                        output_count: 1,
                    },
                    BatchEvent::Completed {
                        total_items: 2,
                        output_count: 2,
                    },
                ]
            );

            drop(lease);
            OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport)
                .await
                .expect("the normal Export reacquires every resource after the batch");
        });
    }
}
