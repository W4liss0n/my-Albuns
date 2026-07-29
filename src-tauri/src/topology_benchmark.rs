use std::{collections::HashMap, path::PathBuf};

use myalbuns_logging::{ProcessRole, safe_log_identifier};
use serde::{Deserialize, Serialize};
use tauri::{State, WebviewWindow};

use crate::{project_host::ProjectHost, topology_spike::TopologySpike};

pub(crate) const PROBE_GATE_ENV: &str = "MYALBUNS_TOPOLOGY_PROBE_GATE";
pub(crate) const EXPORT_GATE_ENV: &str = "MYALBUNS_TOPOLOGY_EXPORT_GATE";
const WARMUP_FRAMES: usize = 24;
const PAN_FRAMES: usize = 120;
const ZOOM_FRAMES: usize = 120;

pub(crate) struct TopologyBenchmarkState {
    topology: &'static str,
    gate_path: Option<PathBuf>,
    export_gate_path: Option<PathBuf>,
    windows: HashMap<&'static str, bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopologyBenchmarkConfig {
    probe_key: String,
    gate_open: bool,
    export_gate_open: bool,
    warmup_frames: usize,
    pan_frames: usize,
    zoom_frames: usize,
    run_export: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CanvasBenchmarkMeasurement {
    frame_id: String,
    texture_backed: bool,
    pan: FrameTimingSummary,
    zoom: FrameTimingSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameTimingSummary {
    sample_count: usize,
    duration_ms: f64,
    first_frame_latency_ms: f64,
    mean_frame_ms: f64,
    p50_frame_ms: f64,
    p95_frame_ms: f64,
    p99_frame_ms: f64,
    max_frame_ms: f64,
    frames_over16_ms: usize,
    frames_over33_ms: usize,
}

impl TopologyBenchmarkState {
    pub(crate) fn from_environment(topology: &TopologySpike) -> Result<Self, String> {
        let gate_path = std::env::var_os(PROBE_GATE_ENV).map(PathBuf::from);
        let export_gate_path = std::env::var_os(EXPORT_GATE_ENV).map(PathBuf::from);
        Self::new(
            topology.label(),
            topology.benchmark_window_settings(),
            gate_path,
            export_gate_path,
        )
    }

    fn new(
        topology: &'static str,
        windows: Vec<(&'static str, bool)>,
        gate_path: Option<PathBuf>,
        export_gate_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        if topology == "standard" {
            if gate_path.is_some() || export_gate_path.is_some() {
                return Err(
                    "Os gates do benchmark só podem ser usados durante o spike de topologia."
                        .to_string(),
                );
            }
            return Ok(Self {
                topology,
                gate_path: None,
                export_gate_path: None,
                windows: HashMap::new(),
            });
        }
        let gate_path = gate_path.ok_or_else(|| {
            format!("{PROBE_GATE_ENV} é obrigatório durante o spike de topologia.")
        })?;
        validate_gate_path(&gate_path, PROBE_GATE_ENV)?;
        let export_gate_path = export_gate_path.ok_or_else(|| {
            format!("{EXPORT_GATE_ENV} é obrigatório durante o spike de topologia.")
        })?;
        validate_gate_path(&export_gate_path, EXPORT_GATE_ENV)?;
        if gate_path == export_gate_path {
            return Err("Os gates de Canvas e Exportação precisam ser distintos.".into());
        }
        let windows = windows.into_iter().collect::<HashMap<_, _>>();
        if windows.is_empty() {
            return Err("O benchmark não possui Janelas configuradas.".into());
        }
        Ok(Self {
            topology,
            gate_path: Some(gate_path),
            export_gate_path: Some(export_gate_path),
            windows,
        })
    }

    fn config_for(&self, window_label: &str) -> Result<Option<TopologyBenchmarkConfig>, String> {
        let Some(gate_path) = &self.gate_path else {
            return Ok(None);
        };
        let run_export = self
            .windows
            .get(window_label)
            .copied()
            .ok_or_else(|| format!("Janela fora do benchmark: {window_label}."))?;
        Ok(Some(TopologyBenchmarkConfig {
            probe_key: format!("{}-{}-{window_label}", self.topology, std::process::id()),
            gate_open: gate_path.is_file(),
            export_gate_open: self
                .export_gate_path
                .as_ref()
                .is_some_and(|path| path.is_file()),
            warmup_frames: WARMUP_FRAMES,
            pan_frames: PAN_FRAMES,
            zoom_frames: ZOOM_FRAMES,
            run_export,
        }))
    }

    fn validate_measurement(
        &self,
        window_label: &str,
        measurement: &CanvasBenchmarkMeasurement,
    ) -> Result<(), String> {
        let config = self
            .config_for(window_label)?
            .ok_or_else(|| "O benchmark do Canvas não está ativo.".to_string())?;
        if !config.gate_open {
            return Err("O gate do benchmark do Canvas ainda está fechado.".into());
        }
        if safe_log_identifier(&measurement.frame_id).is_none() {
            return Err("A identidade do Frame medido é inválida.".into());
        }
        if !measurement.texture_backed {
            return Err("O probe não usou uma textura real do Cache.".into());
        }
        measurement.pan.validate(config.pan_frames)?;
        measurement.zoom.validate(config.zoom_frames)
    }
}

fn validate_gate_path(path: &std::path::Path, variable: &str) -> Result<(), String> {
    if !path.is_absolute()
        || path.file_name().is_none()
        || path.extension().and_then(|value| value.to_str()) != Some("ready")
    {
        return Err(format!("{variable} contém um caminho inválido."));
    }
    Ok(())
}

impl FrameTimingSummary {
    fn validate(&self, expected_samples: usize) -> Result<(), String> {
        if self.sample_count != expected_samples
            || self.frames_over16_ms > self.sample_count
            || self.frames_over33_ms > self.frames_over16_ms
        {
            return Err("A contagem de frames do probe é inválida.".into());
        }
        let timings = [
            self.duration_ms,
            self.first_frame_latency_ms,
            self.mean_frame_ms,
            self.p50_frame_ms,
            self.p95_frame_ms,
            self.p99_frame_ms,
            self.max_frame_ms,
        ];
        if timings
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > 600_000.0)
            || self.duration_ms <= 0.0
            || self.mean_frame_ms <= 0.0
            || self.p50_frame_ms > self.p95_frame_ms
            || self.p95_frame_ms > self.p99_frame_ms
            || self.p99_frame_ms > self.max_frame_ms
        {
            return Err("Os tempos de frame do probe são inválidos.".into());
        }
        Ok(())
    }
}

#[tauri::command]
pub(crate) fn topology_benchmark_config(
    window: WebviewWindow,
    state: State<'_, TopologyBenchmarkState>,
) -> Result<Option<TopologyBenchmarkConfig>, String> {
    state.config_for(window.label())
}

#[tauri::command]
pub(crate) fn report_topology_canvas_ready(
    window: WebviewWindow,
    state: State<'_, TopologyBenchmarkState>,
    projects: State<'_, ProjectHost>,
) -> Result<(), String> {
    let config = state
        .config_for(window.label())?
        .ok_or_else(|| "O benchmark do Canvas não está ativo.".to_string())?;
    if !config.gate_open {
        return Err("O gate do benchmark do Canvas ainda está fechado.".into());
    }
    let project_id = projects.projection(window.label())?.state.project_id;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        topology = state.topology,
        window_label = window.label(),
        project_id = safe_log_identifier(&project_id),
        event = "canvas_benchmark_ready",
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn report_topology_canvas_benchmark(
    measurement: CanvasBenchmarkMeasurement,
    window: WebviewWindow,
    state: State<'_, TopologyBenchmarkState>,
    projects: State<'_, ProjectHost>,
) -> Result<(), String> {
    state.validate_measurement(window.label(), &measurement)?;
    let project_id = projects.projection(window.label())?.state.project_id;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        topology = state.topology,
        window_label = window.label(),
        project_id = safe_log_identifier(&project_id),
        frame_id = safe_log_identifier(&measurement.frame_id),
        texture_backed = measurement.texture_backed,
        pan_sample_count = measurement.pan.sample_count,
        pan_duration_ms = measurement.pan.duration_ms,
        pan_first_frame_latency_ms = measurement.pan.first_frame_latency_ms,
        pan_mean_frame_ms = measurement.pan.mean_frame_ms,
        pan_p50_frame_ms = measurement.pan.p50_frame_ms,
        pan_p95_frame_ms = measurement.pan.p95_frame_ms,
        pan_p99_frame_ms = measurement.pan.p99_frame_ms,
        pan_max_frame_ms = measurement.pan.max_frame_ms,
        pan_frames_over16_ms = measurement.pan.frames_over16_ms,
        pan_frames_over33_ms = measurement.pan.frames_over33_ms,
        zoom_sample_count = measurement.zoom.sample_count,
        zoom_duration_ms = measurement.zoom.duration_ms,
        zoom_first_frame_latency_ms = measurement.zoom.first_frame_latency_ms,
        zoom_mean_frame_ms = measurement.zoom.mean_frame_ms,
        zoom_p50_frame_ms = measurement.zoom.p50_frame_ms,
        zoom_p95_frame_ms = measurement.zoom.p95_frame_ms,
        zoom_p99_frame_ms = measurement.zoom.p99_frame_ms,
        zoom_max_frame_ms = measurement.zoom.max_frame_ms,
        zoom_frames_over16_ms = measurement.zoom.frames_over16_ms,
        zoom_frames_over33_ms = measurement.zoom.frames_over33_ms,
        event = "canvas_benchmark_completed",
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn report_topology_benchmark_failure(
    reason: String,
    window: WebviewWindow,
    state: State<'_, TopologyBenchmarkState>,
    projects: State<'_, ProjectHost>,
) -> Result<(), String> {
    let config = state
        .config_for(window.label())?
        .ok_or_else(|| "O benchmark não está ativo.".to_string())?;
    if !config.gate_open {
        return Err("O gate do benchmark ainda está fechado.".into());
    }
    let reason = safe_log_identifier(&reason)
        .ok_or_else(|| "O motivo da falha do benchmark é inválido.".to_string())?;
    let project_id = projects.projection(window.label())?.state.project_id;
    tracing::error!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        process_id = std::process::id(),
        topology = state.topology,
        window_label = window.label(),
        project_id = safe_log_identifier(&project_id),
        reason,
        event = "topology_benchmark_failed",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CanvasBenchmarkMeasurement, FrameTimingSummary, TopologyBenchmarkState};

    fn timing(sample_count: usize) -> FrameTimingSummary {
        FrameTimingSummary {
            sample_count,
            duration_ms: 2_000.0,
            first_frame_latency_ms: 16.0,
            mean_frame_ms: 16.667,
            p50_frame_ms: 16.5,
            p95_frame_ms: 18.0,
            p99_frame_ms: 24.0,
            max_frame_ms: 28.0,
            frames_over16_ms: 30,
            frames_over33_ms: 0,
        }
    }

    #[test]
    fn opens_the_same_gate_for_every_window_but_exports_only_the_primary_project() {
        let directory = tempfile::tempdir().expect("temporary gate directory");
        let gate = directory.path().join("probe.ready");
        let state = TopologyBenchmarkState::new(
            "multiwindow",
            vec![("main", true), ("project-b", false)],
            Some(gate.clone()),
            Some(directory.path().join("export.ready")),
        )
        .expect("the benchmark state is valid");

        assert!(
            !state
                .config_for("main")
                .expect("main config is valid")
                .expect("the benchmark is active")
                .gate_open
        );
        std::fs::write(&gate, []).expect("the runner opens the gate");
        assert!(
            state
                .config_for("main")
                .expect("main config is valid")
                .expect("the benchmark is active")
                .run_export
        );
        assert!(
            !state
                .config_for("main")
                .expect("main config is valid")
                .expect("the benchmark is active")
                .export_gate_open
        );
        std::fs::write(directory.path().join("export.ready"), [])
            .expect("the runner opens the Export gate");
        assert!(
            state
                .config_for("main")
                .expect("main config is valid")
                .expect("the benchmark is active")
                .export_gate_open
        );
        assert!(
            !state
                .config_for("project-b")
                .expect("secondary config is valid")
                .expect("the benchmark is active")
                .run_export
        );

        let measurement = CanvasBenchmarkMeasurement {
            frame_id: "frame-01-a".into(),
            texture_backed: true,
            pan: timing(PAN_FRAMES),
            zoom: timing(ZOOM_FRAMES),
        };
        state
            .validate_measurement("main", &measurement)
            .expect("a correlated texture-backed measurement is valid");
    }

    #[test]
    fn standard_mode_does_not_expose_the_benchmark() {
        let state = TopologyBenchmarkState::new("standard", vec![], None, None)
            .expect("standard mode is valid");
        assert_eq!(
            state
                .config_for("main")
                .expect("standard configuration is valid"),
            None
        );
    }

    use super::{PAN_FRAMES, ZOOM_FRAMES};
}
