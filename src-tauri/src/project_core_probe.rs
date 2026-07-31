use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{ImagingCommand, ImagingResponse, RenderCompletion};
use myalbuns_paths::{AppPaths, OperationPathContext};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    batch_runner::{BatchEvent, BatchItem, BatchPlan, BatchRunner},
    cache_engine::CacheEngine,
    export_pipeline::ExportOptions,
    global_process_spike::{GLOBAL_PROCESS_ROLE, PROCESS_ROLE_ENV},
    imaging_processor::{
        ImagingOperation, ImagingProcessor, ImagingTransport, InvocationControl, InvocationFailure,
        InvocationFailureStage, InvocationFuture,
    },
    operation_gate::{OperationGate, OperationMode},
    operation_lease::OperationLease,
    probe_support::{validate_probe_root, wait_for_file_blocking, write_json_atomic_new},
    sample_project::SampleProject,
};

pub(crate) const PROJECT_CORE_PROBE_ROOT_ENV: &str = "MYALBUNS_PROJECT_CORE_PROBE_ROOT";
pub(crate) const PROJECT_CORE_PROBE_ACTION_ENV: &str = "MYALBUNS_PROJECT_CORE_PROBE_ACTION";

const PROBE_SCHEMA_VERSION: u32 = 1;
const PROBE_FAILED_EXIT_CODE: i32 = 75;
const RUN_MODE: &str = "headless_before_project_host";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeAction {
    Prepare,
    Batch,
}

#[derive(Debug)]
struct ProbeConfig {
    root: PathBuf,
    action: ProbeAction,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedEvent<'a> {
    schema_version: u32,
    process_id: u32,
    state: &'a str,
    input_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadyEvent<'a> {
    schema_version: u32,
    process_id: u32,
    state: &'a str,
    run_mode: &'a str,
    process_role: &'a str,
    project_host_constructed: bool,
    editable_project_owned: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletedEvent<'a> {
    schema_version: u32,
    process_id: u32,
    state: &'a str,
    run_mode: &'a str,
    process_role: &'a str,
    project_host_constructed: bool,
    editable_project_owned: bool,
    input_type: &'a str,
    loaded_revision_count: u32,
    completed_item_count: u32,
    published_output_count: u32,
    batch_completed_event_count: u32,
    items: Vec<LoadedItem>,
    renders: Vec<RenderedRevision>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadedItem {
    item_id: String,
    project_id: String,
    revision: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderedRevision {
    request_id: String,
    project_id: String,
    revision: u64,
    output_bytes: u64,
    output_sha256: String,
}

#[derive(Default)]
struct ProbeTransport {
    renders: Vec<RenderedRevision>,
}

pub(crate) fn requested() -> bool {
    std::env::var_os(PROJECT_CORE_PROBE_ROOT_ENV).is_some()
}

pub(crate) fn run_from_environment() -> Result<(), String> {
    let config = ProbeConfig::from_environment()?;
    match config.action {
        ProbeAction::Prepare => prepare_inputs(&config.root),
        ProbeAction::Batch => run_batch(&config.root),
    }
}

pub(crate) const fn failure_exit_code() -> i32 {
    PROBE_FAILED_EXIT_CODE
}

impl ProbeConfig {
    fn from_environment() -> Result<Self, String> {
        let role = std::env::var(PROCESS_ROLE_ENV)
            .map_err(|_| format!("{PROCESS_ROLE_ENV} precisa ser {GLOBAL_PROCESS_ROLE}."))?;
        if role != GLOBAL_PROCESS_ROLE {
            return Err(format!(
                "{PROJECT_CORE_PROBE_ROOT_ENV} exige {PROCESS_ROLE_ENV}={GLOBAL_PROCESS_ROLE}."
            ));
        }
        let root = std::env::var_os(PROJECT_CORE_PROBE_ROOT_ENV)
            .map(PathBuf::from)
            .ok_or_else(|| format!("{PROJECT_CORE_PROBE_ROOT_ENV} não foi informado."))?;
        validate_probe_root(&root)?;
        let action = std::env::var(PROJECT_CORE_PROBE_ACTION_ENV)
            .map_err(|_| format!("{PROJECT_CORE_PROBE_ACTION_ENV} não foi informado."))?
            .parse()?;
        Ok(Self { root, action })
    }
}

impl std::str::FromStr for ProbeAction {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "prepare" => Ok(Self::Prepare),
            "batch" => Ok(Self::Batch),
            _ => Err(format!(
                "{PROJECT_CORE_PROBE_ACTION_ENV} deve ser prepare ou batch."
            )),
        }
    }
}

