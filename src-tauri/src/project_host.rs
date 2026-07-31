use std::{
    collections::{BTreeSet, HashMap},
    sync::{Mutex, MutexGuard},
};

use myalbuns_core::{
    EditableProject, EditorProjection, LoadedProjectRevision, ProjectCore, ProjectIntent,
    RenderSnapshot,
};
use myalbuns_imaging_protocol::MediaSource;

pub(crate) struct ProjectHost {
    core: ProjectCore,
    projects: HashMap<String, HostedProject>,
}

pub(crate) struct ProjectRevisionForPersistence {
    pub(crate) project_id: String,
    pub(crate) previous_revision: u64,
    pub(crate) persisted_revision: u64,
    pub(crate) source: String,
}

struct HostedProject {
    session: Mutex<EditableProject>,
    media_sources: Vec<MediaSource>,
}

impl ProjectHost {
    pub(crate) fn new(
        core: ProjectCore,
        projects: impl IntoIterator<Item = (&'static str, EditableProject, Vec<MediaSource>)>,
    ) -> Result<Self, String> {
        let mut hosted_projects = HashMap::new();
        let mut project_ids = BTreeSet::new();
        for (window_label, session, sources) in projects {
            if hosted_projects.contains_key(window_label) {
                return Err(format!(
                    "A janela {window_label} possui mais de uma Sessão do Projeto."
                ));
            }
            let project_id = session.state().project_id;
            if !project_ids.insert(project_id.clone()) {
                return Err(format!(
                    "O Projeto {project_id} possui mais de uma Sessão editável neste host."
                ));
            }
            hosted_projects.insert(
                window_label.to_owned(),
                HostedProject {
                    session: Mutex::new(session),
                    media_sources: sources,
                },
            );
        }
        Ok(Self {
            core,
            projects: hosted_projects,
        })
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

    pub(crate) fn revision_for_persistence(
        &self,
        window_label: &str,
        previous_revision: u64,
        expected_revision: u64,
    ) -> Result<ProjectRevisionForPersistence, String> {
        if previous_revision.checked_add(1) != Some(expected_revision) {
            return Err(
                "O probe exige exatamente uma nova revisão documental antes do Salvamento."
                    .to_string(),
            );
        }
        let session = self.session(window_label)?;
        let state = session.state();
        if state.saved_revision != previous_revision
            || state.revision != expected_revision
            || !state.dirty
        {
            return Err(
                "A Sessão do Projeto não corresponde às revisões esperadas pelo probe.".to_string(),
            );
        }
        let source = session
            .persisted_revision()
            .map_err(|error| error.to_string())?;
        Ok(ProjectRevisionForPersistence {
            project_id: state.project_id,
            previous_revision,
            persisted_revision: expected_revision,
            source,
        })
    }

    pub(crate) fn confirm_persisted_revision(
        &self,
        window_label: &str,
        revision: u64,
    ) -> Result<EditorProjection, String> {
        let mut session = self.session(window_label)?;
        session
            .confirm_saved_revision(revision)
            .map_err(|error| error.to_string())?;
        Ok(session.projection())
    }

    pub(crate) fn render_snapshot(&self, window_label: &str) -> Result<RenderSnapshot, String> {
        Ok(self.session(window_label)?.render_snapshot())
    }

    pub(crate) fn load_persisted_revision(
        &self,
        source: &str,
    ) -> Result<LoadedProjectRevision, String> {
        self.core
            .load_persisted_revision(source)
            .map_err(|error| error.to_string())
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
        let required_media = sheet.referenced_media_ids().collect::<BTreeSet<_>>();
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

    fn session(&self, window_label: &str) -> Result<MutexGuard<'_, EditableProject>, String> {
        self.hosted_project(window_label)?
            .session
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

    fn sample_session(core: &ProjectCore, sample: SampleProject) -> EditableProject {
        let source = sample
            .persisted_source(12)
            .expect("the sample project serializes");
        core.open_editable_session(&source)
            .expect("the sample project opens through ProjectCore")
    }

    #[test]
    fn isolates_each_window_project_session() {
        let core = ProjectCore::new();
        let projects = [
            (
                "project-a",
                sample_session(&core, SampleProject::Horizon),
                vec![],
            ),
            (
                "project-b",
                sample_session(&core, SampleProject::Aurora),
                vec![],
            ),
        ];
        let host = ProjectHost::new(core, projects).expect("the unique projects are hosted");

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
    fn exposes_a_revision_for_verified_persistence_before_confirming_it_as_saved() {
        let core = ProjectCore::new();
        let session = sample_session(&core, SampleProject::Horizon);
        let host = ProjectHost::new(core, [("main", session, vec![])])
            .expect("the unique project is hosted");
        host.apply(
            "main",
            ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.25,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            },
        )
        .expect("the session accepts the documentary action");

        let pending = host
            .revision_for_persistence("main", 0, 1)
            .expect("the expected one-revision transition can be persisted");
        assert_eq!(pending.project_id, "project-spike-001");
        assert_eq!(pending.previous_revision, 0);
        assert_eq!(pending.persisted_revision, 1);
        assert!(pending.source.contains("\"revision\": 1"));
        assert!(
            host.projection("main")
                .expect("the session remains available")
                .state
                .dirty,
            "serializing alone cannot announce a successful Save"
        );

        let projection = host
            .confirm_persisted_revision("main", 1)
            .expect("the verified revision can be confirmed");
        assert_eq!(projection.state.saved_revision, 1);
        assert!(!projection.state.dirty);
    }

    #[test]
    fn rejects_a_fault_probe_that_skips_or_replays_a_document_revision() {
        let core = ProjectCore::new();
        let session = sample_session(&core, SampleProject::Horizon);
        let host = ProjectHost::new(core, [("main", session, vec![])])
            .expect("the unique project is hosted");
        host.apply(
            "main",
            ProjectIntent::TransformPhoto {
                frame_id: "frame-01-a".into(),
                delta_pan_x: 0.25,
                delta_pan_y: 0.0,
                delta_zoom: 0.0,
            },
        )
        .expect("the session accepts the documentary action");

        assert!(host.revision_for_persistence("main", 0, 2).is_err());
        assert!(host.revision_for_persistence("main", 1, 1).is_err());
        assert!(host.revision_for_persistence("main", 1, 2).is_err());
    }

    #[test]
    fn selects_only_the_originals_used_by_the_requested_sheet() {
        let core = ProjectCore::new();
        let session = sample_session(&core, SampleProject::Horizon);
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
            core,
            [(
                "main",
                session,
                vec![
                    source("decorative-overlay"),
                    source("media-campo"),
                    source("media-costa"),
                    source("unused-media"),
                ],
            )],
        )
        .expect("the unique project is hosted");

        let sources = host
            .export_sources("main", &snapshot, "lamina-01")
            .expect("the export source plan is valid")
            .expect("the project has linked originals");

        assert_eq!(
            sources
                .iter()
                .map(MediaSource::media_id)
                .collect::<Vec<_>>(),
            ["decorative-overlay", "media-campo", "media-costa"]
        );
    }

    #[test]
    fn rejects_duplicate_window_labels_and_project_identities() {
        let core = ProjectCore::new();
        let duplicate_label = ProjectHost::new(
            core.clone(),
            [
                (
                    "main",
                    sample_session(&core, SampleProject::Horizon),
                    vec![],
                ),
                ("main", sample_session(&core, SampleProject::Aurora), vec![]),
            ],
        )
        .err()
        .expect("a duplicate window label is rejected");
        assert!(duplicate_label.contains("janela main"));

        let first_core = ProjectCore::new();
        let second_core = ProjectCore::new();
        let first = sample_session(&first_core, SampleProject::Horizon);
        let duplicate = sample_session(&second_core, SampleProject::Horizon);
        let duplicate_project = ProjectHost::new(
            first_core,
            [("main", first, vec![]), ("secondary", duplicate, vec![])],
        )
        .err()
        .expect("a duplicate Project identity is rejected");
        assert!(duplicate_project.contains("project-spike-001"));
    }
}
