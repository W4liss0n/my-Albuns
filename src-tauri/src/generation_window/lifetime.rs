//! Admission and draining belong to the native operation, not its lost WebView.
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct Lifetime {
    state: Mutex<State>,
    drained: Condvar,
    pub(crate) cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct State {
    active: bool,
    closing: bool,
}

pub(crate) struct Admission(Arc<Lifetime>);

impl Lifetime {
    pub(super) fn reopen(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Geração indisponível.")?;
        if state.active {
            return Err("Aguarde a operação atual.".into());
        }
        state.closing = false;
        Ok(())
    }

    pub(crate) fn begin(self: &Arc<Self>) -> Result<Admission, String> {
        let mut state = self.state.lock().map_err(|_| "Geração indisponível.")?;
        if state.closing {
            return Err("A janela de geração está fechando.".into());
        }
        if state.active {
            return Err("Aguarde a operação atual.".into());
        }
        self.cancel.store(false, Ordering::Release);
        state.active = true;
        Ok(Admission(self.clone()))
    }

    pub(crate) fn close(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closing = true;
        self.cancel.store(true, Ordering::Release);
        state.active
    }

    pub(super) fn is_closing(&self) -> bool {
        self.state.lock().map_or(true, |state| state.closing)
    }

    pub(super) fn is_active(&self) -> bool {
        self.state.lock().map_or(true, |state| state.active)
    }

    pub(crate) fn wait_until_drained(&self, timeout: Duration) -> Result<(), String> {
        let state = self.state.lock().map_err(|_| "Geração indisponível.")?;
        let (state, _) = self
            .drained
            .wait_timeout_while(state, timeout, |state| state.active)
            .map_err(|_| "Geração indisponível.")?;
        if state.active {
            return Err("A geração anterior ainda não terminou. Aguarde e tente novamente.".into());
        }
        Ok(())
    }
}

impl Drop for Admission {
    fn drop(&mut self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = false;
        self.0.drained.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_ui_operations::ProjectUiOperations;

    #[test]
    fn recovery_retires_generation_without_reopening_editor_admission() {
        let operations = ProjectUiOperations::default();
        let pause = operations.pause_for_batch().unwrap();
        let lifetime = Arc::new(Lifetime::default());
        let accepted = lifetime.begin().unwrap();
        let recovery = pause.recover().unwrap();
        lifetime.close();
        assert!(lifetime.cancel.load(Ordering::Acquire));
        assert!(lifetime.begin().is_err());
        assert!(lifetime.wait_until_drained(Duration::ZERO).is_err());
        assert!(operations.begin().is_err());
        drop(accepted);
        lifetime.wait_until_drained(Duration::ZERO).unwrap();
        drop(pause);
        assert!(recovery.is_drained());
        assert!(operations.begin().is_err());
        drop(recovery);
        assert!(operations.begin().is_ok());
        lifetime.reopen().unwrap();
        let next = lifetime.begin().unwrap();
        assert!(!lifetime.cancel.load(Ordering::Acquire));
        assert!(lifetime.close());
        drop(next);
        lifetime.wait_until_drained(Duration::ZERO).unwrap();
    }

    #[test]
    fn recovery_cannot_bypass_an_independent_batch_or_release_its_pause() {
        let operations = ProjectUiOperations::default();
        let generation = operations.pause_for_batch().unwrap();
        let batch = operations.pause_for_batch().unwrap();
        assert!(generation.recover().is_err());
        drop(generation);
        assert!(operations.begin().is_err());
        assert!(operations.recover().is_err());
        drop(batch);
        assert!(operations.recover().unwrap().is_drained());
    }
}
