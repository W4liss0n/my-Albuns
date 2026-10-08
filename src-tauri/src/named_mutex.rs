use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_ABANDONED_0, WAIT_FAILED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForMultipleObjects, WaitForSingleObject},
};

/// How long one wait of an open-ended acquisition lasts before its worker
/// checks whether the waiter stopped. The handles stay open across waits.
const STOP_POLL: Duration = Duration::from_millis(25);
const MAXIMUM_WAIT_OBJECTS: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct NamedMutex {
    name: Result<Vec<u16>, String>,
    worker_name: &'static str,
}

/// Ownership is thread-affine on Windows: the worker that acquired the mutex
/// keeps it until the grant is dropped, then releases it on that same thread.
#[derive(Debug)]
pub(crate) struct NamedMutexGrant {
    release: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
    /// The previous owner ended without releasing (only tests read it).
    #[cfg_attr(not(test), allow(dead_code))]
    abandoned: bool,
}

/// An acquisition that is still waiting. Dropping it stops the wait; a mutex
/// granted after that is released at once by its worker.
#[derive(Debug)]
pub(crate) struct PendingNamedMutex {
    acquisition: tokio::sync::oneshot::Receiver<WorkerAcquisition>,
    release: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NamedMutexError {
    Conflict,
    Unavailable(String),
}

#[derive(Debug)]
enum WorkerAcquisition {
    Acquired { abandoned: bool },
    Conflict,
    Unavailable(String),
}

enum WaitLimit {
    Within(Duration),
    UntilStopped(Arc<AtomicBool>),
}

enum WaitOutcome {
    Owned { index: usize, abandoned: bool },
    NotOwned(WorkerAcquisition),
}

impl NamedMutex {
    /// Observe ownership without keeping a reservation. The owning worker is
    /// always a different thread, including for callers in the same process.
    pub(crate) fn is_owned(&self) -> Result<bool, NamedMutexError> {
        let name = self
            .name
            .as_ref()
            .map_err(|reason| NamedMutexError::Unavailable(reason.clone()))?;
        // SAFETY: name is a terminated UTF-16 buffer; the handle stays local.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(NamedMutexError::Unavailable(
                io::Error::last_os_error().to_string(),
            ));
        }
        // SAFETY: this is a live mutex handle and a non-blocking wait.
        let result = unsafe { WaitForSingleObject(handle, 0) };
        let observed = match result {
            WAIT_OBJECT_0 | WAIT_ABANDONED => {
                // SAFETY: the preceding wait granted ownership to this thread.
                unsafe {
                    ReleaseMutex(handle);
                }
                Ok(false)
            }
            WAIT_TIMEOUT => Ok(true),
            _ => Err(NamedMutexError::Unavailable(
                io::Error::last_os_error().to_string(),
            )),
        };
        // SAFETY: no pending waits or references outlive this handle.
        unsafe {
            CloseHandle(handle);
        }
        observed
    }

    pub(crate) fn scoped(
        app_paths: &AppPaths,
        kind: &str,
        scope: &str,
        worker_name: &'static str,
    ) -> Self {
        let name = scoped_name(app_paths.local_root(), kind, scope).map(|name| {
            std::ffi::OsStr::new(&name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
        });
        Self { name, worker_name }
    }

    pub(crate) fn try_acquire(&self) -> Result<NamedMutexGrant, NamedMutexError> {
        self.try_acquire_within(Duration::ZERO)
    }

    /// Waits at most `timeout` for ownership; `Conflict` means it stayed owned.
    /// The wait runs on the worker thread that then owns the mutex.
    pub(crate) fn try_acquire_within(
        &self,
        timeout: Duration,
    ) -> Result<NamedMutexGrant, NamedMutexError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (release, worker) = spawn_acquisition(
            std::slice::from_ref(self),
            WaitLimit::Within(timeout),
            move |acquisition| sender.send(acquisition).is_ok(),
        )?;
        let acquisition = receiver.recv().map_err(|error| error.to_string());
        finish_acquisition(acquisition, release, worker)
    }

    /// Starts waiting, with no deadline, for whichever of `mutexes` is free
    /// first. It is the building block of a slot pool: Windows releases the
    /// slot of a holder that dies, and the next acquirer receives it as
    /// abandoned. One worker opens every handle once and waits on all of them
    /// in a single call, checking every `STOP_POLL` whether the pending
    /// acquisition was dropped.
    pub(crate) fn wait_for_any(mutexes: &[Self]) -> Result<PendingNamedMutex, NamedMutexError> {
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, acquisition) = tokio::sync::oneshot::channel();
        let (release, worker) = spawn_acquisition(
            mutexes,
            WaitLimit::UntilStopped(Arc::clone(&stop)),
            move |outcome| sender.send(outcome).is_ok(),
        )?;
        Ok(PendingNamedMutex {
            acquisition,
            release: Some(release),
            worker: Some(worker),
            stop,
        })
    }

    /// Takes the mutex on a thread that ends without releasing it, which
    /// leaves it abandoned exactly like a holder process that died. The
    /// returned handle keeps the mutex object alive: closing the last handle
    /// would destroy it, and the next acquirer would create a fresh one.
    #[cfg(test)]
    pub(crate) fn abandon_in_exited_thread(&self) -> AbandonedMutexHandle {
        let name = self.name.clone().expect("the test mutex has a name");
        // SAFETY: name is a terminated UTF-16 buffer; this handle only keeps
        // the object alive and never waits.
        let keep_alive = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        assert!(!keep_alive.is_null(), "the test mutex is created");
        thread::spawn(move || {
            // SAFETY: name is a terminated UTF-16 buffer. The thread exits
            // while it owns the mutex, so Windows marks it abandoned.
            unsafe {
                let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
                assert!(!handle.is_null(), "the test mutex is opened");
                assert_eq!(WaitForSingleObject(handle, 0), WAIT_OBJECT_0);
                CloseHandle(handle);
            }
        })
        .join()
        .expect("the abandoning thread finishes");
        AbandonedMutexHandle(keep_alive)
    }
}