fn prepare_inputs(root: &Path) -> Result<(), String> {
    let inputs = root.join("inputs");
    std::fs::create_dir(&inputs)
        .map_err(|error| format!("Não foi possível criar as entradas do probe: {error}"))?;
    for (sample, file_name) in input_specs() {
        let source = sample
            .persisted_source(2)
            .map_err(|error| format!("Não foi possível criar a revisão do probe: {error}"))?;
        write_new_file(&inputs.join(file_name), source.as_bytes())?;
    }
    write_json_atomic_new(
        &root.join("prepared.json"),
        &PreparedEvent {
            schema_version: PROBE_SCHEMA_VERSION,
            process_id: std::process::id(),
            state: "prepared",
            input_count: 2,
        },
    )
}

fn run_batch(root: &Path) -> Result<(), String> {
    let inputs = root.join("inputs");
    let outputs = root.join("outputs");
    validate_regular_directory(&inputs)?;
    std::fs::create_dir(&outputs)
        .map_err(|error| format!("Não foi possível criar as saídas do probe: {error}"))?;
    for (_, file_name) in input_specs() {
        validate_regular_file(&inputs.join(file_name))?;
    }

    write_json_atomic_new(
        &root.join("ready.json"),
        &ReadyEvent {
            schema_version: PROBE_SCHEMA_VERSION,
            process_id: std::process::id(),
            state: "ready",
            run_mode: RUN_MODE,
            process_role: GLOBAL_PROCESS_ROLE,
            project_host_constructed: false,
            editable_project_owned: false,
        },
    )?;
    wait_for_file_blocking(
        &root.join("continue.signal"),
        "A autorização do runner para iniciar o lote",
    )?;

    let core = ProjectCore::new();
    let mut loaded_items = Vec::new();
    let mut batch_items = Vec::new();
    for (index, (_, file_name)) in input_specs().into_iter().enumerate() {
        let source = std::fs::read_to_string(inputs.join(file_name))
            .map_err(|error| format!("Não foi possível ler a revisão persistida: {error}"))?;
        let revision = core
            .load_persisted_revision(&source)
            .map_err(|error| format!("Não foi possível carregar a revisão persistida: {error}"))?;
        let snapshot = revision.render_snapshot();
        let sheet_id = snapshot
            .composition
            .sheets
            .first()
            .ok_or_else(|| "A revisão persistida do probe não contém Lâminas.".to_string())?
            .sheet_id
            .clone();
        let item_id = format!("item-{index}");
        let request_id = format!("project-core-{index}");
        loaded_items.push(LoadedItem {
            item_id: item_id.clone(),
            project_id: snapshot.project_id.clone(),
            revision: revision.revision(),
        });
        batch_items.push(BatchItem::from_persisted_revision(
            item_id,
            revision,
            vec![ExportOptions::new(
                request_id,
                outputs.join(format!("output-{index}.png")),
                sheet_id,
                25,
                None,
            )],
        )?);
    }
    let plan = BatchPlan::new(batch_items)?;
    let mut path_context = OperationPathContext::new();
    for path in plan.required_paths() {
        path_context
            .capture(path)
            .map_err(|error| format!("Não foi possível capturar o Destino do lote: {error}"))?;
    }
    let bindings = path_context.freeze();
    let app_paths = AppPaths::from_known_folders(
        &root.join("runtime").join("roaming"),
        &root.join("runtime").join("local"),
    );
    let gate = OperationGate::new(&app_paths);
    let cache = CacheEngine::default();
    let processor = ImagingProcessor::default();
    let events = Mutex::new(Vec::new());
    let observe = |event| {
        events
            .lock()
            .expect("the ProjectCore probe event capture remains available")
            .push(event);
    };
    let mut transport = ProbeTransport::default();
    let result = tauri::async_runtime::block_on(async {
        let lease =
            OperationLease::acquire(&gate, &cache, &processor, OperationMode::BatchExclusive)
                .await
                .map_err(|error| error.to_string())?;
        BatchRunner::execute(&lease, &mut transport, plan, &bindings, &observe)
            .await
            .map_err(|error| error.to_string())
    })?;
    let captured_events = events
        .into_inner()
        .map_err(|_| "Os eventos do lote ficaram indisponíveis.".to_string())?;
    let batch_completed_event_count = u32::try_from(
        captured_events
            .iter()
            .filter(|event| matches!(event, BatchEvent::Completed { .. }))
            .count(),
    )
    .map_err(|_| "A quantidade de eventos do lote excedeu o limite.".to_string())?;

    write_json_atomic_new(
        &root.join("completed.json"),
        &CompletedEvent {
            schema_version: PROBE_SCHEMA_VERSION,
            process_id: std::process::id(),
            state: "completed",
            run_mode: RUN_MODE,
            process_role: GLOBAL_PROCESS_ROLE,
            project_host_constructed: false,
            editable_project_owned: false,
            input_type: "loaded_project_revision",
            loaded_revision_count: u32::try_from(loaded_items.len())
                .map_err(|_| "O probe carregou revisões demais.".to_string())?,
            completed_item_count: result.completed_items,
            published_output_count: u32::try_from(result.published_outputs.len())
                .map_err(|_| "O probe publicou saídas demais.".to_string())?,
            batch_completed_event_count,
            items: loaded_items,
            renders: transport.renders,
        },
    )
}

