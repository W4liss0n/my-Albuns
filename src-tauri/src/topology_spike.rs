use std::path::{Component, Path, PathBuf};

use myalbuns_core::ProjectCore;
use myalbuns_logging::safe_log_identifier;

use crate::{
    benchmark_corpus::BenchmarkCorpus, project_host::ProjectHost, sample_project::SampleProject,
};

pub(crate) const TOPOLOGY_ENV: &str = "MYALBUNS_TOPOLOGY_SPIKE";
pub(crate) const PROJECT_SLOT_ENV: &str = "MYALBUNS_TOPOLOGY_PROJECT";
pub(crate) const CORPUS_MANIFEST_ENV: &str = "MYALBUNS_TOPOLOGY_CORPUS_MANIFEST";
pub(crate) const PROJECT_A_SOURCE_ENV: &str = "MYALBUNS_TOPOLOGY_PROJECT_A_SOURCE";
pub(crate) const PROJECT_B_SOURCE_ENV: &str = "MYALBUNS_TOPOLOGY_PROJECT_B_SOURCE";
const STANDARD_SHEET_COUNT: usize = 12;
const LONG_ALBUM_BENCHMARK_SHEET_COUNT: usize = 100;

pub(crate) struct TopologySpike {
    definition: TopologyDefinition,
    corpus: Option<BenchmarkCorpus>,
    source_overrides: ProjectSourceOverrides,
    run_id: Option<String>,
    sheet_count: usize,
}

#[derive(Default)]
struct ProjectSourceOverrides {
    project_a: Option<PathBuf>,
    project_b: Option<PathBuf>,
}

struct TopologyDefinition {
    label: &'static str,
    primary: ProjectWindow,
    secondary: Option<ProjectWindow>,
}

pub(crate) struct ProjectWindow {
    pub(crate) label: &'static str,
    pub(crate) title: String,
    sample: SampleProject,
}

impl TopologySpike {
    pub(crate) fn from_environment() -> Result<Self, String> {
        let mode = std::env::var(TOPOLOGY_ENV).ok();
        let project_slot = std::env::var(PROJECT_SLOT_ENV).ok();
        let corpus_manifest = std::env::var(CORPUS_MANIFEST_ENV).ok();
        let project_a_source = std::env::var_os(PROJECT_A_SOURCE_ENV).map(PathBuf::from);
        let project_b_source = std::env::var_os(PROJECT_B_SOURCE_ENV).map(PathBuf::from);
        let run_id = std::env::var(crate::global_process_spike::GLOBAL_RUN_ID_ENV).ok();
        Self::from_values_with_inputs(
            mode.as_deref(),
            project_slot.as_deref(),
            corpus_manifest.as_deref().map(Path::new),
            project_a_source,
            project_b_source,
            run_id,
        )
    }

    #[cfg(test)]
    fn from_values(mode: Option<&str>, project_slot: Option<&str>) -> Result<Self, String> {
        Self::from_values_with_corpus(mode, project_slot, None)
    }

    #[cfg(test)]
    fn from_values_with_corpus(
        mode: Option<&str>,
        project_slot: Option<&str>,
        corpus_manifest: Option<&Path>,
    ) -> Result<Self, String> {
        Self::from_values_with_inputs(mode, project_slot, corpus_manifest, None, None, None)
    }

    #[cfg(test)]
    fn from_values_with_sources(
        mode: Option<&str>,
        project_slot: Option<&str>,
        project_a_source: Option<&Path>,
        project_b_source: Option<&Path>,
        run_id: Option<&str>,
    ) -> Result<Self, String> {
        Self::from_values_with_inputs(
            mode,
            project_slot,
            None,
            project_a_source.map(Path::to_path_buf),
            project_b_source.map(Path::to_path_buf),
            run_id.map(str::to_owned),
        )
    }

