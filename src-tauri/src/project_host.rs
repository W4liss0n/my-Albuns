use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use myalbuns_core::{
    EditableProject, EditorProjection, ProjectIntent, RenderSnapshot, SaveProjectError,
    SaveProjectOutcome,
};
use myalbuns_imaging_protocol::MediaSource;

const SESSION_UNAVAILABLE_MESSAGE: &str = "A Sessão do Projeto ficou indisponível.";

/// Owns the single productive editable Project of this Host process.
///
/// There is deliberately no window/session selector: one process owns one
/// Project, and the operating-system process lifetime owns its locks.
#[derive(Clone)]
pub(crate) struct ProjectHost {
    project: Arc<Mutex<Option<EditableProject>>>,
}

pub(crate) struct ProjectHostSaveResult {
    pub(crate) outcome: SaveProjectOutcome,
    pub(crate) projection: EditorProjection,
}

#[derive(Debug)]
pub(crate) enum ProjectHostSaveError {
    Project(SaveProjectError),
    SessionUnavailable,
}

impl ProjectHost {
    pub(crate) fn new(project: EditableProject) -> Self {
        Self {
            project: Arc::new(Mutex::new(Some(project))),
        }
    }

    pub(crate) fn projection(&self) -> Result<EditorProjection, String> {
        Ok(self.project()?.projection())
    }

    pub(crate) fn apply(&self, intent: ProjectIntent) -> Result<EditorProjection, String> {
        self.project()?
            .apply(intent)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn undo(&self) -> Result<EditorProjection, String> {
        self.project()?
            .undo()
            .ok_or_else(|| "Não há uma ação produtiva para desfazer neste corte.".into())
    }

    pub(crate) fn redo(&self) -> Result<EditorProjection, String> {
        self.project()?
            .redo()
            .ok_or_else(|| "Não há uma ação produtiva para refazer neste corte.".into())
    }

    pub(crate) fn save(
        &self,
        expected_revision: u64,
    ) -> Result<ProjectHostSaveResult, ProjectHostSaveError> {
        let mut slot = self
            .project
            .lock()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        let result = slot
            .as_mut()
            .ok_or(ProjectHostSaveError::SessionUnavailable)?
            .save(expected_revision);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(SaveProjectError::SaveStateIndeterminate) => {
                slot.take();
                return Err(ProjectHostSaveError::Project(
                    SaveProjectError::SaveStateIndeterminate,
                ));
            }
            Err(error) => return Err(ProjectHostSaveError::Project(error)),
        };
        let projection = slot
            .as_ref()
            .ok_or(ProjectHostSaveError::SessionUnavailable)?
            .projection();
        Ok(ProjectHostSaveResult {
            outcome,
            projection,
        })
    }

    pub(crate) fn render_snapshot(&self) -> Result<RenderSnapshot, String> {
        Ok(self.project()?.render_snapshot())
    }

    pub(crate) fn linked_media_sources(&self) -> Result<Vec<(String, PathBuf)>, String> {
        Ok(self
            .project()?
            .project()
            .media()
            .iter()
            .map(|media| {
                (
                    media.id().hyphenated().to_string(),
                    media.path().to_path_buf(),
                )
            })
            .collect())
    }

    pub(crate) fn export_sources(
        &self,
        snapshot: &RenderSnapshot,
        sheet_id: &str,
    ) -> Result<Vec<MediaSource>, String> {
        let sheet = snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .ok_or_else(|| "A Lâmina solicitada não existe no snapshot.".to_string())?;
        if sheet.referenced_media_ids().next().is_none() {
            return Ok(Vec::new());
        }
        Err("As fontes originais das mídias ainda não estão disponíveis neste corte.".into())
    }

    fn project(&self) -> Result<ActiveProject<'_>, String> {
        let guard = self
            .project
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        if guard.is_none() {
            return Err(SESSION_UNAVAILABLE_MESSAGE.to_string());
        }
        Ok(ActiveProject { guard })
    }
}

struct ActiveProject<'a> {
    guard: MutexGuard<'a, Option<EditableProject>>,
}

impl std::ops::Deref for ActiveProject<'_> {
    type Target = EditableProject;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("an ActiveProject guard always contains its Project")
    }
}

