use myalbuns_core::{ProjectCore, ProjectSession};

use crate::project_host::ProjectHost;

pub(crate) const TOPOLOGY_ENV: &str = "MYALBUNS_TOPOLOGY_SPIKE";
pub(crate) const PROJECT_SLOT_ENV: &str = "MYALBUNS_TOPOLOGY_PROJECT";

#[derive(Clone, Copy)]
pub(crate) struct TopologySpike {
    mode: TopologyMode,
}

#[derive(Clone, Copy)]
enum TopologyMode {
    Standard,
    Independent(ProjectSlot),
    Multiwindow,
}

#[derive(Clone, Copy)]
enum ProjectSlot {
    A,
    B,
}

#[derive(Clone, Copy)]
pub(crate) struct SecondaryWindow {
    pub(crate) label: &'static str,
    pub(crate) title: &'static str,
}

impl TopologySpike {
    pub(crate) fn from_environment() -> Result<Self, String> {
        let mode = std::env::var(TOPOLOGY_ENV).ok();
        let project_slot = std::env::var(PROJECT_SLOT_ENV).ok();
        Self::from_values(mode.as_deref(), project_slot.as_deref())
    }

    fn from_values(mode: Option<&str>, project_slot: Option<&str>) -> Result<Self, String> {
        let mode = match (mode, project_slot) {
            (None, None) => TopologyMode::Standard,
            (None, Some(_)) => {
                return Err(format!(
                    "{PROJECT_SLOT_ENV} só pode ser usado com {TOPOLOGY_ENV}=independent."
                ));
            }
            (Some("independent"), slot) => {
                TopologyMode::Independent(ProjectSlot::from_value(slot)?)
            }
            (Some("multiwindow"), None) => TopologyMode::Multiwindow,
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

        Ok(Self { mode })
    }

    pub(crate) fn project_host(self) -> ProjectHost {
        match self.mode {
            TopologyMode::Standard => ProjectHost::new([("main", ProjectSlot::A.open_session())]),
            TopologyMode::Independent(slot) => ProjectHost::new([("main", slot.open_session())]),
            TopologyMode::Multiwindow => ProjectHost::new([
                ("main", ProjectSlot::A.open_session()),
                ("project-b", ProjectSlot::B.open_session()),
            ]),
        }
    }

    pub(crate) fn primary_title(self) -> &'static str {
        match self.mode {
            TopologyMode::Standard => "MyAlbuns — Álbum Horizonte",
            TopologyMode::Independent(ProjectSlot::A) => "MyAlbuns — Álbum Horizonte [Topologia A]",
            TopologyMode::Independent(ProjectSlot::B) => "MyAlbuns — Álbum Aurora [Topologia A]",
            TopologyMode::Multiwindow => "MyAlbuns — Álbum Horizonte [Topologia B]",
        }
    }

    pub(crate) fn secondary_window(self) -> Option<SecondaryWindow> {
        matches!(self.mode, TopologyMode::Multiwindow).then_some(SecondaryWindow {
            label: "project-b",
            title: "MyAlbuns — Álbum Aurora [Topologia B]",
        })
    }

    pub(crate) fn label(self) -> &'static str {
        match self.mode {
            TopologyMode::Standard => "standard",
            TopologyMode::Independent(_) => "independent",
            TopologyMode::Multiwindow => "multiwindow",
        }
    }

    pub(crate) fn session_count(self) -> usize {
        match self.mode {
            TopologyMode::Multiwindow => 2,
            TopologyMode::Standard | TopologyMode::Independent(_) => 1,
        }
    }
}

impl ProjectSlot {
    fn from_value(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("a") => Ok(Self::A),
            Some("b") => Ok(Self::B),
            Some(value) => Err(format!(
                "Valor inválido em {PROJECT_SLOT_ENV}: {value}. Use a ou b."
            )),
        }
    }

    fn open_session(self) -> ProjectSession {
        match self {
            Self::A => ProjectCore::open_sample_project_with_identity(
                12,
                "project-spike-001",
                "Álbum Horizonte",
            ),
            Self::B => ProjectCore::open_sample_project_with_identity(
                12,
                "project-spike-002",
                "Álbum Aurora",
            ),
        }
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
