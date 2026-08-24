use std::{
    io,
    sync::{Arc, Mutex, MutexGuard},
};

use myalbuns_core::{
    ComposedOutputUnit, EditableProject, EditorProjection, ImportPhotoDisposition,
    ImportPhotoOutcome, MediaId, PhotoDropTarget, PhotoSourceMetadata, ProjectIdentityAuthority,
    ProjectIntent, ProjectMutationOutcome, RecoveryCheckpoint, RelinkMedia, RenderSnapshot,
    SaveAsProjectError, SaveAsProjectOutcome, SaveAsProjectRequest, SaveProjectError,
    SaveProjectOutcome,
};
use myalbuns_imaging_protocol::RenderSource;

use crate::{
    media_runtime::{MediaBinding, MediaRelinkProposal, PhotoImportProposal},
    project_recovery::RecoveryCoordinator,
};

const SESSION_UNAVAILABLE_MESSAGE: &str = "A Sessão do Projeto ficou indisponível.";

/// Owns the single productive editable Project of this Host process.
///
/// There is deliberately no window/session selector: one process owns one
/// Project, and the operating-system process lifetime owns its locks.
#[derive(Clone)]
pub(crate) struct ProjectHost {
    state: Arc<Mutex<ProjectHostState>>,
    recovery: Option<RecoveryCoordinator>,
}

struct ProjectHostState {
    session: Option<ProjectHostSession>,
}

struct ProjectHostSession {
    project: EditableProject,
    phase: ProjectHostPhase,
}

enum ProjectHostPhase {
    Active,
    RecoveryPending(Box<RecoveryCheckpoint>),
    ClosePending,
}

impl ProjectHostState {
    fn new(project: EditableProject, checkpoint: Option<RecoveryCheckpoint>) -> Self {
        let session = match checkpoint {
            Some(checkpoint) => ProjectHostSession::recovery_pending(project, checkpoint),
            None => ProjectHostSession::active(project),
        };
        Self {
            session: Some(session),
        }
    }

    fn session(&self) -> Result<&ProjectHostSession, String> {
        self.session
            .as_ref()
            .ok_or_else(|| SESSION_UNAVAILABLE_MESSAGE.to_string())
    }

    fn session_mut(&mut self) -> Result<&mut ProjectHostSession, String> {
        self.session
            .as_mut()
            .ok_or_else(|| SESSION_UNAVAILABLE_MESSAGE.to_string())
    }

    fn active_project(&self) -> Result<&EditableProject, String> {
        self.session()?.active_project()
    }

    fn active_project_mut(&mut self) -> Result<&mut EditableProject, String> {
        self.session_mut()?.active_project_mut()
    }

    fn take_session(&mut self) -> Result<ProjectHostSession, String> {
        self.session
            .take()
            .ok_or_else(|| SESSION_UNAVAILABLE_MESSAGE.to_string())
    }

    fn restore_session(&mut self, session: ProjectHostSession) {
        debug_assert!(self.session.is_none());
        self.session = Some(session);
    }

    fn consume(&mut self) {
        self.session = None;
    }
}

impl ProjectHostSession {
    fn active(project: EditableProject) -> Self {
        Self {
            project,
            phase: ProjectHostPhase::Active,
        }
    }

    fn recovery_pending(project: EditableProject, checkpoint: RecoveryCheckpoint) -> Self {
        Self {
            project,
            phase: ProjectHostPhase::RecoveryPending(Box::new(checkpoint)),
        }
    }

    fn startup_projection(&self) -> EditorProjection {
        self.project.projection()
    }

    fn recovery_status(&self) -> ProjectRecoveryStatus {
        if self.phase.is_recovery_pending() {
            ProjectRecoveryStatus::Available
        } else {
            ProjectRecoveryStatus::None
        }
    }

    fn projection(&self) -> Result<EditorProjection, String> {
        if self.phase.permits_projection() {
            Ok(self.project.projection())
        } else {
            Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
        }
    }

    fn active_project(&self) -> Result<&EditableProject, String> {
        if self.phase.is_active() {
            Ok(&self.project)
        } else {
            Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
        }
    }

    fn active_project_mut(&mut self) -> Result<&mut EditableProject, String> {
        if self.phase.is_active() {
            Ok(&mut self.project)
        } else {
            Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
        }
    }

    fn take_recovery(&mut self) -> Result<Box<RecoveryCheckpoint>, String> {
        let phase = std::mem::replace(&mut self.phase, ProjectHostPhase::Active);
        match phase {
            ProjectHostPhase::RecoveryPending(checkpoint) => Ok(checkpoint),
            other => {
                self.phase = other;
                Err(SESSION_UNAVAILABLE_MESSAGE.to_string())
            }
        }
    }

    fn restore_recovery(&mut self, checkpoint: Box<RecoveryCheckpoint>) {
        debug_assert!(self.phase.is_active());
        self.phase = ProjectHostPhase::RecoveryPending(checkpoint);
    }

    fn begin_close_confirmation(&mut self) {
        debug_assert!(self.phase.is_active());
        self.phase = ProjectHostPhase::ClosePending;
    }

    fn cancel_close(&mut self) -> bool {
        if self.phase.is_close_pending() {
            self.phase = ProjectHostPhase::Active;
            true
        } else {
            false
        }
    }
}

impl ProjectHostPhase {
    fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    fn is_recovery_pending(&self) -> bool {
        matches!(self, Self::RecoveryPending(_))
    }

    fn is_close_pending(&self) -> bool {
        matches!(self, Self::ClosePending)
    }

    fn permits_projection(&self) -> bool {
        !self.is_recovery_pending()
    }
}

pub(crate) struct ProjectHostSaveResult {
    pub(crate) outcome: SaveProjectOutcome,
    pub(crate) projection: EditorProjection,
}

pub(crate) struct ProjectHostSaveAsResult {
    pub(crate) outcome: SaveAsProjectOutcome,
    pub(crate) projection: EditorProjection,
}

