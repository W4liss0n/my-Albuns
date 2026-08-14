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
    direct_project: Option<PathBuf>,
}

#[derive(Clone)]
pub(crate) struct GraphicsLaunchGate {
    state: Arc<Mutex<GraphicsLaunchGateState>>,
}

pub(crate) enum GraphicsGateCompletion {
    Ready(Option<PathBuf>),
    Rejected,
    AlreadyFinal,
}

impl GraphicsLaunchGate {
    pub(crate) fn new(direct_project: Option<PathBuf>) -> Self {
        Self {
            state: Arc::new(Mutex::new(GraphicsLaunchGateState {
                status: GraphicsGateStatus::Pending,
                direct_project,
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
                GraphicsGateCompletion::Ready(state.direct_project.take())
            }
            GraphicsGateReport::Unsupported {} => {
                state.status = GraphicsGateStatus::Rejected;
                state.direct_project.take();
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
        state.direct_project.take();
        true
    }

    pub(crate) fn allows_project_host(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            == GraphicsGateStatus::Supported
    }

    pub(crate) fn has_pending_direct_project(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.status == GraphicsGateStatus::Pending && state.direct_project.is_some()
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

        let rejected = GraphicsLaunchGate::new(Some(PathBuf::from("Projeto.myalbuns")));
        assert!(matches!(
            rejected.complete(GraphicsGateReport::Unsupported {}),
            GraphicsGateCompletion::Rejected
        ));
        assert!(matches!(
            rejected.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::AlreadyFinal
        ));
        assert!(!rejected.allows_project_host());

        let expired = GraphicsLaunchGate::new(Some(PathBuf::from("Projeto.myalbuns")));
        assert!(expired.expire());
        assert!(matches!(
            expired.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::AlreadyFinal
        ));
        assert!(!expired.allows_project_host());
    }
}
