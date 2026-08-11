use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    task::Poll,
};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

const ACTIVE: u8 = 0;
const PAUSED: u8 = 1;
const OBSOLETE: u8 = 2;

#[derive(Clone, Debug)]
pub(crate) struct CacheCancellation {
    inner: Arc<CacheCancellationInner>,
}

#[derive(Debug)]
struct CacheCancellationInner {
    requested: AtomicBool,
    reason: AtomicU8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheCancellationReason {
    Paused,
    Obsolete,
}

impl Default for CacheCancellation {
    fn default() -> Self {
        Self {
            inner: Arc::new(CacheCancellationInner {
                requested: AtomicBool::new(false),
                reason: AtomicU8::new(ACTIVE),
            }),
        }
    }
}

impl CacheCancellation {
    pub(crate) fn flag(&self) -> &AtomicBool {
        &self.inner.requested
    }

    pub(crate) fn reason(&self) -> Option<CacheCancellationReason> {
        match self.inner.reason.load(Ordering::Acquire) {
            PAUSED => Some(CacheCancellationReason::Paused),
            OBSOLETE => Some(CacheCancellationReason::Obsolete),
            _ => None,
        }
    }

    pub(crate) fn cancel_obsolete(&self) {
        self.inner.reason.store(OBSOLETE, Ordering::Release);
        self.inner.requested.store(true, Ordering::Release);
    }

    pub(crate) fn resume_after_pause(&self) -> bool {
        self.inner.requested.store(false, Ordering::Release);
        if self
            .inner
            .reason
            .compare_exchange(PAUSED, ACTIVE, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            self.inner.requested.store(true, Ordering::Release);
            return false;
        }
        true
    }

    fn cancel_for_pause(&self) {
        let _ =
            self.inner
                .reason
                .compare_exchange(ACTIVE, PAUSED, Ordering::AcqRel, Ordering::Acquire);
        self.inner.requested.store(true, Ordering::Release);
    }
}

#[derive(Debug)]
pub(crate) struct CacheActivityGate {
    activity: Arc<RwLock<()>>,
    active: Arc<Mutex<HashMap<u64, Weak<CacheCancellationInner>>>>,
    next_id: AtomicU64,
}

impl Default for CacheActivityGate {
    fn default() -> Self {
        Self {
            activity: Arc::new(RwLock::new(())),
            active: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CacheWorkPermit {
    id: u64,
    active: Arc<Mutex<HashMap<u64, Weak<CacheCancellationInner>>>>,
    _guard: OwnedRwLockReadGuard<()>,
}

#[derive(Debug)]
pub(crate) struct CachePause {
    _guard: OwnedRwLockWriteGuard<()>,
}

impl CacheActivityGate {
    pub(crate) async fn begin_cancellable_work(
        &self,
        cancellation: CacheCancellation,
    ) -> CacheWorkPermit {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, Arc::downgrade(&cancellation.inner));
        let guard = self.activity.clone().read_owned().await;
        CacheWorkPermit {
            id,
            active: Arc::clone(&self.active),
            _guard: guard,
        }
    }

    pub(crate) async fn pause(&self) -> CachePause {
        let mut exclusive = Box::pin(self.activity.clone().write_owned());
        let acquired = std::future::poll_fn(|context| {
            Poll::Ready(match exclusive.as_mut().poll(context) {
                Poll::Ready(guard) => Some(guard),
                Poll::Pending => None,
            })
        })
        .await;

        // The first poll either acquired exclusivity or queued this writer. Tokio's
        // documented FIFO, write-preferring RwLock then prevents later readers from
        // entering while the active snapshot is cancelled at its safe endpoint.
        {
            let mut active = self
                .active
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            active.retain(|_, cancellation| {
                let Some(cancellation) = cancellation.upgrade() else {
                    return false;
                };
                CacheCancellation {
                    inner: cancellation,
                }
                .cancel_for_pause();
                true
            });
        }
        CachePause {
            _guard: match acquired {
                Some(guard) => guard,
                None => exclusive.await,
            },
        }
    }
}

impl Drop for CacheWorkPermit {
    fn drop(&mut self) {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CacheActivityGate, CacheCancellation, CacheCancellationReason};

    #[test]
    fn pause_cancels_active_cache_before_waiting_for_its_safe_endpoint() {
        tauri::async_runtime::block_on(async {
            let gate = CacheActivityGate::default();
            let cancellation = CacheCancellation::default();
            let active = gate.begin_cancellable_work(cancellation.clone()).await;
            let mut pause = Box::pin(gate.pause());

            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pause)
                    .await
                    .is_err(),
                "pause waits for active work to release its read permit"
            );
            assert!(
                cancellation
                    .flag()
                    .load(std::sync::atomic::Ordering::Acquire)
            );
            assert_eq!(cancellation.reason(), Some(CacheCancellationReason::Paused));

            drop(active);
            tokio::time::timeout(Duration::from_secs(1), &mut pause)
                .await
                .expect("pause acquires after the cancelled work exits");
        });
    }

