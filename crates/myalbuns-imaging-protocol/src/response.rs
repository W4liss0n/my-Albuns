use serde::{Deserialize, Serialize};

use crate::cache::CacheCompletion;
use crate::render::RenderCompletion;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImagingResponse {
    Completed {
        request_id: String,
        completion: RenderCompletion,
    },
    CacheCompleted {
        request_id: String,
        completion: CacheCompletion,
    },
    CacheReset {
        request_id: String,
        removed_count: usize,
    },
}

impl ImagingResponse {
    pub fn completed(request_id: impl Into<String>, completion: RenderCompletion) -> Self {
        Self::Completed {
            request_id: request_id.into(),
            completion,
        }
    }

    pub fn cache_completed(request_id: impl Into<String>, completion: CacheCompletion) -> Self {
        Self::CacheCompleted {
            request_id: request_id.into(),
            completion,
        }
    }

    pub fn cache_reset(request_id: impl Into<String>, removed_count: usize) -> Self {
        Self::CacheReset {
            request_id: request_id.into(),
            removed_count,
        }
    }

    pub fn completed_for(&self, expected_request_id: &str) -> Option<&RenderCompletion> {
        match self {
            Self::Completed {
                request_id,
                completion,
            } if request_id == expected_request_id => Some(completion),
            _ => None,
        }
    }

    pub fn cache_completed_for(&self, expected_request_id: &str) -> Option<&CacheCompletion> {
        match self {
            Self::CacheCompleted {
                request_id,
                completion,
            } if request_id == expected_request_id => Some(completion),
            _ => None,
        }
    }

    pub fn cache_reset_for(&self, expected_request_id: &str) -> Option<usize> {
        match self {
            Self::CacheReset {
                request_id,
                removed_count,
            } if request_id == expected_request_id => Some(*removed_count),
            _ => None,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Completed { request_id, .. }
            | Self::CacheCompleted { request_id, .. }
            | Self::CacheReset { request_id, .. } => request_id,
        }
    }
}
