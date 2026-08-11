#[cfg(test)]
use std::fmt;

use crate::{
    cache_activity_gate::CachePause,
    cache_engine::CacheEngine,
    imaging_processor::{ImagingProcessor, ProcessorReservation, ProcessorUnavailable},
    operation_gate::{OperationGate, OperationGateError, OperationGrant},
};

#[derive(Debug)]
pub(crate) struct OperationLease {
    processor_reservation: ProcessorReservation,
    _cache_pause: CachePause,
    _gate_grant: OperationGrant,
}

#[derive(Debug)]
pub(crate) struct OperationLeaseAcquisition {
    gate_grant: OperationGrant,
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) enum OperationLeaseError {
    Gate(OperationGateError),
    Processor(ProcessorUnavailable),
}

impl OperationLease {
    pub(crate) fn begin(
        gate: &OperationGate,
    ) -> Result<OperationLeaseAcquisition, OperationGateError> {
        Ok(OperationLeaseAcquisition {
            gate_grant: gate.try_acquire()?,
        })
    }

    #[cfg(test)]
    pub(crate) async fn acquire(
        gate: &OperationGate,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
    ) -> Result<Self, OperationLeaseError> {
        Self::begin(gate)
            .map_err(OperationLeaseError::Gate)?
            .complete(cache, processor)
            .await
            .map_err(OperationLeaseError::Processor)
    }

    pub(crate) fn processor_reservation(&self) -> &ProcessorReservation {
        &self.processor_reservation
    }
}

impl OperationLeaseAcquisition {
    pub(crate) async fn complete(
        self,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
    ) -> Result<OperationLease, ProcessorUnavailable> {
        let cache_pause = cache.pause().await;
        let processor_reservation = processor.reserve().await?;
        Ok(OperationLease {
            processor_reservation,
            _cache_pause: cache_pause,
            _gate_grant: self.gate_grant,
        })
    }
}

#[cfg(test)]
impl fmt::Display for OperationLeaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gate(error) => error.fmt(formatter),
            Self::Processor(error) => error.fmt(formatter),
        }
    }
}

