use std::{
    collections::{BTreeSet, HashMap},
    sync::{Mutex, MutexGuard},
};

use myalbuns_core::{EditorProjection, ProjectIntent, ProjectSession, RenderSnapshot};
use myalbuns_imaging_protocol::MediaSource;

pub(crate) struct ProjectHost {
    projects: HashMap<String, HostedProject>,
}

struct HostedProject {
    session: Mutex<ProjectSession>,
    media_sources: Vec<MediaSource>,
}

impl ProjectHost {
    pub(crate) fn new(
        projects: impl IntoIterator<Item = (&'static str, ProjectSession, Vec<MediaSource>)>,
    ) -> Self {
        let mut hosted_projects = HashMap::new();
        for (window_label, session, sources) in projects {
            hosted_projects.insert(
                window_label.into(),
                HostedProject {
                    session: Mutex::new(session),
                    media_sources: sources,
                },
            );
        }
        Self {
            projects: hosted_projects,
        }
    }

    pub(crate) fn projection(&self, window_label: &str) -> Result<EditorProjection, String> {
        let session = self.session(window_label)?;
        Ok(session.projection())
    }

    pub(crate) fn apply(
        &self,
        window_label: &str,
        intent: ProjectIntent,
    ) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.apply(intent).map_err(|error| error.to_string())?;
        Ok(session.projection())
    }

    pub(crate) fn undo(&self, window_label: &str) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.undo();
        Ok(session.projection())
    }

    pub(crate) fn redo(&self, window_label: &str) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session.redo();
        Ok(session.projection())
    }

    pub(crate) fn render_snapshot(&self, window_label: &str) -> Result<RenderSnapshot, String> {
        Ok(self.session(window_label)?.render_snapshot())
    }

    pub(crate) fn cache_sources(
        &self,
        window_label: &str,
    ) -> Result<Option<Vec<MediaSource>>, String> {
        let project = self.hosted_project(window_label)?;
        Ok((!project.media_sources.is_empty()).then(|| project.media_sources.clone()))
    }

    pub(crate) fn export_sources(
        &self,
        window_label: &str,
        snapshot: &RenderSnapshot,
        sheet_id: &str,
    ) -> Result<Option<Vec<MediaSource>>, String> {
        let available_sources = &self.hosted_project(window_label)?.media_sources;
        if available_sources.is_empty() {
            return Ok(None);
        }
        let sheet = snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .ok_or_else(|| "A Lâmina solicitada não existe no snapshot.".to_string())?;
        let required_media = sheet
            .frames
            .iter()
            .filter_map(|frame| frame.photo.as_ref())
            .map(|photo| photo.media_id.as_str())
            .collect::<BTreeSet<_>>();
        let sources_by_media = available_sources
            .iter()
            .map(|source| (source.media_id(), source))
            .collect::<HashMap<_, _>>();
        required_media
            .into_iter()
            .map(|media_id| {
                sources_by_media
                    .get(media_id)
                    .cloned()
                    .cloned()
                    .ok_or_else(|| {
                        format!("A fonte original da mídia {media_id} não está disponível.")
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    fn hosted_project(&self, window_label: &str) -> Result<&HostedProject, String> {
        self.projects.get(window_label).ok_or_else(|| {
            format!("Não existe uma Sessão do Projeto para a janela {window_label}.")
        })
    }

    fn session(&self, window_label: &str) -> Result<MutexGuard<'_, ProjectSession>, String> {
        self.hosted_project(window_label)?
            .session
            .lock()
            .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use myalbuns_core::{ProjectCore, ProjectIntent, ProjectSession};
    use myalbuns_imaging_protocol::MediaSource;

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

    #[test]
    fn selects_only_the_originals_used_by_the_requested_sheet() {
        let session = sample_session(SampleProject::Horizon);
        let snapshot = session.render_snapshot();
        let source = |media_id: &str| {
            MediaSource::new(
                media_id,
                PathBuf::from(format!(r"C:\Photos\{media_id}.jpg")),
                1024,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("the source is valid")
        };
        let host = ProjectHost::new([(
            "main",
            session,
            vec![
                source("media-campo"),
                source("media-costa"),
                source("unused-media"),
            ],
        )]);

        let sources = host
            .export_sources("main", &snapshot, "lamina-01")
            .expect("the export source plan is valid")
            .expect("the project has linked originals");

        assert_eq!(
            sources
                .iter()
                .map(MediaSource::media_id)
                .collect::<Vec<_>>(),
            ["media-campo", "media-costa"]
        );
    }
}