impl ImagingTransport for ProbeTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a crate::imaging_processor::InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let result = (|| {
            if control.is_cancelled() {
                return Err(InvocationFailure::cancelled(std::process::id()));
            }
            if operation != ImagingOperation::Export || attempt != 1 {
                return Err(InvocationFailure::at_stage(
                    InvocationFailureStage::ImagingProcess,
                    None,
                    "O transport do probe recebeu uma invocação inesperada.",
                ));
            }
            let ImagingCommand::Render(request) = command else {
                return Err(InvocationFailure::at_stage(
                    InvocationFailureStage::ImagingProcess,
                    None,
                    "O transport do probe aceita somente Render.",
                ));
            };
            let payload = format!(
                "{}:{}:{}",
                request.snapshot.project_id, request.snapshot.revision, request.request_id
            )
            .into_bytes();
            std::fs::write(&request.prepared_output_path, &payload).map_err(|error| {
                InvocationFailure::at_stage(
                    InvocationFailureStage::ImagingProcess,
                    None,
                    format!("Não foi possível produzir a saída do probe: {error}"),
                )
            })?;
            let output_sha256 = format!("{:x}", Sha256::digest(&payload));
            let output_bytes = u64::try_from(payload.len()).expect("the probe payload fits u64");
            self.renders.push(RenderedRevision {
                request_id: request.request_id.clone(),
                project_id: request.snapshot.project_id.clone(),
                revision: request.snapshot.revision,
                output_bytes,
                output_sha256: output_sha256.clone(),
            });
            Ok(ImagingResponse::completed(
                &request.request_id,
                RenderCompletion {
                    width_px: 10,
                    height_px: 5,
                    dpi: request.dpi,
                    source_count: 0,
                    source_bytes: 0,
                    output_bytes,
                    output_sha256,
                },
            ))
        })();
        Box::pin(async move { result })
    }
}

fn input_specs() -> [(SampleProject, &'static str); 2] {
    [
        (SampleProject::Horizon, "Horizon.myalbum"),
        (SampleProject::Aurora, "Aurora.myalbum"),
    ]
}

fn validate_regular_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Não foi possível inspecionar a pasta do probe: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("A pasta de entradas do probe é inválida.".into());
    }
    Ok(())
}

fn validate_regular_file(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("Não foi possível inspecionar a entrada do probe: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("A entrada persistida do probe é inválida.".into());
    }
    Ok(())
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Não foi possível criar a entrada do probe: {error}"))?;
    file.write_all(contents)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Não foi possível sincronizar a entrada do probe: {error}"))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{GLOBAL_PROCESS_ROLE, ProbeAction, RUN_MODE, ReadyEvent};

    #[test]
    fn probe_actions_are_closed_to_the_two_headless_modes() {
        assert_eq!(ProbeAction::from_str("prepare"), Ok(ProbeAction::Prepare));
        assert_eq!(ProbeAction::from_str("batch"), Ok(ProbeAction::Batch));
        assert!(ProbeAction::from_str("interactive").is_err());
    }

    #[test]
    fn ready_event_states_that_no_editable_host_was_constructed() {
        let value = serde_json::to_value(ReadyEvent {
            schema_version: 1,
            process_id: 42,
            state: "ready",
            run_mode: RUN_MODE,
            process_role: GLOBAL_PROCESS_ROLE,
            project_host_constructed: false,
            editable_project_owned: false,
        })
        .expect("the ready event serializes");

        assert_eq!(value["runMode"], RUN_MODE);
        assert_eq!(value["processRole"], GLOBAL_PROCESS_ROLE);
        assert_eq!(value["projectHostConstructed"], false);
        assert_eq!(value["editableProjectOwned"], false);
        assert_eq!(value.as_object().expect("the event is an object").len(), 7);
    }
}
