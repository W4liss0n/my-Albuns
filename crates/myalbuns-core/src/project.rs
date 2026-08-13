use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::composition::build_render_snapshot;
use crate::model::{
    AlbumSnapshot, CoreError, DocumentSnapshot, EditorState, PROJECT_DOCUMENT_SCHEMA_VERSION,
    RenderSnapshot,
};
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedProjectHeader {
    schema_version: u32,
}

pub(crate) fn serialize_persisted_revision(state: &EditorState) -> Result<String, CoreError> {
    serde_json::to_string_pretty(&PersistedProject {
        schema_version: PROJECT_DOCUMENT_SCHEMA_VERSION,
        project_id: state.project_id.clone(),
        project_name: state.project_name.clone(),
        revision: state.revision,
        album: state.album.clone(),
    })
    .map_err(|error| CoreError::InvalidProject(error.to_string()))
}

#[derive(Clone, Default)]
pub struct ProjectCore {
    open_projects: Arc<Mutex<HashSet<String>>>,
    pub(crate) identity_lease_root: Option<PathBuf>,
    pub(crate) identity_registry_root: Option<PathBuf>,
}

pub struct EditableProject {
    session: ProjectSession,
    _registration: EditableRegistration,
}

struct EditableRegistration {
    project_id: String,
    open_projects: Arc<Mutex<HashSet<String>>>,
}

impl ProjectCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the independent roots for live identity ownership and the
    /// durable identity-to-location registry used by create/open operations.
    /// Read-only loading does not require either root.
    pub fn with_identity_storage_roots(
        mut self,
        identity_lease_root: PathBuf,
        identity_registry_root: PathBuf,
    ) -> Self {
        self.identity_lease_root = Some(identity_lease_root);
        self.identity_registry_root = Some(identity_registry_root);
        self
    }

    pub(crate) fn identity_lease_root(&self) -> Option<&Path> {
        self.identity_lease_root.as_deref()
    }

    pub(crate) fn identity_registry_root(&self) -> Option<&Path> {
        self.identity_registry_root.as_deref()
    }

    pub fn open_demo_editable_session(&self, source: &str) -> Result<EditableProject, CoreError> {
        let project = parse_persisted_project(source)?;
        let project_id = project.project_id.clone();
        let mut open_projects = self
            .open_projects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !open_projects.insert(project_id.clone()) {
            return Err(CoreError::EditableSessionAlreadyOpen { project_id });
        }
        drop(open_projects);

        Ok(EditableProject {
            session: ProjectSession::from_state(EditorState {
                project_id: project.project_id,
                project_name: project.project_name,
                document: DocumentSnapshot::neutral(),
                album: project.album,
                revision: project.revision,
                saved_revision: project.revision,
                dirty: false,
                can_undo: false,
                can_redo: false,
            }),
            _registration: EditableRegistration {
                project_id,
                open_projects: Arc::clone(&self.open_projects),
            },
        })
    }

    pub fn load_demo_persisted_revision(
        &self,
        source: &str,
    ) -> Result<LoadedProjectRevision, CoreError> {
        let project = parse_persisted_project(source)?;
        Ok(LoadedProjectRevision { project })
    }
}

impl EditableProject {
    pub fn state(&self) -> EditorState {
        self.session.state()
    }

    pub fn apply(&mut self, intent: crate::model::ProjectIntent) -> Result<EditorState, CoreError> {
        self.session.apply(intent)
    }

    pub fn undo(&mut self) -> Option<EditorState> {
        self.session.undo()
    }

    pub fn redo(&mut self) -> Option<EditorState> {
        self.session.redo()
    }

    pub fn projection(&self) -> crate::model::EditorProjection {
        self.session.projection()
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        self.session.render_snapshot()
    }

    pub fn persisted_revision(&self) -> Result<String, CoreError> {
        self.session.persisted_revision()
    }

    pub fn confirm_saved_revision(&mut self, revision: u64) -> Result<EditorState, CoreError> {
        self.session.confirm_saved_revision(revision)
    }
}

impl Drop for EditableRegistration {
    fn drop(&mut self) {
        self.open_projects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.project_id);
    }
}

fn parse_persisted_project(source: &str) -> Result<PersistedProject, CoreError> {
    let header: PersistedProjectHeader = serde_json::from_str(source)
        .map_err(|error| CoreError::InvalidProject(error.to_string()))?;
    if header.schema_version != PROJECT_DOCUMENT_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema(header.schema_version));
    }

    let project: PersistedProject = serde_json::from_str(source)
        .map_err(|error| CoreError::InvalidProject(error.to_string()))?;
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
            DocumentSnapshot::neutral().dpi,
            &self.project.album,
        )
    }
}
