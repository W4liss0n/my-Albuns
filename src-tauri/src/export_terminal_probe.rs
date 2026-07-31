use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use myalbuns_logging::ProcessRole;
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindow, ipc::Channel};

use crate::{
    cache_engine::CacheEngine,
    export_attempts::{CancelDisposition, ExportAttempts},
    export_probe_commands::{self, ExportCommandError, ExportEvent, ExportResult},
    imaging_processor::ImagingProcessor,
    operation_gate::{OperationGate, OperationMode},
    operation_lease::{OperationLease, OperationLeaseError},
    probe_support::{
        ExportProbeCapture as ProbeCapture, PROBE_TIMEOUT, PreparingSnapshot, capture_snapshot,
        execute_real_export, observing_channel, optional_utf8_environment, record_capture_failure,
        record_channel_event, validate_probe_root, verify_and_remove_output, wait_for_file_async,
        wait_for_file_blocking, write_json_atomic_new,
    },
    sample_project::SampleProject,
    topology_spike::TopologySpike,
};

pub(crate) const EXPORT_TERMINAL_PROBE_ROOT_ENV: &str = "MYALBUNS_EXPORT_TERMINAL_PROBE_ROOT";
pub(crate) const EXPORT_TERMINAL_PROBE_SCENARIO_ENV: &str =
    "MYALBUNS_EXPORT_TERMINAL_PROBE_SCENARIO";
pub(crate) const EXPORT_TERMINAL_PROBE_PHASE_ENV: &str = "MYALBUNS_EXPORT_TERMINAL_PROBE_PHASE";