impl std::ops::DerefMut for ActiveProject<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("an ActiveProject guard always contains its Project")
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialBackground, InitialBackgroundContent,
        InitialFrameBorder, InitialOverlay, InitialProject, InitialProjectPersonalization,
        OpenProjectRequest, ProjectCore, ProjectIntent, ProjectLocation, SaveProjectOutcome,
    };
    use myalbuns_paths::OperationPathContext;

    use super::ProjectHost;

    struct Fixture {
        _root: tempfile::TempDir,
        project_path: PathBuf,
        identity_lease_root: PathBuf,
        host: ProjectHost,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().expect("temporary Project Host fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let identity_lease_root = root.path().join("leases");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the fixture root is captured");
        let project = ProjectCore::new()
            .with_identity_lease_root(identity_lease_root.clone())
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the productive Project is created");
        Fixture {
            _root: root,
            project_path,
            identity_lease_root,
            host: ProjectHost::new(project),
        }
    }

    fn open_project(project_path: &Path, identity_lease_root: &Path) -> ProjectHost {
        let mut context = OperationPathContext::new();
        context
            .capture(project_path)
            .expect("the reopened fixture root is captured");
        let project = ProjectCore::new()
            .with_identity_lease_root(identity_lease_root.to_path_buf())
            .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                project_path.to_path_buf(),
                context.freeze(),
            )))
            .expect("the saved Project reopens in a new editable Session");
        ProjectHost::new(project)
    }

    #[test]
    fn owns_one_productive_project_without_demo_content_or_a_window_selector() {
        let fixture = fixture();
        let projection = fixture
            .host
            .projection()
            .expect("the Project remains available");

        assert_eq!(projection.state.project_name, "Projeto");
        assert_eq!(projection.state.revision, 0);
        assert_eq!(projection.composition.sheets.len(), 2);
        assert!(projection.state.album.media.is_empty());
        assert!(
            projection
                .state
                .album
                .sheets
                .iter()
                .all(|sheet| sheet.frames.is_empty())
        );
    }

    #[test]
    fn productive_projection_yields_a_valid_neutral_render_snapshot() {
        let fixture = fixture();
        let snapshot = fixture
            .host
            .render_snapshot()
            .expect("the neutral snapshot is available");

        assert!(snapshot.validate().is_ok());
        assert_eq!(
            fixture
                .host
                .export_sources(&snapshot, &snapshot.composition.sheets[0].sheet_id)
                .expect("a neutral sheet needs no original sources"),
            Vec::new()
        );
    }

    #[test]
    fn delegates_dpi_changes_and_history_to_the_productive_editable_project() {
        let fixture = fixture();

        let applied = fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the productive Host applies the DPI change");
        assert_eq!(applied.state.document.dpi, 240);
        assert_eq!(applied.state.revision, 1);
        assert!(applied.state.dirty);
        assert!(applied.state.can_undo);
        assert!(!applied.state.can_redo);

        let undone = fixture
            .host
            .undo()
            .expect("the productive Host undoes the DPI change");
        assert_eq!(undone.state.document.dpi, 300);
        assert_eq!(undone.state.revision, 0);
        assert!(!undone.state.dirty);
        assert!(!undone.state.can_undo);
        assert!(undone.state.can_redo);

        let redone = fixture
            .host
            .redo()
            .expect("the productive Host redoes the DPI change");
        assert_eq!(redone.state.document.dpi, 240);
        assert_eq!(redone.state.revision, 1);
        assert!(redone.state.dirty);
        assert!(redone.state.can_undo);
        assert!(!redone.state.can_redo);
    }

    #[test]
    fn saves_the_visible_revision_preserves_history_and_reopens_it_in_a_fresh_host() {
        let Fixture {
            _root,
            project_path,
            identity_lease_root,
            host,
        } = fixture();
        let applied = host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the visible revision is created");

        let saved = host
            .save(applied.state.revision)
            .expect("the visible revision is saved");

        assert_eq!(saved.outcome, SaveProjectOutcome::Saved { revision: 1 });
        assert_eq!(saved.projection.state.document.dpi, 240);
        assert_eq!(saved.projection.state.revision, 1);
        assert_eq!(saved.projection.state.saved_revision, 1);
        assert!(!saved.projection.state.dirty);
        assert!(saved.projection.state.can_undo);
        assert!(!saved.projection.state.can_redo);

        let undone = host.undo().expect("Undo remains available after Save");
        assert_eq!(undone.state.document.dpi, 300);
        assert!(undone.state.dirty);
        let redone = host.redo().expect("Redo remains available after Save");
        assert_eq!(redone.state.document.dpi, 240);
        assert!(!redone.state.dirty);

        drop(host);
        let reopened = open_project(&project_path, &identity_lease_root);
        let projection = reopened
            .projection()
            .expect("the persisted revision is projected by the new Host");

        assert_eq!(projection.state.document.dpi, 240);
        assert_eq!(projection.state.revision, 1);
        assert_eq!(projection.state.saved_revision, 1);
        assert!(!projection.state.dirty);
        assert!(!projection.state.can_undo);
        assert!(!projection.state.can_redo);
    }

    #[test]
    fn exposes_persisted_linked_media_to_the_host_without_projecting_pathnames() {
        let root = tempfile::tempdir().expect("temporary linked-media Host fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let background_path = root.path().join("Background.png");
        std::fs::write(&background_path, b"\x89PNG\r\n\x1a\nbackground")
            .expect("the linked background fixture is writable");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the fixture root is captured");
        let initial =
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: background_path.clone(),
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            ));
        let project = ProjectCore::new()
            .with_identity_lease_root(root.path().join("leases"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                initial,
                CreateAuthorization::CreateOnly,
            ))
            .expect("the personalized Project is created");
        let host = ProjectHost::new(project);

        let sources = host
            .linked_media_sources()
            .expect("the Host can resolve its persisted media catalog");
        let projection = host.projection().expect("the Project remains available");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].0, projection.state.album.media[0].id);
        assert_eq!(sources[0].1, background_path);
        let frontend_projection =
            serde_json::to_string(&projection).expect("the editor projection serializes");
        assert!(!frontend_projection.contains(root.path().to_string_lossy().as_ref()));
    }
}
