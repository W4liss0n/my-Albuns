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

    pub fn request_id(&self) -> &str {
        match self {
            Self::Completed { request_id, .. } | Self::CacheCompleted { request_id, .. } => {
                request_id
            }
        }
    }
}
