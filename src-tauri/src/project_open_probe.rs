use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use myalbuns_logging::ProcessRole;
use myalbuns_paths::{ProjectFileLock, ProjectFileLockError};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    cache_engine::CacheEngine,
    imaging_processor::ImagingProcessor,
    operation_gate::{OperationGateError, OperationMode},
    operation_lease::{OperationLease, OperationLeaseError},
    probe_support::{
        PROBE_TIMEOUT, optional_utf8_environment, validate_probe_root, wait_for_file_async,
        write_json_atomic_new,
    },
    sample_project::SampleProject,
    topology_spike::TopologySpike,
};

pub(crate) const PROJECT_OPEN_PROBE_ROOT_ENV: &str = "MYALBUNS_PROJECT_OPEN_PROBE_ROOT";
pub(crate) const PROJECT_OPEN_PROBE_FILE_ENV: &str = "MYALBUNS_PROJECT_OPEN_PROBE_FILE";
pub(crate) const PROJECT_OPEN_PROBE_SCENARIO_ENV: &str = "MYALBUNS_PROJECT_OPEN_PROBE_SCENARIO";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeScenario {
    NormalClose,
    OwnerDeath,
}

impl ProbeScenario {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NormalClose => "normal_close",
            Self::OwnerDeath => "owner_death",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "normal_close" => Ok(Self::NormalClose),
            "owner_death" => Ok(Self::OwnerDeath),
            _ => Err(format!(
                "Valor inválido em {PROJECT_OPEN_PROBE_SCENARIO_ENV}: {value}."
            )),
        }
    }
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
    OwnerBothHeld,
    ChallengerBothConflict,
    OwnerGateReleasedLockHeld,
    ChallengerGateHeldLockConflict,
    OwnerBothReheld,
    OwnerSessionClosed,
    ChallengerBothRecovered,
}

