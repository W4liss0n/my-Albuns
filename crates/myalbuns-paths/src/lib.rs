//! Centralized discovery and guarded application pathname operations.

mod app_paths;
mod atomic_publish;
mod cache;
mod error;
mod export;
mod guarded_fs;
mod native_path_serde;
mod operation;
#[cfg(windows)]
mod project_file_lock;
#[cfg(windows)]
mod project_transition_barrier;
mod resolve;
mod windows_path;

pub use app_paths::{AppPaths, project_data_namespace};
pub use atomic_publish::{publish_new_file, replace_existing_file};
pub use cache::{
    CacheArtifactFormat, CachePathPlan, PendingCachePublication, PreparedCacheStorage,
    SynchronizedCachePublication,
};
pub use error::AppPathsError;
pub use export::{ExportPathPlan, ExportWriteAuthorization, PreparedExportStorage};
pub use native_path_serde::NativePathDto;
pub use operation::{
    OperationPathContext, PathRootKind, RootBinding, RootBindingPlan, validate_external_path,
};
#[cfg(windows)]
pub use project_file_lock::{ProjectFileLock, ProjectFileLockError};
#[cfg(windows)]
pub use project_transition_barrier::{ProjectTransitionBarrier, ProjectTransitionBarrierError};
pub use resolve::{
    ExpectedObject, PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination,
    ResolveError, ResolvedObject,
};
#[cfg(windows)]
pub use windows_path::wide_api_path;