#[derive(Debug)]
pub(crate) struct FrozenSheetExport {
    pub(crate) snapshot: RenderSnapshot,
    pub(crate) output_unit: ComposedOutputUnit,
    pub(crate) sources: Vec<RenderSource>,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorizedMediaCatalog {
    pub(crate) project_id: String,
    pub(crate) bindings: Vec<MediaBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectCloseRequestOutcome {
    CloseImmediately,
    ConfirmationRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectRecoveryStatus {
    None,
    Available,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectRecoveryDecision {
    ReopenAndRecover,
    DiscardCheckpointAndOpenLastSaved,
    NowNot,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProjectRecoveryResolution {
    Recovered(EditorProjection),
    OpenedLastSaved(EditorProjection),
    Deferred,
}

#[derive(Debug)]
pub(crate) enum ProjectHostSaveError {
    Project(SaveProjectError),
    RecoveryCleanupFailed,
    SessionUnavailable,
}

#[derive(Debug)]
pub(crate) enum ProjectHostSaveAsError {
    Project(SaveAsProjectError),
    SessionUnavailable,
}

impl ProjectHost {
    #[cfg(test)]
    pub(crate) fn new(project: EditableProject) -> Self {
        Self {
            state: Arc::new(Mutex::new(ProjectHostState::new(project, None))),
            recovery: None,
        }
    }

    pub(crate) fn with_recovery(
        project: EditableProject,
        recovery: RecoveryCoordinator,
    ) -> io::Result<Self> {
        let authority = project.identity_authority().clone();
        let state = ProjectHostState::new(project, recovery.load(&authority)?);
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            recovery: Some(recovery),
        })
    }

    pub(crate) fn startup_projection(&self) -> Result<EditorProjection, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        Ok(state.session()?.startup_projection())
    }

    pub(crate) fn recovery_status(&self) -> Result<ProjectRecoveryStatus, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        Ok(state.session()?.recovery_status())
    }

    pub(crate) fn resolve_recovery(
        &self,
        decision: ProjectRecoveryDecision,
    ) -> Result<ProjectRecoveryResolution, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let mut session = state.take_session()?;
        let checkpoint = match session.take_recovery() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                state.restore_session(session);
                return Err(error);
            }
        };
        match decision {
            ProjectRecoveryDecision::ReopenAndRecover => {
                match session
                    .project
                    .restore_recovery(checkpoint.as_ref().clone())
                {
                    Ok(projection) => {
                        state.restore_session(session);
                        Ok(ProjectRecoveryResolution::Recovered(projection))
                    }
                    Err(error) => {
                        session.restore_recovery(checkpoint);
                        state.restore_session(session);
                        Err(error.to_string())
                    }
                }
            }
            ProjectRecoveryDecision::DiscardCheckpointAndOpenLastSaved => {
                if let Err(error) = self.finish_recovery(&session.project) {
                    session.restore_recovery(checkpoint);
                    state.restore_session(session);
                    return Err(error.to_string());
                }
                let projection = session.project.projection();
                state.restore_session(session);
                Ok(ProjectRecoveryResolution::OpenedLastSaved(projection))
            }
            ProjectRecoveryDecision::NowNot => {
                drop((session, checkpoint));
                Ok(ProjectRecoveryResolution::Deferred)
            }
        }
    }

    pub(crate) fn projection(&self) -> Result<EditorProjection, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        state.session()?.projection()
    }

    pub(crate) fn apply_with_outcome(
        &self,
        intent: ProjectIntent,
    ) -> Result<ProjectMutationOutcome, String> {
        let mut project = self.project()?;
        let outcome = project
            .apply_with_outcome(intent)
            .map_err(|error| error.to_string())?;
        self.schedule_recovery(&project);
        Ok(outcome)
    }

    pub(crate) fn import_photo(
        &self,
        proposal: PhotoImportProposal,
    ) -> Result<ImportPhotoOutcome, String> {
        let mut project = self.project()?;
        let outcome = project
            .import_photo(proposal.into_command())
            .map_err(|error| error.to_string())?;
        if outcome.disposition == ImportPhotoDisposition::Imported {
            self.schedule_recovery(&project);
        }
        Ok(outcome)
    }

    pub(crate) fn relink_media(
        &self,
        proposal: MediaRelinkProposal,
    ) -> Result<EditorProjection, String> {
        let media_id: MediaId = proposal
            .media_id()
            .parse()
            .map_err(|error| format!("A ocorrência de mídia é inválida: {error}"))?;
        let mut project = self.project()?;
        let current = project
            .project()
            .media()
            .iter()
            .find(|media| media.id() == media_id.into_uuid())
            .ok_or_else(|| format!("A ocorrência de mídia não existe: {media_id}"))?;
        if current.kind() != proposal.kind() || current.path() != proposal.expected_logical_path() {
            return Err("A referência de mídia mudou durante a Religação; tente novamente.".into());
        }
        let source_metadata = proposal.source_metadata().cloned();
        project
            .relink_media(RelinkMedia::new(
                media_id,
                proposal.replacement_path().to_path_buf(),
            ))
            .map_err(|error| error.to_string())?;
        if let Some(source_metadata) = source_metadata {
            project
                .observe_photo_source(media_id, source_metadata)
                .map_err(|error| error.to_string())?;
        }
        let projection = project.projection();
        self.schedule_recovery(&project);
        Ok(projection)
    }

    pub(crate) fn project_photo_drop_target(
        &self,
        sheet_id: &str,
        x_um: i64,
        y_um: i64,
    ) -> Result<PhotoDropTarget, String> {
        self.project()?
            .photo_drop_target(sheet_id, x_um, y_um)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn observe_photo_source(
        &self,
        binding: &MediaBinding,
        metadata: PhotoSourceMetadata,
    ) -> Result<(), String> {
        if binding.kind != myalbuns_core::MediaKind::Photo {
            return Err("A ocorrência observada não é uma Foto.".into());
        }
        let media_id: MediaId = binding
            .media_id
            .parse()
            .map_err(|error| format!("A ocorrência de Foto é inválida: {error}"))?;
        let mut project = self.project()?;
        let current = project
            .project()
            .media()
            .iter()
            .find(|media| media.id() == media_id.into_uuid())
            .ok_or_else(|| format!("A ocorrência de Foto não existe: {media_id}"))?;
        if current.kind() != binding.kind || current.path() != binding.logical_path.as_path() {
            return Err("O vínculo da Foto mudou durante a observação.".into());
        }
        project
            .observe_photo_source(media_id, metadata)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn undo(&self) -> Result<EditorProjection, String> {
        let mut project = self.project()?;
        let projection = project
            .undo()
            .ok_or_else(|| "Não há uma ação produtiva para desfazer neste corte.".to_string())?;
        self.schedule_recovery(&project);
        Ok(projection)
    }

    pub(crate) fn redo(&self) -> Result<EditorProjection, String> {
        let mut project = self.project()?;
        let projection = project
            .redo()
            .ok_or_else(|| "Não há uma ação produtiva para refazer neste corte.".to_string())?;
        self.schedule_recovery(&project);
        Ok(projection)
    }

    pub(crate) fn save(
        &self,
        expected_revision: u64,
    ) -> Result<ProjectHostSaveResult, ProjectHostSaveError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        let result = state
            .active_project_mut()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?
            .save(expected_revision);
        match result {
            Ok(outcome) => {
                let project = state
                    .active_project()
                    .expect("saving an active Project preserves its state");
                if let Err(error) = self.finish_recovery(project) {
                    tracing::error!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "project_recovery_checkpoint_finish_after_save_failed",
                    );
                    return Err(ProjectHostSaveError::RecoveryCleanupFailed);
                }
                Ok(ProjectHostSaveResult {
                    outcome,
                    projection: project.projection(),
                })
            }
            Err(SaveProjectError::SaveStateIndeterminate) => {
                state.consume();
                Err(ProjectHostSaveError::Project(
                    SaveProjectError::SaveStateIndeterminate,
                ))
            }
            Err(error) => Err(ProjectHostSaveError::Project(error)),
        }
    }

    #[cfg(test)]
    pub(crate) fn save_as(
        &self,
        request: SaveAsProjectRequest,
    ) -> Result<ProjectHostSaveAsResult, ProjectHostSaveAsError> {
        let recovery = self.recovery.clone();
        self.save_as_with_transition(request, move |previous, _, _| {
            recovery
                .as_ref()
                .map_or(Ok(false), |recovery| recovery.finish(previous))
                .map(|_| ())
                .map_err(|_| ())
        })
    }

    pub(crate) fn save_as_with_transition(
        &self,
        request: SaveAsProjectRequest,
        transition: impl FnOnce(
            &ProjectIdentityAuthority,
            &ProjectIdentityAuthority,
            SaveAsProjectOutcome,
        ) -> Result<(), ()>,
    ) -> Result<ProjectHostSaveAsResult, ProjectHostSaveAsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProjectHostSaveAsError::SessionUnavailable)?;
        let project = state
            .active_project_mut()
            .map_err(|_| ProjectHostSaveAsError::SessionUnavailable)?;
        let previous_authority = project.identity_authority().clone();
        let result = project.save_as_with_transition(request, move |authority, outcome| {
            transition(&previous_authority, authority, outcome)
        });
        match result {
            Ok(outcome) => {
                let project = state
                    .active_project()
                    .expect("saving an active Project as preserves its Host state");
                Ok(ProjectHostSaveAsResult {
                    outcome,
                    projection: project.projection(),
                })
            }
            Err(error) => Err(ProjectHostSaveAsError::Project(error)),
        }
    }

    pub(crate) fn begin_close(&self) -> Result<ProjectCloseRequestOutcome, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let mut session = state.take_session()?;
        if session.phase.is_recovery_pending() {
            return Ok(ProjectCloseRequestOutcome::CloseImmediately);
        }
        if session.phase.is_close_pending() {
            state.restore_session(session);
            return Ok(ProjectCloseRequestOutcome::ConfirmationRequired);
        }
        if session.project.has_unsaved_changes() {
            session.begin_close_confirmation();
            state.restore_session(session);
            return Ok(ProjectCloseRequestOutcome::ConfirmationRequired);
        }
        match self.finish_recovery(&session.project) {
            Ok(_) => Ok(ProjectCloseRequestOutcome::CloseImmediately),
            Err(error) => {
                state.restore_session(session);
                Err(error.to_string())
            }
        }
    }

    pub(crate) fn cancel_close(&self) -> Result<EditorProjection, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let mut session = state.take_session()?;
        if !session.cancel_close() {
            state.restore_session(session);
            return Err(SESSION_UNAVAILABLE_MESSAGE.to_string());
        }
        let projection = session.project.projection();
        state.restore_session(session);
        Ok(projection)
    }

    pub(crate) fn discard_close(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        let session = state.take_session()?;
        if !session.phase.is_close_pending() {
            state.restore_session(session);
            return Err(SESSION_UNAVAILABLE_MESSAGE.to_string());
        }
        match self.finish_recovery(&session.project) {
            Ok(_) => Ok(()),
            Err(error) => {
                state.restore_session(session);
                Err(error.to_string())
            }
        }
    }

    pub(crate) fn save_and_close(&self) -> Result<SaveProjectOutcome, ProjectHostSaveError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        let mut session = state
            .take_session()
            .map_err(|_| ProjectHostSaveError::SessionUnavailable)?;
        if !session.phase.is_close_pending() {
            state.restore_session(session);
            return Err(ProjectHostSaveError::SessionUnavailable);
        }
        let revision = session.project.revision();
        match session.project.save(revision) {
            Ok(outcome) => match self.finish_recovery(&session.project) {
                Ok(_) => Ok(outcome),
                Err(error) => {
                    tracing::error!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "project_recovery_checkpoint_finish_after_close_save_failed",
                    );
                    session.phase = ProjectHostPhase::Active;
                    state.restore_session(session);
                    Err(ProjectHostSaveError::RecoveryCleanupFailed)
                }
            },
            Err(SaveProjectError::SaveStateIndeterminate) => Err(ProjectHostSaveError::Project(
                SaveProjectError::SaveStateIndeterminate,
            )),
            Err(error) => {
                session.phase = ProjectHostPhase::Active;
                state.restore_session(session);
                Err(ProjectHostSaveError::Project(error))
            }
        }
    }

    pub(crate) fn authorized_media_catalog(&self) -> Result<AuthorizedMediaCatalog, String> {
        let project = self.project()?;
        Ok(AuthorizedMediaCatalog {
            project_id: project.project_id().hyphenated().to_string(),
            bindings: project
                .project()
                .media()
                .iter()
                .map(|media| MediaBinding {
                    media_id: media.id().hyphenated().to_string(),
                    kind: media.kind(),
                    logical_path: media.path().to_path_buf(),
                })
                .collect(),
        })
    }

    #[cfg(test)]
    pub(crate) fn identity_authority(&self) -> Result<ProjectIdentityAuthority, String> {
        Ok(self.project()?.identity_authority().clone())
    }

    #[cfg(test)]
    pub(crate) fn authorized_media_binding(&self, media_id: &str) -> Result<MediaBinding, String> {
        self.authorized_media_catalog()?
            .bindings
            .into_iter()
            .find(|binding| binding.media_id == media_id)
            .ok_or_else(|| "A ocorrência de mídia não pertence ao Projeto atual.".into())
    }

    pub(crate) fn freeze_sheet_export(&self, sheet_id: &str) -> Result<FrozenSheetExport, String> {
        let frozen = self
            .project()?
            .freeze_rendering()
            .into_sheet(sheet_id)
            .map_err(|error| error.to_string())?;
        let (snapshot, output_unit, frozen_sources) = frozen.into_parts();
        let sources = frozen_sources
            .into_iter()
            .map(|media| {
                RenderSource::new(
                    MediaId::try_from(media.id())
                        .expect("persisted MediaRef identities are canonical UUID v4"),
                    media.path().to_path_buf(),
                )
                .map_err(|error| format!("A fonte original congelada é inválida: {error}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(FrozenSheetExport {
            snapshot,
            output_unit,
            sources,
        })
    }

    fn project(&self) -> Result<ActiveProject<'_>, String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| SESSION_UNAVAILABLE_MESSAGE.to_string())?;
        guard.active_project()?;
        Ok(ActiveProject { guard })
    }

    fn schedule_recovery(&self, project: &EditableProject) {
        let Some(recovery) = &self.recovery else {
            return;
        };
        match project.recovery_checkpoint() {
            Ok(checkpoint) => {
                if let Err(error) =
                    recovery.schedule(project.identity_authority().clone(), checkpoint)
                {
                    tracing::error!(
                        target: "myalbuns.desktop",
                        error = %error,
                        event = "project_recovery_checkpoint_schedule_failed",
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    target: "myalbuns.desktop",
                    error = %error,
                    event = "project_recovery_checkpoint_build_failed",
                );
            }
        }
    }

    fn finish_recovery(&self, project: &EditableProject) -> io::Result<bool> {
        self.recovery.as_ref().map_or(Ok(false), |recovery| {
            recovery.finish(project.identity_authority())
        })
    }
}

struct ActiveProject<'a> {
    guard: MutexGuard<'a, ProjectHostState>,
}

