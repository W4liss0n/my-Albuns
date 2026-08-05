use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use myalbuns_core::{
    EditableProject, EditorProjection, ProjectIntent, RenderSnapshot, SaveProjectError,
    SaveProjectOutcome,
};

use crate::path_io::FrozenMediaSource;
const SESSION_UNAVAILABLE_MESSAGE: &str = "A Sessão do Projeto ficou indisponível.";

/// Owns the single productive editable Project of this Host process.
///
/// There is deliberately no window/session selector: one process owns one
/// Project, and the operating-system process lifetime owns its locks.
#[derive(Clone)]
pub(crate) struct ProjectHost {
    state: Arc<Mutex<ProjectHostState>>,
}

enum ProjectHostState {
    Active(EditableProject),
    ClosePending(EditableProject),
    Consumed,
}

pub(crate) struct ProjectHostSaveResult {
    pub(crate) outcome: SaveProjectOutcome,
    pub(crate) projection: EditorProjection,
}

#[derive(Debug)]
pub(crate) struct FrozenSheetExport {
    pub(crate) snapshot: RenderSnapshot,
    pub(crate) source_paths: Vec<FrozenMediaSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectCloseRequestOutcome {
    CloseImmediately,
    ConfirmationRequired,
}

#[derive(Debug)]
pub(crate) enum ProjectHostSaveError {
    Project(SaveProjectError),
    SessionUnavailable,
}

impl ProjectHost {
    pub(crate) fn new(project: EditableProject) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProjectHostState::Active(project))),
        }
    }

    pub(crate) fn projection(&self) -> Result<EditorProjection, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        match &*state {
            ProjectHostState::Active(project) | ProjectHostState::ClosePending(project) => {
                Ok(project.projection())
            }
            ProjectHostState::Consumed => Err(SESSION_UNAVAILABLE_MESSAGE.to_string()),
        }
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
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        let result = match &mut *state {
            ProjectHostState::Active(project) => project.save(expected_revision),
            ProjectHostState::ClosePending(_) | ProjectHostState::Consumed => {
                return Err(ProjectHostSaveError::SessionUnavailable);
            }
        };
        match result {
            Ok(outcome) => {
                let ProjectHostState::Active(project) = &*state else {
                    unreachable!("saving an active Project preserves its state")
                };
                Ok(ProjectHostSaveResult {
                    outcome,
                    projection: project.projection(),
                })
            }
            Err(SaveProjectError::SaveStateIndeterminate) => {
                *state = ProjectHostState::Consumed;
                Err(ProjectHostSaveError::Project(
                    SaveProjectError::SaveStateIndeterminate,
                ))
            }
            Err(error) => Err(ProjectHostSaveError::Project(error)),
        }
    }

    pub(crate) fn begin_close(&self) -> Result<ProjectCloseRequestOutcome, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let current = std::mem::replace(&mut *state, ProjectHostState::Consumed);
        match current {
            ProjectHostState::Active(project) if project.has_unsaved_changes() => {
                *state = ProjectHostState::ClosePending(project);
                Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
            }
            ProjectHostState::Active(_) => Ok(ProjectCloseRequestOutcome::CloseImmediately),
            ProjectHostState::ClosePending(project) => {
                *state = ProjectHostState::ClosePending(project);
                Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
            }
            ProjectHostState::Consumed => Err(SESSION_UNAVAILABLE_MESSAGE.to_string()),
        }
    }

    pub(crate) fn cancel_close(&self) -> Result<EditorProjection, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let current = std::mem::replace(&mut *state, ProjectHostState::Consumed);
        match current {
            ProjectHostState::ClosePending(project) => {
                let projection = project.projection();
                *state = ProjectHostState::Active(project);
                Ok(projection)
            }
            other => {
                *state = other;
                Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
            }
        }
    }

    pub(crate) fn discard_close(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let current = std::mem::replace(&mut *state, ProjectHostState::Consumed);
        match current {
            ProjectHostState::ClosePending(_) => Ok(()),
            other => {
                *state = other;
                Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
            }
        }
    }

    pub(crate) fn save_and_close(&self) -> Result<SaveProjectOutcome, ProjectHostSaveError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        let current = std::mem::replace(&mut *state, ProjectHostState::Consumed);
        let ProjectHostState::ClosePending(mut project) = current else {
            *state = current;
            return Err(ProjectHostSaveError::SessionUnavailable);
        };
        let revision = project.revision();
        match project.save(revision) {
            Ok(outcome) => Ok(outcome),
            Err(SaveProjectError::SaveStateIndeterminate) => Err(ProjectHostSaveError::Project(
                SaveProjectError::SaveStateIndeterminate,
            )),
            Err(error) => {
                *state = ProjectHostState::Active(project);
                Err(ProjectHostSaveError::Project(error))
            }
        }
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

    pub(crate) fn freeze_sheet_export(&self, sheet_id: &str) -> Result<FrozenSheetExport, String> {
        let project = self.project()?;
        let snapshot = project.render_snapshot();
        let sheet = snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .ok_or_else(|| "A Lâmina solicitada não existe no snapshot.".to_string())?;
        let mut source_paths = Vec::new();
        for media_id in sheet.referenced_media_ids() {
            if source_paths
                .iter()
                .any(|source: &FrozenMediaSource| source.media_id() == media_id)
            {
                continue;
            }
            let media = project
                .project()
                .media()
                .iter()
                .find(|media| media.id().hyphenated().to_string() == media_id)
                .ok_or_else(|| {
                    format!(
                        "A fonte original da mídia {media_id} não pertence à mesma revisão do Projeto."
                    )
                })?;
            source_paths.push(FrozenMediaSource::new(media_id, media.path().to_path_buf()));
        }
        Ok(FrozenSheetExport {
            snapshot,
            source_paths,
        })
    }

    fn project(&self) -> Result<ActiveProject<'_>, String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        if !matches!(*guard, ProjectHostState::Active(_)) {
            return Err(SESSION_UNAVAILABLE_MESSAGE.to_string());
        }
        Ok(ActiveProject { guard })
    }
}