    fn from_values_with_inputs(
        mode: Option<&str>,
        project_slot: Option<&str>,
        corpus_manifest: Option<&Path>,
        project_a_source: Option<PathBuf>,
        project_b_source: Option<PathBuf>,
        run_id: Option<String>,
    ) -> Result<Self, String> {
        let definition = match (mode, project_slot) {
            (None, None) => TopologyDefinition::standard(),
            (None, Some(_)) => {
                return Err(format!(
                    "{PROJECT_SLOT_ENV} só pode ser usado com {TOPOLOGY_ENV}=independent."
                ));
            }
            (Some("independent"), slot) => {
                TopologyDefinition::independent(sample_project_from_value(slot)?)
            }
            (Some("multiwindow"), None) => TopologyDefinition::multiwindow(),
            (Some("multiwindow"), Some(_)) => {
                return Err(format!(
                    "{PROJECT_SLOT_ENV} não se aplica a {TOPOLOGY_ENV}=multiwindow."
                ));
            }
            (Some(value), _) => {
                return Err(format!(
                    "Valor inválido em {TOPOLOGY_ENV}: {value}. Use independent ou multiwindow."
                ));
            }
        };

        if corpus_manifest.is_some() && mode.is_none() {
            return Err(format!(
                "{CORPUS_MANIFEST_ENV} só pode ser usado durante o spike de topologia."
            ));
        }
        let corpus = corpus_manifest.map(BenchmarkCorpus::load).transpose()?;
        let source_overrides = ProjectSourceOverrides {
            project_a: project_a_source,
            project_b: project_b_source,
        };
        source_overrides.validate(mode)?;
        if source_overrides.any() {
            let run_id = run_id.as_deref().ok_or_else(|| {
                format!(
                    "{} é obrigatório ao reabrir uma revisão salva pelo runner.",
                    crate::global_process_spike::GLOBAL_RUN_ID_ENV
                )
            })?;
            if safe_log_identifier(run_id).is_none() {
                return Err(format!(
                    "{} contém um identificador inválido.",
                    crate::global_process_spike::GLOBAL_RUN_ID_ENV
                ));
            }
        }
        let sheet_count = if mode.is_some() {
            LONG_ALBUM_BENCHMARK_SHEET_COUNT
        } else {
            STANDARD_SHEET_COUNT
        };
        Ok(Self {
            definition,
            corpus,
            source_overrides,
            run_id,
            sheet_count,
        })
    }

    pub(crate) fn project_host(&self) -> Result<ProjectHost, String> {
        self.definition
            .windows()
            .map(|window| {
                let media_sources = self
                    .corpus
                    .as_ref()
                    .map(|corpus| corpus.album_for(window.sample).media_sources())
                    .unwrap_or_default();
                let session = if let Some(path) = self.source_overrides.for_sample(window.sample) {
                    let source = std::fs::read_to_string(path).map_err(|error| {
                        format!("Não foi possível abrir a revisão salva do probe: {error}")
                    })?;
                    let session = ProjectCore::open_editable_session(&source)
                        .map_err(|error| error.to_string())?;
                    let state = session.state();
                    if state.project_id != window.sample.project_id() {
                        return Err(format!(
                            "A revisão salva não pertence ao Projeto esperado: {}.",
                            window.sample.project_id()
                        ));
                    }
                    session
                } else if let Some(corpus) = &self.corpus {
                    let album = corpus.album_for(window.sample);
                    album.open_session(window.sample, self.sheet_count)?
                } else {
                    let source = window
                        .sample
                        .persisted_source(self.sheet_count)
                        .map_err(|error| error.to_string())?;
                    ProjectCore::open_editable_session(&source)
                        .map_err(|error| error.to_string())?
                };
                Ok((window.label, session, media_sources))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(ProjectHost::new)
    }

    pub(crate) fn primary_title(&self) -> &str {
        &self.definition.primary.title
    }

    pub(crate) fn secondary_window(&self) -> Option<&ProjectWindow> {
        self.definition.secondary.as_ref()
    }

    pub(crate) fn label(&self) -> &'static str {
        self.definition.label
    }

    pub(crate) fn session_count(&self) -> usize {
        1 + usize::from(self.definition.secondary.is_some())
    }

    pub(crate) fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    pub(crate) fn reopened_window_labels(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.definition
            .windows()
            .filter(|window| self.source_overrides.for_sample(window.sample).is_some())
            .map(|window| window.label)
    }

    pub(crate) fn webview_data_namespace(&self) -> &'static str {
        match (self.definition.label, self.definition.primary.sample) {
            ("independent", SampleProject::Horizon) => "topology-independent-project-a",
            ("independent", SampleProject::Aurora) => "topology-independent-project-b",
            ("multiwindow", _) => "topology-multiwindow",
            _ => "standard",
        }
    }