#[cfg(test)]
impl std::error::Error for OperationLeaseError {}

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        path::Path,
        time::Duration,
    };

    use myalbuns_paths::AppPaths;
    use tempfile::tempdir;

    use super::OperationLease;
    use crate::{
        cache_activity_gate::{CacheCancellation, CacheCancellationReason},
        cache_engine::CacheEngine,
        imaging_processor::ImagingProcessor,
        operation_gate::OperationGate,
    };

    fn app_paths(root: &Path) -> AppPaths {
        AppPaths::from_roots(&root.join("roaming"), &root.join("local"), root)
    }

    #[test]
    fn dropping_the_lease_releases_gate_cache_and_processor_together() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the release fixture exists");
            let paths = app_paths(root.path());
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let lease = OperationLease::acquire(&gate, &cache, &processor)
                .await
                .expect("the attempt acquires the common lease");

            assert!(
                OperationLease::acquire(&gate, &cache, &processor)
                    .await
                    .is_err(),
                "another Export is refused without a queue"
            );
            let cache_work = cache.begin_cancellable_work(CacheCancellation::default());
            tokio::pin!(cache_work);
            let processor_work = processor.reserve();
            tokio::pin!(processor_work);
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut cache_work)
                    .await
                    .is_err()
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut processor_work)
                    .await
                    .is_err()
            );

            drop(lease);
            tokio::time::timeout(Duration::from_secs(1), &mut cache_work)
                .await
                .expect("Cache resumes with the lease release");
            tokio::time::timeout(Duration::from_secs(1), &mut processor_work)
                .await
                .expect("the Processor returns with the lease release")
                .expect("the Processor remains healthy");
            OperationLease::acquire(&gate, &cache, &processor)
                .await
                .expect("the next Export acquires immediately");
        });
    }

    #[test]
    fn unwinding_an_attempt_does_not_leak_any_lease_resource() {
        let root = tempdir().expect("the unwind fixture exists");
        let paths = app_paths(root.path());
        let gate = OperationGate::new(&paths);
        let cache = CacheEngine::default();
        let processor = ImagingProcessor::default();

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            tauri::async_runtime::block_on(async {
                let _lease = OperationLease::acquire(&gate, &cache, &processor)
                    .await
                    .expect("the failed attempt acquires");
                panic!("injected operation failure");
            });
        }));
        assert!(outcome.is_err(), "the injected failure must unwind");

        tauri::async_runtime::block_on(async {
            OperationLease::acquire(&gate, &cache, &processor)
                .await
                .expect("the successor acquires every resource after unwind");
        });
    }

    #[test]
    fn beginning_a_lease_resolves_the_global_conflict_before_waiting_for_processor() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the staged acquisition fixture exists");
            let paths = app_paths(root.path());
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let occupied_processor = processor
                .reserve()
                .await
                .expect("the Processor can be reserved");

            let acquisition = OperationLease::begin(&gate)
                .expect("the first caller resolves the gate immediately");
            assert!(
                OperationLease::begin(&gate).is_err(),
                "a concurrent caller receives a conflict before progress can start"
            );

            let mut completion = Box::pin(acquisition.complete(&cache, &processor));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut completion)
                    .await
                    .is_err(),
                "the acquisition waits for the Processor only after owning the gate"
            );
            drop(completion);

            OperationLease::begin(&gate)
                .expect("cancelling the completion releases the partial gate grant");
            drop(occupied_processor);
        });
    }

    #[test]
    fn cancelling_while_waiting_for_processor_releases_gate() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the Processor-cancellation fixture exists");
            let paths = app_paths(root.path());
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let occupied_processor = processor
                .reserve()
                .await
                .expect("the Processor can be reserved");
            let mut pending = Box::pin(OperationLease::acquire(&gate, &cache, &processor));

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pending)
                    .await
                    .is_err(),
                "the attempt must be waiting after acquiring the gate"
            );
            drop(pending);

            gate.try_acquire()
                .expect("cancelling the pending future releases its gate grant");
            tokio::time::timeout(
                Duration::from_secs(1),
                cache.begin_cancellable_work(CacheCancellation::default()),
            )
            .await
            .expect("cancelling the pending future releases its Cache pause");
            drop(occupied_processor);
        });
    }

    #[test]
    fn processor_quarantine_fails_the_lease_without_leaking_gate() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the quarantine fixture exists");
            let paths = app_paths(root.path());
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let reservation = processor
                .reserve()
                .await
                .expect("the healthy Processor can be reserved");
            reservation.quarantine();
            drop(reservation);

            let failure = OperationLease::begin(&gate)
                .expect("the gate can be acquired")
                .complete(&cache, &processor)
                .await
                .expect_err("a quarantined Processor refuses a lease");

            assert_eq!(
                failure.to_string(),
                "o Processador de Imagens está em quarentena porque o encerramento anterior não foi confirmado; reinicie o aplicativo antes de tentar novamente"
            );
            assert!(
                OperationLease::begin(&gate).is_ok(),
                "the failed completion releases the gate"
            );
            tokio::time::timeout(
                Duration::from_secs(1),
                cache.begin_cancellable_work(CacheCancellation::default()),
            )
            .await
            .expect("the failed completion releases the Cache pause");
        });
    }

    #[test]
    fn lease_waits_for_active_cache_before_reserving_the_processor() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the Cache-pause fixture exists");
            let paths = app_paths(root.path());
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let cancellation = CacheCancellation::default();
            let active_cache = cache.begin_cancellable_work(cancellation.clone()).await;
            let mut lease = Box::pin(OperationLease::acquire(&gate, &cache, &processor));

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut lease)
                    .await
                    .is_err(),
                "the lease waits until active Cache work reaches a safe endpoint"
            );
            assert!(
                cancellation
                    .flag()
                    .load(std::sync::atomic::Ordering::Acquire),
                "the export pause causally cancels the active Cache sidecar first"
            );
            assert_eq!(cancellation.reason(), Some(CacheCancellationReason::Paused));

            drop(active_cache);
            let lease = tokio::time::timeout(Duration::from_secs(1), &mut lease)
                .await
                .expect("the Cache pause becomes available")
                .expect("the complete lease is acquired");
            let mut blocked_resume = Box::pin(cache.begin_cancellable_work(cancellation.clone()));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut blocked_resume)
                    .await
                    .is_err(),
                "the Cache cannot resume while Export owns its causal pause"
            );
            drop(lease);
            let resumed = cancellation.resume_after_pause();
            assert!(resumed, "the same non-obsolete demand can resume");
            tokio::time::timeout(Duration::from_secs(1), &mut blocked_resume)
                .await
                .expect("the Cache resumes after every Export terminal drops its lease");
        });
    }
}
