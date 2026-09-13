//! Drain commands owned by a failed editor before reading its replacement projection.
//! Background Cache work is independent and does not belong to this lifetime.
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

#[derive(Clone, Default)]
pub(crate) struct ProjectUiOperations(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    active: usize,
    recovering: bool,
}

pub(crate) struct ProjectUiOperation(ProjectUiOperations);
pub(crate) struct ProjectUiRecovery(ProjectUiOperations);

impl ProjectUiOperations {
    pub(crate) fn begin(&self) -> Result<ProjectUiOperation, String> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| "A interface está indisponível.")?;
        if state.recovering {
            return Err("Aguarde a recuperação da interface.".into());
        }
        state.active += 1;
        Ok(ProjectUiOperation(self.clone()))
    }

    pub(crate) fn recover(&self) -> Result<ProjectUiRecovery, String> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| "A interface está indisponível.")?;
        if state.recovering {
            return Err("A recuperação da interface já está em andamento.".into());
        }
        state.recovering = true;
        Ok(ProjectUiRecovery(self.clone()))
    }
}

impl ProjectUiRecovery {
    pub(crate) fn is_drained(&self) -> bool {
        self.0.0.lock().is_ok_and(|state| state.active == 0)
    }
}

impl Drop for ProjectUiOperation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.0.lock() {
            state.active -= 1;
        }
    }
}

impl Drop for ProjectUiRecovery {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.0.lock() {
            state.recovering = false;
        }
    }
}

pub(crate) fn begin(app: &AppHandle) -> Result<ProjectUiOperation, String> {
    app.state::<ProjectUiOperations>().begin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_drains_accepted_commands_and_reopens_admission_on_success_or_failure() {
        let operations = ProjectUiOperations::default();
        let importing = operations.begin().unwrap();
        let exporting = operations.begin().unwrap();
        let recovery = operations.recover().unwrap();
        assert!(!recovery.is_drained());
        assert!(operations.begin().is_err());
        assert!(operations.recover().is_err());
        drop(importing);
        assert!(!recovery.is_drained());
        drop(exporting);
        assert!(recovery.is_drained());
        drop(recovery);
        let pending = operations.begin().unwrap();
        let failed_recovery = operations.recover().unwrap();
        drop(failed_recovery);
        assert!(operations.begin().is_ok());
        drop(pending);
        assert!(operations.recover().unwrap().is_drained());
    }
}