    #[test]
    fn an_obsolete_demand_is_not_relabelled_as_a_pause() {
        tauri::async_runtime::block_on(async {
            let gate = CacheActivityGate::default();
            let cancellation = CacheCancellation::default();
            let active = gate.begin_cancellable_work(cancellation.clone()).await;
            cancellation.cancel_obsolete();
            let pause = gate.pause();
            tokio::pin!(pause);
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut pause)
                    .await
                    .is_err()
            );
            assert_eq!(
                cancellation.reason(),
                Some(CacheCancellationReason::Obsolete)
            );
            drop(active);
        });
    }

    #[test]
    fn work_started_after_pause_intent_waits_until_the_pause_is_released() {
        tauri::async_runtime::block_on(async {
            let gate = std::sync::Arc::new(CacheActivityGate::default());
            let active_cancellation = CacheCancellation::default();
            let active = gate
                .begin_cancellable_work(active_cancellation.clone())
                .await;
            let pause_gate = std::sync::Arc::clone(&gate);
            let (pause_acquired_tx, pause_acquired_rx) = tokio::sync::oneshot::channel();
            let (release_pause_tx, release_pause_rx) = tokio::sync::oneshot::channel();
            let pause_task = tokio::spawn(async move {
                let pause = pause_gate.pause().await;
                pause_acquired_tx
                    .send(())
                    .expect("the test observes the exclusive pause");
                release_pause_rx
                    .await
                    .expect("the test releases the exclusive pause");
                drop(pause);
            });

            tokio::time::timeout(Duration::from_secs(1), async {
                while !active_cancellation
                    .flag()
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("pause intent causally cancels the already-active work");

            let waiting_gate = std::sync::Arc::clone(&gate);
            let waiting_cancellation = CacheCancellation::default();
            let waiting_cancellation_for_task = waiting_cancellation.clone();
            let mut waiting_work = tokio::spawn(async move {
                waiting_gate
                    .begin_cancellable_work(waiting_cancellation_for_task)
                    .await
            });
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut waiting_work)
                    .await
                    .is_err(),
                "work registered after pause intent cannot enter before exclusivity"
            );

            drop(active);
            pause_acquired_rx
                .await
                .expect("pause reaches its exclusive endpoint");
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut waiting_work)
                    .await
                    .is_err(),
                "new Cache work remains blocked for the whole exclusive pause"
            );
            release_pause_tx
                .send(())
                .expect("the exclusive pause is released");
            let waiting_permit = tokio::time::timeout(Duration::from_secs(1), &mut waiting_work)
                .await
                .expect("new Cache work resumes after the pause")
                .expect("the waiting Cache task remains alive");
            assert_eq!(waiting_cancellation.reason(), None);
            drop(waiting_permit);
            pause_task.await.expect("the pause task completes");
        });
    }
}
