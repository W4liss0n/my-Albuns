//! Descoberta centralizada e operações protegidas dos caminhos do aplicativo.

mod app_paths;
mod cache;
mod error;
mod export;
mod guarded_fs;

pub use app_paths::AppPaths;
pub use cache::{CachePathPlan, PreparedCacheStorage};
pub use error::AppPathsError;
pub use export::{ExportPathPlan, PreparedExportStorage};
