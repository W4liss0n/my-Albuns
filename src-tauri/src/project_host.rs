use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use myalbuns_core::{EditorProjection, ProjectIntent, ProjectSession, RenderSnapshot};
use myalbuns_imaging_protocol::{CacheMediaSource, CacheRequest};
use myalbuns_paths::CachePathPlan;

pub(crate) struct ProjectHost {
    sessions: HashMap<String, Mutex<ProjectSession>>,
    media_sources: HashMap<String, Vec<CacheMediaSource>>,
}

impl ProjectHost {
    pub(crate) fn new(
        projects: impl IntoIterator<Item = (&'static str, ProjectSession, Vec<CacheMediaSource>)>,
    ) -> Self {
        let mut sessions = HashMap::new();
        let mut media_sources = HashMap::new();
        for (window_label, session, sources) in projects {
            sessions.insert(window_label.into(), Mutex::new(session));
            if !sources.is_empty() {
                media_sources.insert(window_label.into(), sources);
            }
        }
        Self {
            sessions,
            media_sources,
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

    pub(crate) fn cache_request(
        &self,
        window_label: &str,
        request_id: String,
        cache_paths: CachePathPlan,
        max_edge_px: u32,
    ) -> Result<Option<CacheRequest>, String> {
        let Some(sources) = self.media_sources.get(window_label) else {
            return Ok(None);
        };
        let project_id = self.session(window_label)?.state().project_id;
        CacheRequest::new(
            request_id,
            project_id,
            cache_paths,
            sources.clone(),
            max_edge_px,
        )
        .map(Some)
    }

    fn session(&self, window_label: &str) -> Result<MutexGuard<'_, ProjectSession>, String> {
        let session = self.sessions.get(window_label).ok_or_else(|| {
            format!("Não existe uma Sessão do Projeto para a janela {window_label}.")
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
    use myalbuns_core::{ProjectCore, ProjectIntent, ProjectSession};

    use super::ProjectHost;
    use crate::sample_project::SampleProject;

    fn sample_session(sample: SampleProject) -> ProjectSession {
        let source = sample
            .persisted_source(12)
            .expect("the sample project serializes");
        ProjectCore::open_editable_session(&source)
            .expect("the sample project opens through ProjectCore")
    }

    #[test]
    fn isolates_each_window_project_session() {
        let host = ProjectHost::new([
            ("project-a", sample_session(SampleProject::Horizon), vec![]),
            ("project-b", sample_session(SampleProject::Aurora), vec![]),
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
