use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportOptions {
    pub source_folder: String,
    pub destination_folder: Option<String>,
    pub format: myalbuns_core::ExportFormat,
    pub mode: myalbuns_core::ExportMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BatchItemStatus {
    Pending,
    Completed,
    Ignored,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BatchProblemKind {
    Placeholder,
    MissingMedia,
    Unavailable,
    InvalidProject,
    Changed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchProblem {
    pub kind: BatchProblemKind,
    pub message: String,
    pub media_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemView {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub destination: String,
    pub status: BatchItemStatus,
    pub problems: Vec<BatchProblem>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BatchPhase {
    Prepared,
    Running,
    Interrupted,
    StorageFull,
    Finished,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportView {
    pub id: String,
    pub options: BatchExportOptions,
    pub phase: BatchPhase,
    pub partial_publication: bool,
    pub items: Vec<BatchItemView>,
    pub has_conflicts: bool,
    pub can_continue: bool,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportProgress {
    pub completed: u32,
    pub total: u32,
    pub percent: f64,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BatchRecoverySummary {
    pub id: String,
    pub source_folder: String,
    pub total: u32,
    pub remaining: u32,
}