#[cfg(test)]
pub(crate) struct AbandonedMutexHandle(HANDLE);

#[cfg(test)]
impl Drop for AbandonedMutexHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by abandon_in_exited_thread and is
        // closed exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl NamedMutexGrant {
    #[cfg(test)]
    pub(crate) fn was_abandoned(&self) -> bool {
        self.abandoned
    }
}

impl PendingNamedMutex {
    /// Resolves once the wait ends. Cancel-safe: dropping this future keeps
    /// the wait running, so a caller may poll it again after a timeout.
    pub(crate) async fn granted(&mut self) -> Result<NamedMutexGrant, NamedMutexError> {
        let acquisition = (&mut self.acquisition)
            .await
            .map_err(|error| error.to_string());
        let (Some(release), Some(worker)) = (self.release.take(), self.worker.take()) else {
            return Err(NamedMutexError::Unavailable(
                "a espera pelo mutex já terminou".into(),
            ));
        };
        finish_acquisition(acquisition, release, worker)
    }
}

impl Drop for PendingNamedMutex {
    fn drop(&mut self) {
        // The worker sees it within STOP_POLL and is left to finish alone:
        // joining here could block an async executor thread.
        self.stop.store(true, Ordering::Release);
    }
}

fn spawn_acquisition(
    mutexes: &[NamedMutex],
    limit: WaitLimit,
    deliver: impl FnOnce(WorkerAcquisition) -> bool + Send + 'static,
) -> Result<(Sender<()>, JoinHandle<()>), NamedMutexError> {
    let Some(first) = mutexes.first() else {
        return Err(NamedMutexError::Unavailable(
            "nenhum mutex foi informado".into(),
        ));
    };
    if mutexes.len() > MAXIMUM_WAIT_OBJECTS {
        return Err(NamedMutexError::Unavailable(
            "o Windows espera no máximo 64 mutexes por vez".into(),
        ));
    }
    let names = mutexes
        .iter()
        .map(|mutex| mutex.name.clone().map_err(NamedMutexError::Unavailable))
        .collect::<Result<Vec<_>, _>>()?;
    let (release_sender, release_receiver) = mpsc::channel::<()>();
    let worker = thread::Builder::new()
        .name(first.worker_name.into())
        .spawn(move || {
            let mut handles = match open_handles(&names) {
                Ok(handles) => handles,
                Err(reason) => {
                    deliver(WorkerAcquisition::Unavailable(reason));
                    return;
                }
            };
            let (index, abandoned) = match wait_on(&handles, &limit) {
                WaitOutcome::Owned { index, abandoned } => (index, abandoned),
                WaitOutcome::NotOwned(acquisition) => {
                    close_handles(&handles);
                    deliver(acquisition);
                    return;
                }
            };
            let owned_handle = handles.swap_remove(index);
            // Only the owned handle stays open while the grant lives.
            close_handles(&handles);
            // A waiter that gave up no longer receives: release at once.
            if deliver(WorkerAcquisition::Acquired { abandoned }) {
                let _ = release_receiver.recv();
            }
            // SAFETY: this thread owns the mutex through owned_handle, which
            // is released and closed exactly once.
            unsafe {
                ReleaseMutex(owned_handle);
                CloseHandle(owned_handle);
            }
        })
        .map_err(|error| NamedMutexError::Unavailable(error.to_string()))?;
    Ok((release_sender, worker))
}

