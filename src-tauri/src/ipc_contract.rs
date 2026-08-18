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
    ExportConflict,
    PublicationFailed,
    InvalidRenderRequest,
    SourceUnavailable,
    UnsupportedSourceFormat,
    UnsupportedSourceVariant,
    UnsupportedColorModel,
    UnsupportedColorProfile,
    DecodeFailed,
    CompositionFailed,
    ResourceLimitExceeded,
    EncodeFailed,
    VerificationFailed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExportPathCode {
    NotFound,
    Unavailable,
    AccessDenied,
    InvalidPath,
    UnexpectedObjectType,
    Conflict,
    IoFailure,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields)]
pub struct ExportCommandError {
    pub(crate) code: ExportCommandErrorCode,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) media_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path_code: Option<ExportPathCode>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MediaPreviewState {
    Ready,
    Absent,
    Unavailable,
}

#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreviewDemand {
    pub(crate) revision: u64,
    pub(crate) visible_media_ids: Vec<String>,
    pub(crate) preload_media_ids: Vec<String>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreview {
    pub(crate) media_id: String,
    pub(crate) state: MediaPreviewState,
    pub(crate) url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkedMediaChanged {
    pub(crate) media_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CacheProcessorState {
    Suspended,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheProcessorWarning {
    pub(crate) state: CacheProcessorState,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheServiceStatus {
    pub(crate) occupied_bytes: u64,
    pub(crate) releasable_bytes: u64,
    pub(crate) namespace_count: usize,
    pub(crate) releasable_namespace_count: usize,
    pub(crate) clear_all_scheduled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheFreeResult {
    pub(crate) measured_releasable_bytes: u64,
    pub(crate) freed_bytes: u64,
    pub(crate) removed_namespace_count: usize,
    pub(crate) skipped_active_namespace_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum CacheClearAllOutcome {
    Cleared { result: CacheFreeResult },
    Scheduled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CacheServiceCommandErrorCode {
    Busy,
    StorageUnavailable,
    ReservationUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheServiceCommandError {
    pub(crate) code: CacheServiceCommandErrorCode,
    pub(crate) message: String,
}

#[cfg(test)]
mod media_change_contract_tests {
    use serde_json::json;

    use super::{CacheProcessorState, CacheProcessorWarning, LinkedMediaChanged};

    #[test]
    fn stable_media_change_event_exposes_only_opaque_media_identities() {
        let event = LinkedMediaChanged {
            media_ids: vec!["photo-a".into(), "overlay-a".into()],
        };

        assert_eq!(
            serde_json::to_value(event).expect("the event serializes"),
            json!({ "mediaIds": ["photo-a", "overlay-a"] })
        );
    }

    #[test]
    fn cache_processor_warning_is_typed_and_does_not_block_project_commands() {
        let warning = CacheProcessorWarning {
            state: CacheProcessorState::Suspended,
            message: "O Cache foi suspenso.".into(),
        };

        assert_eq!(
            serde_json::to_value(warning).expect("the warning serializes"),
            json!({
                "state": "suspended",
                "message": "O Cache foi suspenso."
            })
        );
    }
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectCloseChoice {
    SaveAndClose,
    DiscardAndClose,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind")]
pub enum ProjectCloseRequestOutcome {
    Closed,
    ConfirmationRequired,
}

#[derive(Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectCloseResolution {
    Closed,
    Cancelled {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: Box<EditorProjection>,
    },
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

#[cfg(test)]
mod close_contract_tests {
    use serde_json::json;

    use super::{ProjectCloseRequestOutcome, ProjectCloseResolution};

    #[test]
    fn close_outcomes_keep_the_decision_and_cancel_projection_explicit() {
        assert_eq!(
            serde_json::to_value(ProjectCloseRequestOutcome::ConfirmationRequired)
                .expect("the request outcome serializes"),
            json!({ "kind": "confirmationRequired" })
        );
        assert_eq!(
            serde_json::to_value(ProjectCloseRequestOutcome::Closed)
                .expect("the closed outcome serializes"),
            json!({ "kind": "closed" })
        );
    }

    #[test]
    fn a_closed_resolution_has_no_creative_payload() {
        assert_eq!(
            serde_json::to_value(ProjectCloseResolution::Closed)
                .expect("the resolution serializes"),
            json!({ "kind": "closed" })
        );
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
