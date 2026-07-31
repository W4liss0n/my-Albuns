use std::{
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

use myalbuns_logging::ProcessRole;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{
    cache_engine::CacheEngine,
    imaging_processor::ImagingProcessor,
    operation_gate::{OperationGateError, OperationMode},
    operation_lease::{OperationLease, OperationLeaseError},
    sample_project::SampleProject,
    topology_spike::TopologySpike,
};

pub(crate) const OPERATION_GATE_PROBE_ROOT_ENV: &str = "MYALBUNS_OPERATION_GATE_PROBE_ROOT";
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeRole {
    Owner,
    Challenger,
}

#[derive(Debug)]
pub(crate) struct OperationGateProbe {
    root: PathBuf,
    topology: &'static str,
    participants: Vec<(&'static str, ProbeRole)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeEvent<'a> {
    schema_version: u32,
    process_id: u32,
    topology: &'a str,
    window_label: &'a str,
    state: &'a str,
    operation_mode: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeFailure<'a> {
    schema_version: u32,
    process_id: u32,
    topology: &'a str,
    window_label: &'a str,
    reason: &'a str,
}

impl OperationGateProbe {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Option<Self>, String> {
        let root = std::env::var_os(OPERATION_GATE_PROBE_ROOT_ENV).map(PathBuf::from);
        Self::from_values(root.as_deref(), topology)
    }

    fn from_values(root: Option<&Path>, topology: &TopologySpike) -> Result<Option<Self>, String> {
        let Some(root) = root else {
            return Ok(None);
        };
        validate_probe_root(root)?;
        let windows = topology.project_windows();
        let participants = match topology.label() {
            "independent" => {
                let [(window_label, sample)] = windows.as_slice() else {
                    return Err(
                        "A topologia independente do probe exige exatamente uma Janela.".into(),
                    );
                };
                vec![(
                    *window_label,
                    if *sample == SampleProject::Horizon {
                        ProbeRole::Owner
                    } else {
                        ProbeRole::Challenger
                    },
                )]
            }
            "multiwindow" => {
                if windows.len() != 2 {
                    return Err("O probe multiwindow exige exatamente duas Janelas.".into());
                }
                windows
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
                    .collect()
            }
            _ => {
                return Err(format!(
                    "{OPERATION_GATE_PROBE_ROOT_ENV} só pode ser usado no spike de topologia."
                ));
            }
        };
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
            return Err("Os papéis do probe de exclusividade são inválidos.".into());
        }
        Ok(Some(Self {
            root: root.to_path_buf(),
            topology: topology.label(),
            participants,
        }))
    }

    #[cfg(test)]
    fn participants(&self) -> &[(&'static str, ProbeRole)] {
        &self.participants
    }

    pub(crate) fn start(self, app: &AppHandle) -> Result<(), String> {
        for (window_label, role) in self.participants {
            if app.get_webview_window(window_label).is_none() {
                return Err(format!(
                    "A Janela {window_label} do probe de exclusividade não existe."
                ));
            }
            let app = app.clone();
            let root = self.root.clone();
            let topology = self.topology;
            tauri::async_runtime::spawn(async move {
                if let Err(reason) =
                    run_participant(&app, &root, topology, window_label, role).await
                {
                    let _ = write_failure(&root, topology, window_label, &reason);
                    tracing::error!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        process_id = std::process::id(),
                        topology,
                        window_label,
                        reason,
                        event = "operation_gate_probe_failed",
                    );
                }
            });
        }
        Ok(())
    }
}

fn validate_probe_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute()
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || !root
            .components()
            .any(|component| matches!(component, Component::Normal(value) if value == ".scratch"))
        || !root.is_dir()
    {
        return Err(format!(
            "{OPERATION_GATE_PROBE_ROOT_ENV} precisa apontar para uma pasta absoluta existente sob .scratch."
        ));
    }
    Ok(())
}

async fn run_participant(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
    role: ProbeRole,
) -> Result<(), String> {
    match role {
        ProbeRole::Owner => run_owner(app, root, topology, window_label).await,
        ProbeRole::Challenger => run_challenger(app, root, topology, window_label).await,
    }
}

async fn run_owner(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
) -> Result<(), String> {
    let lease = acquire_lease(app)
        .await
        .map_err(|error| format!("o proprietário não adquiriu o gate: {error}"))?;
    write_event(root, topology, window_label, "owner_ready")?;
    wait_for_file(&root.join("release-owner"), "a liberação do proprietário").await?;
    drop(lease);
    write_event(root, topology, window_label, "owner_released")
}

async fn run_challenger(
    app: &AppHandle,
    root: &Path,
    topology: &'static str,
    window_label: &'static str,
) -> Result<(), String> {
    wait_for_file(
        &root.join("owner-ready.json"),
        "a aquisição do proprietário",
    )
    .await?;
    match acquire_lease(app).await {
        Err(OperationLeaseError::Gate(OperationGateError::Conflict {
            requested: OperationMode::NormalExport,
        })) => {}
        Ok(lease) => {
            drop(lease);
            return Err("o desafiante recebeu uma segunda concessão simultânea".into());
        }
        Err(error) => {
            return Err(format!(
                "o desafiante não observou um conflito tipado: {error}"
            ));
        }
    }
    write_event(root, topology, window_label, "challenger_conflict")?;
    wait_for_file(
        &root.join("owner-released.json"),
        "a confirmação de liberação do proprietário",
    )
    .await?;
    let lease = acquire_after_release(app).await?;
    write_event(root, topology, window_label, "challenger_success")?;
    drop(lease);
    Ok(())
}

async fn acquire_after_release(app: &AppHandle) -> Result<OperationLease, String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match acquire_lease(app).await {
            Ok(lease) => return Ok(lease),
            Err(OperationLeaseError::Gate(OperationGateError::Conflict { .. }))
                if Instant::now() < deadline =>
            {
                tokio::time::sleep(PROBE_POLL_INTERVAL).await;
            }
            Err(error) => {
                return Err(format!(
                    "o desafiante não adquiriu o gate após a liberação: {error}"
                ));
            }
        }
    }
}

