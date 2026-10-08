//! Machine-wide limits shared by every process that starts an Imaging sidecar:
//! Project Hosts, the Global's batch work and the command-line export. Each
//! process keeps its own admission (permits and memory budget); these slots
//! only stop several processes from multiplying that admission.
//!
//! Slots are named mutexes, not a Win32 semaphore: Windows releases a mutex
//! whose holder dies, so a crashed process never leaks a slot.
use std::time::{Duration, Instant};

use myalbuns_paths::AppPaths;

use crate::named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant};

/// How often the waiting side checks cancellation. The wait itself runs on
/// one worker thread per acquisition that keeps its handles open throughout.
const STOP_POLL: Duration = Duration::from_millis(25);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SlotWaitEnd {
    Cancelled,
    Unavailable,
}

pub(super) struct MachineSlots {
    workers: Vec<NamedMutex>,
    low_memory: NamedMutex,
}

/// Held for as long as the sidecar may run. Fields drop in declaration order,
/// so the worker slot is released before the low-memory slot.
#[derive(Debug)]
pub(super) struct MachineSlotGrant {
    _worker: Option<NamedMutexGrant>,
    _low_memory: Option<NamedMutexGrant>,
}

impl MachineSlots {
    pub(super) fn new(app_paths: &AppPaths, capacity: usize) -> Self {
        let workers = (0..capacity.max(1))
            .map(|index| {
                NamedMutex::scoped(
                    app_paths,
                    "ImagingWorkerSlot",
                    &index.to_string(),
                    "myalbuns-imaging-slot",
                )
            })
            .collect();
        let low_memory = NamedMutex::scoped(
            app_paths,
            "ImagingLowMemorySlot",
            "global",
            "myalbuns-imaging-low-memory",
        );
        Self {
            workers,
            low_memory,
        }
    }

    /// Waits for one worker slot and, for a job that local admission let in
    /// only in low-memory mode, first for the single low-memory slot. Every
    /// process takes them in this order, so two waiters cannot block each other.
    /// A slot that cannot be observed is skipped: slots only spread load, and
    /// failing to observe them must not fail the image.
    pub(super) async fn acquire(
        &self,
        low_memory: bool,
        mut stop: impl FnMut() -> Option<SlotWaitEnd>,
    ) -> Result<MachineSlotGrant, SlotWaitEnd> {
        let started = Instant::now();
        let low_memory_grant = if low_memory {
            wait_for_any(std::slice::from_ref(&self.low_memory), &mut stop).await?
        } else {
            None
        };
        let worker = wait_for_any(&self.workers, &mut stop).await?;
        let waited = started.elapsed();
        if waited >= STOP_POLL {
            tracing::info!(
                target: "myalbuns.desktop",
                event = "imaging_machine_slot_acquired",
                waited_ms = waited.as_millis() as u64,
                low_memory,
            );
        }
        Ok(MachineSlotGrant {
            _worker: worker,
            _low_memory: low_memory_grant,
        })
    }
}

