use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use serde::Serialize;
use tokio::sync::Notify;

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
    cancellation: Arc<AttemptCancellation>,
}

#[derive(Debug)]
pub(crate) struct ExportAttempt {
    operation_id: String,
    generation: u64,
    attempts: Arc<ExportAttemptsInner>,
    cancellation: Arc<AttemptCancellation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CancelDisposition {
    Requested,
    AlreadyRequested,
    TooLate,
    NotFound,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BeginExportAttemptError {
    operation_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancellationPhase {
    Running,
    Cancelled,
    Publishing,
}

#[derive(Debug)]
struct AttemptCancellation {
    cancelled: AtomicBool,
    phase: Mutex<CancellationPhase>,
    notification: Notify,
}

impl ExportAttempts {
    pub(crate) fn begin(
        &self,
        operation_id: impl Into<String>,
        window_label: impl Into<String>,
    ) -> Result<ExportAttempt, BeginExportAttemptError> {
        let operation_id = operation_id.into();
        let generation = self.inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let cancellation = Arc::new(AttemptCancellation::new());
        let registration = AttemptRegistration {
            generation,
            window_label: window_label.into(),
            cancellation: Arc::clone(&cancellation),
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
            cancellation,
        })
    }

    pub(crate) fn request_cancel(
        &self,
        operation_id: &str,
        window_label: &str,
    ) -> CancelDisposition {
        let cancellation = {
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
            Arc::clone(&registration.cancellation)
        };
        cancellation.request_cancel()
    }

    pub(crate) fn request_cancel_for_window(&self, window_label: &str) -> usize {
        let cancellations = {
            let registrations = self
                .inner
                .registrations
                .lock()
                .expect("the Export attempt registry remains available");
            registrations
                .values()
                .filter(|registration| registration.window_label == window_label)
                .map(|registration| Arc::clone(&registration.cancellation))
                .collect::<Vec<_>>()
        };

        cancellations
            .into_iter()
            .filter(|cancellation| {
                matches!(
                    cancellation.request_cancel(),
                    CancelDisposition::Requested | CancelDisposition::AlreadyRequested
                )
            })
            .count()
    }
}

impl ExportAttempt {
    pub(crate) fn cancellation_flag(&self) -> &AtomicBool {
        &self.cancellation.cancelled
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        loop {
            let notified = self.cancellation.notification.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }

    pub(crate) fn begin_publishing(&self) -> bool {
        self.cancellation.begin_publishing()
    }

    pub(crate) fn request_cancel(&self) -> CancelDisposition {
        self.cancellation.request_cancel()
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

impl AttemptCancellation {
    fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            phase: Mutex::new(CancellationPhase::Running),
            notification: Notify::new(),
        }
    }

    fn request_cancel(&self) -> CancelDisposition {
        let mut phase = self
            .phase
            .lock()
            .expect("the Export cancellation state remains available");
        match *phase {
            CancellationPhase::Running => {
                self.cancelled.store(true, Ordering::Release);
                *phase = CancellationPhase::Cancelled;
                drop(phase);
                self.notification.notify_one();
                CancelDisposition::Requested
            }
            CancellationPhase::Cancelled => CancelDisposition::AlreadyRequested,
            CancellationPhase::Publishing => CancelDisposition::TooLate,
        }
    }

    fn begin_publishing(&self) -> bool {
        let mut phase = self
            .phase
            .lock()
            .expect("the Export cancellation state remains available");
        match *phase {
            CancellationPhase::Running => {
                *phase = CancellationPhase::Publishing;
                true
            }
            CancellationPhase::Cancelled => false,
            CancellationPhase::Publishing => true,
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

    use super::{CancelDisposition, ExportAttempts};

    #[test]
    fn only_the_owning_window_can_cancel_its_active_export_attempt() {
        tauri::async_runtime::block_on(async {
            let attempts = ExportAttempts::default();
            let attempt = attempts
                .begin("export-17", "main")
                .expect("the first Export attempt starts");

            assert_eq!(
                attempts.request_cancel("export-17", "project-b"),
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
    fn publication_and_cancellation_have_one_atomic_winner() {
        let attempts = ExportAttempts::default();
        let attempt = attempts
            .begin("export-18", "main")
            .expect("the Export attempt starts");

        assert!(attempt.begin_publishing());
        assert_eq!(
            attempts.request_cancel("export-18", "main"),
            CancelDisposition::TooLate
        );
        assert!(!attempt.is_cancelled());
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
            .begin("export-window-b", "project-b")
            .expect("the other window attempt starts");

        assert_eq!(attempts.request_cancel_for_window("project-a"), 2);
        assert!(first.is_cancelled());
        assert!(second.is_cancelled());
        assert!(!other.is_cancelled());
    }
}