impl std::ops::Deref for ActiveProject<'_> {
    type Target = EditableProject;

    fn deref(&self) -> &Self::Target {
        self.guard
            .active_project()
            .expect("an ActiveProject guard always contains an active Project")
    }
}

impl std::ops::DerefMut for ActiveProject<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .active_project_mut()
            .expect("an ActiveProject guard always contains an active Project")
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use image::{GenericImageView, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat,
        ImportPhotoDisposition, InitialBackground, InitialBackgroundContent, InitialFrameBorder,
        InitialOverlay, InitialProject, InitialProjectConfiguration, InitialProjectPersonalization,
        MediaKind, OpenProjectRequest, PhotoPlacementMode, ProjectCore, ProjectIntent,
        ProjectLocation, SaveAsAuthorization, SaveAsProjectRequest, SaveProjectError,
        SaveProjectOutcome,
    };
    use myalbuns_paths::{AppPaths, ExportWriteAuthorization, OperationPathContext};

    use super::{
        ProjectCloseRequestOutcome, ProjectHost, ProjectHostSaveError, ProjectRecoveryDecision,
        ProjectRecoveryResolution, ProjectRecoveryStatus,
    };
    use crate::{
        export_pipeline,
        imaging_processor::InvocationContext,
        imaging_recovery_integration::RealProcessTransport,
        media_runtime::{MediaMonitor, MediaResolver, MediaRuntime},
        path_io,
        project_recovery::{RecoveryCoordinator, RecoveryStore},
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
            .with_identity_storage_roots(
                identity_lease_root.clone(),
                root.path().join("identities"),
            )
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
        let host = ProjectHost::new(open_editable_project(project_path, identity_lease_root));
        hydrate_reopened_photos(&host);
        host
    }

    struct RecoveryFixture {
        _root: tempfile::TempDir,
        project_path: PathBuf,
        identity_lease_root: PathBuf,
        authority: myalbuns_core::ProjectIdentityAuthority,
        store: RecoveryStore,
        coordinator: RecoveryCoordinator,
        host: ProjectHost,
    }

    fn recovery_fixture() -> RecoveryFixture {
        let root = tempfile::tempdir().expect("temporary Recovery Host fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let identity_lease_root = root.path().join("leases");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Recovery fixture root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(
                identity_lease_root.clone(),
                root.path().join("identities"),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the productive Recovery Project is created");
        let authority = project.identity_authority().clone();
        let store = RecoveryStore::new(AppPaths::from_roots(
            &root.path().join("roaming"),
            &root.path().join("local"),
        ));
        let coordinator = RecoveryCoordinator::with_delay(store.clone(), Duration::from_millis(50));
        let host = ProjectHost::with_recovery(project, coordinator.clone())
            .expect("the Host starts without a prior checkpoint");
        RecoveryFixture {
            _root: root,
            project_path,
            identity_lease_root,
            authority,
            store,
            coordinator,
            host,
        }
    }

    fn open_editable_project(
        project_path: &Path,
        identity_lease_root: &Path,
    ) -> myalbuns_core::EditableProject {
        let mut context = OperationPathContext::new();
        context
            .capture(project_path)
            .expect("the reopened fixture root is captured");
        ProjectCore::new()
            .with_identity_storage_roots(
                identity_lease_root.to_path_buf(),
                identity_lease_root
                    .parent()
                    .expect("the fixture lease root has a parent")
                    .join("identities"),
            )
            .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                project_path.to_path_buf(),
                context.freeze(),
            )))
            .expect("the saved Project reopens in a new editable Session")
    }

    fn hydrate_reopened_photos(host: &ProjectHost) {
        for binding in host
            .authorized_media_catalog()
            .expect("the reopened media catalog is available")
            .bindings
            .into_iter()
            .filter(|binding| binding.kind == MediaKind::Photo)
        {
            let metadata = MediaResolver
                .inspect_photo_binding(&binding)
                .expect("the reopened Photo Original is inspected");
            host.observe_photo_source(&binding, metadata)
                .expect("the reopened Photo metadata is hydrated");
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
    fn completed_host_actions_publish_the_latest_checkpoint_and_save_finishes_it() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
                .expect("the first completed Host action is accepted");
            let latest = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 180 })
                .expect("the nearby Host action is accepted")
                .projection;

            tokio::time::sleep(Duration::from_millis(20)).await;
            assert!(
                fixture
                    .store
                    .load(&fixture.authority)
                    .expect("the namespace is readable before publication")
                    .is_none()
            );
            tokio::time::sleep(Duration::from_millis(90)).await;
            let bytes = fixture
                .store
                .load(&fixture.authority)
                .expect("the Host checkpoint is readable")
                .expect("the Host checkpoint is published")
                .to_bytes()
                .expect("the Host checkpoint serializes");
            let checkpoint: serde_json::Value =
                serde_json::from_slice(&bytes).expect("the Host checkpoint is valid JSON");
            assert_eq!(
                checkpoint["creativeState"]["project"]["document"]["dpi"],
                180
            );

            fixture
                .host
                .save(latest.state.revision)
                .expect("Save completes the current checkpoint");
            assert!(
                fixture
                    .store
                    .load(&fixture.authority)
                    .expect("the namespace is readable after Save")
                    .is_none()
            );
        });
    }

    #[test]
    fn save_reports_failed_recovery_cleanup_until_a_later_save_finishes_it() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let dirty = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable")
                .projection;
            tokio::time::sleep(Duration::from_millis(90)).await;
            let checkpoint = fixture
                .store
                .checkpoint_path(&fixture.authority)
                .expect("the checkpoint path is valid");
            std::fs::remove_file(&checkpoint).expect("the published checkpoint is replaceable");
            std::fs::create_dir(&checkpoint)
                .expect("a directory temporarily blocks Recovery cleanup");

            assert!(matches!(
                fixture.host.save(dirty.state.revision),
                Err(ProjectHostSaveError::RecoveryCleanupFailed)
            ));
            let persisted = fixture
                .host
                .projection()
                .expect("the conclusively saved Session remains available");
            assert_eq!(persisted.state.saved_revision, dirty.state.revision);
            assert!(!persisted.state.dirty);
            assert!(checkpoint.is_dir(), "failed cleanup preserves its evidence");

            std::fs::remove_dir(&checkpoint).expect("the local obstruction is released");
            let retried = fixture
                .host
                .save(dirty.state.revision)
                .expect("a later Save completes the pending cleanup");
            assert_eq!(
                retried.outcome,
                SaveProjectOutcome::AlreadyCurrent {
                    revision: dirty.state.revision
                }
            );
            assert!(!checkpoint.exists());
        });
    }

    #[test]
    fn failed_cleanup_after_save_and_close_keeps_the_saved_session_open() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let dirty = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable")
                .projection;
            tokio::time::sleep(Duration::from_millis(90)).await;
            let checkpoint = fixture
                .store
                .checkpoint_path(&fixture.authority)
                .expect("the checkpoint path is valid");
            std::fs::remove_file(&checkpoint).expect("the published checkpoint is replaceable");
            std::fs::create_dir(&checkpoint)
                .expect("a directory temporarily blocks Recovery cleanup");
            assert_eq!(
                fixture.host.begin_close(),
                Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
            );

            assert!(matches!(
                fixture.host.save_and_close(),
                Err(ProjectHostSaveError::RecoveryCleanupFailed)
            ));
            let persisted = fixture
                .host
                .projection()
                .expect("the saved Session remains open after cleanup failure");
            assert_eq!(persisted.state.saved_revision, dirty.state.revision);
            assert!(!persisted.state.dirty);
            assert!(checkpoint.is_dir());

            std::fs::remove_dir(&checkpoint).expect("the local obstruction is released");
            assert_eq!(
                fixture.host.begin_close(),
                Ok(ProjectCloseRequestOutcome::CloseImmediately)
            );
            assert!(!checkpoint.exists());
        });
    }

    #[test]
    fn reimporting_an_existing_photo_does_not_create_a_checkpoint() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let photo_path = fixture._root.path().join("Foto existente.jpg");
            RgbImage::from_pixel(48, 32, Rgb([20, 120, 220]))
                .save_with_format(&photo_path, ImageFormat::Jpeg)
                .expect("the Photo Original is written");
            let imported = fixture
                .host
                .import_photo(
                    MediaResolver
                        .propose_photo_import(photo_path.clone())
                        .expect("the first import is inspected"),
                )
                .expect("the Photo is imported");
            assert_eq!(imported.disposition, ImportPhotoDisposition::Imported);
            fixture
                .host
                .save(imported.projection.state.revision)
                .expect("the imported Photo is saved and its checkpoint is finished");
            assert!(fixture.store.load(&fixture.authority).unwrap().is_none());

            let selected = fixture
                .host
                .import_photo(
                    MediaResolver
                        .propose_photo_import(photo_path)
                        .expect("the repeated import is inspected"),
                )
                .expect("the existing Photo is selected");
            assert_eq!(selected.disposition, ImportPhotoDisposition::Existing);
            assert!(!selected.projection.state.dirty);
            tokio::time::sleep(Duration::from_millis(90)).await;

            assert!(
                fixture.store.load(&fixture.authority).unwrap().is_none(),
                "selecting existing media is not a completed creative action"
            );
        });
    }

    #[test]
    fn a_new_host_blocks_the_editor_until_reopening_and_recovering() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable");
            tokio::time::sleep(Duration::from_millis(90)).await;
            drop(fixture.host);

            let host = ProjectHost::with_recovery(
                open_editable_project(&fixture.project_path, &fixture.identity_lease_root),
                fixture.coordinator.clone(),
            )
            .expect("the next Host detects the prior checkpoint");
            assert_eq!(host.recovery_status(), Ok(ProjectRecoveryStatus::Available));
            assert!(host.projection().is_err());
            assert!(host.undo().is_err());

            let ProjectRecoveryResolution::Recovered(recovered) = host
                .resolve_recovery(ProjectRecoveryDecision::ReopenAndRecover)
                .expect("the user reopens and recovers")
            else {
                panic!("the recovered choice must activate the recovered Session");
            };
            assert_eq!(recovered.state.document.dpi, 360);
            assert!(recovered.state.dirty);
            assert!(!recovered.state.can_undo);
            assert!(!recovered.state.can_redo);
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());
        });
    }

    #[test]
    fn recovery_pending_capabilities_transition_together_after_recovery() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable");
            tokio::time::sleep(Duration::from_millis(90)).await;
            drop(fixture.host);

            let host = ProjectHost::with_recovery(
                open_editable_project(&fixture.project_path, &fixture.identity_lease_root),
                fixture.coordinator.clone(),
            )
            .expect("the next Host detects the prior checkpoint");

            assert_eq!(
                host.startup_projection()
                    .expect("startup may identify the saved Project")
                    .state
                    .document
                    .dpi,
                300
            );
            assert_eq!(host.recovery_status(), Ok(ProjectRecoveryStatus::Available));
            assert!(host.projection().is_err());
            assert!(
                host.apply_with_outcome(ProjectIntent::SetDpi { dpi: 420 })
                    .is_err()
            );

            let ProjectRecoveryResolution::Recovered(recovered) = host
                .resolve_recovery(ProjectRecoveryDecision::ReopenAndRecover)
                .expect("the recovery decision activates one editable Session")
            else {
                panic!("the recovery decision must return the recovered projection");
            };
            assert_eq!(recovered.state.document.dpi, 360);
            assert_eq!(host.recovery_status(), Ok(ProjectRecoveryStatus::None));
            assert_eq!(
                host.apply_with_outcome(ProjectIntent::SetDpi { dpi: 420 })
                    .expect("editing becomes available with the active Session")
                    .projection
                    .state
                    .document
                    .dpi,
                420
            );
        });
    }

    #[test]
    fn explicit_discard_opens_the_last_saved_version_and_finishes_recovery() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable");
            tokio::time::sleep(Duration::from_millis(90)).await;
            drop(fixture.host);
            let host = ProjectHost::with_recovery(
                open_editable_project(&fixture.project_path, &fixture.identity_lease_root),
                fixture.coordinator.clone(),
            )
            .expect("the next Host detects the prior checkpoint");

            assert_eq!(host.recovery_status(), Ok(ProjectRecoveryStatus::Available));
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());

            let ProjectRecoveryResolution::OpenedLastSaved(saved) = host
                .resolve_recovery(ProjectRecoveryDecision::DiscardCheckpointAndOpenLastSaved)
                .expect("the explicit discard decision opens the persisted baseline")
            else {
                panic!("the saved choice must activate the persisted Session");
            };
            assert_eq!(saved.state.document.dpi, 300);
            assert!(!saved.state.dirty);
            assert!(!saved.state.can_undo);
            assert!(!saved.state.can_redo);
            assert!(fixture.store.load(&fixture.authority).unwrap().is_none());
        });
    }

    #[test]
    fn a_checkpoint_from_an_older_saved_base_is_never_discarded_implicitly() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let changed = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the older-base action becomes recoverable")
                .projection;
            tokio::time::sleep(Duration::from_millis(90)).await;
            let older_checkpoint = fixture
                .store
                .load(&fixture.authority)
                .expect("the older checkpoint is readable")
                .expect("the older checkpoint exists");
            fixture
                .host
                .save(changed.state.revision)
                .expect("the persisted baseline advances after the checkpoint");
            drop(fixture.host);
            fixture
                .store
                .publish(&fixture.authority, &older_checkpoint)
                .expect("the interrupted older checkpoint is restored as evidence");

            let host = ProjectHost::with_recovery(
                open_editable_project(&fixture.project_path, &fixture.identity_lease_root),
                fixture.coordinator.clone(),
            )
            .expect("the Host preserves the older-base checkpoint");

            assert_eq!(host.recovery_status(), Ok(ProjectRecoveryStatus::Available));
            assert!(
                host.resolve_recovery(ProjectRecoveryDecision::ReopenAndRecover)
                    .is_err()
            );
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());

            let ProjectRecoveryResolution::OpenedLastSaved(saved) = host
                .resolve_recovery(ProjectRecoveryDecision::DiscardCheckpointAndOpenLastSaved)
                .expect("explicit confirmation discards the older checkpoint")
            else {
                panic!("the saved baseline must become active after confirmation");
            };
            assert_eq!(saved.state.document.dpi, 360);
            assert!(!saved.state.dirty);
            assert!(fixture.store.load(&fixture.authority).unwrap().is_none());
        });
    }

    #[test]
    fn now_not_preserves_recovery_and_releases_the_project_for_another_host() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the completed action becomes recoverable");
            tokio::time::sleep(Duration::from_millis(90)).await;
            drop(fixture.host);
            let host = ProjectHost::with_recovery(
                open_editable_project(&fixture.project_path, &fixture.identity_lease_root),
                fixture.coordinator.clone(),
            )
            .expect("the next Host detects the prior checkpoint");

            assert_eq!(
                host.resolve_recovery(ProjectRecoveryDecision::NowNot),
                Ok(ProjectRecoveryResolution::Deferred)
            );
            assert!(host.projection().is_err());
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());

            let reopened = open_project(&fixture.project_path, &fixture.identity_lease_root);
            assert_eq!(
                reopened
                    .projection()
                    .expect("another Host acquires the released Project")
                    .state
                    .document
                    .dpi,
                300
            );
        });
    }

    #[test]
    fn save_as_failure_preserves_the_prior_recovery_and_success_changes_its_authority() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let dirty = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the prior Session becomes recoverable")
                .projection;
            tokio::time::sleep(Duration::from_millis(90)).await;
            let destination = fixture._root.path().join("Cópia independente.myalbuns");
            let mut context = OperationPathContext::new();
            context
                .capture(&destination)
                .expect("the Save As destination root is captured");
            let location = ProjectLocation::new(destination.clone(), context.freeze());

            assert!(matches!(
                fixture.host.save_as(SaveAsProjectRequest::new(
                    dirty.state.revision + 1,
                    location.clone(),
                    SaveAsAuthorization::CreateOnly,
                )),
                Err(super::ProjectHostSaveAsError::Project(
                    myalbuns_core::SaveAsProjectError::StaleRevision { .. }
                ))
            ));
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());
            assert_eq!(
                fixture.host.identity_authority().unwrap(),
                fixture.authority
            );

            let saved_as = fixture
                .host
                .save_as(SaveAsProjectRequest::new(
                    dirty.state.revision,
                    location,
                    SaveAsAuthorization::CreateOnly,
                ))
                .expect("successful Save As adopts the independent identity");
            assert!(fixture.store.load(&fixture.authority).unwrap().is_none());
            let next_authority = fixture.host.identity_authority().unwrap();
            assert_ne!(next_authority, fixture.authority);
            assert_eq!(next_authority.project_id(), saved_as.outcome.project_id);

            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 420 })
                .expect("new changes belong to the adopted identity");
            tokio::time::sleep(Duration::from_millis(90)).await;
            assert!(fixture.store.load(&fixture.authority).unwrap().is_none());
            assert!(fixture.store.load(&next_authority).unwrap().is_some());
            assert!(destination.is_file());
        });
    }

    #[test]
    fn confirmed_clean_close_discard_and_save_close_finish_the_checkpoint() {
        tauri::async_runtime::block_on(async {
            let clean = recovery_fixture();
            clean
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the action is completed before Undo");
            clean.host.undo().expect("Undo returns to the saved state");
            tokio::time::sleep(Duration::from_millis(90)).await;
            assert!(clean.store.load(&clean.authority).unwrap().is_some());
            assert_eq!(
                clean.host.begin_close(),
                Ok(ProjectCloseRequestOutcome::CloseImmediately)
            );
            assert!(clean.store.load(&clean.authority).unwrap().is_none());

            let discarded = recovery_fixture();
            discarded
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the discarded action is completed");
            tokio::time::sleep(Duration::from_millis(90)).await;
            assert_eq!(
                discarded.host.begin_close(),
                Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
            );
            discarded
                .host
                .discard_close()
                .expect("explicit discard confirms checkpoint removal");
            assert!(
                discarded
                    .store
                    .load(&discarded.authority)
                    .unwrap()
                    .is_none()
            );

            let saved = recovery_fixture();
            saved
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the saved action is completed");
            tokio::time::sleep(Duration::from_millis(90)).await;
            assert_eq!(
                saved.host.begin_close(),
                Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
            );
            saved
                .host
                .save_and_close()
                .expect("Save and close finishes the checkpoint");
            assert!(saved.store.load(&saved.authority).unwrap().is_none());
        });
    }

    #[test]
    fn a_conclusive_close_save_failure_preserves_the_checkpoint_and_session() {
        tauri::async_runtime::block_on(async {
            let fixture = recovery_fixture();
            let dirty = fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 360 })
                .expect("the action is completed before the failed Save")
                .projection;
            tokio::time::sleep(Duration::from_millis(90)).await;
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
            assert_eq!(fixture.host.projection().unwrap(), dirty);
            assert!(fixture.store.load(&fixture.authority).unwrap().is_some());
        });
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
            .expect("the neutral Exportação is frozen");

        assert!(frozen.snapshot.validate().is_ok());
        assert!(frozen.sources.is_empty());
    }

    #[test]
    fn freezes_one_visible_sheet_unsaved_dpi_and_its_exact_originals_without_mutating_project() {
        let root = tempfile::tempdir().expect("temporary Imagem decorativa fixture");
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the current unsaved DPI is applied")
            .projection;
        let persisted_before =
            std::fs::read(&fixture.project_path).expect("the Projeto baseline is readable");
        let sheet_id = dirty.composition.sheets[1].sheet_id.clone();

        let frozen = fixture
            .host
            .freeze_sheet_export(&sheet_id)
            .expect("the noninitial visible Lâmina is frozen atomically");

        assert_eq!(frozen.snapshot.revision, dirty.state.revision);
        assert_eq!(frozen.snapshot.dpi, 240);
        assert_eq!(
            frozen
                .snapshot
                .output_unit(&sheet_id)
                .expect("the selected Lâmina remains in the frozen snapshot")
                .sheet
                .sheet_id,
            sheet_id
        );
        assert_eq!(
            fixture
                .host
                .projection()
                .expect("the Projeto remains readable"),
            dirty
        );
        assert_eq!(
            std::fs::read(&fixture.project_path).expect("the Projeto remains persisted"),
            persisted_before
        );

        let frozen_paths = frozen
            .sources
            .iter()
            .map(|source| source.source_path().to_path_buf())
            .collect::<Vec<_>>();
        assert_eq!(frozen_paths, [shared_path, right_path]);
        assert_eq!(frozen.sources.len(), 2, "a reused original is listed once");

        fixture
            .host
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 180 })
            .expect("the live Projeto may advance after freezing");
        assert_eq!(frozen.snapshot.dpi, 240);
        assert_eq!(frozen.snapshot.revision, dirty.state.revision);
    }

    #[test]
    #[ignore = "executed by scripts/Test-Rust.ps1 with the freshly built real sidecar"]
    fn reopened_project_exports_the_frozen_visible_sheet_through_the_real_processor() {
        tauri::async_runtime::block_on(async {
            let executable = PathBuf::from(
                std::env::var_os(TEST_PROCESSOR_ENV)
                    .expect("the real Processador executable path is configured"),
            );
            assert!(
                executable.is_file(),
                "the real Processador executable exists"
            );

            let media_root = tempfile::tempdir().expect("temporary E2E media fixture");
            let shared_path = media_root.path().join("shared-overlay.png");
            let right_path = media_root.path().join("right-background.jpg");
            let photo_path = media_root.path().join("linked-photo.jpg");
            RgbaImage::from_pixel(48, 32, Rgba([240, 10, 10, 128]))
                .save_with_format(&shared_path, ImageFormat::Png)
                .expect("the transparent shared original is written");
            RgbImage::from_pixel(48, 32, Rgb([10, 20, 240]))
                .save_with_format(&right_path, ImageFormat::Jpeg)
                .expect("the right Background original is written");
            RgbImage::from_pixel(300, 200, Rgb([30, 210, 70]))
                .save_with_format(&photo_path, ImageFormat::Jpeg)
                .expect("the linked Photo Original is written");
            let original_photo_bytes =
                std::fs::read(&photo_path).expect("the Photo Original is readable");
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
            let sheet_id = host
                .projection()
                .expect("the new Projeto projection is available")
                .composition
                .sheets[1]
                .sheet_id
                .clone();
            let imported = host
                .import_photo(
                    MediaResolver
                        .propose_photo_import(photo_path.clone())
                        .expect("the JPEG is inspected through the native import seam"),
                )
                .expect("the Photo link is imported into the Projeto");
            let placed = host
                .apply_with_outcome(ProjectIntent::AddPhoto {
                    sheet_id: sheet_id.clone(),
                    media_id: imported.media_id,
                    mode: PhotoPlacementMode::Normal,
                })
                .expect("the imported Photo receives the first compatible Layout");
            let affected_frame_id = placed
                .affected_frame_id
                .expect("the added Frame is returned to the UI boundary");
            let transformed = host
                .apply_with_outcome(ProjectIntent::TransformPhoto {
                    frame_id: affected_frame_id,
                    delta_pan_x: 0.4,
                    delta_pan_y: 0.2,
                    delta_zoom: 0.5,
                })
                .expect("Pan and user Zoom are persisted through the public intent");
            host.save(transformed.projection.state.revision)
                .expect("the Photo composition is saved before reopening");
            assert_eq!(
                host.begin_close(),
                Ok(ProjectCloseRequestOutcome::CloseImmediately)
            );
            let host = open_project(&project_path, &identity_lease_root);
            let persisted_before =
                std::fs::read(&project_path).expect("the reopened Projeto is readable");
            let dirty = host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 25 })
                .expect("the current unsaved DPI is applied")
                .projection;
            assert_ne!(
                dirty.composition.sheets[0].active_sides, dirty.composition.sheets[1].active_sides,
                "the initial and visible noninitial Lâminas must be semantically distinguishable"
            );
            let reopened_frame = &dirty.composition.sheets[1].frames[0];
            let reopened_photo = reopened_frame
                .photo
                .as_ref()
                .expect("the saved Frame still contains its linked Photo");
            assert_eq!(reopened_photo.media_id, imported.media_id);
            assert!((reopened_photo.placement.current_pan.x - 0.4).abs() < 0.000_001);
            assert!((reopened_photo.placement.current_pan.y - 0.2).abs() < 0.000_001);
            assert!((reopened_photo.placement.current_zoom - 1.5).abs() < 0.000_001);
            assert!(reopened_photo.placement.base_fill_zoom > 0.0);
            assert_ne!(
                reopened_photo.placement.base_fill_zoom, reopened_photo.placement.current_zoom,
                "minimum fill scale and user Zoom remain separate values"
            );
            let frozen = host
                .freeze_sheet_export(&sheet_id)
                .expect("the visible noninitial Lâmina is frozen by the Host");
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
                    sheet_id.clone(),
                    frozen.sources,
                ),
            )
            .expect("the Host snapshot owns the exact Exportação dependencies");
            let empty_cache = project_root.path().join("empty-cache");
            std::fs::create_dir(&empty_cache).expect("the empty Cache proof root exists");
            assert!(
                std::fs::read_dir(&empty_cache)
                    .expect("the Cache proof root is readable")
                    .next()
                    .is_none(),
                "the Exportação starts with an explicitly empty Cache root"
            );
            assert!(
                planned
                    .required_paths()
                    .iter()
                    .all(|path| !path.starts_with(&empty_cache)),
                "the Exportação plan contains Originals and Destino, never Cache paths"
            );
            let operation_paths = planned
                .required_paths()
                .into_iter()
                .map(Path::to_path_buf)
                .collect();
            let root_bindings = path_io::capture_root_bindings(operation_paths)
                .await
                .expect("the Exportação roots are captured once");
            let log_directory = project_root.path().join("processor-logs");
            std::fs::create_dir(&log_directory).expect("the Processador log directory exists");
            let mut transport = RealProcessTransport::stable(executable, log_directory);
            let published = export_pipeline::execute(
                &mut transport,
                planned,
                &root_bindings,
                &export_pipeline::ExportExecutionControl::default(),
                &|_| {},
                &InvocationContext::new(request_id, Some(dirty.state.project_id.clone())),
            )
            .await
            .expect("the real Processador completes Publicação of the frozen visible Lâmina");

            assert_eq!(published.completion.dpi, expected_dpi);
            assert_eq!(published.completion.source_count, 3);
            assert_eq!(
                (
                    published.completion.width_px,
                    published.completion.height_px
                ),
                (591, 295),
                "the Exportação targets the visible internal Lâmina dupla; the initial right-side Lâmina de página única would be 295 × 295"
            );
            assert_eq!(expected_revision, dirty.state.revision);
            let rendered =
                image::open(&output_path).expect("the JPEG produced by Publicação decodes");
            assert_eq!(
                rendered.dimensions(),
                (
                    published.completion.width_px,
                    published.completion.height_px
                )
            );
            let rendered = rendered.to_rgb8();
            let left = rendered.get_pixel(2, rendered.height() / 2);
            let right = rendered.get_pixel(rendered.width() - 3, rendered.height() / 2);
            assert!(
                left[0] > left[2] * 2,
                "the left Background and translucent Overlay remain visibly red"
            );
            assert!(
                right[0] > right[1] * 3 && right[2] > right[1] * 3,
                "the red translucent Overlay is composed over the blue right Background"
            );
            let actual_photo = rendered.get_pixel(rendered.width() / 2, rendered.height() / 2);
            let decoded_original = image::open(&photo_path)
                .expect("the current linked Original decodes")
                .to_rgb8();
            let source_photo = decoded_original
                .get_pixel(decoded_original.width() / 2, decoded_original.height() / 2);
            let expected_photo = [
                ((240_u16 * 128 + u16::from(source_photo[0]) * 127) / 255) as u8,
                ((10_u16 * 128 + u16::from(source_photo[1]) * 127) / 255) as u8,
                ((10_u16 * 128 + u16::from(source_photo[2]) * 127) / 255) as u8,
            ];
            let photo_max_channel_delta = (0..3)
                .map(|channel| actual_photo[channel].abs_diff(expected_photo[channel]))
                .max()
                .expect("three RGB channels are compared");
            assert!(
                photo_max_channel_delta <= 12,
                "Canvas-equivalent Photo composition and JPEG differ by {photo_max_channel_delta} channels at the sampled point"
            );
            assert_eq!(
                std::fs::read(&photo_path).expect("the Original remains readable after Exportação"),
                original_photo_bytes,
                "import, composition, save, reopen and Exportação never modify the Original"
            );
            assert_eq!(
                host.projection().expect("the Projeto remains available"),
                dirty
            );
            assert_eq!(
                std::fs::read(&project_path).expect("the Projeto remains readable"),
                persisted_before,
                "Exportação does not save or mutate the Projeto"
            );

            let missing_output_path = project_root.path().join("missing-original.jpg");
            let missing_frozen = host
                .freeze_sheet_export(&sheet_id)
                .expect("the same visible state is frozen before the Original disappears");
            let missing_plan = export_pipeline::plan(
                missing_frozen.snapshot,
                export_pipeline::ExportOptions::new(
                    "host-pipeline-missing-original",
                    missing_output_path.clone(),
                    ExportWriteAuthorization::CreateOnly,
                    sheet_id,
                    missing_frozen.sources,
                ),
            )
            .expect("the missing-Original attempt uses the same public plan");
            let missing_paths = missing_plan
                .required_paths()
                .into_iter()
                .map(Path::to_path_buf)
                .collect();
            let missing_bindings = path_io::capture_root_bindings(missing_paths)
                .await
                .expect("bindings are frozen while the Original still exists");
            std::fs::remove_file(&photo_path)
                .expect("the linked Original is removed after binding capture");
            let missing_log_directory = project_root.path().join("missing-processor-logs");
            std::fs::create_dir_all(&missing_log_directory)
                .expect("the missing-Original Processador log directory exists");
            let mut missing_transport = RealProcessTransport::stable(
                PathBuf::from(
                    std::env::var_os(TEST_PROCESSOR_ENV)
                        .expect("the real Processador remains configured"),
                ),
                missing_log_directory,
            );
            let missing_failure = export_pipeline::execute(
                &mut missing_transport,
                missing_plan,
                &missing_bindings,
                &export_pipeline::ExportExecutionControl::default(),
                &|_| {},
                &InvocationContext::new(
                    "host-pipeline-missing-original",
                    Some(dirty.state.project_id.clone()),
                ),
            )
            .await
            .expect_err("Cache cannot turn a missing Original into a successful Exportação");
            assert_eq!(
                missing_failure
                    .processor_failure
                    .as_ref()
                    .expect("the Processador reports the missing source")
                    .code,
                myalbuns_imaging_protocol::ImagingFailureCode::SourceUnavailable
            );
            assert!(
                missing_failure.message.contains("Religue"),
                "the missing-Original message tells the user how to recover: {}",
                missing_failure.message
            );
            assert!(!missing_output_path.exists());
        });
    }

    #[test]
    fn delegates_dpi_changes_and_history_to_the_productive_editable_project() {
        let fixture = fixture();

        let applied = fixture
            .host
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the productive Host applies the DPI change")
            .projection;
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing")
            .projection;

        assert_eq!(
            fixture.host.begin_close(),
            Ok(ProjectCloseRequestOutcome::ConfirmationRequired)
        );
        assert!(
            fixture
                .host
                .apply_with_outcome(ProjectIntent::SetDpi { dpi: 180 })
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing")
            .projection;
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
        host.apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing")
            .projection;
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the Project becomes dirty before closing")
            .projection;
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
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the visible revision is created")
            .projection;

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
    fn save_as_serially_adopts_the_new_identity_and_projects_its_name() {
        let fixture = fixture();
        let destination = fixture._root.path().join("Versão independente.myalbuns");
        let original_bytes = std::fs::read(&fixture.project_path)
            .expect("the original Project baseline is readable");
        let dirty = fixture
            .host
            .apply_with_outcome(ProjectIntent::SetDpi { dpi: 240 })
            .expect("the visible revision becomes dirty")
            .projection;
        let mut context = OperationPathContext::new();
        context
            .capture(&destination)
            .expect("the Save As destination root is captured");

        let saved_as = fixture
            .host
            .save_as(SaveAsProjectRequest::new(
                dirty.state.revision,
                ProjectLocation::new(destination.clone(), context.freeze()),
                SaveAsAuthorization::CreateOnly,
            ))
            .expect("the Host serializes and adopts Save As");

        assert_eq!(
            saved_as.projection.state.project_id,
            saved_as.outcome.project_id.to_string()
        );
        assert_eq!(
            saved_as.projection.state.project_name,
            "Versão independente"
        );
        assert_eq!(
            saved_as.projection.state.saved_revision,
            dirty.state.revision
        );
        assert!(!saved_as.projection.state.dirty);
        assert!(saved_as.projection.state.can_undo);
        assert_eq!(
            std::fs::read(&fixture.project_path).expect("the original remains readable"),
            original_bytes
        );
        assert!(destination.is_file());
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
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                initial,
                CreateAuthorization::CreateOnly,
            ))
            .expect("the personalized Project is created");
        let host = ProjectHost::new(project);

        let catalog = host
            .authorized_media_catalog()
            .expect("the Host can authorize its persisted media catalog");
        let projection = host.projection().expect("the Project remains available");

        assert_eq!(catalog.project_id, projection.state.project_id);
        assert_eq!(catalog.bindings.len(), 1);
        assert_eq!(
            catalog.bindings[0].media_id,
            projection.state.album.media[0].id.to_string()
        );
        assert_eq!(catalog.bindings[0].logical_path, background_path);
        let frontend_projection =
            serde_json::to_string(&projection).expect("the editor projection serializes");
        assert!(!frontend_projection.contains(root.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn public_host_runtime_retry_reinspects_without_mutating_media_ref_or_project() {
        let root = tempfile::tempdir().expect("temporary media retry Host fixture");
        let media_path = root.path().join("Background.png");
        std::fs::write(&media_path, b"linked Original")
            .expect("the linked Original fixture is writable");
        let initial =
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: media_path.clone(),
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            ));
        let fixture = fixture_with_initial(initial);
        let before = fixture
            .host
            .projection()
            .expect("the Project is available before retry authorization");
        let media_id = before.state.album.media[0].id.to_string();

        let binding = fixture
            .host
            .authorized_media_binding(&media_id)
            .expect("the Host authorizes the current occurrence binding");
        let mut unavailable_sample = binding.clone();
        unavailable_sample.logical_path = "relative-unavailable-source.png".into();
        let runtime = MediaRuntime::default();
        let resolver = MediaResolver;
        runtime.apply(resolver.observe(1, std::slice::from_ref(&unavailable_sample)));
        let retried = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect("the Runtime repeats the authoritative inspection through the Host binding");

        let after = fixture
            .host
            .projection()
            .expect("the Project remains available after retry authorization");
        assert_eq!(binding.media_id, media_id);
        assert_eq!(binding.logical_path, media_path);
        assert_eq!(
            retried.availability(),
            crate::media_runtime::MediaAvailability::Candidate
        );
        assert_eq!(after.state.revision, before.state.revision);
        assert_eq!(after.state.saved_revision, before.state.saved_revision);
        assert_eq!(after.state.dirty, before.state.dirty);
        assert_eq!(after.state.can_undo, before.state.can_undo);
        assert_eq!(after.state.can_redo, before.state.can_redo);
        assert_eq!(
            fixture
                .host
                .authorized_media_catalog()
                .expect("the authorized catalog is unchanged")
                .bindings[0],
            binding
        );
    }

    #[test]
    fn public_relink_flow_reinspects_and_invalidates_only_the_selected_occurrence() {
        let media_root = tempfile::tempdir().expect("temporary relink media fixture");
        let original_left = media_root.path().join("left.png");
        let original_right = media_root.path().join("right.png");
        let replacement = media_root.path().join("replacement.png");
        RgbaImage::from_pixel(32, 24, Rgba([40, 80, 120, 255]))
            .save_with_format(&original_left, ImageFormat::Png)
            .expect("the first occurrence is writable");
        std::fs::hard_link(&original_left, &original_right)
            .expect("the second occurrence aliases the same physical file");
        std::fs::hard_link(&original_left, &replacement)
            .expect("the relink candidate aliases the same physical file");
        let initial =
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::PerSide {
                    left: InitialBackgroundContent::Media {
                        path: original_left.clone(),
                    },
                    right: InitialBackgroundContent::Media {
                        path: original_right.clone(),
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            ));
        let fixture = fixture_with_initial(initial);
        let before = fixture
            .host
            .authorized_media_catalog()
            .expect("the persisted occurrences are authorized");
        let selected = before
            .bindings
            .iter()
            .find(|binding| binding.logical_path == original_left)
            .expect("the selected occurrence is present")
            .clone();
        let untouched = before
            .bindings
            .iter()
            .find(|binding| binding.logical_path == original_right)
            .expect("the other occurrence is present")
            .clone();
        let resolver = MediaResolver;
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();
        assert!(monitor.poll(&runtime, &before.bindings).update().is_none());
        assert!(monitor.poll(&runtime, &before.bindings).update().is_some());

        let proposal = resolver
            .propose_relink(&selected, replacement.clone())
            .expect("MediaResolver authoritatively validates the selected candidate");
        let projection = fixture
            .host
            .relink_media(proposal)
            .expect("ProjectSession owns the RelinkMedia command");

        assert_eq!(projection.state.revision, 1);
        assert!(projection.state.dirty);
        assert!(projection.state.can_undo);
        let after = fixture
            .host
            .authorized_media_catalog()
            .expect("the relinked catalog remains authorized");
        assert_eq!(
            after
                .bindings
                .iter()
                .find(|binding| binding.media_id == selected.media_id)
                .expect("the selected occurrence remains present")
                .logical_path,
            replacement
        );
        assert_eq!(
            after
                .bindings
                .iter()
                .find(|binding| binding.media_id == untouched.media_id)
                .expect("the other occurrence remains present")
                .logical_path,
            original_right
        );
        assert!(monitor.poll(&runtime, &after.bindings).update().is_none());
        let stable = monitor.poll(&runtime, &after.bindings);
        assert_eq!(
            stable.update().unwrap().changed_media_ids(),
            std::slice::from_ref(&selected.media_id)
        );
        assert_eq!(
            stable.update().unwrap().invalidated_media_ids(),
            std::slice::from_ref(&selected.media_id),
            "Cache reacts by occurrence even when every path aliases one physical file"
        );
    }
}