    pub(crate) fn benchmark_window_settings(&self) -> Vec<(&'static str, bool)> {
        self.definition
            .windows()
            .map(|window| {
                (
                    window.label,
                    matches!(window.sample, SampleProject::Horizon),
                )
            })
            .collect()
    }
}

impl ProjectSourceOverrides {
    fn any(&self) -> bool {
        self.project_a.is_some() || self.project_b.is_some()
    }

    fn for_sample(&self, sample: SampleProject) -> Option<&Path> {
        match sample {
            SampleProject::Horizon => self.project_a.as_deref(),
            SampleProject::Aurora => self.project_b.as_deref(),
        }
    }

    fn validate(&self, mode: Option<&str>) -> Result<(), String> {
        if self.any() && mode.is_none() {
            return Err(format!(
                "{PROJECT_A_SOURCE_ENV} e {PROJECT_B_SOURCE_ENV} só podem ser usados durante o spike de topologia."
            ));
        }
        for (variable, path) in [
            (PROJECT_A_SOURCE_ENV, self.project_a.as_deref()),
            (PROJECT_B_SOURCE_ENV, self.project_b.as_deref()),
        ] {
            let Some(path) = path else {
                continue;
            };
            if !path.is_absolute()
                || path.extension().and_then(|value| value.to_str()) != Some("json")
                || path.components().any(|part| part == Component::ParentDir)
                || !path
                    .components()
                    .any(|part| matches!(part, Component::Normal(value) if value == ".scratch"))
                || !path.is_file()
            {
                return Err(format!("{variable} contém um caminho inválido."));
            }
        }
        Ok(())
    }
}

impl TopologyDefinition {
    fn standard() -> Self {
        Self {
            label: "standard",
            primary: ProjectWindow::new("main", SampleProject::Horizon, None),
            secondary: None,
        }
    }

    fn independent(sample: SampleProject) -> Self {
        Self {
            label: "independent",
            primary: ProjectWindow::new("main", sample, Some("Topologia A")),
            secondary: None,
        }
    }

    fn multiwindow() -> Self {
        Self {
            label: "multiwindow",
            primary: ProjectWindow::new("main", SampleProject::Horizon, Some("Topologia B")),
            secondary: Some(ProjectWindow::new(
                "project-b",
                SampleProject::Aurora,
                Some("Topologia B"),
            )),
        }
    }

    fn windows(&self) -> impl Iterator<Item = &ProjectWindow> {
        std::iter::once(&self.primary).chain(self.secondary.iter())
    }
}

impl ProjectWindow {
    fn new(label: &'static str, sample: SampleProject, topology: Option<&str>) -> Self {
        let title = match topology {
            Some(topology) => {
                format!("MyAlbuns — {} [{topology}]", sample.project_name())
            }
            None => format!("MyAlbuns — {}", sample.project_name()),
        };
        Self {
            label,
            title,
            sample,
        }
    }
}

