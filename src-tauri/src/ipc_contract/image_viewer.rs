use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ViewerPreviewState {
    Loading,
    Ready,
    Absent,
    Unavailable,
    CacheUnavailable,
    CachePaused,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewerPresentation {
    pub(crate) session_id: String,
    #[ts(type = "number")]
    pub(crate) revision: u64,
    pub(crate) media_id: String,
    pub(crate) name: String,
    pub(crate) url: Option<String>,
    pub(crate) state: ViewerPreviewState,
    pub(crate) can_previous: bool,
    pub(crate) can_next: bool,
    #[serde(default)]
    #[ts(optional)]
    pub(crate) correction: Option<ViewerCorrectionPresentation>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewerAction {
    pub(crate) session_id: String,
    pub(crate) offset: i8,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewerCorrectionPresentation {
    pub(crate) phase: String,
    pub(crate) reference_media_id: String,
    pub(crate) reference_name: String,
    pub(crate) reference_url: Option<String>,
    pub(crate) reference_state: ViewerPreviewState,
    pub(crate) can_previous_reference: bool,
    pub(crate) can_next_reference: bool,
    pub(crate) result_url: Option<String>,
    pub(crate) error: Option<String>,
}