impl ProbeState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerBothHeld => "owner_both_held",
            Self::ChallengerBothConflict => "challenger_both_conflict",
            Self::OwnerGateReleasedLockHeld => "owner_gate_released_lock_held",
            Self::ChallengerGateHeldLockConflict => "challenger_gate_held_lock_conflict",
            Self::OwnerBothReheld => "owner_both_reheld",
            Self::OwnerSessionClosed => "owner_session_closed",
            Self::ChallengerBothRecovered => "challenger_both_recovered",
        }
    }

    fn file_name(self) -> String {
        format!("{}.json", self.as_str().replace('_', "-"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum MechanismState {
    Held,
    Conflict,
    Released,
    Recovered,
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
    operation_gate_state: MechanismState,
    project_file_lock_state: MechanismState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeSignal {
    schema_version: u32,
    process_id: u32,
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

impl ProbeEvent {
    fn new(
        role: ProbeRole,
        scenario: ProbeScenario,
        state: ProbeState,
        operation_gate_state: MechanismState,
        project_file_lock_state: MechanismState,
    ) -> Self {
        Self {
            schema_version: 1,
            process_id: std::process::id(),
            role,
            scenario,
            state,
            operation_mode: "normal_export",
            operation_gate_state,
            project_file_lock_state,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ProjectOpenProbe {
    root: PathBuf,
    project_file: PathBuf,
    scenario: ProbeScenario,
    role: ProbeRole,
    window_label: &'static str,
}

impl ProjectOpenProbe {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Option<Self>, String> {
        let root = std::env::var_os(PROJECT_OPEN_PROBE_ROOT_ENV).map(PathBuf::from);
        let project_file = std::env::var_os(PROJECT_OPEN_PROBE_FILE_ENV).map(PathBuf::from);
        let scenario = optional_utf8_environment(PROJECT_OPEN_PROBE_SCENARIO_ENV)?;
        Self::from_values(
            root.as_deref(),
            project_file.as_deref(),
            scenario.as_deref(),
            topology,
        )
    }

    fn from_values(
        root: Option<&Path>,
        project_file: Option<&Path>,
        scenario: Option<&str>,
        topology: &TopologySpike,
    ) -> Result<Option<Self>, String> {
        let configured = [root.is_some(), project_file.is_some(), scenario.is_some()];
        if configured.iter().all(|configured| !configured) {
            return Ok(None);
        }
        if !configured.iter().all(|configured| *configured) {
            return Err(format!(
                "{PROJECT_OPEN_PROBE_ROOT_ENV}, {PROJECT_OPEN_PROBE_FILE_ENV} e {PROJECT_OPEN_PROBE_SCENARIO_ENV} precisam ser definidos juntos."
            ));
        }
        let root = root.expect("the complete probe configuration has a root");
        let project_file = project_file.expect("the complete probe configuration has a file");
        validate_probe_root(root)?;
        validate_project_fixture(root, project_file)?;
        if topology.label() != "independent" {
            return Err("O probe de abertura exige dois hosts independentes.".into());
        }
        let windows = topology.project_windows();
        let [(window_label, sample)] = windows.as_slice() else {
            return Err("O probe de abertura exige exatamente uma Janela por host.".into());
        };
        let role = if *sample == SampleProject::Horizon {
            ProbeRole::Owner
        } else {
            ProbeRole::Challenger
        };

        Ok(Some(Self {
            root: root.to_path_buf(),
            project_file: project_file.to_path_buf(),
            scenario: ProbeScenario::parse(
                scenario.expect("the complete probe configuration has a scenario"),
            )?,
            role,
            window_label: *window_label,
        }))
    }

    pub(crate) fn start(self, app: &AppHandle) -> Result<(), String> {
        if app.get_webview_window(self.window_label).is_none() {
            return Err(format!(
                "A Janela {} do probe de abertura não existe.",
                self.window_label
            ));
        }
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(reason) = self.run(&app).await {
                let _ = self.write_failure(&reason);
                tracing::error!(
                    target: "myalbuns.desktop",
                    process_role = ProcessRole::DesktopHost.as_str(),
                    process_id = std::process::id(),
                    role = self.role.as_str(),
                    scenario = self.scenario.as_str(),
                    reason,
                    event = "project_open_probe_failed",
                );
            }
        });
        Ok(())
    }

    async fn run(&self, app: &AppHandle) -> Result<(), String> {
        match self.role {
            ProbeRole::Owner => self.run_owner(app).await,
            ProbeRole::Challenger => self.run_challenger(app).await,
        }
    }

    async fn run_owner(&self, app: &AppHandle) -> Result<(), String> {
        let project_lock = ProjectFileLock::try_acquire(&self.project_file)
            .map_err(|error| format!("o owner não adquiriu o Bloqueio de abertura: {error}"))?;
        let operation_lease = acquire_operation_lease(app)
            .await
            .map_err(|error| format!("o owner não adquiriu o OperationGate: {error}"))?;
        self.write_event(
            ProbeState::OwnerBothHeld,
            MechanismState::Held,
            MechanismState::Held,
        )?;

        wait_for_file_async(
            &self.root.join("release-owner-gate"),
            "a liberação isolada do OperationGate",
        )
        .await?;
        drop(operation_lease);
        self.write_event(
            ProbeState::OwnerGateReleasedLockHeld,
            MechanismState::Released,
            MechanismState::Held,
        )?;

        wait_for_file_async(
            &self.root.join("challenger-gate-released.json"),
            "a devolução do OperationGate pelo challenger",
        )
        .await?;
        let operation_lease = acquire_operation_lease_after_release(app).await?;
        self.write_event(
            ProbeState::OwnerBothReheld,
            MechanismState::Held,
            MechanismState::Held,
        )?;

        match self.scenario {
            ProbeScenario::NormalClose => {
                wait_for_file_async(
                    &self.root.join("close-owner-session"),
                    "o fechamento normal da Sessão do Projeto",
                )
                .await?;
                drop(operation_lease);
                drop(project_lock);
                self.write_event(
                    ProbeState::OwnerSessionClosed,
                    MechanismState::Released,
                    MechanismState::Released,
                )
            }
            ProbeScenario::OwnerDeath => {
                wait_for_file_async(
                    &self.root.join("terminate-owner"),
                    "a terminação externa do owner",
                )
                .await?;
                Err("o cenário de queda não pode terminar cooperativamente".into())
            }
        }
    }

    async fn run_challenger(&self, app: &AppHandle) -> Result<(), String> {
        wait_for_file_async(
            &self.root.join(ProbeState::OwnerBothHeld.file_name()),
            "a aquisição inicial dos dois mecanismos pelo owner",
        )
        .await?;
        expect_operation_conflict(app).await?;
        expect_project_lock_conflict(&self.project_file)?;
        self.write_event(
            ProbeState::ChallengerBothConflict,
            MechanismState::Conflict,
            MechanismState::Conflict,
        )?;

        wait_for_file_async(
            &self
                .root
                .join(ProbeState::OwnerGateReleasedLockHeld.file_name()),
            "a liberação isolada do OperationGate pelo owner",
        )
        .await?;
        let challenger_lease = acquire_operation_lease_after_release(app).await?;
        expect_project_lock_conflict(&self.project_file)?;
        self.write_event(
            ProbeState::ChallengerGateHeldLockConflict,
            MechanismState::Held,
            MechanismState::Conflict,
        )?;
        drop(challenger_lease);
        write_json_atomic_new(
            &self.root.join("challenger-gate-released.json"),
            &ProbeSignal {
                schema_version: 1,
                process_id: std::process::id(),
            },
        )?;

        let release_evidence = match self.scenario {
            ProbeScenario::NormalClose => ProbeState::OwnerSessionClosed.file_name(),
            ProbeScenario::OwnerDeath => "owner-terminated".into(),
        };
        wait_for_file_async(
            &self.root.join(release_evidence),
            "a liberação terminal da Sessão do Projeto",
        )
        .await?;
        let (_project_lock, _operation_lease) =
            acquire_both_after_terminal(app, &self.project_file).await?;
        self.write_event(
            ProbeState::ChallengerBothRecovered,
            MechanismState::Recovered,
            MechanismState::Recovered,
        )
    }

    fn write_event(
        &self,
        state: ProbeState,
        operation_gate_state: MechanismState,
        project_file_lock_state: MechanismState,
    ) -> Result<(), String> {
        write_json_atomic_new(
            &self.root.join(state.file_name()),
            &ProbeEvent::new(
                self.role,
                self.scenario,
                state,
                operation_gate_state,
                project_file_lock_state,
            ),
        )
    }

    fn write_failure(&self, reason: &str) -> Result<(), String> {
        write_json_atomic_new(
            &self
                .root
                .join(format!("failure-{}.json", std::process::id())),
            &ProbeFailure {
                schema_version: 1,
                process_id: std::process::id(),
                role: self.role,
                scenario: self.scenario,
                reason,
            },
        )
    }

    #[cfg(test)]
    const fn role(&self) -> ProbeRole {
        self.role
    }

    #[cfg(test)]
    const fn scenario(&self) -> ProbeScenario {
        self.scenario
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

async fn acquire_operation_lease(app: &AppHandle) -> Result<OperationLease, OperationLeaseError> {
    OperationLease::acquire(
        &app.state(),
        &app.state::<CacheEngine>(),
        &app.state::<ImagingProcessor>(),
        OperationMode::NormalExport,
    )
    .await
}

async fn expect_operation_conflict(app: &AppHandle) -> Result<(), String> {
    match acquire_operation_lease(app).await {
        Err(OperationLeaseError::Gate(OperationGateError::Conflict {
            requested: OperationMode::NormalExport,
        })) => Ok(()),
        Ok(lease) => {
            drop(lease);
            Err("o challenger recebeu uma segunda concessão do OperationGate".into())
        }
        Err(error) => Err(format!(
            "o conflito do OperationGate perdeu sua forma tipada: {error}"
        )),
    }
}

fn expect_project_lock_conflict(project_file: &Path) -> Result<(), String> {
    match ProjectFileLock::try_acquire(project_file) {
        Err(ProjectFileLockError::Conflict) => Ok(()),
        Ok(project_lock) => {
            drop(project_lock);
            Err("o challenger recebeu um segundo Bloqueio de abertura".into())
        }
        Err(error) => Err(format!(
            "o conflito do Bloqueio de abertura perdeu sua forma tipada: {error}"
        )),
    }
}

async fn acquire_operation_lease_after_release(app: &AppHandle) -> Result<OperationLease, String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match acquire_operation_lease(app).await {
            Ok(lease) => return Ok(lease),
            Err(OperationLeaseError::Gate(OperationGateError::Conflict { .. }))
                if Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => {
                return Err(format!(
                    "o OperationGate não ficou disponível após a liberação: {error}"
                ));
            }
        }
    }
}

async fn acquire_both_after_terminal(
    app: &AppHandle,
    project_file: &Path,
) -> Result<(ProjectFileLock, OperationLease), String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match ProjectFileLock::try_acquire(project_file) {
            Ok(project_lock) => match acquire_operation_lease(app).await {
                Ok(operation_lease) => return Ok((project_lock, operation_lease)),
                Err(OperationLeaseError::Gate(OperationGateError::Conflict { .. }))
                    if Instant::now() < deadline =>
                {
                    drop(project_lock);
                }
                Err(error) => {
                    return Err(format!(
                        "o OperationGate não foi recuperado depois do terminal: {error}"
                    ));
                }
            },
            Err(ProjectFileLockError::Conflict) if Instant::now() < deadline => {}
            Err(error) => {
                return Err(format!(
                    "o Bloqueio de abertura não foi recuperado depois do terminal: {error}"
                ));
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

fn validate_project_fixture(root: &Path, project_file: &Path) -> Result<(), String> {
    if !project_file.is_absolute()
        || project_file.parent() != Some(root)
        || project_file.file_name().and_then(|value| value.to_str()) != Some("Projeto.myalbum")
    {
        return Err("O Projeto do probe precisa ser o fixture direto da raiz autorizada.".into());
    }
    let metadata = std::fs::symlink_metadata(project_file)
        .map_err(|error| format!("Não foi possível inspecionar o Projeto do probe: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("O Projeto do probe precisa ser um arquivo regular.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MechanismState, ProbeEvent, ProbeRole, ProbeScenario, ProbeState, ProjectOpenProbe,
    };
    use crate::topology_spike::TopologySpike;

    #[test]
    fn maps_two_independent_hosts_to_the_session_owner_and_challenger() {
        let directory = probe_root();
        let project = directory.path().join("Projeto.myalbum");
        std::fs::write(&project, b"persisted project revision")
            .expect("the Project fixture is writable");
        let owner_topology = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the owner topology is valid");
        let challenger_topology = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("the challenger topology is valid");

        for scenario in [ProbeScenario::NormalClose, ProbeScenario::OwnerDeath] {
            let owner = ProjectOpenProbe::from_values(
                Some(directory.path()),
                Some(&project),
                Some(scenario.as_str()),
                &owner_topology,
            )
            .expect("the owner configuration is valid")
            .expect("the owner probe is enabled");
            let challenger = ProjectOpenProbe::from_values(
                Some(directory.path()),
                Some(&project),
                Some(scenario.as_str()),
                &challenger_topology,
            )
            .expect("the challenger configuration is valid")
            .expect("the challenger probe is enabled");

            assert_eq!(owner.role(), ProbeRole::Owner);
            assert_eq!(challenger.role(), ProbeRole::Challenger);
            assert_eq!(owner.scenario(), scenario);
            assert_eq!(challenger.scenario(), scenario);
        }
    }

    #[test]
    fn stays_disabled_normally_and_rejects_partial_or_multiwindow_configuration() {
        let directory = probe_root();
        let project = directory.path().join("Projeto.myalbum");
        std::fs::write(&project, b"persisted project revision")
            .expect("the Project fixture is writable");
        let standard = TopologySpike::from_values(None, None).expect("standard mode is valid");
        let independent = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("the independent topology is valid");
        let multiwindow =
            TopologySpike::from_values(Some("multiwindow"), None).expect("multiwindow is valid");

        assert!(
            ProjectOpenProbe::from_values(None, None, None, &standard)
                .expect("an absent probe is valid")
                .is_none()
        );
        assert!(
            ProjectOpenProbe::from_values(
                Some(directory.path()),
                None,
                Some("normal_close"),
                &independent,
            )
            .is_err()
        );
        assert!(
            ProjectOpenProbe::from_values(
                Some(directory.path()),
                Some(&project),
                Some("normal_close"),
                &multiwindow,
            )
            .is_err()
        );
    }

    #[test]
    fn event_contract_names_the_two_mechanisms_instead_of_merging_resources() {
        let event = ProbeEvent::new(
            ProbeRole::Owner,
            ProbeScenario::OwnerDeath,
            ProbeState::OwnerGateReleasedLockHeld,
            MechanismState::Released,
            MechanismState::Held,
        );
        let value = serde_json::to_value(event).expect("the event serializes");
        let object = value.as_object().expect("the event is a JSON object");

        assert_eq!(object.len(), 8);
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["role"], "owner");
        assert_eq!(value["scenario"], "owner_death");
        assert_eq!(value["state"], "owner_gate_released_lock_held");
        assert_eq!(value["operationMode"], "normal_export");
        assert_eq!(value["operationGateState"], "released");
        assert_eq!(value["projectFileLockState"], "held");
    }

    fn probe_root() -> tempfile::TempDir {
        let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".scratch")
            .join("project-open-probe-tests");
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
