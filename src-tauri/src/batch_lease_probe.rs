use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use myalbuns_core::ProjectCore;
use myalbuns_paths::ExportPathPlan;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use crate::{
    batch_runner::{BatchEvent, BatchItem, BatchPlan, BatchRunFailure, BatchRunner},
    cache_engine::CacheEngine,
    export_pipeline::{ExportFailureStage, ExportOptions},
    export_probe_commands::{ExportCommandError, ExportResult},
    imaging_processor::ImagingProcessor,
    logging::LoggingState,
    operation_gate::OperationGate,
    path_io,
    probe_support::{
        ExportProbeCapture as ProbeCapture, capture_snapshot, execute_real_export,
        observing_channel, optional_utf8_environment, validate_probe_root,
        verify_and_remove_output, wait_for_file_async, wait_for_file_blocking,
        write_json_atomic_new,
    },
    sample_project::SampleProject,
    topology_spike::TopologySpike,
};

pub(crate) const BATCH_LEASE_PROBE_ROOT_ENV: &str = "MYALBUNS_BATCH_LEASE_PROBE_ROOT";
pub(crate) const BATCH_LEASE_PROBE_SCENARIO_ENV: &str = "MYALBUNS_BATCH_LEASE_PROBE_SCENARIO";

const LEASE_RESOURCES: [&str; 3] = ["operation_gate", "cache_pause", "processor_reservation"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeScenario {
    Success,
    BeforePreparation,
    BetweenPromotions,
    OwnerDeath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeRole {
    Owner,
    Challenger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeState {
    OwnerReady,
    ChallengerConflict,
    BetweenItemsReady,
    BetweenItemsConflict,
    OwnerTerminal,
    SuccessorSuccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeTerminal {
    Success,
    Failed,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeResourceState {
    Held,
    Blocked,
    Released,
    Reacquired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvidence {
    name: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeEvent {
    schema_version: u32,
    process_id: u32,
    role: ProbeRole,
    scenario: ProbeScenario,
    state: ProbeState,
    operation_mode: &'static str,
    operation_id: Option<String>,
    terminal: Option<ProbeTerminal>,
    item_index: Option<u32>,
    total_items: u32,
    completed_items: u32,
    promoted_outputs: Option<u32>,
    total_outputs: Option<u32>,
    failure_stage: Option<String>,
    progress_stages: Vec<String>,
    resources: Vec<&'static str>,
    resource_state: ProbeResourceState,
    output_evidence: Vec<OutputEvidence>,
    output_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeFailure<'a> {
    schema_version: u32,
    process_id: u32,
    role: ProbeRole,
    scenario: ProbeScenario,
    reason: &'a str,
}

#[derive(Clone, Debug, Default)]
struct OwnerCapture {
    progress_stages: Vec<String>,
    item_completed: Vec<u32>,
    completed: bool,
    failure: Option<String>,
}

struct BatchFixture {
    plan: BatchPlan,
    output_paths: Vec<PathBuf>,
    injected_preparation: Option<PathBuf>,
}

struct OwnerTerminalOutcome {
    terminal: ProbeTerminal,
    completed_items: u32,
    promoted_outputs: Option<u32>,
    total_outputs: Option<u32>,
    failure_stage: Option<String>,
}

pub(crate) struct BatchLeaseProbe {
    root: PathBuf,
    scenario: ProbeScenario,
    role: ProbeRole,
}

impl BatchLeaseProbe {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Option<Self>, String> {
        let root = optional_utf8_environment(BATCH_LEASE_PROBE_ROOT_ENV)?;
        let scenario = optional_utf8_environment(BATCH_LEASE_PROBE_SCENARIO_ENV)?;
        Self::from_values(
            root.as_deref().map(Path::new),
            scenario.as_deref(),
            topology,
        )
    }

    fn from_values(
        root: Option<&Path>,
        scenario: Option<&str>,
        topology: &TopologySpike,
    ) -> Result<Option<Self>, String> {
        let (root, scenario) = match (root, scenario) {
            (None, None) => return Ok(None),
            (Some(root), Some(scenario)) => (root, ProbeScenario::parse(scenario)?),
            _ => {
                return Err(format!(
                    "{BATCH_LEASE_PROBE_ROOT_ENV} e {BATCH_LEASE_PROBE_SCENARIO_ENV} precisam ser definidos juntos."
                ));
            }
        };
        if topology.label() != "independent" {
            return Err("O probe do lote exige dois hosts independentes.".into());
        }
        validate_probe_root(root)?;
        let windows = topology.project_windows();
        let [(_, sample)] = windows.as_slice() else {
            return Err("O probe do lote exige exatamente um Projeto por host.".into());
        };
        let role = if *sample == SampleProject::Horizon {
            ProbeRole::Owner
        } else {
            ProbeRole::Challenger
        };
        Ok(Some(Self {
            root: root.to_path_buf(),
            scenario,
            role,
        }))
    }

    #[cfg(test)]
    fn role(&self) -> ProbeRole {
        self.role
    }

    pub(crate) fn start(self, app: &AppHandle) -> Result<(), String> {
        let app = app.clone();
        thread::Builder::new()
            .name(format!(
                "batch-lease-probe-{}-{}",
                self.role.as_str(),
                self.scenario.as_str()
            ))
            .spawn(move || {
                let result = tauri::async_runtime::block_on(async {
                    match self.role {
                        ProbeRole::Owner => run_owner(&app, &self.root, self.scenario).await,
                        ProbeRole::Challenger => {
                            run_challenger(&app, &self.root, self.scenario).await
                        }
                    }
                });
                if let Err(reason) = result {
                    let _ = write_failure(&self.root, self.role, self.scenario, &reason);
                }
            })
            .map_err(|error| format!("Não foi possível iniciar o probe do lote: {error}"))?;
        Ok(())
    }
}

impl ProbeScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "success" => Ok(Self::Success),
            "before_preparation" => Ok(Self::BeforePreparation),
            "between_promotions" => Ok(Self::BetweenPromotions),
            "owner_death" => Ok(Self::OwnerDeath),
            _ => Err(format!(
                "Valor inválido em {BATCH_LEASE_PROBE_SCENARIO_ENV}: {value}."
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::BeforePreparation => "before_preparation",
            Self::BetweenPromotions => "between_promotions",
            Self::OwnerDeath => "owner_death",
        }
    }

    const fn total_items(self) -> u32 {
        match self {
            Self::Success | Self::BeforePreparation => 2,
            Self::BetweenPromotions | Self::OwnerDeath => 1,
        }
    }

    const fn has_between_items_barrier(self) -> bool {
        matches!(self, Self::Success | Self::BeforePreparation)
    }
}

impl ProbeRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Challenger => "challenger",
        }
    }
}

impl ProbeState {
    const fn file_name(self) -> &'static str {
        match self {
            Self::OwnerReady => "owner-ready.json",
            Self::ChallengerConflict => "challenger-conflict.json",
            Self::BetweenItemsReady => "between-items-ready.json",
            Self::BetweenItemsConflict => "between-items-conflict.json",
            Self::OwnerTerminal => "owner-terminal.json",
            Self::SuccessorSuccess => "successor-success.json",
        }
    }
}

async fn run_owner(app: &AppHandle, root: &Path, scenario: ProbeScenario) -> Result<(), String> {
    let fixture = build_fixture(root, scenario)?;
    let operation_paths = fixture
        .plan
        .required_paths()
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    let root_bindings = path_io::capture_root_bindings(operation_paths).await?;
    let capture = Arc::new(Mutex::new(OwnerCapture::default()));
    let observed = Arc::clone(&capture);
    let root_for_events = root.to_path_buf();
    let observe = move |event: BatchEvent| {
        if let Err(reason) = observe_owner_event(&root_for_events, scenario, &observed, event) {
            record_owner_failure(&observed, reason);
        }
    };

    let result = BatchRunner::run(
        app,
        &app.state::<LoggingState>(),
        &app.state::<OperationGate>(),
        &app.state::<CacheEngine>(),
        &app.state::<ImagingProcessor>(),
        fixture.plan,
        &root_bindings,
        &observe,
    )
    .await;
    let capture = capture
        .lock()
        .expect("the batch owner capture remains available")
        .clone();
    if let Some(reason) = capture.failure {
        return Err(reason);
    }
    if scenario == ProbeScenario::OwnerDeath {
        return Err("O owner_death retornou sem o encerramento externo do processo.".into());
    }

    let outcome = validate_owner_result(scenario, result, &capture)?;
    if let Some(injected_preparation) = &fixture.injected_preparation {
        std::fs::remove_dir(injected_preparation).map_err(|error| {
            format!("Não foi possível remover a injeção anterior à preparação: {error}")
        })?;
    }
    let evidence_paths = if scenario == ProbeScenario::BetweenPromotions {
        &fixture.output_paths[..1]
    } else {
        fixture.output_paths.as_slice()
    };
    let output_evidence = readable_output_evidence(evidence_paths)?;
    let successor_completed_items = outcome.completed_items;
    write_event(
        root,
        &ProbeEvent {
            schema_version: 1,
            process_id: std::process::id(),
            role: ProbeRole::Owner,
            scenario,
            state: ProbeState::OwnerTerminal,
            operation_mode: "batch_exclusive",
            operation_id: Some(batch_operation_id(scenario)),
            terminal: Some(outcome.terminal),
            item_index: match scenario {
                ProbeScenario::BeforePreparation => Some(1),
                ProbeScenario::BetweenPromotions => Some(0),
                ProbeScenario::Success | ProbeScenario::OwnerDeath => None,
            },
            total_items: scenario.total_items(),
            completed_items: outcome.completed_items,
            promoted_outputs: outcome.promoted_outputs,
            total_outputs: outcome.total_outputs,
            failure_stage: outcome.failure_stage,
            progress_stages: capture.progress_stages,
            resources: LEASE_RESOURCES.to_vec(),
            resource_state: ProbeResourceState::Released,
            output_evidence,
            output_bytes: None,
        },
    )?;

    let successor_bytes = execute_normal_successor(app).await?;
    write_event(
        root,
        &ProbeEvent {
            schema_version: 1,
            process_id: std::process::id(),
            role: ProbeRole::Owner,
            scenario,
            state: ProbeState::SuccessorSuccess,
            operation_mode: "normal_export",
            operation_id: None,
            terminal: Some(ProbeTerminal::Success),
            item_index: None,
            total_items: scenario.total_items(),
            completed_items: successor_completed_items,
            promoted_outputs: None,
            total_outputs: None,
            failure_stage: None,
            progress_stages: Vec::new(),
            resources: LEASE_RESOURCES.to_vec(),
            resource_state: ProbeResourceState::Reacquired,
            output_evidence: Vec::new(),
            output_bytes: Some(successor_bytes),
        },
    )
}

async fn run_challenger(
    app: &AppHandle,
    root: &Path,
    scenario: ProbeScenario,
) -> Result<(), String> {
    wait_for_file_async(&root.join("owner-ready.json"), "o início do lote").await?;
    expect_normal_conflict(app).await?;
    write_event(
        root,
        &conflict_event(scenario, ProbeState::ChallengerConflict, 0),
    )?;

    if scenario.has_between_items_barrier() {
        wait_for_file_async(
            &root.join("between-items-ready.json"),
            "o intervalo entre os itens do lote",
        )
        .await?;
        expect_normal_conflict(app).await?;
        write_event(
            root,
            &conflict_event(scenario, ProbeState::BetweenItemsConflict, 1),
        )?;
    }

    if scenario == ProbeScenario::OwnerDeath {
        wait_for_file_async(
            &root.join("allow-successor"),
            "a autorização da Exportação sucessora",
        )
        .await?;
        let output_bytes = execute_normal_successor(app).await?;
        write_event(
            root,
            &ProbeEvent {
                schema_version: 1,
                process_id: std::process::id(),
                role: ProbeRole::Challenger,
                scenario,
                state: ProbeState::SuccessorSuccess,
                operation_mode: "normal_export",
                operation_id: None,
                terminal: Some(ProbeTerminal::Success),
                item_index: None,
                total_items: scenario.total_items(),
                completed_items: 0,
                promoted_outputs: None,
                total_outputs: None,
                failure_stage: None,
                progress_stages: Vec::new(),
                resources: LEASE_RESOURCES.to_vec(),
                resource_state: ProbeResourceState::Reacquired,
                output_evidence: Vec::new(),
                output_bytes: Some(output_bytes),
            },
        )?;
    }
    Ok(())
}

fn observe_owner_event(
    root: &Path,
    scenario: ProbeScenario,
    capture: &Arc<Mutex<OwnerCapture>>,
    event: BatchEvent,
) -> Result<(), String> {
    match event {
        BatchEvent::Started { total_items } => {
            if total_items != scenario.total_items() {
                return Err("O BatchRunner iniciou com quantidade de itens inesperada.".into());
            }
            write_event(
                root,
                &ProbeEvent {
                    schema_version: 1,
                    process_id: std::process::id(),
                    role: ProbeRole::Owner,
                    scenario,
                    state: ProbeState::OwnerReady,
                    operation_mode: "batch_exclusive",
                    operation_id: Some(batch_operation_id(scenario)),
                    terminal: None,
                    item_index: None,
                    total_items,
                    completed_items: 0,
                    promoted_outputs: None,
                    total_outputs: None,
                    failure_stage: None,
                    progress_stages: Vec::new(),
                    resources: LEASE_RESOURCES.to_vec(),
                    resource_state: ProbeResourceState::Held,
                    output_evidence: Vec::new(),
                    output_bytes: None,
                },
            )?;
            wait_for_file_blocking(
                &root.join("allow-batch-start"),
                "a autorização para executar o lote",
            )?;
        }
        BatchEvent::ItemStarted { .. } => {}
        BatchEvent::ItemProgress {
            item_index,
            progress,
            ..
        } => {
            capture
                .lock()
                .expect("the batch owner capture remains available")
                .progress_stages
                .push(format!("{item_index}:{}", progress.stage.as_str()));
        }
        BatchEvent::ItemCompleted {
            item_index,
            total_items,
            ..
        } => {
            capture
                .lock()
                .expect("the batch owner capture remains available")
                .item_completed
                .push(item_index);
            if item_index == 0 && scenario.has_between_items_barrier() {
                write_event(
                    root,
                    &ProbeEvent {
                        schema_version: 1,
                        process_id: std::process::id(),
                        role: ProbeRole::Owner,
                        scenario,
                        state: ProbeState::BetweenItemsReady,
                        operation_mode: "batch_exclusive",
                        operation_id: Some(batch_operation_id(scenario)),
                        terminal: None,
                        item_index: Some(0),
                        total_items,
                        completed_items: 1,
                        promoted_outputs: None,
                        total_outputs: None,
                        failure_stage: None,
                        progress_stages: Vec::new(),
                        resources: LEASE_RESOURCES.to_vec(),
                        resource_state: ProbeResourceState::Held,
                        output_evidence: Vec::new(),
                        output_bytes: None,
                    },
                )?;
                wait_for_file_blocking(
                    &root.join("allow-next-item"),
                    "a autorização para o próximo item do lote",
                )?;
            }
        }
        BatchEvent::Completed { .. } => {
            capture
                .lock()
                .expect("the batch owner capture remains available")
                .completed = true;
        }
    }
    Ok(())
}

fn validate_owner_result(
    scenario: ProbeScenario,
    result: Result<crate::batch_runner::BatchRunResult, BatchRunFailure>,
    capture: &OwnerCapture,
) -> Result<OwnerTerminalOutcome, String> {
    match scenario {
        ProbeScenario::Success => {
            let result = result.map_err(|error| format!("O lote deveria concluir: {error}"))?;
            if result.completed_items != 2
                || result.published_outputs.len() != 2
                || capture.item_completed != [0, 1]
                || !capture.completed
            {
                return Err("O lote de sucesso não percorreu dois itens serialmente.".into());
            }
            Ok(OwnerTerminalOutcome {
                terminal: ProbeTerminal::Success,
                completed_items: 2,
                promoted_outputs: Some(2),
                total_outputs: Some(2),
                failure_stage: None,
            })
        }
        ProbeScenario::BeforePreparation => {
            let error = result.expect_err("the injected preparation must fail");
            if error.completed_items() != 1
                || error.item_index() != Some(1)
                || error.item_id() != Some("item-1")
                || error.export_stage() != Some(ExportFailureStage::Prepare)
                || capture.item_completed != [0]
                || capture.completed
            {
                return Err("A falha anterior à preparação ocorreu fora do ponto esperado.".into());
            }
            Ok(OwnerTerminalOutcome {
                terminal: ProbeTerminal::Failed,
                completed_items: 1,
                promoted_outputs: Some(0),
                total_outputs: Some(1),
                failure_stage: Some(ExportFailureStage::Prepare.as_str().into()),
            })
        }
        ProbeScenario::BetweenPromotions => {
            let error = result.expect_err("the second promotion must fail");
            let expected_stage = ExportFailureStage::Publish {
                promoted_outputs: 1,
                total_outputs: 2,
            };
            if error.completed_items() != 0
                || error.item_index() != Some(0)
                || error.item_id() != Some("item-0")
                || error.export_stage() != Some(expected_stage)
                || !capture.item_completed.is_empty()
                || capture.completed
            {
                return Err("A falha entre promoções ocorreu fora do ponto esperado.".into());
            }
            Ok(OwnerTerminalOutcome {
                terminal: ProbeTerminal::Failed,
                completed_items: 0,
                promoted_outputs: Some(1),
                total_outputs: Some(2),
                failure_stage: Some(expected_stage.as_str().into()),
            })
        }
        ProbeScenario::OwnerDeath => unreachable!("owner_death has no cooperative terminal"),
    }
}

fn build_fixture(root: &Path, scenario: ProbeScenario) -> Result<BatchFixture, String> {
    let destination = root.join("destination");
    std::fs::create_dir(&destination)
        .map_err(|error| format!("Não foi possível criar o Destino do lote: {error}"))?;
    let output_paths = output_paths_for(&destination, scenario);
    if scenario == ProbeScenario::BetweenPromotions {
        std::fs::write(&output_paths[0], b"previous-first-output")
            .and_then(|_| std::fs::write(&output_paths[1], b"previous-second-output"))
            .map_err(|error| format!("Não foi possível criar as saídas anteriores: {error}"))?;
    }

    let item_specs: Vec<(SampleProject, Vec<(String, PathBuf)>)> = match scenario {
        ProbeScenario::Success | ProbeScenario::BeforePreparation => vec![
            (
                SampleProject::Horizon,
                vec![(request_id(scenario, 0, 0), output_paths[0].clone())],
            ),
            (
                SampleProject::Aurora,
                vec![(request_id(scenario, 1, 0), output_paths[1].clone())],
            ),
        ],
        ProbeScenario::BetweenPromotions => vec![(
            SampleProject::Horizon,
            vec![
                (request_id(scenario, 0, 0), output_paths[0].clone()),
                (request_id(scenario, 0, 1), output_paths[1].clone()),
            ],
        )],
        ProbeScenario::OwnerDeath => vec![(
            SampleProject::Horizon,
            vec![(request_id(scenario, 0, 0), output_paths[0].clone())],
        )],
    };

    let mut items = Vec::with_capacity(item_specs.len());
    for (item_index, (sample, outputs)) in item_specs.into_iter().enumerate() {
        let source = sample
            .persisted_source(2)
            .map_err(|error| format!("Não foi possível criar o Projeto do lote: {error}"))?;
        let revision = ProjectCore::new()
            .load_persisted_revision(&source)
            .map_err(|error| format!("Não foi possível carregar a revisão persistida: {error}"))?;
        let snapshot = revision.render_snapshot();
        let sheet_id = snapshot
            .composition
            .sheets
            .first()
            .ok_or_else(|| "A revisão persistida não contém Lâminas.".to_string())?
            .sheet_id
            .clone();
        let mut output_options = Vec::with_capacity(outputs.len());
        for (request_id, output_path) in outputs {
            output_options.push(ExportOptions::new(
                request_id,
                output_path,
                sheet_id.clone(),
                25,
                None,
            ));
        }
        items.push(BatchItem::from_persisted_revision(
            format!("item-{item_index}"),
            revision,
            output_options,
        )?);
    }
    let plan = BatchPlan::new(items)?;
    let injected_preparation = if scenario == ProbeScenario::BeforePreparation {
        let path_plan = ExportPathPlan::new(output_paths[1].clone(), &request_id(scenario, 1, 0))
            .map_err(|error| format!("Não foi possível planejar a injeção: {error}"))?;
        std::fs::create_dir(path_plan.preparation_directory()).map_err(|error| {
            format!("Não foi possível injetar a falha anterior à preparação: {error}")
        })?;
        Some(path_plan.preparation_directory().to_path_buf())
    } else {
        None
    };
    Ok(BatchFixture {
        plan,
        output_paths,
        injected_preparation,
    })
}

fn output_paths_for(destination: &Path, scenario: ProbeScenario) -> Vec<PathBuf> {
    match scenario {
        ProbeScenario::Success => vec![
            destination.join("success-item-1.png"),
            destination.join("success-item-2.png"),
        ],
        ProbeScenario::BeforePreparation => vec![
            destination.join("before-preparation-item-1.png"),
            destination.join("before-preparation-item-2.png"),
        ],
        ProbeScenario::BetweenPromotions => vec![
            destination.join("between-promotions-1.png"),
            destination.join("between-promotions-2.png"),
        ],
        ProbeScenario::OwnerDeath => vec![destination.join("owner-death.png")],
    }
}

fn request_id(scenario: ProbeScenario, item_index: u32, output_index: u32) -> String {
    format!(
        "batch-{}-{item_index}-{output_index}",
        scenario.as_str().replace('_', "-")
    )
}

fn batch_operation_id(scenario: ProbeScenario) -> String {
    format!("batch-{}-{}", std::process::id(), scenario.as_str())
}

async fn expect_normal_conflict(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "A Janela do challenger não existe.".to_string())?;
    let capture = Arc::new(Mutex::new(ProbeCapture::default()));
    let result = execute_real_export(app, &window, observing_channel(Arc::clone(&capture))).await;
    let capture = capture_snapshot(&capture);
    if let Some(reason) = capture.failure() {
        cleanup_normal_success(&result)?;
        return Err(reason.to_string());
    }
    match result {
        Err(error)
            if error.code() == "conflict"
                && capture.operation_id().is_none()
                && capture.progress_stages().is_empty()
                && capture.cancellation_disposition().is_none() =>
        {
            Ok(())
        }
        Ok(result) => {
            verify_and_remove_output(&result)?;
            Err("A Exportação normal avançou durante o BatchExclusive.".into())
        }
        Err(error) => Err(format!(
            "A Exportação normal deveria receber conflict, mas retornou {}: {}",
            error.code(),
            error.message()
        )),
    }
}

async fn execute_normal_successor(app: &AppHandle) -> Result<u64, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "A Janela da Exportação sucessora não existe.".to_string())?;
    let capture = Arc::new(Mutex::new(ProbeCapture::default()));
    let result = execute_real_export(app, &window, observing_channel(Arc::clone(&capture))).await;
    let capture = capture_snapshot(&capture);
    if let Some(reason) = capture.failure() {
        cleanup_normal_success(&result)?;
        return Err(reason.to_string());
    }
    let result = result.map_err(|error| {
        format!(
            "A Exportação normal sucessora deveria concluir, mas retornou {}: {}",
            error.code(),
            error.message()
        )
    })?;
    verify_and_remove_output(&result)
}