struct ActiveProject<'a> {
    guard: MutexGuard<'a, ProjectHostState>,
}

impl std::ops::Deref for ActiveProject<'_> {
    type Target = EditableProject;

    fn deref(&self) -> &Self::Target {
        match &*self.guard {
            ProjectHostState::Active(project) => project,
            ProjectHostState::ClosePending(_) | ProjectHostState::Consumed => {
                unreachable!("an ActiveProject guard always contains an active Project")
            }
        }
    }
}

impl std::ops::DerefMut for ActiveProject<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut *self.guard {
            ProjectHostState::Active(project) => project,
            ProjectHostState::ClosePending(_) | ProjectHostState::Consumed => {
                unreachable!("an ActiveProject guard always contains an active Project")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use image::{GenericImageView, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat, InitialBackground,
        InitialBackgroundContent, InitialFrameBorder, InitialOverlay, InitialProject,
        InitialProjectConfiguration, InitialProjectPersonalization, OpenProjectRequest,
        ProjectCore, ProjectIntent, ProjectLocation, SaveProjectError, SaveProjectOutcome,
    };
    use myalbuns_paths::{ExportWriteAuthorization, OperationPathContext};

    use super::{ProjectCloseRequestOutcome, ProjectHost, ProjectHostSaveError};
    use crate::{
        export_pipeline, imaging_processor::InvocationContext,
        imaging_recovery_integration::RealProcessTransport, path_io,
    };

    const TEST_PROCESSOR_ENV: &str = "MYALBUNS_TEST_IMAGING_PROCESSOR";

    struct Fixture {
        _root: tempfile::TempDir,
        project_path: PathBuf,
        identity_lease_root: PathBuf,
        host: ProjectHost,
    }

    fn fixture_with_initial(initial: InitialProject) -> Fixture {
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
                initial,
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

    fn fixture() -> Fixture {
        fixture_with_initial(InitialProject::neutral())
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
        let frozen = fixture
            .host
            .freeze_sheet_export(
                &fixture
                    .host
                    .projection()
                    .expect("the neutral projection is available")
                    .composition
                    .sheets[0]
                    .sheet_id,
            )
            .expect("the neutral Export is frozen");

        assert!(frozen.snapshot.validate().is_ok());
        assert!(frozen.source_paths.is_empty());
    }

    #[test]
    fn freezes_one_visible_sheet_unsaved_dpi_and_its_exact_originals_without_mutating_project() {
        let root = tempfile::tempdir().expect("temporary decorative media fixture");
        let shared_path = root.path().join("shared.png");
        let right_path = root.path().join("right.png");
        std::fs::write(&shared_path, b"shared original").expect("the shared original is writable");
        std::fs::write(&right_path, b"right original").expect("the right original is writable");
        let personalized =
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::PerSide {
                    left: InitialBackgroundContent::Media {
                        path: shared_path.clone(),
                    },
                    right: InitialBackgroundContent::Media {
                        path: right_path.clone(),
                    },
                },
                InitialOverlay::BothSides {
                    both: Some(myalbuns_core::InitialOverlayContent::Media {
                        path: shared_path.clone(),
                    }),
                },
                InitialFrameBorder::None,
            ));
        let fixture = fixture_with_initial(personalized);
        let dirty = fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the current unsaved DPI is applied");
        let persisted_before =
            std::fs::read(&fixture.project_path).expect("the Project baseline is readable");
        let sheet_id = dirty.composition.sheets[1].sheet_id.clone();

        let frozen = fixture
            .host
            .freeze_sheet_export(&sheet_id)
            .expect("the noninitial visible sheet is frozen atomically");

        assert_eq!(frozen.snapshot.revision, dirty.state.revision);
        assert_eq!(frozen.snapshot.dpi, 240);
        assert_eq!(
            frozen
                .snapshot
                .output_unit(&sheet_id)
                .expect("the selected sheet remains in the frozen snapshot")
                .sheet
                .sheet_id,
            sheet_id
        );
        assert_eq!(
            fixture
                .host
                .projection()
                .expect("the Project remains readable"),
            dirty
        );
        assert_eq!(
            std::fs::read(&fixture.project_path).expect("the Project remains persisted"),
            persisted_before
        );

        let frozen_paths = frozen
            .source_paths
            .iter()
            .map(|source| source.source_path().to_path_buf())
            .collect::<Vec<_>>();
        assert_eq!(frozen_paths, [shared_path, right_path]);
        assert_eq!(
            frozen.source_paths.len(),
            2,
            "a reused original is listed once"
        );

        fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 180 })
            .expect("the live Project may advance after freezing");
        assert_eq!(frozen.snapshot.dpi, 240);
        assert_eq!(frozen.snapshot.revision, dirty.state.revision);
    }

    #[test]
    #[ignore = "executed by scripts/Test-Rust.ps1 with the freshly built real sidecar"]
    fn reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor() {
        tauri::async_runtime::block_on(async {
            let executable = PathBuf::from(
                std::env::var_os(TEST_PROCESSOR_ENV)
                    .expect("the real imaging executable path is configured"),
            );
            assert!(executable.is_file(), "the real imaging executable exists");

            let media_root = tempfile::tempdir().expect("temporary E2E media fixture");
            let shared_path = media_root.path().join("shared-overlay.png");
            let right_path = media_root.path().join("right-background.jpg");
            RgbaImage::from_pixel(48, 32, Rgba([240, 10, 10, 128]))
                .save_with_format(&shared_path, ImageFormat::Png)
                .expect("the transparent shared original is written");
            RgbImage::from_pixel(48, 32, Rgb([10, 20, 240]))
                .save_with_format(&right_path, ImageFormat::Jpeg)
                .expect("the right Background original is written");
            let personalized = InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                3_000,
                3_000,
                3,
                EndSheetFormat::SinglePage,
                EndSheetFormat::SinglePage,
            ))
            .with_personalization(InitialProjectPersonalization::new(
                InitialBackground::PerSide {
                    left: InitialBackgroundContent::Media {
                        path: shared_path.clone(),
                    },
                    right: InitialBackgroundContent::Media {
                        path: right_path.clone(),
                    },
                },
                InitialOverlay::BothSides {
                    both: Some(myalbuns_core::InitialOverlayContent::Media { path: shared_path }),
                },
                InitialFrameBorder::None,
            ));
            let Fixture {
                _root: project_root,
                project_path,
                identity_lease_root,
                host,
            } = fixture_with_initial(personalized);
            assert_eq!(
                host.begin_close(),
                Ok(ProjectCloseRequestOutcome::CloseImmediately)
            );
            let host = open_project(&project_path, &identity_lease_root);
            let persisted_before =
                std::fs::read(&project_path).expect("the reopened Project is readable");
            let dirty = host
                .apply(ProjectIntent::SetDpi { dpi: 25 })
                .expect("the current unsaved DPI is applied");
            assert_ne!(
                dirty.composition.sheets[0].active_sides, dirty.composition.sheets[1].active_sides,
                "the initial and visible noninitial Sheets must be semantically distinguishable"
            );
            let sheet_id = dirty.composition.sheets[1].sheet_id.clone();
            let frozen = host
                .freeze_sheet_export(&sheet_id)
                .expect("the visible noninitial Sheet is frozen by the Host");
            let expected_dpi = frozen.snapshot.dpi;
            let expected_revision = frozen.snapshot.revision;
            let output_path = project_root.path().join("visible-sheet.jpg");
            let request_id = "host-pipeline-real-processor";
            let planned = export_pipeline::plan(
                frozen.snapshot,
                export_pipeline::ExportOptions::new(
                    request_id,
                    output_path.clone(),
                    ExportWriteAuthorization::CreateOnly,
                    sheet_id,
                    frozen.source_paths,
                ),
            )
            .expect("the Host snapshot owns the exact Export dependencies");
            let operation_paths = planned
                .required_paths()
                .into_iter()
                .map(Path::to_path_buf)
                .collect();
            let root_bindings = path_io::capture_root_bindings(operation_paths)
                .await
                .expect("the Export roots are captured once");
            let sources = path_io::fingerprint_media_sources(
                root_bindings.clone(),
                planned.source_dependencies().to_vec(),
            )
            .await
            .expect("the planned originals are fingerprinted");
            let plan = planned
                .bind_sources(sources)
                .expect("the fingerprints bind only to their frozen dependencies");
            let log_directory = project_root.path().join("processor-logs");
            std::fs::create_dir(&log_directory).expect("the processor log directory exists");
            let mut transport = RealProcessTransport::stable(executable, log_directory);
            let published = export_pipeline::execute(
                &mut transport,
                plan,
                &root_bindings,
                &export_pipeline::ExportExecutionControl::default(),
                &|_| {},
                &InvocationContext::new(request_id, Some(dirty.state.project_id.clone())),
            )
            .await
            .expect("the real processor publishes the frozen visible Sheet");

            assert_eq!(published.completion.dpi, expected_dpi);
            assert_eq!(published.completion.source_count, 2);
            assert_eq!(
                (
                    published.completion.width_px,
                    published.completion.height_px
                ),
                (591, 295),
                "the visible internal double Sheet is exported; the initial right-only Sheet would be 295 × 295"
            );
            assert_eq!(expected_revision, dirty.state.revision);
            let rendered = image::open(&output_path).expect("the published JPEG decodes");
            assert_eq!(
                rendered.dimensions(),
                (
                    published.completion.width_px,
                    published.completion.height_px
                )
            );
            let rendered = rendered.to_rgb8();
            let left = rendered.get_pixel(rendered.width() / 4, rendered.height() / 2);
            let right = rendered.get_pixel(rendered.width() * 3 / 4, rendered.height() / 2);
            assert!(
                left[0] > left[2] * 2,
                "the left Background and translucent Overlay remain visibly red"
            );
            assert!(
                right[0] > right[1] * 3 && right[2] > right[1] * 3,
                "the red translucent Overlay is composed over the blue right Background"
            );
            assert_eq!(
                host.projection().expect("the Project remains available"),
                dirty
            );
            assert_eq!(
                std::fs::read(&project_path).expect("the Project remains readable"),
                persisted_before,
                "Export does not save or mutate the Project"
            );
        });
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
    fn clean_close_consumes_the_session_and_releases_editable_ownership() {
        let Fixture {
            _root,
            project_path,
            identity_lease_root,
            host,
        } = fixture();

        assert_eq!(
            host.begin_close(),
            Ok(ProjectCloseRequestOutcome::CloseImmediately)
        );
        assert!(host.projection().is_err());

        let reopened = open_project(&project_path, &identity_lease_root);
        let projection = reopened
            .projection()
            .expect("closing releases the Project for a new editable Session");
        assert_eq!(projection.state.revision, 0);
        assert!(!projection.state.dirty);
    }

    #[test]
    fn dirty_close_requires_a_decision_and_blocks_creative_commands() {
        let fixture = fixture();
        let dirty = fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing");

        assert_eq!(
            fixture.host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );
        assert!(
            fixture
                .host
                .apply(ProjectIntent::SetDpi { dpi: 180 })
                .is_err()
        );
        assert!(fixture.host.undo().is_err());
        assert!(fixture.host.redo().is_err());
        assert!(fixture.host.save(dirty.state.revision).is_err());

        let pending = fixture
            .host
            .projection()
            .expect("the pending decision keeps a readable projection");
        assert_eq!(pending.state.document.dpi, 240);
        assert_eq!(pending.state.revision, dirty.state.revision);
        assert!(pending.state.dirty);
    }

    #[test]
    fn cancelling_close_preserves_the_session_history_and_persisted_bytes() {
        let fixture = fixture();
        let persisted_before =
            std::fs::read(&fixture.project_path).expect("the persisted baseline is readable");
        let dirty = fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing");
        assert_eq!(
            fixture.host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );

        let cancelled = fixture
            .host
            .cancel_close()
            .expect("cancelling keeps the editable Session");

        assert_eq!(cancelled, dirty);
        assert_eq!(
            std::fs::read(&fixture.project_path).expect("the persisted file remains readable"),
            persisted_before
        );
        let undone = fixture
            .host
            .undo()
            .expect("the original History remains available after cancelling");
        assert_eq!(undone.state.document.dpi, 300);
        assert!(!undone.state.dirty);
    }

    #[test]
    fn discarding_close_consumes_the_session_without_persisting_changes() {
        let Fixture {
            _root,
            project_path,
            identity_lease_root,
            host,
        } = fixture();
        host.apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing");
        assert_eq!(
            host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );

        host.discard_close()
            .expect("discarding consumes the editable Session");
        assert!(host.projection().is_err());

        let reopened = open_project(&project_path, &identity_lease_root);
        let projection = reopened
            .projection()
            .expect("discarding releases ownership for a fresh Session");
        assert_eq!(projection.state.document.dpi, 300);
        assert_eq!(projection.state.revision, 0);
        assert_eq!(projection.state.saved_revision, 0);
        assert!(!projection.state.dirty);
        assert!(!projection.state.can_undo);
        assert!(!projection.state.can_redo);
    }

    #[test]
    fn saving_close_persists_the_current_revision_then_consumes_the_session() {
        let Fixture {
            _root,
            project_path,
            identity_lease_root,
            host,
        } = fixture();
        let dirty = host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing");
        assert_eq!(dirty.state.revision, 1);
        assert_eq!(
            host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );

        assert_eq!(
            host.save_and_close()
                .expect("Save and close confirms the current revision"),
            SaveProjectOutcome::Saved { revision: 1 }
        );
        assert!(host.projection().is_err());

        let reopened = open_project(&project_path, &identity_lease_root);
        let projection = reopened
            .projection()
            .expect("the confirmed revision reopens in a fresh Session");
        assert_eq!(projection.state.document.dpi, 240);
        assert_eq!(projection.state.revision, 1);
        assert_eq!(projection.state.saved_revision, 1);
        assert!(!projection.state.dirty);
        assert!(!projection.state.can_undo);
        assert!(!projection.state.can_redo);
    }

    #[test]
    fn a_conclusive_save_failure_keeps_the_dirty_session_and_reenables_history() {
        let fixture = fixture();
        let dirty = fixture
            .host
            .apply(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing");
        assert_eq!(
            fixture.host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );
        std::fs::write(&fixture.project_path, b"externally replaced")
            .expect("an external writer changes the persisted baseline");

        assert!(matches!(
            fixture.host.save_and_close(),
            Err(ProjectHostSaveError::Project(
                SaveProjectError::PersistedBaselineConflict
            ))
        ));

        assert_eq!(
            fixture
                .host
                .projection()
                .expect("a conclusive failure preserves the Session"),
            dirty
        );
        let undone = fixture
            .host
            .undo()
            .expect("creative commands resume after the conclusive failure");
        assert_eq!(undone.state.document.dpi, 300);
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
