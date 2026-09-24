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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ViewerCorrectionPhase {
    Browse,
    Select,
    Processing,
    Preview,
    Applying,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ViewerCorrectionActionKind {
    Start,
    Browse,
    Select,
    Preview,
    Apply,
    Cancel,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
pub struct ViewerFacePoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct ViewerFace(pub Vec<ViewerFacePoint>);

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ViewerCorrectionAction {
    pub session_id: String,
    pub kind: ViewerCorrectionActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference_media_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_face: Option<ViewerFace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference_face: Option<ViewerFace>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PreparedEyeCorrection {
    pub token: String,
    pub url: String,
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
    pub(crate) phase: ViewerCorrectionPhase,
    pub(crate) reference_media_id: String,
    pub(crate) reference_name: String,
    pub(crate) reference_url: Option<String>,
    pub(crate) reference_state: ViewerPreviewState,
    pub(crate) can_previous_reference: bool,
    pub(crate) can_next_reference: bool,
    pub(crate) result_url: Option<String>,
    pub(crate) error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_wire_contract_keeps_actions_phases_and_point_arrays() {
        let action = serde_json::json!({
            "sessionId": "viewer-1", "kind": "preview", "referenceMediaId": "reference-1",
            "targetUrl": "target-version", "referenceUrl": "reference-version",
            "targetFace": [{"x": 0.25, "y": 0.5, "z": -0.125}],
            "referenceFace": [{"x": 0.75, "y": 0.5, "z": 0.125}]
        });
        let decoded: ViewerCorrectionAction = serde_json::from_value(action.clone()).unwrap();
        assert_eq!(decoded.kind, ViewerCorrectionActionKind::Preview);
        assert_eq!(serde_json::to_value(decoded).unwrap(), action);
        for phase in ["browse", "select", "processing", "preview", "applying"] {
            let decoded: ViewerCorrectionPhase =
                serde_json::from_str(&format!("\"{phase}\"")).unwrap();
            assert_eq!(
                serde_json::to_string(&decoded).unwrap(),
                format!("\"{phase}\"")
            );
        }
        assert!(serde_json::from_str::<ViewerCorrectionPhase>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<ViewerCorrectionActionKind>("\"unknown\"").is_err());
        assert_eq!(
            serde_json::to_value(PreparedEyeCorrection {
                token: "token".into(),
                url: "opaque".into()
            })
            .unwrap(),
            serde_json::json!({"token":"token","url":"opaque"})
        );
    }
}