fn finish_acquisition(
    acquisition: Result<WorkerAcquisition, String>,
    release: Sender<()>,
    worker: JoinHandle<()>,
) -> Result<NamedMutexGrant, NamedMutexError> {
    let failure = match acquisition {
        Ok(WorkerAcquisition::Acquired { abandoned }) => {
            return Ok(NamedMutexGrant {
                release: Some(release),
                worker: Some(worker),
                abandoned,
            });
        }
        Ok(WorkerAcquisition::Conflict) => NamedMutexError::Conflict,
        Ok(WorkerAcquisition::Unavailable(reason)) | Err(reason) => {
            NamedMutexError::Unavailable(reason)
        }
    };
    // The worker owns nothing and is closing its handles.
    let _ = worker.join();
    Err(failure)
}

fn open_handles(names: &[Vec<u16>]) -> Result<Vec<HANDLE>, String> {
    let mut handles = Vec::with_capacity(names.len());
    for name in names {
        // SAFETY: name is a terminated UTF-16 buffer; the handle belongs to
        // the calling worker, which closes it.
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            let reason = io::Error::last_os_error().to_string();
            close_handles(&handles);
            return Err(reason);
        }
        handles.push(handle);
    }
    Ok(handles)
}

/// Waits on every handle at once. A bounded wait loops until the full timeout
/// has really elapsed (a Windows wait can end slightly early); an open-ended
/// wait loops in `STOP_POLL` slices until the waiter stops it.
fn wait_on(handles: &[HANDLE], limit: &WaitLimit) -> WaitOutcome {
    let started = Instant::now();
    let count = handles.len() as u32;
    loop {
        let slice = match limit {
            WaitLimit::Within(timeout) => timeout.saturating_sub(started.elapsed()),
            WaitLimit::UntilStopped(stop) => {
                if stop.load(Ordering::Acquire) {
                    return WaitOutcome::NotOwned(WorkerAcquisition::Conflict);
                }
                STOP_POLL
            }
        };
        // SAFETY: every handle is a live mutex handle owned by this thread
        // and the count matches the buffer (at most 64).
        let result =
            unsafe { WaitForMultipleObjects(count, handles.as_ptr(), 0, wait_milliseconds(slice)) };
        if (WAIT_OBJECT_0..WAIT_OBJECT_0 + count).contains(&result) {
            return WaitOutcome::Owned {
                index: (result - WAIT_OBJECT_0) as usize,
                abandoned: false,
            };
        }
        if (WAIT_ABANDONED_0..WAIT_ABANDONED_0 + count).contains(&result) {
            // The previous owner ended without releasing: the mutex is ours.
            return WaitOutcome::Owned {
                index: (result - WAIT_ABANDONED_0) as usize,
                abandoned: true,
            };
        }
        match result {
            WAIT_TIMEOUT => {
                if let WaitLimit::Within(timeout) = limit
                    && started.elapsed() >= *timeout
                {
                    return WaitOutcome::NotOwned(WorkerAcquisition::Conflict);
                }
            }
            WAIT_FAILED => {
                return WaitOutcome::NotOwned(WorkerAcquisition::Unavailable(
                    io::Error::last_os_error().to_string(),
                ));
            }
            other => {
                return WaitOutcome::NotOwned(WorkerAcquisition::Unavailable(format!(
                    "resultado inesperado do mutex: {other}"
                )));
            }
        }
    }
}

/// Rounds up so a remaining fraction of a millisecond still waits; INFINITE
/// is `u32::MAX`, so a bounded wait stays below it.
fn wait_milliseconds(duration: Duration) -> u32 {
    let milliseconds = duration.as_nanos().div_ceil(1_000_000);
    u32::try_from(milliseconds)
        .unwrap_or(u32::MAX - 1)
        .min(u32::MAX - 1)
}

fn close_handles(handles: &[HANDLE]) {
    for handle in handles {
        // SAFETY: each handle was opened by the calling worker and is closed once.
        unsafe {
            CloseHandle(*handle);
        }
    }
}

pub(crate) fn scoped_name(local_root: &Path, kind: &str, scope: &str) -> Result<String, String> {
    let mut digest = Sha256::new();
    for unit in windows_filesystem_case(local_root)? {
        digest.update(unit.to_le_bytes());
    }
    if !scope.is_empty() {
        digest.update([0]);
        for byte in scope.bytes() {
            digest.update([byte.to_ascii_lowercase()]);
        }
    }
    let digest = digest.finalize();
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(r"Local\MyAlbuns.{kind}.v1.{suffix}"))
}

