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
    batch_pauses: usize,
}

pub(crate) struct ProjectUiOperation(ProjectUiOperations);
pub(crate) struct ProjectUiRecovery(ProjectUiOperations);
pub(crate) struct ProjectUiBatchPause(ProjectUiOperations);

impl ProjectUiOperations {
    pub(crate) fn pause_for_batch(&self) -> Result<ProjectUiBatchPause, String> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| "A interface está indisponível.")?;
        state.batch_pauses += 1;
        Ok(ProjectUiBatchPause(self.clone()))
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.0
            .lock()
            .is_ok_and(|state| state.active == 0 && !state.recovering)
    }
    pub(crate) fn begin(&self) -> Result<ProjectUiOperation, String> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| "A interface está indisponível.")?;
        if state.recovering {
            return Err("Aguarde a recuperação da interface.".into());
        }
        if state.batch_pauses > 0 {
            return Err("Aguarde o término da exportação em lote.".into());
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
        if state.batch_pauses > 0 {
            return Err("Aguarde o término da exportação em lote.".into());
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

impl Drop for ProjectUiBatchPause {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.0.lock() {
            state.batch_pauses -= 1;
        }
    }
}

pub(crate) fn begin(app: &AppHandle) -> Result<ProjectUiOperation, String> {
    if app
        .try_state::<crate::application_modality::ApplicationModality>()
        .is_some_and(|state| state.batch_active())
    {
        return Err("Aguarde o término da exportação em lote.".into());
    }
    app.state::<ProjectUiOperations>().begin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_closes_admission_before_draining_and_reopens_it_when_released() {
        let operations = ProjectUiOperations::default();
        let accepted = operations.begin().unwrap();
        let recovery = operations.recover().unwrap();
        let pause = operations.pause_for_batch().unwrap();
        assert!(!operations.is_idle());
        assert!(operations.begin().is_err());
        drop(accepted);
        assert!(!operations.is_idle());
        drop(recovery);
        assert!(operations.is_idle());
        assert!(operations.begin().is_err());
        assert!(operations.recover().is_err());
        drop(pause);
        assert!(operations.begin().is_ok());
    }

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
