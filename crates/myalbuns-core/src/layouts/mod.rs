use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::RectUm;

mod generator;
mod rules;

pub use generator::generate_layouts;
pub use rules::{LayoutPatch, LayoutRules};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FrameOrientation {
    Vertical,
    Horizontal,
    Square,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LayoutSurfaceKind {
    SinglePage,
    DoubleSheet,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSurface {
    #[serde(rename = "type")]
    pub kind: LayoutSurfaceKind,
    pub width_um: i64,
    pub height_um: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LayoutPermission {
    PagesOnly,
    #[default]
    PagesAndSheet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LayoutScope {
    Page,
    Sheet,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutParameters {
    pub margin_um: i64,
    pub gap_um: i64,
    pub minimum_side_um: i64,
}

impl LayoutParameters {
    pub(crate) fn is_valid(&self) -> bool {
        let max = crate::project_document::MAX_SAFE_INTEGER as i64;
        (0..=max).contains(&self.margin_um)
            && (0..=max).contains(&self.gap_um)
            && (1..=max).contains(&self.minimum_side_um)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSettings {
    pub permission: LayoutPermission,
    #[serde(flatten)]
    pub parameters: LayoutParameters,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSelection {
    pub query_id: String,
    pub candidate_index: usize,
}

/// An explicit request for future placeholder profiles; never inferred from Photos.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutExpansion {
    pub additional_positions: usize,
    pub orientation: FrameOrientation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutExportProblem {
    pub sheet_id: String,
    pub sheet_number: usize,
    pub frame_id: String,
    pub frame_number: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutQueryResult {
    pub query_id: String,
    pub project_id: String,
    pub revision: u64,
    pub sheet_id: String,
    pub frame_count: usize,
    pub locked: bool,
    pub settings: LayoutSettings,
    pub listing: LayoutListing,
}

impl Default for LayoutParameters {
    fn default() -> Self {
        Self {
            margin_um: 15_000,
            gap_um: 5_000,
            minimum_side_um: 20_000,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutQuery {
    pub surface: LayoutSurface,
    pub frame_orientations: Vec<FrameOrientation>,
    pub permission: LayoutPermission,
    #[serde(flatten)]
    pub parameters: LayoutParameters,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDefinition {
    pub surface: LayoutSurface,
    pub scope: LayoutScope,
    pub positions: Vec<RectUm>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LayoutOrigin {
    Automatic,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StoredLayout {
    pub definition: LayoutDefinition,
    pub origin: LayoutOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutCandidate {
    pub layout: StoredLayout,
    pub is_last_applied: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutListing {
    pub algorithm_version: u32,
    pub generation_status: LayoutGenerationStatus,
    pub candidates: Vec<LayoutCandidate>,
}

impl LayoutSurface {
    pub(crate) fn is_valid(&self) -> bool {
        let max = crate::project_document::MAX_SAFE_INTEGER as i64;
        (1..=max).contains(&self.width_um)
            && (1..=max).contains(&self.height_um)
            && (self.kind != LayoutSurfaceKind::DoubleSheet || self.width_um >= 2)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedLayout {
    pub definition: LayoutDefinition,
    pub family: String,
    pub quality: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LayoutGenerationStatus {
    Candidates,
    Empty,
    NoCandidates,
    OutsideCoverage,
    InvalidQuery,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayoutGeneration {
    pub algorithm_version: u32,
    pub status: LayoutGenerationStatus,
    pub candidates: Vec<GeneratedLayout>,
}
