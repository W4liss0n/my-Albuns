//! Descoberta centralizada e operações protegidas dos caminhos do aplicativo.

mod app_paths;
mod cache;
mod error;
mod export;
mod guarded_fs;
mod native_path_serde;
mod operation;
#[cfg(windows)]
mod project_file_lock;
mod resolve;
mod windows_path;

pub use app_paths::{AppPaths, PreparedExportPreviewDirectory, project_data_namespace};
pub use cache::{
    CacheArtifactFormat, CachePathPlan, PendingCachePublication, PreparedCacheStorage,
    SynchronizedCachePublication,
};
pub use error::AppPathsError;
pub use export::{ExportPathPlan, PreparedExportStorage};
pub use native_path_serde::NativePathDto;
pub use operation::{
    OperationPathContext, PathRootKind, RootBinding, RootBindingPlan, validate_external_path,
};
#[cfg(windows)]
pub use project_file_lock::{ProjectFileLock, ProjectFileLockError};
pub use resolve::{
    ExpectedObject, PhysicalFileIdentity, PhysicalIdentityEvidence, PreparedFileDestination,
    ResolveError, ResolvedObject,
};
#[cfg(windows)]
pub use windows_path::wide_api_path;
