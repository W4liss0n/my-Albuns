use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;

use crate::project_bootstrap::LaunchCancellation;

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
    /// Set once the activation's files started before the check reported.
    early_launch: Option<LaunchCancellation>,
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
                early_launch: None,
            })),
        }
    }

    /// Starts the activation's files while the Welcome checks the graphics.
    /// Their Hosts prepare the images meanwhile; each editor shows only once
    /// its own page passes the same check. A rejected check stops the Hosts
    /// that have not reported yet. Returns the files once, while pending.
    pub(crate) fn begin_early_launch(&self) -> Option<Vec<PathBuf>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.status != GraphicsGateStatus::Pending
            || state.activation_projects.is_empty()
            || state.early_launch.is_some()
        {
            return None;
        }
        state.early_launch = Some(LaunchCancellation::default());
        Some(state.activation_projects.clone())
    }

    /// What stops the Hosts of an early launch when the check rejects.
    pub(crate) fn early_launch_cancellation(&self) -> Option<LaunchCancellation> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .early_launch
            .clone()
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
                let projects = std::mem::take(&mut state.activation_projects);
                // Files that started early are not started again.
                GraphicsGateCompletion::Ready(if state.early_launch.is_some() {
                    Vec::new()
                } else {
                    projects
                })
            }
            GraphicsGateReport::Unsupported {} => {
                state.reject();
                GraphicsGateCompletion::Rejected
            }
        }
    }

    /// The check did not report in time. Files that started early keep
    /// opening: their own editors check the graphics.
    pub(crate) fn expire(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.status != GraphicsGateStatus::Pending || state.early_launch.is_some() {
            return false;
        }
        state.reject();
        true
    }

    /// An editor became ready, so its page passed the same check.
    pub(crate) fn confirm_by_editor(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.status == GraphicsGateStatus::Pending {
            state.status = GraphicsGateStatus::Supported;
            state.activation_projects.clear();
        }
    }

    /// Whether files opened by Windows may start their Hosts now.
    pub(crate) fn admits_activation(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.status == GraphicsGateStatus::Supported
            || (state.status == GraphicsGateStatus::Pending && state.early_launch.is_some())
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

    /// Files opened by Windows that wait for the check, or that started
    /// before it reported.
    pub(crate) fn has_pending_activation(&self) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.status == GraphicsGateStatus::Pending && !state.activation_projects.is_empty()
    }
}

impl GraphicsLaunchGateState {
    fn reject(&mut self) {
        self.status = GraphicsGateStatus::Rejected;
        self.activation_projects.clear();
        if let Some(early_launch) = &self.early_launch {
            early_launch.cancel();
        }
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

    #[test]
    fn files_started_early_are_not_started_again_and_stop_when_the_check_rejects() {
        let projects = vec![PathBuf::from(r"C:\Projetos\Primeiro.myalbuns")];

        let supported = GraphicsLaunchGate::new(projects.clone());
        assert!(!supported.admits_activation());
        assert_eq!(supported.begin_early_launch(), Some(projects.clone()));
        assert_eq!(supported.begin_early_launch(), None);
        assert!(supported.admits_activation());
        assert!(supported.has_pending_activation());
        let cancellation = supported.early_launch_cancellation().unwrap();
        assert!(matches!(
            supported.complete(GraphicsGateReport::Supported {}),
            GraphicsGateCompletion::Ready(released) if released.is_empty()
        ));
        assert!(!cancellation.is_cancelled());

        let rejected = GraphicsLaunchGate::new(projects.clone());
        rejected.begin_early_launch().unwrap();
        let cancellation = rejected.early_launch_cancellation().unwrap();
        assert!(matches!(
            rejected.complete(GraphicsGateReport::Unsupported {}),
            GraphicsGateCompletion::Rejected
        ));
        assert!(cancellation.is_cancelled());
        assert!(!rejected.admits_activation());
    }

    #[test]
    fn an_early_launch_outlives_a_slow_check_and_an_editor_settles_it() {
        let gate = GraphicsLaunchGate::new(vec![PathBuf::from("Projeto.myalbuns")]);
        gate.begin_early_launch().unwrap();
        assert!(!gate.expire());
        assert!(!gate.early_launch_cancellation().unwrap().is_cancelled());

        gate.confirm_by_editor();
        assert!(gate.allows_project_host());
        assert!(!gate.has_pending_activation());
        assert!(matches!(
            gate.complete(GraphicsGateReport::Unsupported {}),
            GraphicsGateCompletion::AlreadyFinal
        ));
    }

    #[test]
    fn without_an_early_launch_the_check_still_expires() {
        let gate = GraphicsLaunchGate::new(vec![PathBuf::from("Projeto.myalbuns")]);
        assert!(gate.expire());
        assert!(gate.begin_early_launch().is_none());
    }
}
