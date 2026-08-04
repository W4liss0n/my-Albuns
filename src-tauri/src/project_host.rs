use std::{path::PathBuf, sync::Mutex};

use myalbuns_core::{EditableProject, EditorProjection, ProjectIntent, RenderSnapshot};
use myalbuns_imaging_protocol::MediaSource;

/// Owns the single productive editable Project of this Host process.
///
/// There is deliberately no window/session selector: one process owns one
/// Project, and the operating-system process lifetime owns its locks.
pub(crate) struct ProjectHost {
    project: Mutex<EditableProject>,
}

impl ProjectHost {
    pub(crate) fn new(project: EditableProject) -> Self {
        Self {
            project: Mutex::new(project),
        }
    }

    pub(crate) fn projection(&self) -> Result<EditorProjection, String> {
        Ok(self.project()?.projection())
    }

    pub(crate) fn apply(&self, _intent: ProjectIntent) -> Result<EditorProjection, String> {
        Err("A edição criativa do Documento v1 entra nos próximos cortes da fase.".into())
    }

    pub(crate) fn undo(&self) -> Result<EditorProjection, String> {
        Err("Não há uma ação produtiva para desfazer neste corte.".into())
    }

    pub(crate) fn redo(&self) -> Result<EditorProjection, String> {
        Err("Não há uma ação produtiva para refazer neste corte.".into())
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

    fn project(&self) -> Result<std::sync::MutexGuard<'_, EditableProject>, String> {
        self.project
            .lock()
            .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialBackground, InitialBackgroundContent,
        InitialFrameBorder, InitialOverlay, InitialProject, InitialProjectPersonalization,
        ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::OperationPathContext;

    use super::ProjectHost;

    struct Fixture {
        _root: tempfile::TempDir,
        host: ProjectHost,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().expect("temporary Project Host fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the fixture root is captured");
        let project = ProjectCore::new()
            .with_identity_lease_root(root.path().join("leases"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the productive Project is created");
        Fixture {
            _root: root,
            host: ProjectHost::new(project),
        }
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
