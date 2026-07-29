use std::path::PathBuf;

use myalbuns_core::RenderSnapshot;
use serde::{Deserialize, Serialize};

pub const IMAGING_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagingRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub output_path: PathBuf,
    pub snapshot: RenderSnapshot,
}

impl ImagingRequest {
    pub fn new(
        request_id: impl Into<String>,
        output_path: PathBuf,
        snapshot: RenderSnapshot,
    ) -> Self {
        Self {
            protocol_version: IMAGING_PROTOCOL_VERSION,
            request_id: request_id.into(),
            output_path,
            snapshot,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImagingResponse {
    Completed {
        request_id: String,
        width_px: u32,
        height_px: u32,
    },
}

impl ImagingResponse {
    pub fn completed(request_id: impl Into<String>, width_px: u32, height_px: u32) -> Self {
        Self::Completed {
            request_id: request_id.into(),
            width_px,
            height_px,
        }
    }

    pub fn completed_dimensions_for(&self, expected_request_id: &str) -> Option<(u32, u32)> {
        match self {
            Self::Completed {
                request_id,
                width_px,
                height_px,
            } if request_id == expected_request_id => Some((*width_px, *height_px)),
            _ => None,
        }
    }
}
