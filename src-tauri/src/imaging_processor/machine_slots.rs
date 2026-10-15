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
    /// The slots of work for the window in use: one core stays free for it.
    foreground: Vec<NamedMutex>,
    /// Every slot, the one kept free for the window in use last, for work
    /// below normal priority.
    background: Vec<NamedMutex>,
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
    /// `foreground` slots are shared by work for the window in use;
    /// `background` slots, which include them, by work below normal priority.
    pub(super) fn new(app_paths: &AppPaths, foreground: usize, background: usize) -> Self {
        let background = background.max(foreground).max(1);
        let slot = |index: usize| {
            NamedMutex::scoped(
                app_paths,
                "ImagingWorkerSlot",
                &index.to_string(),
                "myalbuns-imaging-slot",
            )
        };
        // A wait takes the first free slot of its list, so background work
        // tries the extra slots before the ones the window in use may need.
        let foreground_slots = (0..foreground.clamp(1, background)).map(slot).collect();
        let background_slots = (0..background).rev().map(slot).collect();
        let low_memory = NamedMutex::scoped(
            app_paths,
            "ImagingLowMemorySlot",
            "global",
            "myalbuns-imaging-low-memory",
        );
        Self {
            foreground: foreground_slots,
            background: background_slots,
            low_memory,
        }
    }

    /// Waits for one worker slot and, for a job that local admission let in
    /// only in low-memory mode, first for the single low-memory slot. Every
    /// process takes them in this order, so two waiters cannot block each other.
    /// A slot that cannot be observed is skipped: slots only spread load, and
    /// failing to observe them must not fail the image. `background` work may
    /// also take the slot kept free for the window in use.
    pub(super) async fn acquire(
        &self,
        low_memory: bool,
        background: bool,
        mut stop: impl FnMut() -> Option<SlotWaitEnd>,
    ) -> Result<MachineSlotGrant, SlotWaitEnd> {
        let started = Instant::now();
        let low_memory_grant = if low_memory {
            wait_for_any(std::slice::from_ref(&self.low_memory), &mut stop).await?
        } else {
            None
        };
        let workers = if background {
            &self.background
        } else {
            &self.foreground
        };
        let worker = wait_for_any(workers, &mut stop).await?;
        let waited = started.elapsed();
        if waited >= STOP_POLL {
            tracing::info!(
                target: "myalbuns.desktop",
                event = "imaging_machine_slot_acquired",
                waited_ms = waited.as_millis() as u64,
                low_memory,
                background,
            );
        }
        Ok(MachineSlotGrant {
            _worker: worker,
            _low_memory: low_memory_grant,
        })
    }
}

/// Projects opened together prepare their opening images one after the
/// other: each one finishes sooner than if they shared the slots, and the
/// first editor no longer waits for the photos of the others. A Host wants the
/// turn while its opening preparation runs, takes it before its first sidecar
/// and keeps it until that preparation ends; an opening whose previews are all
/// in the Cache starts no sidecar and never waits.
#[derive(Debug, Default)]
pub(crate) struct OpeningTurn {
    state: std::sync::Mutex<TurnState>,
    /// Lets one sidecar of this Host wait for the turn while the others wait
    /// for it to be taken.
    taking: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
enum TurnState {
    #[default]
    NotWanted,
    Wanted,
    /// `None` when the turn could not be observed: it only orders openings,
    /// so the preparation goes on without it.
    Held {
        _grant: Option<NamedMutexGrant>,
    },
}

/// Ends the wish for the turn and releases it.
#[must_use]
pub(crate) struct OpeningTurnRequest<'a>(&'a OpeningTurn);

impl Drop for OpeningTurnRequest<'_> {
    fn drop(&mut self) {
        *self.0.state() = TurnState::NotWanted;
    }
}

impl OpeningTurn {
    pub(crate) fn want(&self) -> OpeningTurnRequest<'_> {
        *self.state() = TurnState::Wanted;
        OpeningTurnRequest(self)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, TurnState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn is_wanted(&self) -> bool {
        matches!(*self.state(), TurnState::Wanted)
    }