/// One waiting worker opens the candidate slots once and waits on all of them
/// (`NamedMutex::wait_for_any`); this side never blocks the executor and only
/// checks `stop` between bounded awaits. Returning early drops the pending
/// acquisition, which stops the worker.
async fn wait_for_any(
    slots: &[NamedMutex],
    stop: &mut impl FnMut() -> Option<SlotWaitEnd>,
) -> Result<Option<NamedMutexGrant>, SlotWaitEnd> {
    if let Some(end) = stop() {
        return Err(end);
    }
    let unavailable = |reason: String| {
        tracing::warn!(
            target: "myalbuns.desktop",
            event = "imaging_machine_slot_unavailable",
            reason = %reason,
        );
        Ok(None)
    };
    let mut pending = match NamedMutex::wait_for_any(slots) {
        Ok(pending) => pending,
        Err(NamedMutexError::Unavailable(reason)) => return unavailable(reason),
        Err(NamedMutexError::Conflict) => return unavailable("conflict".into()),
    };
    loop {
        match tokio::time::timeout(STOP_POLL, pending.granted()).await {
            Ok(Ok(grant)) => return Ok(Some(grant)),
            Ok(Err(NamedMutexError::Unavailable(reason))) => return unavailable(reason),
            // Only a stopped wait reports a conflict, and this side has not
            // stopped it.
            Ok(Err(NamedMutexError::Conflict)) => return unavailable("conflict".into()),
            Err(_still_waiting) => {}
        }
        if let Some(end) = stop() {
            return Err(end);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use myalbuns_paths::AppPaths;

    use super::{MachineSlots, SlotWaitEnd};

    fn slots(root: &std::path::Path, capacity: usize) -> MachineSlots {
        MachineSlots::new(
            &AppPaths::from_roots(&root.join("roaming"), &root.join("local")),
            capacity,
        )
    }

    fn never() -> Option<SlotWaitEnd> {
        None
    }

    #[test]
    fn exhausted_slots_block_a_third_process_until_one_is_released() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        // Each instance stands for a different process: they share only names.
        let first_process = slots(root.path(), 2);
        let second_process = slots(root.path(), 2);
        let third_process = slots(root.path(), 2);
        tauri::async_runtime::block_on(async {
            let first = first_process.acquire(false, never).await.unwrap();
            let _second = second_process.acquire(false, never).await.unwrap();
            let mut third = Box::pin(third_process.acquire(false, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut third)
                    .await
                    .is_err(),
                "a third sidecar waits while every machine slot is held"
            );
            drop(first);
            tokio::time::timeout(Duration::from_secs(5), third)
                .await
                .expect("the released slot is taken by the waiter")
                .expect("the waiter was not stopped");
        });
    }

    #[test]
    fn cancellation_ends_a_slot_wait() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        let holder = slots(root.path(), 1);
        let waiter = slots(root.path(), 1);
        let cancelled = Arc::new(AtomicBool::new(false));
        tauri::async_runtime::block_on(async {
            let _held = holder.acquire(false, never).await.unwrap();
            let flag = Arc::clone(&cancelled);
            let canceller = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                flag.store(true, Ordering::Release);
            });
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                waiter.acquire(false, || {
                    cancelled
                        .load(Ordering::Acquire)
                        .then_some(SlotWaitEnd::Cancelled)
                }),
            )
            .await
            .expect("cancellation ends the wait");
            assert_eq!(result.unwrap_err(), SlotWaitEnd::Cancelled);
            canceller.join().unwrap();
        });
    }

    #[test]
    fn a_slot_whose_holder_ended_without_releasing_is_taken_again() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        let pool = slots(root.path(), 1);
        // The handle keeps the abandoned mutex alive, as the other processes
        // that opened a slot do.
        let _other_process = pool.workers[0].abandon_in_exited_thread();
        tauri::async_runtime::block_on(async {
            let grant = tokio::time::timeout(Duration::from_secs(5), pool.acquire(false, never))
                .await
                .expect("an abandoned slot does not stay taken")
                .expect("the slot is acquired");
            assert!(
                grant
                    ._worker
                    .as_ref()
                    .expect("the slot was observed")
                    .was_abandoned(),
                "the waiter received the slot as abandoned"
            );
        });
    }

    #[test]
    fn low_memory_jobs_run_one_at_a_time_across_processes() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        let first_process = slots(root.path(), 8);
        let second_process = slots(root.path(), 8);
        tauri::async_runtime::block_on(async {
            let serial = first_process.acquire(true, never).await.unwrap();
            let mut next_serial = Box::pin(second_process.acquire(true, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut next_serial)
                    .await
                    .is_err(),
                "a second low-memory job waits for the first"
            );
            let parallel =
                tokio::time::timeout(Duration::from_secs(5), second_process.acquire(false, never))
                    .await
                    .expect("a job admitted with headroom does not wait for low memory")
                    .unwrap();
            drop(serial);
            tokio::time::timeout(Duration::from_secs(5), next_serial)
                .await
                .expect("the low-memory slot passes to the next job")
                .unwrap();
            drop(parallel);
        });
    }
}
