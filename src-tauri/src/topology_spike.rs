use myalbuns_core::{ProjectCore, SampleProject};

use crate::project_host::ProjectHost;

pub(crate) const TOPOLOGY_ENV: &str = "MYALBUNS_TOPOLOGY_SPIKE";
pub(crate) const PROJECT_SLOT_ENV: &str = "MYALBUNS_TOPOLOGY_PROJECT";

pub(crate) struct TopologySpike {
    definition: TopologyDefinition,
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
        Self::from_values(mode.as_deref(), project_slot.as_deref())
    }

    fn from_values(mode: Option<&str>, project_slot: Option<&str>) -> Result<Self, String> {
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

        Ok(Self { definition })
    }

    pub(crate) fn project_host(&self) -> ProjectHost {
        ProjectHost::new(self.definition.windows().map(|window| {
            (
                window.label,
                ProjectCore::open_sample_project(12, window.sample),
            )
        }))
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
    use super::TopologySpike;

    #[test]
    fn builds_comparable_independent_and_multiwindow_hosts() {
        let independent_b = TopologySpike::from_values(Some("independent"), Some("b"))
            .expect("independent project B is a valid spike configuration");
        let multiwindow = TopologySpike::from_values(Some("multiwindow"), None)
            .expect("multiwindow is a valid spike configuration");

        assert_eq!(
            independent_b
                .project_host()
                .projection("main")
                .expect("the independent host owns its main session")
                .state
                .project_id,
            "project-spike-002"
        );
        assert_eq!(
            multiwindow
                .project_host()
                .projection("main")
                .expect("the multiwindow host owns project A")
                .state
                .project_id,
            "project-spike-001"
        );
        assert_eq!(
            multiwindow
                .project_host()
                .projection("project-b")
                .expect("the multiwindow host owns project B")
                .state
                .project_id,
            "project-spike-002"
        );
    }
}
