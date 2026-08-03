use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    export_pipeline::{ExportCancellationResult, ExportExecutionControl},
    ipc_contract::CancelDisposition,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ExportAttempts {
    inner: Arc<ExportAttemptsInner>,
}

#[derive(Debug, Default)]
struct ExportAttemptsInner {
    registrations: Mutex<HashMap<String, AttemptRegistration>>,
    next_generation: AtomicU64,
}

#[derive(Clone, Debug)]
struct AttemptRegistration {
    generation: u64,
    window_label: String,
    control: Arc<ExportExecutionControl>,
}

#[derive(Debug)]
pub(crate) struct ExportAttempt {
    operation_id: String,
    generation: u64,
    attempts: Arc<ExportAttemptsInner>,
    control: Arc<ExportExecutionControl>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BeginExportAttemptError {
    operation_id: String,
}

impl ExportAttempts {
    pub(crate) fn begin(
        &self,
        operation_id: impl Into<String>,
        window_label: impl Into<String>,
    ) -> Result<ExportAttempt, BeginExportAttemptError> {
        let operation_id = operation_id.into();
        let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let control = Arc::new(ExportExecutionControl::default());
        let registration = AttemptRegistration {
            generation,
            window_label: window_label.into(),
            control: Arc::clone(&control),
        };
        let mut registrations = self
            .inner
            .registrations
            .lock()
            .expect("the Export attempt registry remains available");
        if registrations.contains_key(&operation_id) {
            return Err(BeginExportAttemptError { operation_id });
        }
        registrations.insert(operation_id.clone(), registration);
        drop(registrations);

        Ok(ExportAttempt {
            operation_id,
            generation,
            attempts: Arc::clone(&self.inner),
            control,
        })
    }

    pub(crate) fn request_cancel(
        &self,
        operation_id: &str,
        window_label: &str,
    ) -> CancelDisposition {
        let control = {
            let registrations = self
                .inner
                .registrations
                .lock()
                .expect("the Export attempt registry remains available");
            let Some(registration) = registrations.get(operation_id) else {
                return CancelDisposition::NotFound;
            };
            if registration.window_label != window_label {
                return CancelDisposition::NotFound;
            }
            Arc::clone(&registration.control)
        };
        control.request_cancel().into()
    }

    pub(crate) fn request_cancel_for_window(&self, window_label: &str) -> usize {
        let controls = {
            let registrations = self
                .inner
                .registrations
                .lock()
                .expect("the Export attempt registry remains available");
            registrations
                .values()
                .filter(|registration| registration.window_label == window_label)
                .map(|registration| Arc::clone(&registration.control))
                .collect::<Vec<_>>()
        };

        controls
            .into_iter()
            .filter(|control| {
                matches!(
                    control.request_cancel(),
                    ExportCancellationResult::Requested
                        | ExportCancellationResult::AlreadyRequested
                )
            })
            .count()
    }
}

impl ExportAttempt {
    pub(crate) fn execution_control(&self) -> &ExportExecutionControl {
        &self.control
    }

    #[cfg(test)]
    fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }

    pub(crate) async fn cancelled(&self) {
        self.control.cancelled().await;
    }

    pub(crate) fn request_cancel(&self) -> CancelDisposition {
        self.control.request_cancel().into()
    }
}

impl Drop for ExportAttempt {
    fn drop(&mut self) {
        let mut registrations = self
            .attempts
            .registrations
            .lock()
            .expect("the Export attempt registry remains available");
        if registrations
            .get(&self.operation_id)
            .is_some_and(|registration| registration.generation == self.generation)
        {
            registrations.remove(&self.operation_id);
        }
    }
}

impl From<ExportCancellationResult> for CancelDisposition {
    fn from(result: ExportCancellationResult) -> Self {
        match result {
            ExportCancellationResult::Requested => Self::Requested,
            ExportCancellationResult::AlreadyRequested => Self::AlreadyRequested,
            ExportCancellationResult::TooLate => Self::TooLate,
        }
    }
}

impl fmt::Display for BeginExportAttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a tentativa de Exportação {} já está registrada",
            self.operation_id
        )
    }
}

impl std::error::Error for BeginExportAttemptError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::ExportAttempts;
    use crate::ipc_contract::CancelDisposition;

    #[test]
    fn only_the_owning_window_can_cancel_its_active_export_attempt() {
        tauri::async_runtime::block_on(async {
            let attempts = ExportAttempts::default();
            let attempt = attempts
                .begin("export-17", "main")
                .expect("the first Export attempt starts");

            assert_eq!(
                attempts.request_cancel("export-17", "other-window"),
                CancelDisposition::NotFound
            );
            assert_eq!(
                attempts.request_cancel("export-17", "main"),
                CancelDisposition::Requested
            );
            tokio::time::timeout(Duration::from_secs(1), attempt.cancelled())
                .await
                .expect("the owner cancellation wakes the attempt");
            assert_eq!(
                attempts.request_cancel("export-17", "main"),
                CancelDisposition::AlreadyRequested
            );
        });
    }

    #[test]
    fn dropping_an_attempt_cannot_remove_a_later_registration() {
        let attempts = ExportAttempts::default();
        let first = attempts
            .begin("export-reused", "main")
            .expect("the first registration starts");
        assert!(
            attempts.begin("export-reused", "main").is_err(),
            "an active ID cannot be reused"
        );
        drop(first);

        let second = attempts
            .begin("export-reused", "main")
            .expect("the ID can be reused after the owner leaves");
        assert_eq!(
            attempts.request_cancel("export-reused", "main"),
            CancelDisposition::Requested
        );
        assert!(second.is_cancelled());
    }

    #[test]
    fn cancellation_results_have_a_stable_ipc_contract() {
        assert_eq!(
            [
                serde_json::to_value(CancelDisposition::Requested).expect("Requested serializes"),
                serde_json::to_value(CancelDisposition::AlreadyRequested)
                    .expect("AlreadyRequested serializes"),
                serde_json::to_value(CancelDisposition::TooLate).expect("TooLate serializes"),
                serde_json::to_value(CancelDisposition::NotFound).expect("NotFound serializes"),
            ],
            [
                json!("requested"),
                json!("already_requested"),
                json!("too_late"),
                json!("not_found"),
            ]
        );
    }

    #[test]
    fn destroying_a_window_cancels_only_its_owned_attempts() {
        let attempts = ExportAttempts::default();
        let first = attempts
            .begin("export-window-a-1", "project-a")
            .expect("the first attempt starts");
        let second = attempts
            .begin("export-window-a-2", "project-a")
            .expect("the second attempt starts");
        let other = attempts
            .begin("export-window-b", "other-window")
            .expect("the other window attempt starts");

        assert_eq!(attempts.request_cancel_for_window("project-a"), 2);
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
        assert!(!other.is_cancelled());
    }
}
