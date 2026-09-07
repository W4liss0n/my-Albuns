use std::{collections::HashMap, sync::Mutex};

use myalbuns_imaging_protocol::{ImagingProgress, ImagingProgressStage, PhotoImportRequest};

use crate::{image_processing::ImageProcessingBatch, ipc_contract::ImageProcessingProgress};

/// Attempt-local accounting for concurrent batches and their recovery retries.
/// Notifications carry preparation counts only; they never publish the catalog.
pub(super) struct NativeImportProgress<F: FnMut(ImageProcessingProgress)> {
    state: Mutex<ProgressState<F>>,
}

struct ProgressState<F: FnMut(ImageProcessingProgress)> {
    batch: ImageProcessingBatch<F>,
    requests: HashMap<String, (u32, u32)>,
    completed: u32,
}

impl<F: FnMut(ImageProcessingProgress)> NativeImportProgress<F> {
    pub(super) fn new(batch: ImageProcessingBatch<F>, requests: &[PhotoImportRequest]) -> Self {
        Self {
            state: Mutex::new(ProgressState {
                batch,
                requests: requests
                    .iter()
                    .map(|request| {
                        (
                            request.request_id.clone(),
                            (request.candidates.len() as u32, 0),
                        )
                    })
                    .collect(),
                completed: 0,
            }),
        }
    }

    pub(super) fn report(&self, event: ImagingProgress) {
        if event.stage != ImagingProgressStage::PreparingPhotos {
            return;
        }
        let mut state = self.state.lock().expect("import progress is available");
        let Some((total, previous)) = state.requests.get_mut(&event.request_id) else {
            return;
        };
        if event.total_units != *total || event.completed_units > *total {
            return;
        }
        // A retry starts its own stream at zero. Keep the attempt monotonic and
        // count only the portion that exceeds this batch's prior high-water mark.
        let newly_prepared = event.completed_units.saturating_sub(*previous);
        *previous = (*previous).max(event.completed_units);
        for _ in 0..newly_prepared {
            state.completed += 1;
            state.batch.complete(None);
        }
    }

    pub(super) fn finish(self, new_sources_and_unsupported: u32) -> ImageProcessingBatch<F> {
        let mut state = self
            .state
            .into_inner()
            .expect("import progress is available");
        // Alternate inspection and rejected selections become terminal only
        // after the Host finishes them. Existing catalog entries are counted by
        // their own Cache preparation after this native phase.
        for _ in state.completed..new_sources_and_unsupported {
            state.batch.complete(None);
        }
        state.batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(request: &str, completed: u32, total: u32) -> ImagingProgress {
        ImagingProgress::new(
            request,
            ImagingProgressStage::PreparingPhotos,
            completed,
            total,
        )
        .unwrap()
    }

    #[test]
    fn concurrent_batches_and_retries_advance_once_and_leave_fallbacks_pending() {
        let mut observed = Vec::new();
        let batch =
            ImageProcessingBatch::new(6, |progress| observed.push(progress.completed_files));
        let progress = NativeImportProgress {
            state: Mutex::new(ProgressState {
                batch,
                requests: HashMap::from([("first".into(), (3, 0)), ("second".into(), (2, 0))]),
                completed: 0,
            }),
        };
        std::thread::scope(|scope| {
            scope.spawn(|| {
                progress.report(event("first", 1, 3));
                progress.report(event("first", 0, 3));
                progress.report(event("first", 1, 3));
                progress.report(event("first", 2, 3));
            });
            scope.spawn(|| {
                progress.report(event("second", 1, 2));
                progress.report(event("second", 2, 2));
                progress.report(event("unknown", 1, 1));
                progress.report(event("first", 3, 4));
            });
        });
        assert_eq!(progress.state.lock().unwrap().completed, 4);
        let mut batch = progress.finish(5);
        batch.complete(None); // one existing photo is prepared separately
        assert_eq!(observed, vec![0, 1, 2, 3, 4, 5, 6]);
    }
}