async fn acquire_lease(app: &AppHandle) -> Result<OperationLease, OperationLeaseError> {
    OperationLease::acquire(
        &app.state(),
        &app.state::<CacheEngine>(),
        &app.state::<ImagingProcessor>(),
        OperationMode::NormalExport,
    )
    .await
}

async fn wait_for_file(path: &Path, description: &str) -> Result<(), String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        tokio::time::sleep(PROBE_POLL_INTERVAL).await;
    }
    Err(format!("{description} excedeu o limite do probe"))
}

fn write_event(root: &Path, topology: &str, window_label: &str, state: &str) -> Result<(), String> {
    let event = ProbeEvent {
        schema_version: 1,
        process_id: std::process::id(),
        topology,
        window_label,
        state,
        operation_mode: OperationMode::NormalExport.as_str(),
    };
    write_json(
        &root.join(format!("{}.json", state.replace('_', "-"))),
        &event,
    )
}

fn write_failure(
    root: &Path,
    topology: &str,
    window_label: &str,
    reason: &str,
) -> Result<(), String> {
    let failure = ProbeFailure {
        schema_version: 1,
        process_id: std::process::id(),
        topology,
        window_label,
        reason,
    };
    write_json(
        &root.join(format!(
            "failure-{}-{window_label}.json",
            std::process::id()
        )),
        &failure,
    )
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("não foi possível serializar o probe: {error}"))?;
    std::fs::write(path, json)
        .map_err(|error| format!("não foi possível registrar o probe: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{OperationGateProbe, ProbeRole, write_event};
    use crate::topology_spike::TopologySpike;

    #[test]
    fn maps_real_windows_to_owner_and_challenger_in_both_topologies() {
        let directory = probe_root();
        let independent_a = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("independent A is valid");
        let independent_b = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("independent B is valid");
        let multiwindow =
            TopologySpike::from_values(Some("multiwindow"), None).expect("multiwindow is valid");

        let independent_a = OperationGateProbe::from_values(Some(directory.path()), &independent_a)
            .expect("the owner host is configured")
            .expect("the probe is enabled");
        let independent_b = OperationGateProbe::from_values(Some(directory.path()), &independent_b)
            .expect("the challenger host is configured")
            .expect("the probe is enabled");
        let multiwindow = OperationGateProbe::from_values(Some(directory.path()), &multiwindow)
            .expect("the multiwindow host is configured")
            .expect("the probe is enabled");

        assert_eq!(independent_a.participants(), &[("main", ProbeRole::Owner)]);
        assert_eq!(
            independent_b.participants(),
            &[("main", ProbeRole::Challenger)]
        );
        assert_eq!(
            multiwindow.participants(),
            &[
                ("main", ProbeRole::Owner),
                ("project-b", ProbeRole::Challenger),
            ]
        );
    }

    #[test]
    fn remains_disabled_in_regular_runs_and_rejects_a_probe_outside_a_topology() {
        let directory = probe_root();
        let standard = TopologySpike::from_values(None, None).expect("standard mode is valid");

        assert!(
            OperationGateProbe::from_values(None, &standard)
                .expect("an absent probe is valid")
                .is_none()
        );
        assert!(
            OperationGateProbe::from_values(Some(directory.path()), &standard).is_err(),
            "measurement instrumentation cannot leak into the regular product run"
        );
    }

    #[test]
    fn event_state_and_file_name_keep_the_probe_contract_distinct() {
        let directory = probe_root();

        write_event(
            directory.path(),
            "multiwindow",
            "project-b",
            "challenger_conflict",
        )
        .expect("the event is written");

        let path = directory.path().join("challenger-conflict.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).expect("the event file is readable"))
                .expect("the event is valid JSON");
        assert_eq!(value["state"], "challenger_conflict");
        assert_eq!(value["operationMode"], "normal_export");
    }

    fn probe_root() -> tempfile::TempDir {
        let parent = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".scratch")
            .join("operation-gate-tests");
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
