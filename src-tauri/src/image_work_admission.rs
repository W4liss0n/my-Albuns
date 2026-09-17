use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason, CacheWorkPermit},
    cache_engine::CacheEngine,
    imaging_processor::{
        ImageMemoryEstimate, ImagingProcessor, ProcessorAdmissionFailure, ProcessorReservation,
    },
};

pub(crate) enum ImageWorkKind {
    Cache,
    Inspection,
}

/// Holds causal activity while the caller observes a source or adopts an existing inspection.
pub(crate) struct ImageWorkAdmission {
    activity: CacheWorkPermit,
    cancellation: CacheCancellation,
}

/// Keep this lease until started work has drained. Fields release capacity before activity.
pub(crate) struct ImageWorkLease {
    reservation: ProcessorReservation,
    _activity: CacheWorkPermit,
}

impl ImageWorkAdmission {
    pub(crate) async fn begin(
        engine: &CacheEngine,
        cancellation: &CacheCancellation,
    ) -> Result<Self, ProcessorAdmissionFailure> {
        loop {
            match cancellation.reason() {
                Some(CacheCancellationReason::Obsolete) => {
                    return Err(ProcessorAdmissionFailure::Cancelled);
                }
                Some(CacheCancellationReason::Paused) if !cancellation.resume_after_pause() => {
                    return Err(ProcessorAdmissionFailure::Cancelled);
                }
                _ => {}
            }
            let activity = engine.begin_cancellable_work(cancellation.clone()).await;
            if cancellation.reason().is_some() {
                drop(activity);
                continue;
            }
            return Ok(Self {
                activity,
                cancellation: cancellation.clone(),
            });
        }
    }

    pub(crate) async fn reserve(
        self,
        processor: &ImagingProcessor,
        estimate: ImageMemoryEstimate,
        kind: ImageWorkKind,
    ) -> Result<ImageWorkLease, ProcessorAdmissionFailure> {
        let reservation = match kind {
            ImageWorkKind::Cache => {
                processor
                    .reserve_cache_for(estimate, self.cancellation.flag())
                    .await?
            }
            ImageWorkKind::Inspection => {
                processor
                    .reserve_inspection(estimate, self.cancellation.flag())
                    .await?
            }
        };
        if self
            .cancellation
            .flag()
            .load(std::sync::atomic::Ordering::Acquire)
        {
            drop(reservation);
            return Err(ProcessorAdmissionFailure::Cancelled);
        }
        Ok(ImageWorkLease {
            reservation,
            _activity: self.activity,
        })
    }
}

impl ImageWorkLease {
    pub(crate) fn reservation(&self) -> &ProcessorReservation {
        &self.reservation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn obsolete_work_releases_capacity_wait_and_cannot_resume() {
        tauri::async_runtime::block_on(async {
            let engine = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let occupied = processor.reserve().await.unwrap();
            let cancellation = CacheCancellation::default();
            let admission = ImageWorkAdmission::begin(&engine, &cancellation)
                .await
                .unwrap();
            let estimate = ImageMemoryEstimate::in_plan(
                &myalbuns_paths::OperationPathContext::new().freeze(),
                [],
            );
            let pending = admission.reserve(&processor, estimate, ImageWorkKind::Cache);
            tokio::pin!(pending);
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pending)
                    .await
                    .is_err()
            );
            cancellation.cancel_obsolete();
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(5), pending)
                    .await
                    .unwrap(),
                Err(ProcessorAdmissionFailure::Cancelled)
            ));
            assert!(matches!(
                ImageWorkAdmission::begin(&engine, &cancellation).await,
                Err(ProcessorAdmissionFailure::Cancelled)
            ));
            let _pause = tokio::time::timeout(Duration::from_secs(5), engine.pause())
                .await
                .unwrap();
            drop(occupied);
        });
    }

    #[test]
    fn started_cache_and_inspection_hold_activity_until_the_joint_lease_drains() {
        tauri::async_runtime::block_on(async {
            for kind in [ImageWorkKind::Cache, ImageWorkKind::Inspection] {
                let engine = CacheEngine::default();
                let processor = ImagingProcessor::default();
                let cancellation = CacheCancellation::default();
                let estimate = ImageMemoryEstimate::in_plan(
                    &myalbuns_paths::OperationPathContext::new().freeze(),
                    [],
                );
                let lease = ImageWorkAdmission::begin(&engine, &cancellation)
                    .await
                    .unwrap()
                    .reserve(&processor, estimate, kind)
                    .await
                    .unwrap();
                let pause = engine.pause();
                tokio::pin!(pause);
                assert!(
                    tokio::time::timeout(Duration::from_millis(20), &mut pause)
                        .await
                        .is_err()
                );
                assert_eq!(cancellation.reason(), Some(CacheCancellationReason::Paused));
                drop(lease);
                let paused = tokio::time::timeout(Duration::from_secs(5), pause)
                    .await
                    .unwrap();
                let resumed = ImageWorkAdmission::begin(&engine, &cancellation);
                tokio::pin!(resumed);
                assert!(
                    tokio::time::timeout(Duration::from_millis(20), &mut resumed)
                        .await
                        .is_err()
                );
                drop(paused);
                let admitted = tokio::time::timeout(Duration::from_secs(5), resumed)
                    .await
                    .unwrap()
                    .unwrap();
                drop(admitted);
                let _capacity = tokio::time::timeout(Duration::from_secs(5), processor.reserve())
                    .await
                    .unwrap()
                    .unwrap();
            }
        });
    }
}
