use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use myalbuns_imaging_protocol::{
    ImagingProgress, ImagingProgressStage, PhotoImportRequest, PhotoImportSourceId,
};

use crate::{image_processing::ImageProcessingBatch, ipc_contract::ImageProcessingProgress};

/// Attempt-local accounting for concurrent batches and their recovery retries.
/// Notifications carry preparation counts only; they never publish the catalog.
pub(super) struct NativeImportProgress<F: FnMut(ImageProcessingProgress)> {
    state: Mutex<ProgressState<F>>,
}

struct ProgressState<F: FnMut(ImageProcessingProgress)> {
    batch: ImageProcessingBatch<F>,
    requests: HashMap<String, (u32, u32)>,
    source_requests: HashMap<PhotoImportSourceId, String>,
    completed_sources: HashSet<PhotoImportSourceId>,
    unattributed: HashMap<String, u32>,
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
                source_requests: requests
                    .iter()
                    .flat_map(|request| {
                        request.candidates.iter().map(|candidate| {
                            (candidate.source_id.clone(), request.request_id.clone())
                        })
                    })
                    .collect(),
                completed_sources: HashSet::new(),
                unattributed: HashMap::new(),
                completed: 0,
            }),
        }
    }

    /// Native streams have drained. Attribute their counts to validated sources;
    /// a failed process can leave counts whose source identities were not returned.
    pub(super) fn begin_inspection<'a>(
        &self,
        validated: impl Iterator<Item = &'a PhotoImportSourceId>,
    ) {
        let mut state = self.state.lock().expect("import progress is available");
        let mut validated_counts = HashMap::<String, u32>::new();
        for source in validated {
            if state.completed_sources.insert(source.clone())
                && let Some(request) = state.source_requests.get(source)
            {
                *validated_counts.entry(request.clone()).or_default() += 1;
            }
        }
        for (request, (_, reported)) in state.requests.clone() {
            let validated = validated_counts.get(&request).copied().unwrap_or_default();
            state
                .unattributed
                .insert(request, reported.saturating_sub(validated));
            for _ in reported..validated {
                state.completed += 1;
                state.batch.complete(None);
            }
        }
    }

    pub(super) fn complete_inspection(&self, source: &PhotoImportSourceId) {
        let mut state = self.state.lock().expect("import progress is available");
        if !state.completed_sources.insert(source.clone()) {
            return;
        }
        if let Some(request) = state.source_requests.get(source).cloned()
            && let Some(remaining) = state.unattributed.get_mut(&request)
            && *remaining > 0
        {
            *remaining -= 1;
            return;
        }
        state.completed += 1;
        state.batch.complete(None);
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
        // Rejected selections without a source identity are terminal now.
        // Existing catalog entries retain their own Cache preparation counts.
        for _ in state.completed..new_sources_and_unsupported {
            state.batch.complete(None);
        }
        state.batch
    }

    pub(super) fn interrupt(self) -> ImageProcessingBatch<F> {
        self.state
            .into_inner()
            .expect("import progress is available")
            .batch
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
                source_requests: HashMap::new(),
                completed_sources: HashSet::new(),
                unattributed: HashMap::new(),
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

    #[test]
    fn interruption_keeps_unprocessed_sources_pending() {
        let mut observed = Vec::new();
        let batch = ImageProcessingBatch::new(5, |event| observed.push(event.completed_files));
        let progress = NativeImportProgress::new(batch, &[]);
        let source = PhotoImportSourceId::new("prepared").unwrap();
        progress.complete_inspection(&source);
        drop(progress.interrupt());
        assert_eq!(observed, [0, 1]);
    }

    #[test]
    fn alternate_inspections_count_each_source_once_after_partial_native_failure() {
        let ids = (0..5)
            .map(|index| PhotoImportSourceId::new(format!("source-{index}")).unwrap())
            .collect::<Vec<_>>();
        let observed = Mutex::new(Vec::new());
        let progress = NativeImportProgress {
            state: Mutex::new(ProgressState {
                batch: ImageProcessingBatch::new(5, |event| {
                    observed.lock().unwrap().push(event.completed_files)
                }),
                requests: HashMap::from([("completed".into(), (2, 0)), ("failed".into(), (2, 0))]),
                source_requests: HashMap::from([
                    (ids[0].clone(), "completed".into()),
                    (ids[1].clone(), "completed".into()),
                    (ids[2].clone(), "failed".into()),
                    (ids[3].clone(), "failed".into()),
                ]),
                completed_sources: HashSet::new(),
                unattributed: HashMap::new(),
                completed: 0,
            }),
        };
        progress.report(event("completed", 1, 2));
        progress.report(event("failed", 1, 2));
        progress.begin_inspection([&ids[0]].into_iter());
        progress.complete_inspection(&ids[0]); // changed after its native preparation
        progress.complete_inspection(&ids[1]); // known native fallback
        assert_eq!(*observed.lock().unwrap(), [0, 1, 2, 3]);
        progress.complete_inspection(&ids[2]); // already included in the failed stream
        assert_eq!(*observed.lock().unwrap(), [0, 1, 2, 3]);
        progress.complete_inspection(&ids[3]);
        progress.complete_inspection(&ids[3]);
        progress.complete_inspection(&ids[4]); // source without an admitted native request
        progress.finish(5);
        assert_eq!(*observed.lock().unwrap(), [0, 1, 2, 3, 4, 5]);
    }
}
