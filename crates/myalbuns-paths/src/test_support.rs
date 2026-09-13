//! Scoped filesystem failures for tests of real Cache publication paths.
//! This module is enabled only by test dependencies, never by application builds.

use std::{cell::RefCell, io, marker::PhantomData, path::Path, path::PathBuf, rc::Rc};

thread_local! {
    static WRITE_LIMIT: RefCell<Option<WriteLimit>> = const { RefCell::new(None) };
}

struct WriteLimit {
    destination: PathBuf,
    remaining: usize,
    written: usize,
    failures: usize,
    fail_rename: bool,
}

/// Simulates a full disk after real writes to one exact Cache destination.
/// The fault is thread-local and removed on drop, including during unwinding.
pub struct CacheDiskFull {
    _same_thread: PhantomData<Rc<()>>,
}

impl CacheDiskFull {
    pub fn after_bytes(destination: &Path, bytes: usize) -> Self {
        WRITE_LIMIT.with_borrow_mut(|current| {
            assert!(
                current.is_none(),
                "nested Cache write limits are unsupported"
            );
            *current = Some(WriteLimit {
                destination: destination.to_owned(),
                remaining: bytes,
                written: 0,
                failures: 0,
                fail_rename: false,
            });
        });
        Self {
            _same_thread: PhantomData,
        }
    }

    pub fn on_rename(destination: &Path) -> Self {
        let fault = Self::after_bytes(destination, usize::MAX);
        WRITE_LIMIT.with_borrow_mut(|current| current.as_mut().unwrap().fail_rename = true);
        fault
    }

    pub fn written_bytes(&self) -> usize {
        WRITE_LIMIT.with_borrow(|current| current.as_ref().unwrap().written)
    }

    pub fn failure_count(&self) -> usize {
        WRITE_LIMIT.with_borrow(|current| current.as_ref().unwrap().failures)
    }
}

impl Drop for CacheDiskFull {
    fn drop(&mut self) {
        WRITE_LIMIT.with_borrow_mut(|current| *current = None);
    }
}

pub(crate) fn write(
    destination: &Path,
    buffer: &[u8],
    writer: impl FnOnce(&[u8]) -> io::Result<usize>,
) -> io::Result<usize> {
    WRITE_LIMIT.with_borrow_mut(|current| {
        let Some(limit) = current
            .as_mut()
            .filter(|limit| limit.destination == destination && !limit.fail_rename)
        else {
            return writer(buffer);
        };
        if buffer.is_empty() {
            return writer(buffer);
        }
        if limit.remaining == 0 {
            limit.failures += 1;
            #[cfg(windows)]
            return Err(io::Error::from_raw_os_error(112)); // ERROR_DISK_FULL
            #[cfg(not(windows))]
            return Err(io::ErrorKind::StorageFull.into());
        }
        let written = writer(&buffer[..buffer.len().min(limit.remaining)])?;
        limit.remaining -= written;
        limit.written += written;
        Ok(written)
    })
}

pub(crate) fn rename(destination: &Path) -> io::Result<()> {
    WRITE_LIMIT.with_borrow_mut(|current| {
        if let Some(limit) = current
            .as_mut()
            .filter(|limit| limit.destination == destination && limit.fail_rename)
        {
            limit.failures += 1;
            return Err(io::ErrorKind::StorageFull.into());
        }
        Ok(())
    })
}