fn cleanup_normal_success(result: &Result<ExportResult, ExportCommandError>) -> Result<(), String> {
    if let Ok(result) = result {
        verify_and_remove_output(result)?;
    }
    Ok(())
}

fn conflict_event(scenario: ProbeScenario, state: ProbeState, completed_items: u32) -> ProbeEvent {
    ProbeEvent {
        schema_version: 1,
        process_id: std::process::id(),
        role: ProbeRole::Challenger,
        scenario,
        state,
        operation_mode: "normal_export",
        operation_id: None,
        terminal: Some(ProbeTerminal::Conflict),
        item_index: (completed_items > 0).then_some(completed_items - 1),
        total_items: scenario.total_items(),
        completed_items,
        promoted_outputs: None,
        total_outputs: None,
        failure_stage: None,
        progress_stages: Vec::new(),
        resources: Vec::new(),
        resource_state: ProbeResourceState::Blocked,
        output_evidence: Vec::new(),
        output_bytes: None,
    }
}

fn readable_output_evidence(paths: &[PathBuf]) -> Result<Vec<OutputEvidence>, String> {
    paths
        .iter()
        .filter(|path| path.is_file())
        .map(|path| {
            let metadata = std::fs::symlink_metadata(path).map_err(|error| {
                format!("Não foi possível inspecionar a saída do lote: {error}")
            })?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err("A saída do lote não é um arquivo regular.".into());
            }
            let bytes = std::fs::read(path)
                .map_err(|error| format!("Não foi possível ler a saída do lote: {error}"))?;
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| "A saída do lote possui nome inválido.".to_string())?
                .to_owned();
            Ok(OutputEvidence {
                name,
                bytes: metadata.len(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
            })
        })
        .collect()
}