fn windows_filesystem_case(path: &Path) -> Result<Vec<u16>, String> {
    use windows_sys::Win32::{
        Foundation::GetLastError,
        Globalization::{LCMAP_UPPERCASE, LCMapStringEx, LOCALE_NAME_INVARIANT},
    };

    let source = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let source_len = i32::try_from(source.len())
        .map_err(|_| "a raiz local excede o limite de case mapping do Windows".to_string())?;
    // SAFETY: source is a live UTF-16 buffer of source_len units. A null
    // destination with length zero asks Windows for the exact required size.
    let required = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if required == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        return Err(format!(
            "não foi possível normalizar a raiz local para o mutex (Windows {code})"
        ));
    }
    let mut mapped = vec![0_u16; required as usize];
    // SAFETY: mapped has the exact capacity returned by the preceding call;
    // all remaining pointers and lengths follow the same documented contract.
    let written = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            mapped.as_mut_ptr(),
            required,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if written == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        return Err(format!(
            "não foi possível materializar a raiz local normalizada (Windows {code})"
        ));
    }
    mapped.truncate(written as usize);
    Ok(mapped)
}

impl Drop for NamedMutexGrant {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            let _ = release.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use myalbuns_paths::AppPaths;

    use super::{NamedMutex, NamedMutexError};

    /// Windows timer resolution can end a wait up to one tick early; the
    /// acquisition loop compensates, but assertions keep a margin anyway.
    const TIMER_MARGIN: Duration = Duration::from_millis(40);

    fn mutex(root: &std::path::Path, scope: &str) -> NamedMutex {
        NamedMutex::scoped(
            &AppPaths::from_roots(&root.join("roaming"), &root.join("local")),
            "NamedMutexTest",
            scope,
            "myalbuns-named-mutex-test",
        )
    }

    #[test]
    fn a_bounded_wait_lasts_its_timeout_and_then_reports_a_conflict() {
        let root = tempfile::tempdir().expect("temporary mutex fixture");
        let held = mutex(root.path(), "bounded").try_acquire().unwrap();
        let waiting = Duration::from_millis(150);

        let started = Instant::now();
        assert_eq!(
            mutex(root.path(), "bounded")
                .try_acquire_within(waiting)
                .unwrap_err(),
            NamedMutexError::Conflict
        );
        assert!(started.elapsed() + TIMER_MARGIN >= waiting);

        let releaser = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(held);
        });
        mutex(root.path(), "bounded")
            .try_acquire_within(Duration::from_secs(5))
            .expect("a release during the wait grants the mutex");
        releaser.join().unwrap();
    }

    #[test]
    fn an_open_ended_wait_takes_the_first_mutex_released_and_stops_when_dropped() {
        let root = tempfile::tempdir().expect("temporary mutex fixture");
        let pool = [mutex(root.path(), "pool-0"), mutex(root.path(), "pool-1")];
        let first = pool[0].try_acquire().unwrap();
        let second = pool[1].try_acquire().unwrap();
        tauri::async_runtime::block_on(async {
            let mut pending = NamedMutex::wait_for_any(&pool).unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(100), pending.granted())
                    .await
                    .is_err(),
                "every mutex of the pool is held"
            );
            drop(second);
            let granted = tokio::time::timeout(Duration::from_secs(5), pending.granted())
                .await
                .expect("the released mutex is granted to the same pending wait")
                .unwrap();
            assert!(pool[1].is_owned().unwrap(), "the waiter now owns it");
            drop(granted);

            let abandoned_wait = NamedMutex::wait_for_any(&pool[..1]).unwrap();
            drop(abandoned_wait);
            drop(first);
            // The stopped waiter must not keep or take the released mutex.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match pool[0].try_acquire() {
                    Ok(_) => break,
                    Err(NamedMutexError::Conflict) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("a stopped wait kept the mutex: {error:?}"),
                }
            }
        });
    }

    #[test]
    fn a_mutex_whose_owner_thread_exited_is_received_as_abandoned() {
        let root = tempfile::tempdir().expect("temporary mutex fixture");
        let slot = mutex(root.path(), "abandoned");
        let _keep_alive = slot.abandon_in_exited_thread();

        let grant = slot
            .try_acquire()
            .expect("an abandoned mutex is acquirable");
        assert!(grant.was_abandoned());
        drop(grant);
        assert!(
            !slot.try_acquire().unwrap().was_abandoned(),
            "a released mutex is no longer abandoned"
        );
    }
}
