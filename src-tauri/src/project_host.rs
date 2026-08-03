use std::{collections::BTreeSet, sync::Mutex};

use myalbuns_core::{EditableProject, EditorProjection, ProjectIntent, RenderSnapshot};
use myalbuns_imaging_protocol::MediaSource;

/// Owns the single editable Project Session of this desktop host.
///
/// A project window never selects a session by label: topology A gives each
/// project its own host process, so accepting a label here would reintroduce
/// the discarded multi-project-host topology.
pub(crate) struct ProjectHost {
    session: Mutex<EditableProject>,
    media_sources: Vec<MediaSource>,
}

impl ProjectHost {
    pub(crate) fn new(session: EditableProject, media_sources: Vec<MediaSource>) -> Self {
        Self {
            session: Mutex::new(session),
            media_sources,
        }
    }

    pub(crate) fn projection(&self) -> Result<EditorProjection, String> {
        Ok(self.session()?.projection())
    }

    pub(crate) fn apply(&self, intent: ProjectIntent) -> Result<EditorProjection, String> {
        let mut session = self.session()?;
        session.apply(intent).map_err(|error| error.to_string())?;
        Ok(session.projection())
    }

    pub(crate) fn undo(&self) -> Result<EditorProjection, String> {
        let mut session = self.session()?;
        session.undo();
        Ok(session.projection())
    }

    pub(crate) fn redo(&self) -> Result<EditorProjection, String> {
        let mut session = self.session()?;
        session.redo();
        Ok(session.projection())
    }

    pub(crate) fn render_snapshot(&self) -> Result<RenderSnapshot, String> {
        Ok(self.session()?.render_snapshot())
    }

    pub(crate) fn cache_sources(&self) -> Option<Vec<MediaSource>> {
        (!self.media_sources.is_empty()).then(|| self.media_sources.clone())
    }

    pub(crate) fn export_sources(
        &self,
        snapshot: &RenderSnapshot,
        sheet_id: &str,
    ) -> Result<Vec<MediaSource>, String> {
        if self.media_sources.is_empty() {
            return Err(
                "As fontes originais das mídias não estão disponíveis para a Exportação."
                    .to_string(),
            );
        }
        let sheet = snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .ok_or_else(|| "A Lâmina solicitada não existe no snapshot.".to_string())?;
        let required_media = sheet.referenced_media_ids().collect::<BTreeSet<_>>();
        let sources_by_media = self
            .media_sources
            .iter()
            .map(|source| (source.media_id(), source))
            .collect::<std::collections::HashMap<_, _>>();
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
            .collect()
    }

    fn session(&self) -> Result<std::sync::MutexGuard<'_, EditableProject>, String> {
        self.session
            .lock()
            .map_err(|_| "A Sessão do Projeto ficou indisponível.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use myalbuns_core::{EditableProject, ProjectCore, ProjectIntent};
    use myalbuns_imaging_protocol::MediaSource;

    use super::ProjectHost;
    use crate::sample_project::SampleProject;

    fn sample_session(core: &ProjectCore) -> EditableProject {
        let source = SampleProject::Horizon
            .persisted_source(12)
            .expect("the sample project serializes");
        core.open_editable_session(&source)
            .expect("the sample project opens through ProjectCore")
    }

    #[test]
    fn owns_one_session_without_a_window_label_selector() {
        let core = ProjectCore::new();
        let host = ProjectHost::new(sample_session(&core), vec![]);

        host.apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.25,
            delta_pan_y: 0.0,
            delta_zoom: 0.0,
        })
        .expect("the host accepts an intent");

        assert_eq!(
            host.projection()
                .expect("the project remains available")
                .state
                .revision,
            1
        );
    }

    #[test]
    fn selects_only_the_originals_used_by_the_requested_sheet() {
        let core = ProjectCore::new();
        let session = sample_session(&core);
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
        let host = ProjectHost::new(
            session,
            vec![
                source("decorative-overlay"),
                source("media-campo"),
                source("media-costa"),
                source("unused-media"),
            ],
        );

        let sources = host
            .export_sources(&snapshot, "lamina-01")
            .expect("the export source plan is valid");

        assert_eq!(
            sources
                .iter()
                .map(MediaSource::media_id)
                .collect::<Vec<_>>(),
            ["decorative-overlay", "media-campo", "media-costa"]
        );
    }

    #[test]
    fn rejects_export_when_original_sources_are_unavailable() {
        let core = ProjectCore::new();
        let session = sample_session(&core);
        let snapshot = session.render_snapshot();
        let host = ProjectHost::new(session, vec![]);

        let error = host
            .export_sources(&snapshot, "lamina-01")
            .expect_err("a production export requires linked originals");

        assert!(error.contains("fontes originais"));
    }
}
