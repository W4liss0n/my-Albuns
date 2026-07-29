use serde::{Deserialize, Serialize};

use crate::composition::build_render_snapshot;
use crate::model::{AlbumSnapshot, CoreError, EditorState, PROJECT_SCHEMA_VERSION, RenderSnapshot};
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
    pub fn open_editable_session(source: &str) -> Result<ProjectSession, CoreError> {
        let project = parse_persisted_project(source)?;
        Ok(ProjectSession::from_state(EditorState {
            project_id: project.project_id,
            project_name: project.project_name,
            album: project.album,
            revision: project.revision,
            saved_revision: project.revision,
            dirty: false,
            can_undo: false,
            can_redo: false,
        }))
    }

    pub fn load_persisted_revision(source: &str) -> Result<LoadedProjectRevision, CoreError> {
        let project = parse_persisted_project(source)?;
        Ok(LoadedProjectRevision { project })
    }
}

fn parse_persisted_project(source: &str) -> Result<PersistedProject, CoreError> {
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
    Ok(project)
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
