use crate::{
    cache_engine::{CacheEngine, CachePause},
    imaging_processor::{ImagingProcessor, ProcessorReservation},
    operation_gate::{OperationGate, OperationGateError, OperationGrant, OperationMode},
};

#[derive(Debug)]
pub(crate) struct OperationLease {
    processor_reservation: Option<ProcessorReservation>,
    cache_pause: Option<CachePause>,
    gate_grant: Option<OperationGrant>,
}

impl OperationLease {
    pub(crate) async fn acquire(
        gate: &OperationGate,
        cache: &CacheEngine,
        processor: &ImagingProcessor,
        mode: OperationMode,
    ) -> Result<Self, OperationGateError> {
        let gate_grant = gate.try_acquire(mode)?;
        let cache_pause = cache.pause().await;
        let processor_reservation = processor.reserve().await;
        Ok(Self {
            processor_reservation: Some(processor_reservation),
            cache_pause: Some(cache_pause),
            gate_grant: Some(gate_grant),
        })
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
                .expect("the Processor returns with the lease release");
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
            let occupied_processor = processor.reserve().await;
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
}
