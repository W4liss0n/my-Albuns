//! Child processes and readiness files shared by process-level tests.

use std::{
    io,
    ops::{Deref, DerefMut},
    path::Path,
    process::Child,
};

/// Terminates a spawned fixture when the test leaves scope, so a failed
/// assertion cannot strand a process that only sleeps until it is killed.
pub(crate) struct ChildGuard(Child);

impl ChildGuard {
    pub(crate) fn new(child: Child) -> Self {
        Self(child)
    }
}

impl Deref for ChildGuard {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Publishes a readiness file whole, so a reader that sees it can parse it.
pub(crate) fn publish_ready(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let staged = path.with_extension("staging");
    std::fs::write(&staged, contents)?;
    std::fs::rename(staged, path)
}
