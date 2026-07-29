use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use myalbuns_core::{EditorProjection, ProjectIntent, ProjectSession, RenderSnapshot};

pub(crate) struct ProjectHost {
    sessions: HashMap<String, Mutex<ProjectSession>>,
}

impl ProjectHost {
    pub(crate) fn new<const N: usize>(sessions: [(&str, ProjectSession); N]) -> Self {
        Self {
            sessions: sessions
                .into_iter()
                .map(|(window_label, session)| (window_label.into(), Mutex::new(session)))
                .collect(),
        }
    }

    pub(crate) fn projection(&self, window_label: &str) -> Result<EditorProjection, String> {
        let session = self.session(window_label)?;
        Ok(project(&session))
    }

    pub(crate) fn apply(
        &self,
        window_label: &str,
        intent: ProjectIntent,
    ) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.apply(intent).map_err(|error| error.to_string())?;
        Ok(project(&session))
    }

    pub(crate) fn undo(&self, window_label: &str) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.undo();
        Ok(project(&session))
    }

    pub(crate) fn redo(&self, window_label: &str) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.redo();
        Ok(project(&session))
    }

    pub(crate) fn render_snapshot(&self, window_label: &str) -> Result<RenderSnapshot, String> {
        Ok(self.session(window_label)?.render_snapshot())
    }

    fn session(&self, window_label: &str) -> Result<MutexGuard<'_, ProjectSession>, String> {
        let session = self.sessions.get(window_label).ok_or_else(|| {
            format!("Não existe uma Sessão de Projeto para a janela {window_label}.")
        })?;
        session
            .lock()
            .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())
    }
}

fn project(session: &ProjectSession) -> EditorProjection {
    EditorProjection {
        state: session.state(),
        composition: session.composition_plan(),
    }
}

#[cfg(test)]
mod tests {
    use myalbuns_core::{ProjectCore, ProjectIntent};

    use super::ProjectHost;

    #[test]
    fn isolates_each_window_project_session() {
        let host = ProjectHost::new([
            (
                "project-a",
                ProjectCore::open_sample_project_with_identity(
                    12,
                    "project-spike-001",
                    "Álbum Horizonte",
                ),
            ),
            (
                "project-b",
                ProjectCore::open_sample_project_with_identity(
                    12,
                    "project-spike-002",
                    "Álbum Aurora",
                ),
            ),
        ]);

        host.apply(
            "project-a",
            ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.25,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            },
        )
        .expect("the first window accepts a pan");

        assert_eq!(
            host.projection("project-a")
                .expect("the first window remains available")
                .state
                .revision,
            1
        );
        assert_eq!(
            host.projection("project-b")
                .expect("the second window remains available")
                .state
                .revision,
            0
        );
    }
}
