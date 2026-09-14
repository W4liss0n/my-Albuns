//! Scoped filesystem failures for tests of real Cache and Export output paths.
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
    operation: Operation,
}

#[derive(Clone, Copy, PartialEq)]
enum Operation {
    Write,
    Rename,
    Create,
    Sync,
}

/// Simulates a full disk at one exact destination.
/// The fault is thread-local and removed on drop, including during unwinding.
pub struct DiskFull {
    _same_thread: PhantomData<Rc<()>>,
}

impl DiskFull {
    pub fn after_bytes(destination: &Path, bytes: usize) -> Self {
        WRITE_LIMIT.with_borrow_mut(|current| {
            assert!(current.is_none(), "nested disk-full faults are unsupported");
            *current = Some(WriteLimit {
                destination: destination.to_owned(),
                remaining: bytes,
                written: 0,
                failures: 0,
                operation: Operation::Write,
            });
        });
        Self {
            _same_thread: PhantomData,
        }
    }

    pub fn on_rename(destination: &Path) -> Self {
        Self::at_operation(destination, Operation::Rename)
    }

    pub fn on_create(destination: &Path) -> Self {
        Self::at_operation(destination, Operation::Create)
    }

    pub fn on_sync(destination: &Path) -> Self {
        Self::at_operation(destination, Operation::Sync)
    }

    fn at_operation(destination: &Path, operation: Operation) -> Self {
        let fault = Self::after_bytes(destination, usize::MAX);
        WRITE_LIMIT.with_borrow_mut(|current| current.as_mut().unwrap().operation = operation);
        fault
    }

    pub fn written_bytes(&self) -> usize {
        WRITE_LIMIT.with_borrow(|current| current.as_ref().unwrap().written)
    }

    pub fn failure_count(&self) -> usize {
        WRITE_LIMIT.with_borrow(|current| current.as_ref().unwrap().failures)
    }
}

impl Drop for DiskFull {
    fn drop(&mut self) {
        WRITE_LIMIT.with_borrow_mut(|current| *current = None);
    }
}

pub fn write(
    destination: &Path,
    buffer: &[u8],
    writer: impl FnOnce(&[u8]) -> io::Result<usize>,
) -> io::Result<usize> {
    WRITE_LIMIT.with_borrow_mut(|current| {
        let Some(limit) = current.as_mut().filter(|limit| {
            limit.destination == destination && limit.operation == Operation::Write
        }) else {
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
    fail_operation(destination, Operation::Rename)
}

pub fn create(destination: &Path) -> io::Result<()> {
    fail_operation(destination, Operation::Create)
}

pub fn sync(destination: &Path) -> io::Result<()> {
    fail_operation(destination, Operation::Sync)
}

fn fail_operation(destination: &Path, operation: Operation) -> io::Result<()> {
    WRITE_LIMIT.with_borrow_mut(|current| {
        if let Some(limit) = current
            .as_mut()
            .filter(|limit| limit.destination == destination && limit.operation == operation)
        {
            limit.failures += 1;
            return Err(io::ErrorKind::StorageFull.into());
        }
        Ok(())
    })
}
