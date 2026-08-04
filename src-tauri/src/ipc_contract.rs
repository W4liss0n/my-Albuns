use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CancelDisposition {
    Requested,
    AlreadyRequested,
    TooLate,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportCommandErrorCode {
    Cancelled,
    Conflict,
    Failed,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportCommandError {
    pub(crate) code: ExportCommandErrorCode,
    pub(crate) message: String,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub(crate) output_path: String,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportProgressStagePayload {
    Preparing,
    LoadingSources,
    Composing,
    EncodingOutput,
    Verifying,
    Publishing,
    Completed,
}

#[derive(Clone, Copy, Debug, Serialize, TS)]
#[serde(
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(tag = "kind")]
pub enum ExportProgressUnitsPayload {
    Unmeasured,
    Measured {
        completed_units: u32,
        total_units: u32,
    },
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
#[ts(tag = "event", content = "data")]
pub enum ExportEvent {
    Started {
        operation_id: String,
        cancellable: bool,
    },
    Progress {
        operation_id: String,
        stage: ExportProgressStagePayload,
        units: ExportProgressUnitsPayload,
        cancellable: bool,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreview {
    pub(crate) media_id: String,
    pub(crate) url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MediaPreviewCommandErrorCode {
    Unavailable,
    UnsupportedImage,
    ReadFailed,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreviewCommandError {
    pub(crate) code: MediaPreviewCommandErrorCode,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FrontendLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(optional_fields)]
pub struct FrontendLogEvent {
    pub(crate) level: FrontendLogLevel,
    pub(crate) component: String,
    pub(crate) event: String,
    pub(crate) project_id: Option<String>,
    pub(crate) operation_id: Option<String>,
    pub(crate) instance_id: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) sheet_count: Option<usize>,
}
