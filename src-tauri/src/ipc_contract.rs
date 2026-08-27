use std::collections::BTreeMap;

use myalbuns_core::EditorProjection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind")]
pub enum ProjectDialogProgress {
    Indeterminate {
        status: String,
    },
    Determinate {
        completed: u64,
        status: String,
        total: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDialogDetail {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectDialogState {
    AlbumInformationConfirmation {
        busy: bool,
        details: Vec<ProjectDialogDetail>,
    },
    ProjectCloseConfirmation {
        busy: bool,
    },
    ProjectCloseFailure {
        message: String,
    },
    ProjectOperationFailure {
        message: String,
    },
    ExportProgress {
        cancel_requested: bool,
        cancellable: bool,
        progress: ProjectDialogProgress,
    },
    ExportFailure {
        cancelled: bool,
        message: String,
        retry_disabled: bool,
    },
    ExportSuccess {
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDialogPresentation {
    pub(crate) session_id: String,
    pub(crate) state: ProjectDialogState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectDialogAction {
    CancelAlbumInformation,
    CancelExport,
    CancelProjectClose,
    DiscardAndClose,
    ConfirmAlbumInformation,
    DismissExport,
    DismissProjectCloseFailure,
    DismissProjectOperationFailure,
    RetryExport,
    SaveAndClose,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDialogActionEvent {
    pub(crate) action: ProjectDialogAction,
    pub(crate) session_id: String,
}

#[cfg(test)]
mod project_dialog_contract_tests {
    use serde_json::json;

    use super::{
        ProjectDialogAction, ProjectDialogActionEvent, ProjectDialogDetail,
        ProjectDialogPresentation, ProjectDialogProgress, ProjectDialogState,
    };

    #[test]
    fn every_project_dialog_action_uses_its_stable_camel_case_wire_value() {
        let cases = [
            (
                ProjectDialogAction::CancelAlbumInformation,
                "cancelAlbumInformation",
            ),
            (ProjectDialogAction::CancelExport, "cancelExport"),
            (
                ProjectDialogAction::CancelProjectClose,
                "cancelProjectClose",
            ),
            (ProjectDialogAction::DiscardAndClose, "discardAndClose"),
            (
                ProjectDialogAction::ConfirmAlbumInformation,
                "confirmAlbumInformation",
            ),
            (ProjectDialogAction::DismissExport, "dismissExport"),
            (
                ProjectDialogAction::DismissProjectCloseFailure,
                "dismissProjectCloseFailure",
            ),
            (
                ProjectDialogAction::DismissProjectOperationFailure,
                "dismissProjectOperationFailure",
            ),
            (ProjectDialogAction::RetryExport, "retryExport"),
            (ProjectDialogAction::SaveAndClose, "saveAndClose"),
        ];

        for (action, expected) in cases {
            let encoded = serde_json::to_value(action).expect("dialog action serializes");
            assert_eq!(encoded, json!(expected));
            assert_eq!(
                serde_json::from_value::<ProjectDialogAction>(encoded)
                    .expect("dialog action deserializes"),
                action
            );
        }
    }

    #[test]
    fn every_project_dialog_state_round_trips_through_its_discriminated_union() {
        let states = [
            ProjectDialogState::AlbumInformationConfirmation {
                busy: false,
                details: vec![ProjectDialogDetail {
                    label: "DPI".into(),
                    value: "300 → 240".into(),
                }],
            },
            ProjectDialogState::ProjectCloseConfirmation { busy: true },
            ProjectDialogState::ProjectCloseFailure {
                message: "Falha ao fechar".into(),
            },
            ProjectDialogState::ProjectOperationFailure {
                message: "Falha ao salvar".into(),
            },
            ProjectDialogState::ExportProgress {
                cancel_requested: false,
                cancellable: true,
                progress: ProjectDialogProgress::Determinate {
                    completed: 2,
                    status: "Exportando".into(),
                    total: 5,
                },
            },
            ProjectDialogState::ExportFailure {
                cancelled: false,
                message: "Falha ao exportar".into(),
                retry_disabled: false,
            },
            ProjectDialogState::ExportSuccess {
                message: "Exportação concluída".into(),
            },
        ];
        let expected_kinds = [
            "albumInformationConfirmation",
            "projectCloseConfirmation",
            "projectCloseFailure",
            "projectOperationFailure",
            "exportProgress",
            "exportFailure",
            "exportSuccess",
        ];

        for (state, expected_kind) in states.into_iter().zip(expected_kinds) {
            let encoded = serde_json::to_value(&state).expect("dialog state serializes");
            assert_eq!(encoded["kind"], json!(expected_kind));
            assert_eq!(
                serde_json::from_value::<ProjectDialogState>(encoded)
                    .expect("dialog state deserializes"),
                state
            );
        }
    }

    #[test]
    fn dialog_actions_keep_the_logical_owner_on_the_wire() {
        assert_eq!(
            serde_json::to_value(ProjectDialogActionEvent {
                action: ProjectDialogAction::ConfirmAlbumInformation,
                session_id: "album-information-7".into(),
            })
            .expect("the owned action serializes"),
            json!({
                "action": "confirmAlbumInformation",
                "sessionId": "album-information-7"
            })
        );
    }

    #[test]
    fn dialog_presentations_keep_owner_and_state_atomic_on_the_wire() {
        assert_eq!(
            serde_json::to_value(ProjectDialogPresentation {
                session_id: "export-8".into(),
                state: ProjectDialogState::ExportSuccess {
                    message: "Exportação concluída".into(),
                },
            })
            .expect("the owned presentation serializes"),
            json!({
                "sessionId": "export-8",
                "state": {
                    "kind": "exportSuccess",
                    "message": "Exportação concluída"
                }
            })
        );
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreferences {
    pub(crate) inspector_sections: BTreeMap<String, bool>,
    pub(crate) media_thumbnail_sizes: MediaThumbnailSizes,
    pub(crate) workspace_panels: WorkspacePanelPreferences,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaThumbnailSizes {
    pub(crate) decorative: u16,
    pub(crate) photo: u16,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePanelPreferences {
    pub(crate) inspector: Option<WorkspacePanelPreference>,
    pub(crate) media: Option<WorkspacePanelPreference>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePanelPreference {
    pub(crate) size: u16,
    pub(crate) visible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum WorkspacePreferenceChange {
    InspectorSection {
        preference_key: String,
        open: bool,
    },
    MediaThumbnailSize {
        media_kind: MediaPreferenceKind,
        size: u16,
    },
    WorkspacePanelSize {
        panel: WorkspacePanelKind,
        size: u16,
    },
    WorkspacePanelVisibility {
        panel: WorkspacePanelKind,
        visible: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaPreferenceKind {
    Decorative,
    Photo,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum WorkspacePanelKind {
    Inspector,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSettings {
    pub(crate) media_panel: MediaPanelSettings,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPanelSettings {
    pub(crate) decorative: MediaPanelTabSettings,
    pub(crate) photo: MediaPanelTabSettings,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaPanelTabSettings {
    pub(crate) sort_direction: MediaSortDirection,
    pub(crate) usage_filter: MediaUsageFilter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaSortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaUsageFilter {
    All,
    Used,
    Unused,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum SettingsPreferenceChange {
    MediaPanelSortDirection {
        media_kind: MediaPreferenceKind,
        sort_direction: MediaSortDirection,
    },
    MediaPanelUsageFilter {
        media_kind: MediaPreferenceKind,
        usage_filter: MediaUsageFilter,
    },
}

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
    CacheUnavailable,
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

#[derive(Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ImportPhotoResult {
    Cancelled {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: EditorProjection,
    },
    Imported {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: EditorProjection,
        media_id: String,
    },
    Selected {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: EditorProjection,
        media_id: String,
    },
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum SaveAsProjectOutcome {
    Cancelled,
    SavedAs {
        previous_project_id: String,
        project_id: String,
        revision: u64,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveAsProjectResult {
    pub(crate) outcome: SaveAsProjectOutcome,
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
pub enum SaveAsProjectCommandError {
    StaleRevision {
        expected_revision: u64,
        current_revision: u64,
    },
    SameTarget,
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    NotFound,
    Unavailable,
    AccessDenied,
    InvalidPath,
    UnexpectedObjectType,
    Conflict,
    IoFailure,
    SaveAsStateIndeterminate,
    SessionUnavailable,
    DialogUnavailable,
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
    RecoveryCleanupFailed,
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
mod save_as_contract_tests {
    use serde_json::json;

    use super::{SaveAsProjectCommandError, SaveAsProjectOutcome};

    #[test]
    fn save_as_outcomes_keep_cancellation_and_adopted_identity_explicit() {
        assert_eq!(
            serde_json::to_value(SaveAsProjectOutcome::Cancelled)
                .expect("the cancelled outcome serializes"),
            json!({ "kind": "cancelled" })
        );
        assert_eq!(
            serde_json::to_value(SaveAsProjectOutcome::SavedAs {
                previous_project_id: "4b594571-6b51-4cad-a37c-8fd8cedb7dd2".into(),
                project_id: "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5".into(),
                revision: 7,
            })
            .expect("the SavedAs outcome serializes"),
            json!({
                "kind": "savedAs",
                "previousProjectId": "4b594571-6b51-4cad-a37c-8fd8cedb7dd2",
                "projectId": "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5",
                "revision": 7
            })
        );
    }

    #[test]
    fn save_as_errors_are_stable_structured_data_without_messages() {
        let serialized = serde_json::to_value(SaveAsProjectCommandError::StaleRevision {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind")]
pub enum ProjectRecoveryStatus {
    None,
    Available,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRecoveryDecision {
    ReopenAndRecover,
    DiscardCheckpointAndOpenLastSaved,
    NowNot,
}

#[derive(Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectRecoveryResolution {
    Recovered {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: Box<EditorProjection>,
    },
    OpenedLastSaved {
        #[ts(type = "import(\"../../domain/project\").EditorProjection")]
        projection: Box<EditorProjection>,
    },
    Deferred,
}

#[cfg(test)]
mod recovery_contract_tests {
    use serde_json::json;

    use super::{ProjectRecoveryDecision, ProjectRecoveryResolution, ProjectRecoveryStatus};

    #[test]
    fn recovery_status_and_decisions_are_closed_and_stable() {
        assert_eq!(
            serde_json::to_value(ProjectRecoveryStatus::Available)
                .expect("the available status serializes"),
            json!({ "kind": "available" })
        );
        assert_eq!(
            serde_json::from_value::<ProjectRecoveryDecision>(json!("reopenAndRecover"))
                .expect("the recover decision deserializes"),
            ProjectRecoveryDecision::ReopenAndRecover
        );
        assert_eq!(
            serde_json::from_value::<ProjectRecoveryDecision>(json!(
                "discardCheckpointAndOpenLastSaved"
            ))
            .expect("the confirmed discard decision deserializes"),
            ProjectRecoveryDecision::DiscardCheckpointAndOpenLastSaved
        );
        assert_eq!(
            serde_json::from_value::<ProjectRecoveryDecision>(json!("nowNot"))
                .expect("the defer decision deserializes"),
            ProjectRecoveryDecision::NowNot
        );
        assert!(serde_json::from_value::<ProjectRecoveryDecision>(json!("openLastSaved")).is_err());
        assert!(
            serde_json::from_value::<ProjectRecoveryDecision>(json!({
                "choice": "openLastSaved",
                "checkpointDiscardConfirmed": true
            }))
            .is_err()
        );
    }

    #[test]
    fn deferred_recovery_has_no_creative_payload() {
        assert_eq!(
            serde_json::to_value(ProjectRecoveryResolution::Deferred)
                .expect("the deferred resolution serializes"),
            json!({ "kind": "deferred" })
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
