use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GenerationOptions {
    pub source_folder: String,
    pub destination_folder: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GenerationDecision {
    Replace,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GenerationItemStatus {
    Pending,
    Completed,
    Ignored,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum GenerationPhase {
    Prepared,
    Running,
    Finished,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GenerationItemView {
    pub id: String,
    pub name: String,
    pub destination: String,
    pub status: GenerationItemStatus,
    pub problems: Vec<String>,
    pub conflict: bool,
    pub can_replace: bool,
    pub decision: Option<GenerationDecision>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GenerationView {
    pub id: String,
    pub options: GenerationOptions,
    pub phase: GenerationPhase,
    pub items: Vec<GenerationItemView>,
    pub can_continue: bool,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GenerationProgress {
    pub completed: u32,
    pub total: u32,
}
