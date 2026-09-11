//! Admission estimates belong to the Processor, independently of the caller.
//! Codec limits remain authoritative; this budget preserves system headroom.
use std::{
    io::BufReader,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use myalbuns_paths::{ExpectedObject, RootBindingPlan};
use tokio::sync::{Notify, Semaphore};

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const RESOURCE_REFRESH: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageMemoryEstimate(u64);

impl ImageMemoryEstimate {
    pub(crate) fn in_plan<'a>(
        plan: &RootBindingPlan,
        paths: impl IntoIterator<Item = &'a Path>,
    ) -> Self {
        Self(
            paths
                .into_iter()
                .map(|path| estimate_source(plan, path))
                .max()
                .unwrap_or(64 * MIB),
        )
    }
}

fn estimate_source(plan: &RootBindingPlan, path: &Path) -> u64 {
    let inspected = (|| {
        let source = plan
            .resolve_existing(path, ExpectedObject::RegularFile)
            .ok()?;
        let file = source.reopen_for_read().ok()?;
        let compressed = file.metadata().ok()?.len();
        let reader = image::ImageReader::new(BufReader::new(file))
            .with_guessed_format()
            .ok()?;
        let (width, height) = reader.into_dimensions().ok()?;
        let pixels = u64::from(width).checked_mul(u64::from(height))?;
        // Includes concurrent decoder, conversion/orientation and reduced-image
        // buffers. It is an admission estimate, not a replacement for codec limits.
        pixels
            .checked_mul(16)?
            .checked_add(compressed.saturating_mul(2))?
            .checked_add(64 * MIB)
    })();
    inspected.unwrap_or(GIB)
}

#[derive(Clone, Copy, Debug)]
struct Resources {
    physical_total: u64,
    physical_available: u64,
    commit_available: u64,
}

impl Resources {
    fn ceiling(self) -> u64 {
        (self.physical_total / 4).min(4 * GIB)
    }

    fn available(self) -> u64 {
        let headroom = (self.physical_total / 8).clamp(512 * MIB, 2 * GIB);
        self.ceiling()
            .min(self.physical_available.saturating_sub(headroom) / 2)
            .min(self.commit_available.saturating_sub(headroom) / 2)
    }

    fn serial_available(self) -> u64 {
        // Windows can back committed memory with RAM or the paging file.
        // Resident headroom governs concurrency, not whether one job may run.
        self.ceiling().min(self.commit_available)
    }
}

#[cfg(windows)]
fn system_resources() -> Option<Resources> {
    use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    // SAFETY: this stack-owned structure and its byte size match the Win32 ABI;
    // the function writes it synchronously and retains no pointer.
    let mut info: PERFORMANCE_INFORMATION = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as u32;
    info.cb = size;
    if unsafe { GetPerformanceInfo(&mut info, size) } == 0 || info.PageSize == 0 {
        return None;
    }
    let bytes = |pages: usize| (pages as u64).checked_mul(info.PageSize as u64);
    Some(Resources {
        physical_total: bytes(info.PhysicalTotal)?,
        physical_available: bytes(info.PhysicalAvailable)?,
        commit_available: bytes(info.CommitLimit.saturating_sub(info.CommitTotal))?,
    })
}

#[cfg(not(windows))]
fn system_resources() -> Option<Resources> {
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProcessorAdmissionFailure {
    Cancelled,
    Unavailable,
    MemoryLimit,
    MemoryPressure,
}

impl std::fmt::Display for ProcessorAdmissionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "a espera pelo Processador foi cancelada",
            Self::Unavailable => "o Processador está em quarentena; reinicie o aplicativo",
            Self::MemoryLimit => {
                "a imagem excede o orçamento de memória disponível para processamento"
            }
            Self::MemoryPressure => {
                "Não foi possível continuar o processamento por falta de memória. Tente novamente mais tarde."
            }
        })
    }
}

#[derive(Default, Debug)]
struct Usage {
    bytes: u64,
    active: usize,
}

