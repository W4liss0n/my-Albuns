#[cfg(test)]
use std::fmt;

use crate::{
    cache_engine::{CacheEngine, CachePause},
    imaging_processor::{ImagingProcessor, ProcessorReservation, ProcessorUnavailable},
    operation_gate::{OperationGate, OperationGateError, OperationGrant, OperationMode},
};

#[derive(Debug)]
pub(crate) struct OperationLease {
    processor_reservation: Option<ProcessorReservation>,
    cache_pause: Option<CachePause>,
    gate_grant: Option<OperationGrant>,
}

#[derive(Debug)]
pub(crate) struct OperationLeaseAcquisition {
    gate_grant: Option<OperationGrant>,
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
        mode: OperationMode,
    ) -> Result<OperationLeaseAcquisition, OperationGateError> {
        Ok(OperationLeaseAcquisition {
            gate_grant: Some(gate.try_acquire(mode)?),
        })
    }

    #[cfg(test)]
    pub(crate) async fn acquire(
        gate: &OperationGate,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
        mode: OperationMode,
    ) -> Result<Self, OperationLeaseError> {
        Self::begin(gate, mode)
            .map_err(OperationLeaseError::Gate)?
            .complete(cache, processor)
            .await
            .map_err(OperationLeaseError::Processor)
    }

    pub(crate) fn mode(&self) -> OperationMode {
        self.gate_grant
            .as_ref()
            .expect("an OperationLease keeps its gate grant until drop")
            .mode()
    }

    pub(crate) fn processor_reservation(&self) -> &ProcessorReservation {
        self.processor_reservation
            .as_ref()
            .expect("an OperationLease keeps its Processor reservation until drop")
    }
}

