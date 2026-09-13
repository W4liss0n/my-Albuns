use std::{
    fmt,
    io::{self, Write},
};

use myalbuns_paths::AppPathsError;

#[derive(Debug)]
pub(crate) enum CacheError {
    StorageFull,
    Other(String),
}

/// Some encoders erase the original I/O cause when constructing their error.
/// Observe the actual writer so classification never parses localized text.
pub(crate) struct CacheWriteMonitor<W> {
    inner: W,
    pub(crate) storage_full: bool,
}

impl<W: Write> CacheWriteMonitor<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            storage_full: false,
        }
    }
}

impl<W: Write> Write for CacheWriteMonitor<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes).inspect_err(|error| {
            self.storage_full |= AppPathsError::cache_io(error) == AppPathsError::CacheStorageFull;
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush().inspect_err(|error| {
            self.storage_full |= AppPathsError::cache_io(error) == AppPathsError::CacheStorageFull;
        })
    }
}

impl CacheError {
    pub(crate) fn paths(context: &str, error: AppPathsError) -> Self {
        if error == AppPathsError::CacheStorageFull {
            Self::StorageFull
        } else {
            Self::Other(format!("{context}: {error}"))
        }
    }

    pub(crate) fn io(context: &str, error: std::io::Error) -> Self {
        if AppPathsError::cache_io(&error) == AppPathsError::CacheStorageFull {
            Self::StorageFull
        } else {
            Self::Other(format!("{context}: {error}"))
        }
    }

    pub(crate) fn image(context: &str, error: image::ImageError) -> Self {
        match error {
            image::ImageError::IoError(error) => Self::io(context, error),
            error => Self::Other(format!("{context}: {error}")),
        }
    }
}

impl From<String> for CacheError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<CacheError> for String {
    fn from(error: CacheError) -> Self {
        error.to_string()
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StorageFull => AppPathsError::CacheStorageFull.fmt(formatter),
            Self::Other(message) => formatter.write_str(message),
        }
    }
}
