use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;

pub(crate) const GRAPHICS_GATE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status", deny_unknown_fields)]
pub(crate) enum GraphicsGateReport {
    Supported {},
    Unsupported {},
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GraphicsGateStatus {
    Pending,
    Supported,
    Rejected,
}

struct GraphicsLaunchGateState {
    status: GraphicsGateStatus,
    activation_projects: Vec<PathBuf>,
}

#[derive(Clone)]
pub(crate) struct GraphicsLaunchGate {
    state: Arc<Mutex<GraphicsLaunchGateState>>,
}

pub(crate) enum GraphicsGateCompletion {
    Ready(Vec<PathBuf>),
    Rejected,
    AlreadyFinal,
}

impl GraphicsLaunchGate {
    pub(crate) fn new(activation_projects: Vec<PathBuf>) -> Self {
        Self {
            state: Arc::new(Mutex::new(GraphicsLaunchGateState {
                status: GraphicsGateStatus::Pending,
                activation_projects,
            })),
        }
    }

    pub(crate) fn complete(&self, report: GraphicsGateReport) -> GraphicsGateCompletion {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.status != GraphicsGateStatus::Pending {
            return GraphicsGateCompletion::AlreadyFinal;
        }
        match report {
            GraphicsGateReport::Supported {} => {
                state.status = GraphicsGateStatus::Supported;
                GraphicsGateCompletion::Ready(std::mem::take(&mut state.activation_projects))
            }
            GraphicsGateReport::Unsupported {} => {
                state.status = GraphicsGateStatus::Rejected;
                state.activation_projects.clear();
                GraphicsGateCompletion::Rejected
            }
        }
    }

    pub(crate) fn expire(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.status != GraphicsGateStatus::Pending {
            return false;
        }
        state.status = GraphicsGateStatus::Rejected;
        state.activation_projects.clear();
        true
    }

    pub(crate) fn allows_project_host(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            == GraphicsGateStatus::Supported
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            == GraphicsGateStatus::Pending
    }

    pub(crate) fn has_pending_activation(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.status == GraphicsGateStatus::Pending && !state.activation_projects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn terminals_are_closed_and_cannot_be_reversed() {
        assert!(
            serde_json::from_value::<GraphicsGateReport>(serde_json::json!({
                "status": "supported",
                "extra": true
            }))
            .is_err()
        );

        let rejected = GraphicsLaunchGate::new(vec![PathBuf::from("Projeto.myalbuns")]);
        assert!(matches!(
            rejected.complete(GraphicsGateReport::Unsupported {}),
            GraphicsGateCompletion::Rejected
        ));
        assert!(matches!(
            rejected.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::AlreadyFinal
        ));
        assert!(!rejected.allows_project_host());

        let expired = GraphicsLaunchGate::new(vec![PathBuf::from("Projeto.myalbuns")]);
        assert!(expired.expire());
        assert!(matches!(
            expired.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::AlreadyFinal
        ));
        assert!(!expired.allows_project_host());
    }

    #[test]
    fn supported_gate_releases_the_full_native_activation_in_order() {
        let projects = vec![
            PathBuf::from(r"C:\Projetos\Primeiro.myalbuns"),
            PathBuf::from(r"\\servidor\Albuns\Segundo.myalbuns"),
        ];
        let gate = GraphicsLaunchGate::new(projects.clone());

        assert!(matches!(
            gate.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::Ready(released) if released == projects
        ));
    }
}
