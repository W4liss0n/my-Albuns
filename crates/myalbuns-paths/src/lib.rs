//! Descoberta centralizada e operações protegidas dos caminhos do aplicativo.

mod app_paths;
mod cache;
mod error;
mod export;
mod guarded_fs;
mod operation;

pub use app_paths::AppPaths;
pub use cache::{
    CacheArtifactFormat, CachePathPlan, PendingCachePublication, PreparedCacheStorage,
    SynchronizedCachePublication,
};
pub use error::AppPathsError;
pub use export::{ExportPathPlan, PreparedExportStorage};
pub use operation::{OperationPathContext, PathRootKind, RootBinding, RootBindingPlan};