impl OperationLeaseAcquisition {
    pub(crate) async fn complete(
        mut self,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
    ) -> Result<OperationLease, ProcessorUnavailable> {
        let cache_pause = cache.pause().await;
        let processor_reservation = processor.reserve().await?;
        Ok(OperationLease {
            processor_reservation: Some(processor_reservation),
            cache_pause: Some(cache_pause),
            gate_grant: self.gate_grant.take(),
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

impl Drop for OperationLease {
    fn drop(&mut self) {
        self.processor_reservation.take();
        self.cache_pause.take();
        self.gate_grant.take();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        time::Duration,
    };

    use myalbuns_paths::AppPaths;
    use tempfile::tempdir;

    use super::OperationLease;
    use crate::{
        cache_engine::CacheEngine,
        imaging_processor::ImagingProcessor,
        operation_gate::{OperationGate, OperationMode},
    };

    #[test]
    fn lease_waits_for_active_cache_before_it_reserves_the_operation() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the lease fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let active_cache = cache.begin_work().await;
            let lease =
                OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport);
            tokio::pin!(lease);

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut lease)
                    .await
                    .is_err(),
                "the lease must wait for active Cache work before reserving the Processor"
            );

            drop(active_cache);
            let lease = tokio::time::timeout(Duration::from_secs(1), &mut lease)
                .await
                .expect("the lease starts when Cache reaches its safe endpoint")
                .expect("the operation grant remains available");
            assert_eq!(lease.mode(), OperationMode::NormalExport);
        });
    }

    #[test]
    fn dropping_the_lease_releases_gate_cache_and_processor_together() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the release fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let lease =
                OperationLease::acquire(&gate, &cache, &processor, OperationMode::BatchExclusive)
                    .await
                    .expect("the batch uses the common lease");

            assert!(
                OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport,)
                    .await
                    .is_err(),
                "another Export is refused without a queue"
            );
            let cache_work = cache.begin_work();
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
            OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport)
                .await
                .expect("the next Export acquires immediately");
        });
    }

    #[test]
    fn unwinding_an_attempt_does_not_leak_any_lease_resource() {
        let root = tempdir().expect("the unwind fixture exists");
        let paths =
            AppPaths::from_known_folders(&root.path().join("roaming"), &root.path().join("local"));
        let gate = OperationGate::new(&paths);
        let cache = CacheEngine::default();
        let processor = ImagingProcessor::default();

        let outcome = catch_unwind(AssertUnwindSafe(|| {
            tauri::async_runtime::block_on(async {
                let _lease =
                    OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport)
                        .await
                        .expect("the failed attempt acquires");
                panic!("injected operation failure");
            });
        }));
        assert!(outcome.is_err(), "the injected failure must unwind");

        tauri::async_runtime::block_on(async {
            OperationLease::acquire(&gate, &cache, &processor, OperationMode::NormalExport)
                .await
                .expect("the successor acquires every resource after unwind");
        });
    }

    #[test]
    fn cancelling_while_waiting_for_cache_releases_the_partial_gate_grant() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the cache-cancellation fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let active_cache = cache.begin_work().await;
            let mut pending = Box::pin(OperationLease::acquire(
                &gate,
                &cache,
                &processor,
                OperationMode::NormalExport,
            ));

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pending)
                    .await
                    .is_err(),
                "the attempt must be waiting after acquiring only the global gate"
            );
            drop(pending);

            gate.try_acquire(OperationMode::BatchExclusive)
                .expect("cancelling the pending future releases its partial gate grant");
            drop(active_cache);
        });
    }

    #[test]
    fn beginning_a_lease_resolves_the_global_conflict_before_waiting_for_cache() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the staged acquisition fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let active_cache = cache.begin_work().await;

            let acquisition = OperationLease::begin(&gate, OperationMode::NormalExport)
                .expect("the first caller resolves the gate immediately");
            assert!(
                OperationLease::begin(&gate, OperationMode::NormalExport).is_err(),
                "a concurrent caller receives a conflict before progress can start"
            );

            let mut completion = Box::pin(acquisition.complete(&cache, &processor));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut completion)
                    .await
                    .is_err(),
                "the acquisition waits for active Cache work only after owning the gate"
            );
            drop(completion);

            OperationLease::begin(&gate, OperationMode::NormalExport)
                .expect("cancelling the completion releases the partial gate grant");
            drop(active_cache);
        });
    }

    #[test]
    fn cancelling_while_waiting_for_processor_releases_gate_and_cache_pause() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the Processor-cancellation fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let occupied_processor = processor
                .reserve()
                .await
                .expect("the Processor can be reserved");
            let mut pending = Box::pin(OperationLease::acquire(
                &gate,
                &cache,
                &processor,
                OperationMode::NormalExport,
            ));

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pending)
                    .await
                    .is_err(),
                "the attempt must be waiting after acquiring the gate and Cache pause"
            );
            drop(pending);

            gate.try_acquire(OperationMode::CacheMaintenance)
                .expect("cancelling the pending future releases its gate grant");
            tokio::time::timeout(Duration::from_secs(1), cache.begin_work())
                .await
                .expect("cancelling the pending future releases its Cache pause");
            drop(occupied_processor);
        });
    }

    #[test]
    fn processor_quarantine_fails_the_lease_without_leaking_gate_or_cache_pause() {
        tauri::async_runtime::block_on(async {
            let root = tempdir().expect("the quarantine fixture exists");
            let paths = AppPaths::from_known_folders(
                &root.path().join("roaming"),
                &root.path().join("local"),
            );
            let gate = OperationGate::new(&paths);
            let cache = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let reservation = processor
                .reserve()
                .await
                .expect("the healthy Processor can be reserved");
            reservation.quarantine();
            drop(reservation);

            let failure = OperationLease::begin(&gate, OperationMode::NormalExport)
                .expect("the gate can be acquired")
                .complete(&cache, &processor)
                .await
                .expect_err("a quarantined Processor refuses a lease");

            assert_eq!(
                failure.to_string(),
                "o Processador de Imagens está em quarentena porque o encerramento anterior não foi confirmado; reinicie o aplicativo antes de tentar novamente"
            );
            assert!(
                OperationLease::begin(&gate, OperationMode::NormalExport).is_ok(),
                "the failed completion releases the gate"
            );
            tokio::time::timeout(Duration::from_secs(1), cache.begin_work())
                .await
                .expect("the failed completion releases the Cache pause");
        });
    }
}
