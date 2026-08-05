use myalbuns_core::EditorProjection;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum SaveProjectOutcome {
    Saved { revision: u64 },
    AlreadyCurrent { revision: u64 },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectResult {
    pub(crate) outcome: SaveProjectOutcome,
    #[ts(type = "import(\"../../domain/project\").EditorProjection")]
    pub(crate) projection: EditorProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "code")]
pub enum SaveProjectCommandError {
    StaleRevision {
        expected_revision: u64,
        current_revision: u64,
    },
    PersistedBaselineConflict,
    NotFound,
    Unavailable,
    AccessDenied,
    InvalidPath,
    UnexpectedObjectType,
    Conflict,
    IoFailure,
    SaveStateIndeterminate,
    SessionUnavailable,
}

#[cfg(test)]
mod save_contract_tests {
    use serde_json::json;

    use super::{SaveProjectCommandError, SaveProjectOutcome};

    #[test]
    fn save_outcomes_use_a_camel_case_kind_and_structured_revision() {
        assert_eq!(
            serde_json::to_value(SaveProjectOutcome::Saved { revision: 7 })
                .expect("the Saved outcome serializes"),
            json!({ "kind": "saved", "revision": 7 })
        );
        assert_eq!(
            serde_json::to_value(SaveProjectOutcome::AlreadyCurrent { revision: 7 })
                .expect("the AlreadyCurrent outcome serializes"),
            json!({ "kind": "alreadyCurrent", "revision": 7 })
        );
    }

    #[test]
    fn save_errors_carry_stable_codes_and_context_without_a_localized_message() {
        let serialized = serde_json::to_value(SaveProjectCommandError::StaleRevision {
            expected_revision: 3,
            current_revision: 4,
        })
        .expect("the stale-revision error serializes");

        assert_eq!(
            serialized,
            json!({
                "code": "stale_revision",
                "expectedRevision": 3,
                "currentRevision": 4
            })
        );
        assert!(serialized.get("message").is_none());
    }
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