fn record_owner_failure(capture: &Arc<Mutex<OwnerCapture>>, reason: String) {
    let mut capture = capture
        .lock()
        .expect("the batch owner capture remains available");
    if capture.failure.is_none() {
        capture.failure = Some(reason);
    }
}

fn write_failure(
    root: &Path,
    role: ProbeRole,
    scenario: ProbeScenario,
    reason: &str,
) -> Result<(), String> {
    write_json_atomic_new(
        &root.join(format!(
            "failure-{}-{}.json",
            role.as_str(),
            std::process::id()
        )),
        &ProbeFailure {
            schema_version: 1,
            process_id: std::process::id(),
            role,
            scenario,
            reason,
        },
    )
}

fn write_event(root: &Path, event: &ProbeEvent) -> Result<(), String> {
    write_json_atomic_new(&root.join(event.state.file_name()), event)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{BatchLeaseProbe, ProbeRole};
    use crate::topology_spike::TopologySpike;

    fn probe_root() -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempdir().expect("the batch probe fixture exists");
        let root = directory.path().join(".scratch").join("batch-lease-probe");
        std::fs::create_dir_all(&root).expect("the batch probe root exists");
        (directory, root)
    }

    #[test]
    fn remains_disabled_when_both_batch_probe_values_are_absent() {
        let topology = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the independent topology is valid");

        assert!(
            BatchLeaseProbe::from_values(None, None, &topology)
                .expect("an absent probe is valid")
                .is_none()
        );
    }

    #[test]
    fn rejects_partial_invalid_and_non_independent_configuration() {
        let (_directory, root) = probe_root();
        let independent = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the independent topology is valid");
        let multiwindow = TopologySpike::from_values(Some("multiwindow"), None)
            .expect("the multiwindow topology is valid");
        let standard =
            TopologySpike::from_values(None, None).expect("the standard topology is valid");

        assert!(BatchLeaseProbe::from_values(Some(&root), None, &independent).is_err());
        assert!(BatchLeaseProbe::from_values(None, Some("success"), &independent).is_err());
        assert!(BatchLeaseProbe::from_values(Some(&root), Some("unknown"), &independent).is_err());
        assert!(BatchLeaseProbe::from_values(Some(&root), Some("success"), &multiwindow).is_err());
        assert!(BatchLeaseProbe::from_values(Some(&root), Some("success"), &standard).is_err());
    }

    #[test]
    fn maps_horizon_to_owner_and_aurora_to_challenger_for_every_scenario() {
        let (_directory, root) = probe_root();
        let horizon = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the Horizon host is valid");
        let aurora = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("the Aurora host is valid");

        for scenario in [
            "success",
            "before_preparation",
            "between_promotions",
            "owner_death",
        ] {
            let owner = BatchLeaseProbe::from_values(Some(&root), Some(scenario), &horizon)
                .expect("the owner probe configuration is valid")
                .expect("the owner probe is enabled");
            let challenger = BatchLeaseProbe::from_values(Some(&root), Some(scenario), &aurora)
                .expect("the challenger probe configuration is valid")
                .expect("the challenger probe is enabled");

            assert_eq!(owner.role(), ProbeRole::Owner);
            assert_eq!(challenger.role(), ProbeRole::Challenger);
        }
    }
}