const LEASE_RESOURCES: [&str; 3] = ["operation_gate", "cache_pause", "processor_reservation"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeScenario {
    Success,
    Failure,
    Cancellation,
    OwnerDeath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbePhase {
    Matrix,
    Successor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeRole {
    Owner,
    Challenger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeState {
    OwnerReady,
    ChallengerConflict,
    OwnerTerminal,
    ChallengerSuccess,
}

impl ProbeState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerReady => "owner_ready",
            Self::ChallengerConflict => "challenger_conflict",
            Self::OwnerTerminal => "owner_terminal",
            Self::ChallengerSuccess => "challenger_success",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeTerminal {
    Success,
    Failed,
    Cancelled,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeEvent {
    schema_version: u32,
    process_id: u32,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
    state: ProbeState,
    operation_mode: &'static str,
    operation_id: Option<String>,
    terminal: Option<ProbeTerminal>,
    progress_stages: Vec<String>,
    cancellation_disposition: Option<CancelDisposition>,
    resources: Vec<&'static str>,
    resource_state: ProbeResourceState,
    output_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeFailure<'a> {
    schema_version: u32,
    process_id: u32,
    topology: &'a str,
    window_label: &'a str,
    scenario: ProbeScenario,
    reason: &'a str,
}

impl ProbeEvent {
    fn owner_ready(
        topology: &'static str,
        window_label: &'static str,
        scenario: ProbeScenario,
        operation_id: String,
        progress_stages: Vec<String>,
    ) -> Self {
        Self {
            schema_version: 1,
            process_id: std::process::id(),
            topology,
            window_label,
            scenario,
            state: ProbeState::OwnerReady,
            operation_mode: "normal_export",
            operation_id: Some(operation_id),
            terminal: None,
            progress_stages,
            cancellation_disposition: None,
            resources: LEASE_RESOURCES.to_vec(),
            resource_state: ProbeResourceState::Held,
            output_bytes: None,
        }
    }

    fn challenger_conflict(
        topology: &'static str,
        window_label: &'static str,
        scenario: ProbeScenario,
    ) -> Self {
        Self {
            schema_version: 1,
            process_id: std::process::id(),
            topology,
            window_label,
            scenario,
            state: ProbeState::ChallengerConflict,
            operation_mode: OperationMode::NormalExport.as_str(),
            operation_id: None,
            terminal: Some(ProbeTerminal::Conflict),
            progress_stages: Vec::new(),
            cancellation_disposition: None,
            resources: Vec::new(),
            resource_state: ProbeResourceState::Blocked,
            output_bytes: None,
        }
    }

    fn owner_terminal(
        topology: &'static str,
        window_label: &'static str,
        scenario: ProbeScenario,
        terminal: ProbeTerminal,
        capture: ProbeCapture,
        output_bytes: Option<u64>,
    ) -> Result<Self, String> {
        Ok(Self {
            schema_version: 1,
            process_id: std::process::id(),
            topology,
            window_label,
            scenario,
            state: ProbeState::OwnerTerminal,
            operation_mode: OperationMode::NormalExport.as_str(),
            operation_id: Some(capture.operation_id.ok_or_else(|| {
                "O owner não observou o identificador da Exportação.".to_string()
            })?),
            terminal: Some(terminal),
            progress_stages: capture.progress_stages,
            cancellation_disposition: capture.cancellation_disposition,
            resources: Vec::new(),
            resource_state: ProbeResourceState::Released,
            output_bytes,
        })
    }

    fn challenger_success(
        topology: &'static str,
        window_label: &'static str,
        scenario: ProbeScenario,
        capture: ProbeCapture,
        output_bytes: u64,
    ) -> Result<Self, String> {
        Ok(Self {
            schema_version: 1,
            process_id: std::process::id(),
            topology,
            window_label,
            scenario,
            state: ProbeState::ChallengerSuccess,
            operation_mode: OperationMode::NormalExport.as_str(),
            operation_id: Some(capture.operation_id.ok_or_else(|| {
                "O successor não observou o identificador da Exportação.".to_string()
            })?),
            terminal: Some(ProbeTerminal::Success),
            progress_stages: capture.progress_stages,
            cancellation_disposition: capture.cancellation_disposition,
            resources: LEASE_RESOURCES.to_vec(),
            resource_state: ProbeResourceState::Reacquired,
            output_bytes: Some(output_bytes),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ExportTerminalProbe {
    root: PathBuf,
    topology: &'static str,
    scenario: ProbeScenario,
    phase: ProbePhase,
    participants: Vec<(&'static str, ProbeRole)>,
}

impl ExportTerminalProbe {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Option<Self>, String> {
        let root = std::env::var_os(EXPORT_TERMINAL_PROBE_ROOT_ENV).map(PathBuf::from);
        let scenario = optional_utf8_environment(EXPORT_TERMINAL_PROBE_SCENARIO_ENV)?;
        let phase = optional_utf8_environment(EXPORT_TERMINAL_PROBE_PHASE_ENV)?;
        Self::from_values(
            root.as_deref(),
            scenario.as_deref(),
            phase.as_deref(),
            topology,
        )
    }

    fn from_values(
        root: Option<&Path>,
        scenario: Option<&str>,
        phase: Option<&str>,
        topology: &TopologySpike,
    ) -> Result<Option<Self>, String> {
        match (root, scenario, phase) {
            (None, None, None) => Ok(None),
            (Some(root), Some(scenario), Some(phase)) => {
                validate_probe_root(root)?;
                if !matches!(topology.label(), "independent" | "multiwindow") {
                    return Err(format!(
                        "{EXPORT_TERMINAL_PROBE_ROOT_ENV} só pode ser usado no spike de topologia."
                    ));
                }
                let scenario = ProbeScenario::parse(scenario)?;
                let phase = ProbePhase::parse(phase)?;
                if phase == ProbePhase::Successor && scenario != ProbeScenario::OwnerDeath {
                    return Err(format!(
                        "{EXPORT_TERMINAL_PROBE_PHASE_ENV}=successor só pode ser usado com owner_death."
                    ));
                }
                let participants = participants_for(topology, phase)?;
                Ok(Some(Self {
                    root: root.to_path_buf(),
                    topology: topology.label(),
                    scenario,
                    phase,
                    participants,
                }))
            }
            _ => Err(format!(
                "{EXPORT_TERMINAL_PROBE_ROOT_ENV}, {EXPORT_TERMINAL_PROBE_SCENARIO_ENV} e {EXPORT_TERMINAL_PROBE_PHASE_ENV} devem ser informados juntos."
            )),
        }
    }

    #[cfg(test)]
    fn participants(&self) -> &[(&'static str, ProbeRole)] {
        &self.participants
    }

    pub(crate) fn start(self, app: &AppHandle) -> Result<(), String> {
        for (window_label, _) in &self.participants {
            if app.get_webview_window(window_label).is_none() {
                return Err(format!(
                    "A Janela {window_label} do probe terminal não existe."
                ));
            }
        }

        for (window_label, role) in self.participants {
            let app = app.clone();
            let root = self.root.clone();
            let topology = self.topology;
            let scenario = self.scenario;
            let phase = self.phase;
            thread::Builder::new()
                .name(format!(
                    "export-terminal-{}-{window_label}",
                    scenario.as_str()
                ))
                .spawn(move || {
                    let outcome = tauri::async_runtime::block_on(run_participant(
                        &app,
                        &root,
                        topology,
                        window_label,
                        scenario,
                        phase,
                        role,
                    ));
                    if let Err(reason) = outcome {
                        let _ = write_failure(&root, topology, window_label, scenario, &reason);
                        tracing::error!(
                            target: "myalbuns.desktop",
                            process_role = ProcessRole::DesktopHost.as_str(),
                            process_id = std::process::id(),
                            topology,
                            window_label,
                            scenario = scenario.as_str(),
                            reason,
                            event = "export_terminal_probe_failed",
                        );
                    }
                })
                .map_err(|error| {
                    format!("Não foi possível iniciar a thread do probe terminal: {error}")
                })?;
        }
        Ok(())
    }
}

impl ProbeScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            "cancellation" => Ok(Self::Cancellation),
            "owner_death" => Ok(Self::OwnerDeath),
            _ => Err(format!(
                "Valor inválido em {EXPORT_TERMINAL_PROBE_SCENARIO_ENV}: {value}."
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancellation => "cancellation",
            Self::OwnerDeath => "owner_death",
        }
    }
}

impl ProbePhase {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "matrix" => Ok(Self::Matrix),
            "successor" => Ok(Self::Successor),
            _ => Err(format!(
                "Valor inválido em {EXPORT_TERMINAL_PROBE_PHASE_ENV}: {value}."
            )),
        }
    }
}

fn participants_for(
    topology: &TopologySpike,
    phase: ProbePhase,
) -> Result<Vec<(&'static str, ProbeRole)>, String> {
    let windows = topology.project_windows();
    if phase == ProbePhase::Successor {
        let Some((window_label, _)) = windows
            .into_iter()
            .find(|(_, sample)| *sample == SampleProject::Aurora)
        else {
            return Err("A fase successor exige o host do Projeto B.".into());
        };
        return Ok(vec![(window_label, ProbeRole::Challenger)]);
    }

    let participants = windows
        .into_iter()
        .map(|(window_label, sample)| {
            (
                window_label,
                if sample == SampleProject::Horizon {
                    ProbeRole::Owner
                } else {
                    ProbeRole::Challenger
                },
            )
        })
        .collect::<Vec<_>>();
    if topology.label() == "multiwindow"
        && (participants
            .iter()
            .filter(|(_, role)| *role == ProbeRole::Owner)
            .count()
            != 1
            || participants
                .iter()
                .filter(|(_, role)| *role == ProbeRole::Challenger)
                .count()
                != 1)
    {
        return Err("Os papéis do probe terminal são inválidos.".into());
    }
    Ok(participants)
}

async fn run_participant(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
    phase: ProbePhase,
    role: ProbeRole,
) -> Result<(), String> {
    match role {
        ProbeRole::Owner => run_owner(app, root, topology, window_label, scenario).await,
        ProbeRole::Challenger => {
            run_challenger(app, root, topology, window_label, scenario, phase).await
        }
    }
}

async fn run_owner(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
) -> Result<(), String> {
    let window = app
        .get_webview_window(window_label)
        .ok_or_else(|| format!("A Janela {window_label} do owner não existe."))?;
    let capture = Arc::new(Mutex::new(ProbeCapture::default()));
    let channel = owner_channel(
        app.clone(),
        window.clone(),
        root.to_path_buf(),
        topology,
        window_label,
        scenario,
        Arc::clone(&capture),
    );
    let result = execute_real_export(app, &window, channel).await;
    let capture = capture_snapshot(&capture);
    if let Some(reason) = capture.failure.clone() {
        cleanup_success_if_present(&result)?;
        return Err(reason);
    }

    match scenario {
        ProbeScenario::Success => {
            let result = result.map_err(|error| {
                format!(
                    "o owner deveria concluir com sucesso, mas retornou {}",
                    error.code()
                )
            })?;
            let output_bytes = verify_and_remove_output(&result)?;
            prove_owner_resources_released(app).await?;
            write_event(
                root,
                &ProbeEvent::owner_terminal(
                    topology,
                    window_label,
                    scenario,
                    ProbeTerminal::Success,
                    capture,
                    Some(output_bytes),
                )?,
            )
        }
        ProbeScenario::Failure => match result {
            Ok(result) => {
                verify_and_remove_output(&result)?;
                Err("o owner concluiu, mas o cenário exigia falha".into())
            }
            Err(error)
                if error.code() == "failed"
                    && error.message().contains("Processador de Imagens") =>
            {
                prove_owner_resources_released(app).await?;
                write_event(
                    root,
                    &ProbeEvent::owner_terminal(
                        topology,
                        window_label,
                        scenario,
                        ProbeTerminal::Failed,
                        capture,
                        None,
                    )?,
                )
            }
            Err(error) => Err(format!(
                "o owner deveria falhar no Processador de Imagens, mas retornou {}: {}",
                error.code(),
                error.message()
            )),
        },
        ProbeScenario::Cancellation => {
            if capture.cancellation_disposition != Some(CancelDisposition::Requested) {
                return Err("o cancelamento do owner não retornou requested".into());
            }
            match result {
                Ok(result) => {
                    verify_and_remove_output(&result)?;
                    Err("o owner concluiu, mas deveria ter sido cancelado".into())
                }
                Err(error) if error.code() == "cancelled" => {
                    prove_owner_resources_released(app).await?;
                    write_event(
                        root,
                        &ProbeEvent::owner_terminal(
                            topology,
                            window_label,
                            scenario,
                            ProbeTerminal::Cancelled,
                            capture,
                            None,
                        )?,
                    )
                }
                Err(error) => Err(format!(
                    "o owner deveria ser cancelado, mas retornou {}",
                    error.code()
                )),
            }
        }
        ProbeScenario::OwnerDeath => match result {
            Err(error) if error.code() == "cancelled" => Ok(()),
            Err(error) => Err(format!(
                "o owner encerrado deveria cancelar, mas retornou {}",
                error.code()
            )),
            Ok(result) => {
                verify_and_remove_output(&result)?;
                Err("o owner_death concluiu sem a queda do proprietário".into())
            }
        },
    }
}

async fn run_challenger(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
    phase: ProbePhase,
) -> Result<(), String> {
    let window = app
        .get_webview_window(window_label)
        .ok_or_else(|| format!("A Janela {window_label} do challenger não existe."))?;
    if phase == ProbePhase::Matrix {
        wait_for_file_async(
            &root.join("owner-ready.json"),
            "a confirmação de que o owner possui a lease",
        )
        .await?;
        let conflict_capture = Arc::new(Mutex::new(ProbeCapture::default()));
        let conflict_result = execute_real_export(
            app,
            &window,
            observing_channel(Arc::clone(&conflict_capture)),
        )
        .await;
        let conflict_capture = capture_snapshot(&conflict_capture);
        if let Some(reason) = conflict_capture.failure {
            cleanup_success_if_present(&conflict_result)?;
            return Err(reason);
        }
        match conflict_result {
            Err(error) if error.code() == "conflict" => {
                if conflict_capture.operation_id.is_some()
                    || !conflict_capture.progress_stages.is_empty()
                    || conflict_capture.cancellation_disposition.is_some()
                    || conflict_capture.preparing_claimed
                {
                    return Err(
                        "o challenger recebeu eventos antes do Conflict normal_export".into(),
                    );
                }
            }
            Ok(result) => {
                verify_and_remove_output(&result)?;
                return Err("o challenger concluiu apesar da lease simultânea".into());
            }
            Err(error) => {
                return Err(format!(
                    "o challenger não recebeu Conflict normal_export; retornou {}: {}",
                    error.code(),
                    error.message()
                ));
            }
        }
        write_event(
            root,
            &ProbeEvent::challenger_conflict(topology, window_label, scenario),
        )?;
        if scenario != ProbeScenario::OwnerDeath {
            wait_for_file_async(&root.join("owner-terminal.json"), "o terminal do owner").await?;
        }
    }

    wait_for_file_async(&root.join("allow-successor"), "a autorização do successor").await?;
    let capture = Arc::new(Mutex::new(ProbeCapture::default()));
    let channel = observing_channel(Arc::clone(&capture));
    let result = execute_real_export(app, &window, channel).await;
    let capture = capture_snapshot(&capture);
    if let Some(reason) = capture.failure.clone() {
        cleanup_success_if_present(&result)?;
        return Err(reason);
    }
    let result = result.map_err(|error| {
        format!(
            "o successor deveria exportar após a liberação, mas retornou {}",
            error.code()
        )
    })?;
    let output_bytes = verify_and_remove_output(&result)?;
    write_event(
        root,
        &ProbeEvent::challenger_success(topology, window_label, scenario, capture, output_bytes)?,
    )
}

async fn acquire_lease(app: &AppHandle) -> Result<OperationLease, OperationLeaseError> {
    OperationLease::acquire(
        &app.state::<OperationGate>(),
        &app.state::<CacheEngine>(),
        &app.state::<ImagingProcessor>(),
        OperationMode::NormalExport,
    )
    .await
}

async fn prove_owner_resources_released(app: &AppHandle) -> Result<(), String> {
    let lease = tokio::time::timeout(PROBE_TIMEOUT, acquire_lease(app))
        .await
        .map_err(|_| {
            "a reaquisição defensiva de Gate, Cache e Processador excedeu o limite do probe"
                .to_string()
        })?
        .map_err(|error| {
            format!("o owner não readquiriu Gate, Cache e Processador após o terminal: {error}")
        })?;
    drop(lease);
    Ok(())
}

fn owner_channel(
    app: AppHandle,
    window: WebviewWindow,
    root: PathBuf,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
    capture: Arc<Mutex<ProbeCapture>>,
) -> Channel<ExportEvent> {
    Channel::new(move |body| {
        match record_channel_event(body, &capture, true) {
            Ok(Some(preparing)) => {
                if let Err(reason) = cross_owner_barrier(
                    &app,
                    &window,
                    &root,
                    topology,
                    window_label,
                    scenario,
                    &capture,
                    preparing,
                ) {
                    record_capture_failure(&capture, reason);
                }
            }
            Ok(None) => {}
            Err(reason) => record_capture_failure(&capture, reason),
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
fn cross_owner_barrier(
    app: &AppHandle,
    window: &WebviewWindow,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
    scenario: ProbeScenario,
    capture: &Arc<Mutex<ProbeCapture>>,
    preparing: PreparingSnapshot,
) -> Result<(), String> {
    write_event(
        root,
        &ProbeEvent::owner_ready(
            topology,
            window_label,
            scenario,
            preparing.operation_id.clone(),
            preparing.progress_stages,
        ),
    )?;
    match scenario {
        ProbeScenario::Success | ProbeScenario::Failure => {
            wait_for_file_blocking(&root.join("continue-owner"), "a continuação do owner")
        }
        ProbeScenario::Cancellation => {
            wait_for_file_blocking(
                &root.join("cancel-owner"),
                "a solicitação de cancelamento do owner",
            )?;
            let disposition = export_probe_commands::cancel_export_spike(
                window.clone(),
                preparing.operation_id,
                app.state::<ExportAttempts>(),
            );
            capture
                .lock()
                .expect("the export terminal capture remains available")
                .cancellation_disposition = Some(disposition);
            if disposition != CancelDisposition::Requested {
                return Err(format!(
                    "cancel_export_spike retornou {disposition:?}, não Requested"
                ));
            }
            Ok(())
        }
        ProbeScenario::OwnerDeath => {
            let result = wait_for_file_blocking(
                &root.join("continue-owner"),
                "a confirmação defensiva da queda do owner",
            );
            if result.is_err() {
                let disposition = export_probe_commands::cancel_export_spike(
                    window.clone(),
                    preparing.operation_id,
                    app.state::<ExportAttempts>(),
                );
                capture
                    .lock()
                    .expect("the export terminal capture remains available")
                    .cancellation_disposition = Some(disposition);
            }
            result
        }
    }
}

fn cleanup_success_if_present(
    result: &Result<ExportResult, ExportCommandError>,
) -> Result<(), String> {
    if let Ok(result) = result {
        verify_and_remove_output(result)?;
    }
    Ok(())
}

fn write_failure(
    root: &Path,
    topology: &str,
    window_label: &str,
    scenario: ProbeScenario,
    reason: &str,
) -> Result<(), String> {
    let failure = ProbeFailure {
        schema_version: 1,
        process_id: std::process::id(),
        topology,
        window_label,
        scenario,
        reason,
    };
    write_json_atomic_new(
        &root.join(format!(
            "failure-{}-{window_label}.json",
            std::process::id()
        )),
        &failure,
    )
}

fn write_event(root: &Path, event: &ProbeEvent) -> Result<(), String> {
    write_json_atomic_new(
        &root.join(format!("{}.json", event.state.as_str().replace('_', "-"))),
        event,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ExportTerminalProbe, ProbeEvent, ProbeRole, ProbeScenario, write_event};
    use crate::topology_spike::TopologySpike;

    #[test]
    fn remains_disabled_when_every_export_terminal_probe_value_is_absent() {
        let standard =
            TopologySpike::from_values(None, None).expect("the standard topology is valid");

        assert!(
            ExportTerminalProbe::from_values(None, None, None, &standard)
                .expect("an absent probe is valid")
                .is_none()
        );
    }

    #[test]
    fn rejects_partial_invalid_and_regular_export_terminal_probe_configuration() {
        let directory = probe_root();
        let standard =
            TopologySpike::from_values(None, None).expect("the standard topology is valid");
        let independent = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the independent topology is valid");

        assert!(
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                None,
                Some("matrix"),
                &independent,
            )
            .is_err()
        );
        assert!(
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("unsupported"),
                Some("matrix"),
                &independent,
            )
            .is_err()
        );
        assert!(
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("success"),
                Some("matrix"),
                &standard,
            )
            .is_err()
        );
        assert!(
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("success"),
                Some("successor"),
                &independent,
            )
            .is_err(),
            "the successor phase exists only for owner death"
        );
        assert!(
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("success"),
                Some("matrix"),
                &independent,
            )
            .expect("the complete matrix configuration is valid")
            .is_some()
        );
    }

    #[test]
    fn matrix_maps_horizon_to_owner_and_the_other_project_to_challenger() {
        let directory = probe_root();
        let independent_a = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("independent A is valid");
        let independent_b = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("independent B is valid");
        let multiwindow =
            TopologySpike::from_values(Some("multiwindow"), None).expect("multiwindow is valid");

        let configure = |topology: &TopologySpike| {
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("success"),
                Some("matrix"),
                topology,
            )
            .expect("the matrix configuration is valid")
            .expect("the probe is enabled")
        };

        assert_eq!(
            configure(&independent_a).participants(),
            &[("main", ProbeRole::Owner)]
        );
        assert_eq!(
            configure(&independent_b).participants(),
            &[("main", ProbeRole::Challenger)]
        );
        assert_eq!(
            configure(&multiwindow).participants(),
            &[
                ("main", ProbeRole::Owner),
                ("project-b", ProbeRole::Challenger),
            ]
        );
    }

    #[test]
    fn owner_death_successor_selects_only_project_b() {
        let directory = probe_root();
        let independent_a = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("independent A is valid");
        let independent_b = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("independent B is valid");
        let multiwindow =
            TopologySpike::from_values(Some("multiwindow"), None).expect("multiwindow is valid");
        let configure = |topology: &TopologySpike| {
            ExportTerminalProbe::from_values(
                Some(directory.path()),
                Some("owner_death"),
                Some("successor"),
                topology,
            )
        };

        assert!(configure(&independent_a).is_err());
        assert_eq!(
            configure(&independent_b)
                .expect("independent project B is the successor")
                .expect("the probe is enabled")
                .participants(),
            &[("main", ProbeRole::Challenger)]
        );
        assert_eq!(
            configure(&multiwindow)
                .expect("multiwindow project B is the successor")
                .expect("the probe is enabled")
                .participants(),
            &[("project-b", ProbeRole::Challenger)]
        );
    }

    #[test]
    fn owner_ready_event_has_the_closed_uniform_schema_and_kebab_case_file_name() {
        let directory = probe_root();

        write_event(
            directory.path(),
            &ProbeEvent::owner_ready(
                "multiwindow",
                "main",
                ProbeScenario::Cancellation,
                "export-42".into(),
                vec!["preparing".into()],
            ),
        )
        .expect("the event is written atomically");

        let path = directory.path().join("owner-ready.json");
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(path).expect("the owner-ready event is readable"),
        )
        .expect("the owner-ready event is valid JSON");
        assert_eq!(
            value,
            json!({
                "schemaVersion": 1,
                "processId": std::process::id(),
                "topology": "multiwindow",
                "windowLabel": "main",
                "scenario": "cancellation",
                "state": "owner_ready",
                "operationMode": "normal_export",
                "operationId": "export-42",
                "terminal": null,
                "progressStages": ["preparing"],
                "cancellationDisposition": null,
                "resources": [
                    "operation_gate",
                    "cache_pause",
                    "processor_reservation",
                ],
                "resourceState": "held",
                "outputBytes": null,
            })
        );
    }

    fn probe_root() -> tempfile::TempDir {
        let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".scratch")
            .join("export-terminal-tests");
        std::fs::create_dir_all(&parent).expect("the test scratch parent exists");
        let parent = parent
            .canonicalize()
            .expect("the test scratch parent has an absolute normalized path");
        tempfile::Builder::new()
            .prefix("probe-")
            .tempdir_in(parent)
            .expect("the probe root exists")
    }
}