    /// Returns at once unless the turn is wanted and not yet held.
    pub(super) async fn take(
        &self,
        app_paths: &AppPaths,
        mut stop: impl FnMut() -> Option<SlotWaitEnd>,
    ) -> Result<(), SlotWaitEnd> {
        if !self.is_wanted() {
            return Ok(());
        }
        let _taking = loop {
            if let Ok(taking) = tokio::time::timeout(STOP_POLL, self.taking.lock()).await {
                break taking;
            }
            if let Some(end) = stop() {
                return Err(end);
            }
        };
        if !self.is_wanted() {
            return Ok(());
        }
        let started = Instant::now();
        let turn = NamedMutex::scoped(
            app_paths,
            "ImagingOpeningTurn",
            "global",
            "myalbuns-imaging-opening-turn",
        );
        let grant = wait_for_any(std::slice::from_ref(&turn), &mut stop).await?;
        tracing::info!(
            target: "myalbuns.desktop",
            event = "imaging_opening_turn_acquired",
            waited_ms = started.elapsed().as_millis() as u64,
        );
        let mut state = self.state();
        // The preparation may have ended during the wait; the grant then
        // drops here.
        if matches!(*state, TurnState::Wanted) {
            *state = TurnState::Held { _grant: grant };
        }
        Ok(())
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

    use super::{MachineSlots, OpeningTurn, SlotWaitEnd};

    fn slots(root: &std::path::Path, capacity: usize) -> MachineSlots {
        slots_with(root, capacity, capacity)
    }

    fn slots_with(root: &std::path::Path, foreground: usize, background: usize) -> MachineSlots {
        MachineSlots::new(
            &AppPaths::from_roots(&root.join("roaming"), &root.join("local")),
            foreground,
            background,
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
            let first = first_process.acquire(false, false, never).await.unwrap();
            let _second = second_process.acquire(false, false, never).await.unwrap();
            let mut third = Box::pin(third_process.acquire(false, false, never));
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
            let _held = holder.acquire(false, false, never).await.unwrap();
            let flag = Arc::clone(&cancelled);
            let canceller = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                flag.store(true, Ordering::Release);
            });
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                waiter.acquire(false, false, || {
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
        let _other_process = pool.foreground[0].abandon_in_exited_thread();
        tauri::async_runtime::block_on(async {
            let grant =
                tokio::time::timeout(Duration::from_secs(5), pool.acquire(false, false, never))
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
    fn background_work_also_takes_the_slot_kept_for_the_window_in_use() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        // Two cores: one slot for the window in use, two for background work.
        let opening = slots_with(root.path(), 1, 2);
        let editor = slots_with(root.path(), 1, 2);
        tauri::async_runtime::block_on(async {
            let first = opening.acquire(false, true, never).await.unwrap();
            let in_use =
                tokio::time::timeout(Duration::from_secs(5), editor.acquire(false, false, never))
                    .await
                    .expect("background work took the extra slot first")
                    .unwrap();
            let mut second = Box::pin(opening.acquire(false, true, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut second)
                    .await
                    .is_err(),
                "background work waits once every slot is held"
            );
            drop(first);
            let second = tokio::time::timeout(Duration::from_secs(5), second)
                .await
                .expect("the released slot passes to background work")
                .unwrap();
            let mut more = Box::pin(editor.acquire(false, false, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut more)
                    .await
                    .is_err(),
                "work for the window in use never takes more than its slot"
            );
            drop(in_use);
            tokio::time::timeout(Duration::from_secs(5), more)
                .await
                .expect("the window in use gets its slot back")
                .unwrap();
            drop(second);
        });
    }

    #[test]
    fn projects_opened_together_prepare_their_images_one_after_the_other() {
        let root = tempfile::tempdir().expect("temporary turn fixture");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        // Each instance stands for the Host of a different Project.
        let first_host = OpeningTurn::default();
        let second_host = OpeningTurn::default();
        let opened_earlier = OpeningTurn::default();
        tauri::async_runtime::block_on(async {
            let first_request = first_host.want();
            first_host.take(&paths, never).await.unwrap();
            // The other sidecars of the same opening do not wait again.
            tokio::time::timeout(Duration::from_secs(5), first_host.take(&paths, never))
                .await
                .expect("the Host that holds the turn keeps it for every sidecar")
                .unwrap();
            let second_request = second_host.want();
            let mut second = Box::pin(second_host.take(&paths, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut second)
                    .await
                    .is_err(),
                "a second opening waits while the first prepares its images"
            );
            tokio::time::timeout(Duration::from_secs(5), opened_earlier.take(&paths, never))
                .await
                .expect("work after an opening does not wait for the turn")
                .unwrap();
            drop(first_request);
            tokio::time::timeout(Duration::from_secs(5), second)
                .await
                .expect("the turn passes once the first preparation ends")
                .unwrap();
            drop(second_request);
        });
    }

    #[test]
    fn cancellation_ends_a_wait_for_the_turn() {
        let root = tempfile::tempdir().expect("temporary turn fixture");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let holder = OpeningTurn::default();
        let waiter = OpeningTurn::default();
        tauri::async_runtime::block_on(async {
            let _held = holder.want();
            holder.take(&paths, never).await.unwrap();
            let _wanted = waiter.want();
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                waiter.take(&paths, || Some(SlotWaitEnd::Cancelled)),
            )
            .await
            .expect("cancellation ends the wait");
            assert_eq!(result.unwrap_err(), SlotWaitEnd::Cancelled);
        });
    }

    #[test]
    fn low_memory_jobs_run_one_at_a_time_across_processes() {
        let root = tempfile::tempdir().expect("temporary slot fixture");
        let first_process = slots(root.path(), 8);
        let second_process = slots(root.path(), 8);
        tauri::async_runtime::block_on(async {
            let serial = first_process.acquire(true, false, never).await.unwrap();
            let mut next_serial = Box::pin(second_process.acquire(true, false, never));
            assert!(
                tokio::time::timeout(Duration::from_millis(150), &mut next_serial)
                    .await
                    .is_err(),
                "a second low-memory job waits for the first"
            );
            let parallel = tokio::time::timeout(
                Duration::from_secs(5),
                second_process.acquire(false, false, never),
            )
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