pub(super) struct ResourceBudget {
    usage: Mutex<Usage>,
    changed: Notify,
    probe: Box<dyn Fn() -> Option<Resources> + Send + Sync>,
}

impl std::fmt::Debug for ResourceBudget {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResourceBudget")
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::with_probe(system_resources)
    }
}

#[derive(Debug)]
pub(super) struct MemoryReservation {
    budget: Arc<ResourceBudget>,
    bytes: u64,
}

impl ResourceBudget {
    #[cfg(test)]
    pub(super) fn with_available_memory_for_test(physical_mib: u64, commit_mib: u64) -> Self {
        Self::with_probe(move || {
            Some(Resources {
                physical_total: 24 * GIB,
                physical_available: physical_mib * MIB,
                commit_available: commit_mib * MIB,
            })
        })
    }

    fn with_probe(probe: impl Fn() -> Option<Resources> + Send + Sync + 'static) -> Self {
        Self {
            usage: Mutex::new(Usage::default()),
            changed: Notify::new(),
            probe: Box::new(probe),
        }
    }

    pub(super) async fn reserve(
        self: &Arc<Self>,
        estimate: ImageMemoryEstimate,
        cancellation: &AtomicBool,
        permits: &Semaphore,
    ) -> Result<MemoryReservation, ProcessorAdmissionFailure> {
        let mut waiting = false;
        loop {
            // notify_waiters observes an already-created future even before its
            // first poll; checking the condition afterwards cannot lose release.
            let changed = self.changed.notified();
            if cancellation.load(Ordering::Acquire) {
                return Err(ProcessorAdmissionFailure::Cancelled);
            }
            if permits.is_closed() {
                return Err(ProcessorAdmissionFailure::Unavailable);
            }
            let resources = (self.probe)();
            let ceiling = resources.map_or(GIB, Resources::ceiling);
            if estimate.0 > ceiling {
                return Err(ProcessorAdmissionFailure::MemoryLimit);
            }
            {
                let mut usage = self
                    .usage
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let available = resources.map_or(GIB, Resources::available);
                let serial_available = resources.map_or(GIB, Resources::serial_available);
                let parallel_admission = estimate.0 <= available.saturating_sub(usage.bytes);
                let serial_admission = usage.active == 0 && estimate.0 <= serial_available;
                // Missing telemetry admits one conservative reservation. Never
                // invent free RAM or use an earlier successful observation.
                if (resources.is_some() || usage.active == 0)
                    && (parallel_admission || serial_admission)
                {
                    if waiting || !parallel_admission {
                        tracing::info!(
                            event = "processor_memory_admitted",
                            mode = if parallel_admission {
                                "parallel"
                            } else {
                                "serial"
                            },
                            waited = waiting,
                            estimate_bytes = estimate.0,
                            reserved_bytes = usage.bytes,
                            physical_available_bytes =
                                resources.map(|value| value.physical_available),
                            commit_available_bytes = resources.map(|value| value.commit_available),
                        );
                    }
                    usage.bytes += estimate.0;
                    usage.active += 1;
                    return Ok(MemoryReservation {
                        budget: Arc::clone(self),
                        bytes: estimate.0,
                    });
                }
                if usage.active == 0 {
                    // No local worker can release memory. Return control so the
                    // operation can report the interruption and be retried.
                    tracing::warn!(
                        event = "processor_memory_unavailable",
                        estimate_bytes = estimate.0,
                        serial_available_bytes = serial_available,
                        physical_available_bytes = resources.map(|value| value.physical_available),
                        commit_available_bytes = resources.map(|value| value.commit_available),
                    );
                    return Err(ProcessorAdmissionFailure::MemoryPressure);
                }
                if !waiting {
                    tracing::info!(
                        event = "processor_memory_wait_started",
                        estimate_bytes = estimate.0,
                        reserved_bytes = usage.bytes,
                        active_workers = usage.active,
                    );
                    waiting = true;
                }
            }
            // Refresh external memory pressure even when no local worker exits;
            // the same interval also bounds cancellation and quarantine latency.
            let _ = tokio::time::timeout(RESOURCE_REFRESH, changed).await;
        }
    }
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        let mut usage = self
            .budget
            .usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        usage.bytes -= self.bytes;
        usage.active -= 1;
        drop(usage);
        self.budget.changed.notify_waiters();
    }
}

