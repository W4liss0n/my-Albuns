use crate::project_document::{ProjectDocument, ProjectRevision};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct PersistentProjectSession {
    current: ProjectRevision,
    saved_revision: u64,
    undo: Vec<ProjectRevision>,
    redo: Vec<ProjectRevision>,
}

impl PersistentProjectSession {
    pub(crate) fn from_persisted(current: ProjectRevision) -> Self {
        let saved_revision = current.revision;
        Self {
            current,
            saved_revision,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub(crate) fn project_id(&self) -> Uuid {
        self.current.project_id
    }

    pub(crate) fn revision(&self) -> u64 {
        self.current.revision
    }

    pub(crate) fn saved_revision(&self) -> u64 {
        self.saved_revision
    }

    pub(crate) fn project(&self) -> &ProjectDocument {
        &self.current.project
    }

    pub(crate) fn has_unsaved_changes(&self) -> bool {
        self.current.revision != self.saved_revision
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
