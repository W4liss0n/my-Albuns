use std::{
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use myalbuns_core::{EditorProjection, ProjectCore};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{State, WebviewWindow};

use crate::{
    global_process_spike::{
        GLOBAL_ENDPOINT_ENV, GLOBAL_RUN_ID_ENV, GlobalStatusProbeError, GlobalStatusResponse,
        probe_global_status,
    },
    project_host::ProjectHost,
    topology_spike::TopologySpike,
};

pub(crate) const FAULT_GATE_ENV: &str = "MYALBUNS_TOPOLOGY_FAULT_GATE";
pub(crate) const FAULT_OUTPUT_ROOT_ENV: &str = "MYALBUNS_TOPOLOGY_FAULT_OUTPUT_ROOT";
const GLOBAL_STATUS_TIMEOUT: Duration = Duration::from_millis(750);
const MAXIMUM_GATE_BYTES: u64 = 4_096;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct TopologyFaultProbeState {
    topology: &'static str,
    gate_path: Option<PathBuf>,
    output_root: Option<PathBuf>,
    global_endpoint: Option<SocketAddr>,
    run_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyFaultProbeConfig {
    probe_id: String,
    expected_global_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyFaultProbeConfigState {
    enabled: bool,
    config: Option<TopologyFaultProbeConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FaultGate {
    run_id: String,
    probe_id: String,
    expected_global_available: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyFaultProbeResult {
    projection: EditorProjection,
    probe_id: String,
    previous_revision: u64,
    persisted_revision: u64,
    bytes: u64,
    sha256: String,
    global_available: bool,
    global_process_id: Option<u32>,
    global_round_trip_ms: f64,
}

impl TopologyFaultProbeState {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Self, String> {
        let gate_path = std::env::var_os(FAULT_GATE_ENV).map(PathBuf::from);
        let output_root = std::env::var_os(FAULT_OUTPUT_ROOT_ENV).map(PathBuf::from);
        let global_endpoint = std::env::var(GLOBAL_ENDPOINT_ENV)
            .ok()
            .map(|value| {
                value
                    .parse::<SocketAddr>()
                    .map_err(|_| format!("{GLOBAL_ENDPOINT_ENV} contém um endpoint inválido."))
            })
            .transpose()?;
        let run_id = std::env::var(GLOBAL_RUN_ID_ENV).ok();
        Self::new(
            topology.label(),
            gate_path,
            output_root,
            global_endpoint,
            run_id,
        )
    }

    fn new(
        topology: &'static str,
        gate_path: Option<PathBuf>,
        output_root: Option<PathBuf>,
        global_endpoint: Option<SocketAddr>,
        run_id: Option<String>,
    ) -> Result<Self, String> {
        let values_present = gate_path.is_some()
            || output_root.is_some()
            || global_endpoint.is_some()
            || run_id.is_some();
        if topology == "standard" {
            if values_present {
                return Err(
                    "O probe de continuidade só pode ser configurado no spike de topologia."
                        .to_string(),
                );
            }
            return Ok(Self {
                topology,
                gate_path: None,
                output_root: None,
                global_endpoint: None,
                run_id: None,
            });
        }

        let gate_path =
            gate_path.ok_or_else(|| format!("{FAULT_GATE_ENV} é obrigatório no spike."))?;
        validate_scratch_path(&gate_path, FAULT_GATE_ENV, Some("json"))?;
        let output_root = output_root
            .ok_or_else(|| format!("{FAULT_OUTPUT_ROOT_ENV} é obrigatório no spike."))?;
        validate_scratch_path(&output_root, FAULT_OUTPUT_ROOT_ENV, None)?;
        let global_endpoint = global_endpoint
            .filter(|value| value.ip().is_loopback() && value.port() != 0)
            .ok_or_else(|| {
                format!("{GLOBAL_ENDPOINT_ENV} precisa usar um endpoint local de loopback.")
            })?;
        let run_id =
            run_id.ok_or_else(|| format!("{GLOBAL_RUN_ID_ENV} é obrigatório no spike."))?;
        validate_identifier(GLOBAL_RUN_ID_ENV, &run_id)?;

        Ok(Self {
            topology,
            gate_path: Some(gate_path),
            output_root: Some(output_root),
            global_endpoint: Some(global_endpoint),
            run_id: Some(run_id),
        })
    }

    fn gate_config(&self) -> Result<Option<TopologyFaultProbeConfig>, String> {
        let Some(gate_path) = &self.gate_path else {
            return Ok(None);
        };
        let metadata = match std::fs::symlink_metadata(gate_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "Não foi possível inspecionar o gate do probe de continuidade: {error}"
                ));
            }
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAXIMUM_GATE_BYTES
        {
            return Err("O gate do probe de continuidade é inválido.".into());
        }
        let source = std::fs::read(gate_path)
            .map_err(|error| format!("Não foi possível ler o gate do probe: {error}"))?;
        let gate: FaultGate = serde_json::from_slice(&source)
            .map_err(|error| format!("Gate do probe de continuidade inválido: {error}"))?;
        validate_identifier("runId", &gate.run_id)?;
        validate_identifier("probeId", &gate.probe_id)?;
        if Some(gate.run_id.as_str()) != self.run_id.as_deref() {
            return Err("O gate não pertence à execução atual do spike.".into());
        }
        Ok(Some(TopologyFaultProbeConfig {
            probe_id: gate.probe_id,
            expected_global_available: gate.expected_global_available,
        }))
    }

    fn config_state(&self) -> Result<TopologyFaultProbeConfigState, String> {
        if self.gate_path.is_none() {
            return Ok(TopologyFaultProbeConfigState {
                enabled: false,
                config: None,
            });
        }
        Ok(TopologyFaultProbeConfigState {
            enabled: true,
            config: self.gate_config()?,
        })
    }

    fn persist_with_global_probe<F>(
        &self,
        window_label: &str,
        probe_id: &str,
        previous_revision: u64,
        expected_revision: u64,
        projects: &ProjectHost,
        global_probe: F,
    ) -> Result<TopologyFaultProbeResult, String>
    where
        F: FnOnce(
            SocketAddr,
            &str,
            &str,
            Duration,
        ) -> Result<GlobalStatusResponse, GlobalStatusProbeError>,
    {
        let config = self
            .gate_config()?
            .ok_or_else(|| "O gate do probe de continuidade ainda está fechado.".to_string())?;
        validate_identifier("probeId", probe_id)?;
        if config.probe_id != probe_id {
            return Err("O probe solicitado não corresponde ao gate atual.".into());
        }

        let revision = projects.revision_for_persistence(
            window_label,
            previous_revision,
            expected_revision,
        )?;
        validate_identifier("projectId", &revision.project_id)?;
        let output_root = self
            .output_root
            .as_ref()
            .ok_or_else(|| "O probe de continuidade não está ativo.".to_string())?;
        let persisted_file_name = format!(
            "{}-r{}.json",
            revision.project_id, revision.persisted_revision
        );
        let persisted_path = output_root.join(&persisted_file_name);
        persist_atomic_new(&persisted_path, revision.source.as_bytes())?;

        let reopened_bytes = std::fs::read(&persisted_path)
            .map_err(|error| format!("Não foi possível reler a revisão persistida: {error}"))?;
        if reopened_bytes != revision.source.as_bytes() {
            return Err("A revisão relida não corresponde aos bytes persistidos.".into());
        }
        let reopened_source = std::str::from_utf8(&reopened_bytes)
            .map_err(|_| "A revisão persistida não contém JSON UTF-8 válido.".to_string())?;
        let reopened = ProjectCore::load_persisted_revision(reopened_source)
            .map_err(|error| format!("A revisão persistida não pôde ser reaberta: {error}"))?;
        let reopened_snapshot = reopened.render_snapshot();
        if reopened_snapshot.project_id != revision.project_id
            || reopened.revision() != revision.persisted_revision
        {
            return Err("A identidade ou a revisão relida não corresponde à Sessão salva.".into());
        }
        let bytes = u64::try_from(reopened_bytes.len())
            .map_err(|_| "O tamanho da revisão persistida excedeu o limite.".to_string())?;
        let sha256 = sha256_hex(&reopened_bytes);

        let endpoint = self
            .global_endpoint
            .ok_or_else(|| "O endpoint do processo global não está configurado.".to_string())?;
        let run_id = self
            .run_id
            .as_deref()
            .ok_or_else(|| "A execução do spike não está configurada.".to_string())?;
        let global_started = Instant::now();
        let global_result = global_probe(endpoint, run_id, probe_id, GLOBAL_STATUS_TIMEOUT);
        let global_round_trip_ms = global_started.elapsed().as_secs_f64() * 1_000.0;
        let (global_available, global_process_id) = match global_result {
            Ok(response) => {
                if response.run_id != run_id
                    || response.probe_id != probe_id
                    || response.topology != self.topology
                    || response.process_id == 0
                {
                    return Err("A resposta do processo global não corresponde ao probe.".into());
                }
                (true, Some(response.process_id))
            }
            Err(GlobalStatusProbeError::Unavailable(_)) => (false, None),
            Err(error) => {
                return Err(format!(
                    "O status do processo global não pôde ser validado: {error}"
                ));
            }
        };
        if global_available != config.expected_global_available {
            return Err("A disponibilidade global divergiu do estado esperado pelo gate.".into());
        }

        let projection =
            projects.confirm_persisted_revision(window_label, revision.persisted_revision)?;
        if projection.state.project_id != revision.project_id
            || projection.state.revision != revision.persisted_revision
            || projection.state.saved_revision != revision.persisted_revision
            || projection.state.dirty
        {
            return Err("A Sessão não confirmou corretamente a revisão salva.".into());
        }

        Ok(TopologyFaultProbeResult {
            projection,
            probe_id: probe_id.into(),
            previous_revision: revision.previous_revision,
            persisted_revision: revision.persisted_revision,
            bytes,
            sha256,
            global_available,
            global_process_id,
            global_round_trip_ms,
        })
    }
}

fn persist_atomic_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "A revisão persistida não possui diretório pai.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Não foi possível criar o diretório do probe: {error}"))?;
    let metadata = std::fs::symlink_metadata(parent)
        .map_err(|error| format!("Não foi possível inspecionar o diretório do probe: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("O diretório de saída do probe é inválido.".into());
    }
    if path.exists() {
        return Err("A revisão de saída do probe já existe.".into());
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "O nome da revisão persistida é inválido.".to_string())?;
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let write_result = (|| {
        let mut temporary = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| format!("Não foi possível criar o temporário do probe: {error}"))?;
        temporary
            .write_all(bytes)
            .and_then(|_| temporary.flush())
            .and_then(|_| temporary.sync_all())
            .map_err(|error| format!("Não foi possível sincronizar a revisão do probe: {error}"))?;
        drop(temporary);
        std::fs::rename(&temporary_path, path)
            .map_err(|error| format!("Não foi possível publicar a revisão do probe: {error}"))
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    write_result
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_scratch_path(
    path: &Path,
    variable: &str,
    extension: Option<&str>,
) -> Result<(), String> {
    if !path.is_absolute()
        || path.components().any(|part| part == Component::ParentDir)
        || !path
            .components()
            .any(|part| matches!(part, Component::Normal(value) if value == ".scratch"))
        || extension.is_some_and(|expected| {
            path.extension().and_then(|value| value.to_str()) != Some(expected)
        })
    {
        return Err(format!("{variable} contém um caminho inválido."));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if safe_log_identifier(value).is_none() {
        return Err(format!("{label} contém um identificador inválido."));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn topology_fault_probe_config(
    state: State<'_, TopologyFaultProbeState>,
) -> Result<TopologyFaultProbeConfigState, String> {
    state.config_state()
}

#[tauri::command]
pub(crate) fn persist_topology_fault_probe(
    probe_id: String,
    previous_revision: u64,
    expected_revision: u64,
    window: WebviewWindow,
    state: State<'_, TopologyFaultProbeState>,
    projects: State<'_, ProjectHost>,
) -> Result<TopologyFaultProbeResult, String> {
    let result = state.persist_with_global_probe(
        window.label(),
        &probe_id,
        previous_revision,
        expected_revision,
        &projects,
        probe_global_status,
    )?;
    let persisted_file_name = format!(
        "{}-r{}.json",
        result.projection.state.project_id, result.persisted_revision
    );
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        run_id = state.run_id.as_deref(),
        probe_id = result.probe_id.as_str(),
        topology = state.topology,
        window_label = window.label(),
        project_id = safe_log_identifier(&result.projection.state.project_id),
        previous_revision = result.previous_revision,
        persisted_revision = result.persisted_revision,
        dirty = result.projection.state.dirty,
        persisted_bytes = result.bytes,
        persisted_sha256 = safe_log_identifier(&result.sha256),
        persisted_file_name,
        reopened_revision = result.persisted_revision,
        global_available = result.global_available,
        global_process_id = result.global_process_id,
        global_round_trip_ms = result.global_round_trip_ms,
        event = "topology_fault_probe_completed",
    );
    Ok(result)
}

#[tauri::command]
pub(crate) fn report_topology_fault_probe_failure(
    probe_id: String,
    reason: String,
    window: WebviewWindow,
    state: State<'_, TopologyFaultProbeState>,
    projects: State<'_, ProjectHost>,
) -> Result<(), String> {
    let config = state
        .gate_config()?
        .ok_or_else(|| "O gate do probe de continuidade ainda está fechado.".to_string())?;
    validate_identifier("probeId", &probe_id)?;
    validate_identifier("reason", &reason)?;
    if config.probe_id != probe_id {
        return Err("A falha reportada não corresponde ao gate atual.".into());
    }
    let project_id = projects.projection(window.label())?.state.project_id;
    tracing::error!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        run_id = state.run_id.as_deref(),
        probe_id,
        topology = state.topology,
        window_label = window.label(),
        project_id = safe_log_identifier(&project_id),
        reason,
        event = "topology_fault_probe_failed",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::Path,
    };

    use myalbuns_core::{ProjectCore, ProjectIntent};

    use super::{GLOBAL_STATUS_TIMEOUT, TopologyFaultProbeState};
    use crate::{
        global_process_spike::{GlobalStatusProbeError, GlobalStatusResponse},
        project_host::ProjectHost,
        sample_project::SampleProject,
    };

    fn probe_paths(directory: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let root = directory.join(".scratch").join("topology-fault-probe");
        std::fs::create_dir_all(&root).expect("the probe scratch directory is created");
        (root.join("gate.json"), root.join("saved"))
    }

    #[test]
    fn exposes_only_a_correlated_safe_gate_to_the_frontend() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let (gate, output_root) = probe_paths(directory.path());
        let state = TopologyFaultProbeState::new(
            "independent",
            Some(gate.clone()),
            Some(output_root),
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42_151)),
            Some("run-001".into()),
        )
        .expect("the probe state is valid");

        assert_eq!(
            state.gate_config().expect("an absent gate is valid"),
            None,
            "the frontend polls until the runner publishes a gate"
        );
        assert_eq!(
            serde_json::to_string(
                &state
                    .config_state()
                    .expect("the spike config state is available"),
            )
            .expect("the config state serializes"),
            r#"{"enabled":true,"config":null}"#
        );
        std::fs::write(
            &gate,
            r#"{"runId":"run-001","probeId":"probe-007","expectedGlobalAvailable":false}"#,
        )
        .expect("the runner gate is published");

        let config = state
            .gate_config()
            .expect("the correlated gate is valid")
            .expect("the published gate is exposed");
        assert_eq!(config.probe_id, "probe-007");
        assert!(!config.expected_global_available);
    }

    #[test]
    fn persists_reopens_and_only_then_confirms_the_document_revision() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let (gate, output_root) = probe_paths(directory.path());
        std::fs::write(
            &gate,
            r#"{"runId":"run-001","probeId":"probe-008","expectedGlobalAvailable":false}"#,
        )
        .expect("the runner gate is published");
        let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42_151);
        let state = TopologyFaultProbeState::new(
            "independent",
            Some(gate),
            Some(output_root.clone()),
            Some(endpoint),
            Some("run-001".into()),
        )
        .expect("the probe state is valid");
        let source = SampleProject::Horizon
            .persisted_source(100)
            .expect("the sample project serializes");
        let session =
            ProjectCore::open_editable_session(&source).expect("the sample session opens");
        let host = ProjectHost::new([("main", session, vec![])]);
        host.apply(
            "main",
            ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.01,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            },
        )
        .expect("the action reaches the existing ProjectSession command seam");

        let result = state
            .persist_with_global_probe(
                "main",
                "probe-008",
                0,
                1,
                &host,
                |actual_endpoint, run_id, probe_id, timeout| {
                    assert_eq!(actual_endpoint, endpoint);
                    assert_eq!(run_id, "run-001");
                    assert_eq!(probe_id, "probe-008");
                    assert_eq!(timeout, GLOBAL_STATUS_TIMEOUT);
                    Err(GlobalStatusProbeError::Unavailable(
                        "the global process was terminated by the runner".into(),
                    ))
                },
            )
            .expect("local Save continues while the global process is unavailable");

        assert_eq!(result.probe_id, "probe-008");
        assert_eq!(result.previous_revision, 0);
        assert_eq!(result.persisted_revision, 1);
        assert!(!result.projection.state.dirty);
        assert_eq!(result.projection.state.saved_revision, 1);
        assert!(!result.global_available);
        assert_eq!(result.global_process_id, None);
        assert!(result.global_round_trip_ms >= 0.0);
        assert_eq!(result.sha256.len(), 64);

        let saved_path = output_root.join("project-spike-001-r1.json");
        let saved = std::fs::read_to_string(saved_path).expect("the atomic artifact is published");
        let reopened = ProjectCore::load_persisted_revision(&saved)
            .expect("the published artifact reopens through ProjectCore");
        assert_eq!(reopened.revision(), 1);
        assert_eq!(
            result.bytes,
            u64::try_from(saved.len()).expect("the fixture size fits u64")
        );
    }

    #[test]
    fn rejects_an_unexpected_global_status_without_marking_the_session_saved() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let (gate, output_root) = probe_paths(directory.path());
        std::fs::write(
            &gate,
            r#"{"runId":"run-001","probeId":"probe-009","expectedGlobalAvailable":false}"#,
        )
        .expect("the runner gate is published");
        let endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42_151);
        let state = TopologyFaultProbeState::new(
            "multiwindow",
            Some(gate),
            Some(output_root),
            Some(endpoint),
            Some("run-001".into()),
        )
        .expect("the probe state is valid");
        let source = SampleProject::Horizon
            .persisted_source(100)
            .expect("the sample project serializes");
        let session =
            ProjectCore::open_editable_session(&source).expect("the sample session opens");
        let host = ProjectHost::new([("main", session, vec![])]);
        host.apply(
            "main",
            ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.01,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            },
        )
        .expect("the documentary action is applied");

        let error = state
            .persist_with_global_probe(
                "main",
                "probe-009",
                0,
                1,
                &host,
                |_endpoint, run_id, probe_id, _timeout| {
                    Ok(GlobalStatusResponse {
                        process_id: 9_001,
                        run_id: run_id.into(),
                        topology: "multiwindow".into(),
                        probe_id: probe_id.into(),
                    })
                },
            )
            .expect_err("an available global process contradicts this gate");

        assert!(error.contains("disponibilidade global"));
        let projection = host
            .projection("main")
            .expect("the local session remains available");
        assert_eq!(projection.state.saved_revision, 0);
        assert!(projection.state.dirty);
    }

    #[test]
    fn rejects_uncorrelated_or_extensible_gate_payloads() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let (gate, output_root) = probe_paths(directory.path());
        let state = TopologyFaultProbeState::new(
            "independent",
            Some(gate.clone()),
            Some(output_root),
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42_151)),
            Some("run-001".into()),
        )
        .expect("the probe state is valid");

        std::fs::write(
            &gate,
            r#"{"runId":"other-run","probeId":"probe-010","expectedGlobalAvailable":true}"#,
        )
        .expect("the mismatched gate is published");
        assert!(state.gate_config().is_err());

        std::fs::write(
            &gate,
            r#"{"runId":"run-001","probeId":"probe-010","expectedGlobalAvailable":true,"path":"C:\\Users\\someone"}"#,
        )
        .expect("the extensible gate is published");
        assert!(state.gate_config().is_err());
    }

    #[test]
    fn disables_frontend_polling_outside_the_topology_spike() {
        let state = TopologyFaultProbeState::new("standard", None, None, None, None)
            .expect("the standard application has no fault probe");

        assert_eq!(
            serde_json::to_string(
                &state
                    .config_state()
                    .expect("the disabled state is available"),
            )
            .expect("the disabled state serializes"),
            r#"{"enabled":false,"config":null}"#
        );
    }
}
