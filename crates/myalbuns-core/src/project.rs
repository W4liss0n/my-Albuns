use serde::{Deserialize, Serialize};

use crate::composition::build_render_snapshot;
use crate::model::{AlbumSnapshot, CoreError, EditorState, PROJECT_SCHEMA_VERSION, RenderSnapshot};
use crate::sample_project::sample_editor_state;
use crate::session::ProjectSession;
use crate::validation::validate_album;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedProject {
    pub(crate) schema_version: u32,
    pub(crate) project_id: String,
    pub(crate) project_name: String,
    pub(crate) revision: u64,
    pub(crate) album: AlbumSnapshot,
}

pub(crate) fn serialize_persisted_revision(state: &EditorState) -> Result<String, CoreError> {
    serde_json::to_string_pretty(&PersistedProject {
        schema_version: PROJECT_SCHEMA_VERSION,
        project_id: state.project_id.clone(),
        project_name: state.project_name.clone(),
        revision: state.revision,
        album: state.album.clone(),
    })
    .map_err(|error| CoreError::InvalidProject(error.to_string()))
}

pub struct ProjectCore;

impl ProjectCore {
    pub fn open_sample_project(sheet_count: usize) -> ProjectSession {
        ProjectSession::from_state(sample_editor_state(sheet_count))
    }

    pub fn load_persisted_revision(source: &str) -> Result<LoadedProjectRevision, CoreError> {
        let project: PersistedProject = serde_json::from_str(source)
            .map_err(|error| CoreError::InvalidProject(error.to_string()))?;
        if project.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(CoreError::UnsupportedSchema(project.schema_version));
        }
        if project.project_id.trim().is_empty() {
            return Err(CoreError::InvalidProject(
                "a Identidade do Projeto está vazia".into(),
            ));
        }
        validate_album(&project.album)?;

        Ok(LoadedProjectRevision { project })
    }
}

pub struct LoadedProjectRevision {
    project: PersistedProject,
}

impl LoadedProjectRevision {
    pub fn revision(&self) -> u64 {
        self.project.revision
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        build_render_snapshot(
            &self.project.project_id,
            &self.project.project_name,
            self.project.revision,
            &self.project.album,
        )
    }
}
