use std::sync::Arc;

#[cfg(test)]
use tokio::sync::OwnedRwLockReadGuard;
use tokio::sync::{OwnedRwLockWriteGuard, RwLock};

#[derive(Debug, Default)]
pub(crate) struct CacheActivityGate {
    activity: Arc<RwLock<()>>,
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct CacheWorkPermit {
    _guard: OwnedRwLockReadGuard<()>,
}

#[derive(Debug)]
pub(crate) struct CachePause {
    _guard: OwnedRwLockWriteGuard<()>,
}

impl CacheActivityGate {
    #[cfg(test)]
    pub(crate) async fn begin_work(&self) -> CacheWorkPermit {
        CacheWorkPermit {
            _guard: self.activity.clone().read_owned().await,
        }
    }

    pub(crate) async fn pause(&self) -> CachePause {
        CachePause {
            _guard: self.activity.clone().write_owned().await,
        }
    }
}