fn sample_project_from_value(value: Option<&str>) -> Result<SampleProject, String> {
    match value {
        None | Some("a") => Ok(SampleProject::Horizon),
        Some("b") => Ok(SampleProject::Aurora),
        Some(value) => Err(format!(
            "Valor inválido em {PROJECT_SLOT_ENV}: {value}. Use a ou b."
        )),
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{ProjectCore, ProjectIntent};

    use super::TopologySpike;
    use crate::sample_project::SampleProject;

    fn saved_override(directory: &std::path::Path, sample: SampleProject) -> std::path::PathBuf {
        let scratch = directory.join(".scratch").join("topology-fault-probe");
        std::fs::create_dir_all(&scratch).expect("the test scratch directory is created");
        let path = scratch.join(format!("{}-r1.json", sample.project_id()));
        let source = sample
            .persisted_source(100)
            .expect("the sample project serializes");
        let mut session =
            ProjectCore::open_editable_session(&source).expect("the sample session opens");
        session
            .apply(ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.25,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            })
            .expect("the documentary action is applied");
        std::fs::write(
            &path,
            session
                .persisted_revision()
                .expect("the saved override serializes"),
        )
        .expect("the saved override is written");
        path
    }

    #[test]
    fn builds_comparable_independent_and_multiwindow_hosts() {
        let independent_a = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("independent project A is a valid spike configuration");
        let independent_b = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("independent project B is a valid spike configuration");
        let multiwindow = TopologySpike::from_values(Some("multiwindow"), None)
            .expect("multiwindow is a valid spike configuration");

        assert_eq!(
            independent_b
                .project_host()
                .expect("the independent host is built")
                .projection("main")
                .expect("the independent host owns its main session")
                .state
                .project_id,
            "project-spike-002"
        );
        assert_eq!(
            multiwindow
                .project_host()
                .expect("the multiwindow host is built")
                .projection("main")
                .expect("the multiwindow host owns project A")
                .state
                .project_id,
            "project-spike-001"
        );
        assert_eq!(
            multiwindow
                .project_host()
                .expect("the multiwindow host is built")
                .projection("project-b")
                .expect("the multiwindow host owns project B")
                .state
                .project_id,
            "project-spike-002"
        );

        assert_eq!(
            independent_b.benchmark_window_settings(),
            vec![("main", false)]
        );
        assert_eq!(
            multiwindow.benchmark_window_settings(),
            vec![("main", true), ("project-b", false)]
        );
        assert_eq!(
            independent_a.webview_data_namespace(),
            "topology-independent-project-a"
        );
        assert_eq!(
            independent_b.webview_data_namespace(),
            "topology-independent-project-b"
        );
        assert_eq!(multiwindow.webview_data_namespace(), "topology-multiwindow");
    }

    #[test]
    fn keeps_the_standard_sample_small_but_runs_the_topology_spike_with_a_long_album() {
        let standard =
            TopologySpike::from_values(None, None).expect("standard mode is a valid configuration");
        let independent = TopologySpike::from_values(Some("independent"), Some("a"))
            .expect("independent mode is a valid configuration");
        let multiwindow = TopologySpike::from_values(Some("multiwindow"), None)
            .expect("multiwindow mode is a valid configuration");

        assert_eq!(
            standard
                .project_host()
                .expect("the standard host is built")
                .projection("main")
                .expect("the standard projection is available")
                .composition
                .sheets
                .len(),
            12
        );
        assert_eq!(
            independent
                .project_host()
                .expect("the independent host is built")
                .projection("main")
                .expect("the independent projection is available")
                .composition
                .sheets
                .len(),
            100
        );
        assert_eq!(
            multiwindow
                .project_host()
                .expect("the multiwindow host is built")
                .projection("project-b")
                .expect("the secondary projection is available")
                .state
                .album
                .sheets
                .len(),
            100
        );
    }

    #[test]
    fn reopens_the_last_explicitly_saved_revision_from_a_runner_override() {
        let directory = tempfile::tempdir().expect("temporary override directory");
        let project_a = saved_override(directory.path(), SampleProject::Horizon);
        let topology = TopologySpike::from_values_with_sources(
            Some("independent"),
            Some("a"),
            Some(project_a.as_path()),
            None,
            Some("run-001"),
        )
        .expect("the absolute saved override is a valid restart source");
        assert_eq!(
            topology.reopened_window_labels().collect::<Vec<_>>(),
            vec!["main"]
        );
        assert_eq!(topology.run_id(), Some("run-001"));

        let projection = topology
            .project_host()
            .expect("the restarted host opens")
            .projection("main")
            .expect("the restarted projection is available");

        assert_eq!(projection.state.project_id, "project-spike-001");
        assert_eq!(projection.state.revision, 1);
        assert_eq!(projection.state.saved_revision, 1);
        assert!(!projection.state.dirty);
    }
}
