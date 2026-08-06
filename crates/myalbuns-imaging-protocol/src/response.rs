use serde::{Deserialize, Serialize};

use crate::cache::CacheCompletion;
use crate::command::{ImagingFailure, ImagingFailureCode, ImagingPathCode};
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
    Failed {
        request_id: String,
        #[serde(flatten)]
        failure: ImagingFailure,
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

    pub fn failed<M: Into<String>>(
        request_id: impl Into<String>,
        code: ImagingFailureCode,
        media_id: Option<M>,
        path_code: Option<ImagingPathCode>,
    ) -> Self {
        Self::Failed {
            request_id: request_id.into(),
            failure: ImagingFailure {
                code,
                media_id: media_id.map(Into::into),
                path_code,
            },
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

    pub fn failure_for(&self, expected_request_id: &str) -> Option<ImagingFailure> {
        match self {
            Self::Failed {
                request_id,
                failure,
            } if request_id == expected_request_id => Some(failure.clone()),
            _ => None,
        }
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Completed { request_id, .. }
            | Self::CacheCompleted { request_id, .. }
            | Self::Failed { request_id, .. } => request_id,
        }
    }
}