pub(super) async fn cancellable_reservation<T>(
    reservation: impl std::future::Future<Output = Result<T, super::ProcessorUnavailable>>,
    cancellation: &AtomicBool,
) -> Result<T, ProcessorAdmissionFailure> {
    tokio::pin!(reservation);
    loop {
        if cancellation.load(Ordering::Acquire) {
            return Err(ProcessorAdmissionFailure::Cancelled);
        }
        match tokio::time::timeout(RESOURCE_REFRESH, &mut reservation).await {
            Ok(result) => return result.map_err(|_| ProcessorAdmissionFailure::Unavailable),
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abundant() -> Option<Resources> {
        Some(Resources {
            physical_total: 16 * GIB,
            physical_available: 12 * GIB,
            commit_available: 12 * GIB,
        })
    }

    #[test]
    fn low_memory_keeps_one_image_moving_without_parallel_headroom() {
        tauri::async_runtime::block_on(async {
            let budget = Arc::new(ResourceBudget::with_probe(|| {
                Some(Resources {
                    physical_total: 24 * GIB,
                    physical_available: 350 * MIB,
                    commit_available: 7 * GIB,
                })
            }));
            let permits = Semaphore::new(8);
            let cancelled = AtomicBool::new(false);
            let first = tokio::time::timeout(
                Duration::from_millis(200),
                budget.reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits),
            )
            .await
            .expect("one image must advance below the parallel headroom")
            .unwrap();
            let mut next =
                Box::pin(budget.reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut next)
                    .await
                    .is_err()
            );
            drop(first);
            drop(
                tokio::time::timeout(Duration::from_millis(200), next)
                    .await
                    .unwrap()
                    .unwrap(),
            );
        });
    }

    #[test]
    fn insufficient_memory_without_active_work_finishes_with_a_recoverable_failure() {
        tauri::async_runtime::block_on(async {
            let current = Arc::new(Mutex::new(Resources {
                physical_total: 24 * GIB,
                physical_available: 256 * MIB,
                commit_available: 200 * MIB,
            }));
            let probe = Arc::clone(&current);
            let budget = Arc::new(ResourceBudget::with_probe(move || {
                Some(*probe.lock().unwrap())
            }));
            let permits = Semaphore::new(8);
            let cancelled = AtomicBool::new(false);
            let result = tokio::time::timeout(
                Duration::from_millis(200),
                budget.reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits),
            )
            .await
            .expect("an idle Processor cannot wait indefinitely for external memory");
            assert_eq!(
                result.unwrap_err(),
                ProcessorAdmissionFailure::MemoryPressure
            );
            *current.lock().unwrap() = abundant().unwrap();
            current.lock().unwrap().commit_available = 200 * MIB;
            assert_eq!(
                budget
                    .reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits)
                    .await
                    .unwrap_err(),
                ProcessorAdmissionFailure::MemoryPressure,
                "commit also limits serial admission"
            );
            *current.lock().unwrap() = abundant().unwrap();
            drop(
                budget
                    .reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits)
                    .await
                    .unwrap(),
            );
            current.lock().unwrap().physical_available = 0;
            drop(
                budget
                    .reserve(ImageMemoryEstimate(236 * MIB), &cancelled, &permits)
                    .await
                    .expect("free physical pages are not the commit capacity"),
            );
        });
    }

    #[test]
    fn resource_budget_limits_aggregate_memory_and_releases_every_reservation() {
        tauri::async_runtime::block_on(async {
            let budget = Arc::new(ResourceBudget::with_probe(abundant));
            let permits = Semaphore::new(8);
            let cancelled = AtomicBool::new(false);
            let first = budget
                .reserve(ImageMemoryEstimate(3 * GIB), &cancelled, &permits)
                .await
                .unwrap();
            let mut second =
                Box::pin(budget.reserve(ImageMemoryEstimate(2 * GIB), &cancelled, &permits));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut second)
                    .await
                    .is_err()
            );
            drop(first);
            let second = second.await.unwrap();
            assert_eq!(budget.usage.lock().unwrap().bytes, 2 * GIB);
            drop(second);
            assert_eq!(budget.usage.lock().unwrap().bytes, 0);
            assert_eq!(
                budget
                    .reserve(ImageMemoryEstimate(5 * GIB), &cancelled, &permits)
                    .await
                    .unwrap_err(),
                ProcessorAdmissionFailure::MemoryLimit
            );
        });
    }

    #[test]
    fn active_work_waits_for_recovery_and_cancellation_and_quarantine_wake_waiters() {
        tauri::async_runtime::block_on(async {
            let current = Arc::new(Mutex::new(abundant().unwrap()));
            let probe = Arc::clone(&current);
            let budget = Arc::new(ResourceBudget::with_probe(move || {
                Some(*probe.lock().unwrap())
            }));
            let permits = Semaphore::new(8);
            let cancelled = AtomicBool::new(false);
            let active = budget
                .reserve(ImageMemoryEstimate(128 * MIB), &cancelled, &permits)
                .await
                .unwrap();
            current.lock().unwrap().physical_available = 256 * MIB;
            let mut waiting =
                Box::pin(budget.reserve(ImageMemoryEstimate(128 * MIB), &cancelled, &permits));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut waiting)
                    .await
                    .is_err()
            );
            assert_eq!(budget.usage.lock().unwrap().active, 1);
            *current.lock().unwrap() = abundant().unwrap();
            let reservation = tokio::time::timeout(Duration::from_secs(1), &mut waiting)
                .await
                .unwrap()
                .unwrap();
            drop(reservation);
            current.lock().unwrap().commit_available = 0;
            let mut waiting =
                Box::pin(budget.reserve(ImageMemoryEstimate(128 * MIB), &cancelled, &permits));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut waiting)
                    .await
                    .is_err()
            );
            cancelled.store(true, Ordering::Release);
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(1), waiting)
                    .await
                    .unwrap()
                    .unwrap_err(),
                ProcessorAdmissionFailure::Cancelled
            );
            cancelled.store(false, Ordering::Release);
            permits.close();
            assert_eq!(
                budget
                    .reserve(ImageMemoryEstimate(128 * MIB), &cancelled, &permits)
                    .await
                    .unwrap_err(),
                ProcessorAdmissionFailure::Unavailable
            );
            drop(active);
        });
    }

    #[test]
    fn missing_telemetry_uses_one_worker_and_estimates_use_the_frozen_source() {
        tauri::async_runtime::block_on(async {
            let budget = Arc::new(ResourceBudget::with_probe(|| None));
            let permits = Semaphore::new(8);
            let cancelled = AtomicBool::new(false);
            let first = budget
                .reserve(ImageMemoryEstimate(64 * MIB), &cancelled, &permits)
                .await
                .unwrap();
            let mut next =
                Box::pin(budget.reserve(ImageMemoryEstimate(64 * MIB), &cancelled, &permits));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut next)
                    .await
                    .is_err()
            );
            drop(first);
            drop(next.await.unwrap());
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source.jpg");
            image::RgbImage::new(200, 100).save(&source).unwrap();
            let mut context = myalbuns_paths::OperationPathContext::new();
            context.capture(&source).unwrap();
            let estimate = ImageMemoryEstimate::in_plan(&context.freeze(), [source.as_path()]);
            assert!(estimate.0 >= 200 * 100 * 16 + 64 * MIB);
            assert!(estimate.0 < 65 * MIB);
        });
    }
}
