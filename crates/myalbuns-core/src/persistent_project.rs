use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use myalbuns_paths::{
    ExpectedObject, OperationPathContext, PhysicalFileIdentity, PhysicalIdentityEvidence,
    ProcessInstanceId,
};
use uuid::Uuid;

use crate::{
    model::{
        CoreError, EditorProjection, ImportPhoto, ImportPhotoDisposition, ImportPhotoOutcome,
        ImportPhotosOutcome, MediaId, PhotoDropTarget, PhotoSourceMetadata, ProjectIntent,
        ProjectMutationOutcome, RelinkMedia, RenderSnapshot, RenderSnapshotMetadata,
        RenderSnapshotRef,
    },
    persistent_projection,
    persistent_session::PersistentProjectSession,
    project_document::{
        AlbumInformation, AlbumInformationValidation, InitialProject, MediaRef, ProjectDocument,
        ProjectRevision,
    },
    project_recovery::{RecoveryCheckpoint, RecoveryCheckpointError},
    project_store::{
        self, CreateStoreError, DocumentFailure, IdentityLeaseError, IdentityLeaseObservation,
        IdentityRegistryLookup, OpenStoreError, PathFailure, PendingProjectIdentityLease,
        ProjectIdentityLease, ProjectIdentityRegistry, ProjectLocation, ProjectStore,
        SaveStoreError, SaveStoreResult,
    },
};

mod new_publication;

use new_publication::{
    NewProjectTarget, NewPublicationError, PublishedNewProject, publish_new_project,
};

/// Small public seam for productive Project persistence and editable ownership.
///
/// Session, store and identity coordination remain private. Each process
/// configures its live lease and durable identity roots, then creates, opens or
/// loads Projects through this type.
#[derive(Clone, Debug, Default)]
pub struct ProjectCore {
    identity_lease_root: Option<PathBuf>,
    identity_registry_root: Option<PathBuf>,
}

/// Derives the human-facing Project name from its native pathname.
pub fn project_name_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Projeto".into())
}

impl ProjectCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_identity_storage_roots(
        mut self,
        identity_lease_root: PathBuf,
        identity_registry_root: PathBuf,
    ) -> Self {
        self.identity_lease_root = Some(identity_lease_root);
        self.identity_registry_root = Some(identity_registry_root);
        self
    }

    fn identity_lease_root(&self) -> Option<&Path> {
        self.identity_lease_root.as_deref()
    }

    fn identity_registry_root(&self) -> Option<&Path> {
        self.identity_registry_root.as_deref()
    }
}

#[derive(Debug)]
pub struct LoadedProjectRevision {
    revision: ProjectRevision,
    project_path: PathBuf,
    content_sha256: String,
}

/// Frozen creative state for independent Projects; it contains no live lease or History.
#[derive(Clone, Debug)]
pub struct ProjectTemplate {
    project: ProjectDocument,
}

impl ProjectTemplate {
    pub fn media(&self) -> &[MediaRef] {
        self.project.media()
    }
}

impl LoadedProjectRevision {
    /// Identifies exactly the bytes decoded by the read-only load, including
    /// external rewrites that did not advance the creative revision.
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    /// Resolves the persisted document through the same composition owner as
    /// an editor, without an editable identity lease, History or disk writes.
    pub fn freeze_rendering(&self) -> FrozenProjectRendering {
        FrozenProjectRendering {
            snapshot: persistent_projection::render_snapshot(
                &self.revision.project,
                self.revision.project_id,
                &project_name_from_path(&self.project_path),
                self.revision.revision,
                &HashMap::new(),
            ),
            sources: self.revision.project.media().to_vec(),
        }
    }

    /// Composes only the first saved Sheet using oriented reduced-image dimensions.
    pub fn first_sheet_preview(
        &self,
        dimensions: &HashMap<MediaId, (u32, u32)>,
    ) -> Option<crate::ComposedSheet> {
        persistent_projection::first_sheet_preview(&self.revision.project, dimensions)
    }

    pub fn project_id(&self) -> Uuid {
        self.revision.project_id
    }

    pub fn revision(&self) -> u64 {
        self.revision.revision
    }

    pub fn project(&self) -> &ProjectDocument {
        &self.revision.project
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateAuthorization {
    CreateOnly,
    ReplaceConfirmed,
    ReplaceTargetConfirmed(PhysicalFileIdentity),
}

#[derive(Clone, Debug)]
pub struct CreateProjectRequest {
    location: ProjectLocation,
    initial_project: InitialProject,
    authorization: CreateAuthorization,
}

impl CreateProjectRequest {
    pub fn new(
        location: ProjectLocation,
        initial_project: InitialProject,
        authorization: CreateAuthorization,
    ) -> Self {
        Self {
            location,
            initial_project,
            authorization,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenProjectRequest {
    location: ProjectLocation,
}

impl OpenProjectRequest {
    pub fn new(location: ProjectLocation) -> Self {
        Self { location }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateProjectError {
    InvalidInitialProject,
    Path(PathFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    CreateStateIndeterminate,
}

#[derive(Debug)]
pub enum OpenProjectError {
    Path(PathFailure),
    Document(DocumentFailure),
    ProjectInUse,
    FocusExisting {
        project_id: Uuid,
        owner_process: ProcessInstanceId,
    },
    ExternalCopyRequiresInteractiveResolution,
    ExternalCopyNotWritable(Box<ExternalCopySource>),
    IdentityIndeterminate,
}

impl PartialEq for OpenProjectError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Path(left), Self::Path(right)) => left == right,
            (Self::Document(left), Self::Document(right)) => left == right,
            (Self::ProjectInUse, Self::ProjectInUse)
            | (
                Self::ExternalCopyRequiresInteractiveResolution,
                Self::ExternalCopyRequiresInteractiveResolution,
            )
            | (Self::ExternalCopyNotWritable(_), Self::ExternalCopyNotWritable(_))
            | (Self::IdentityIndeterminate, Self::IdentityIndeterminate) => true,
            (
                Self::FocusExisting {
                    project_id: left_id,
                    owner_process: left_process,
                },
                Self::FocusExisting {
                    project_id: right_id,
                    owner_process: right_process,
                },
            ) => left_id == right_id && left_process == right_process,
            _ => false,
        }
    }
}

impl Eq for OpenProjectError {}

#[derive(Debug)]
pub struct ExternalCopySource {
    revision: ProjectRevision,
    store: ProjectStore,
}

#[derive(Debug)]
pub struct SaveCopyAsRequest {
    source: ExternalCopySource,
    destination: ProjectLocation,
    authorization: CreateAuthorization,
}

impl SaveCopyAsRequest {
    pub fn new(
        source: ExternalCopySource,
        destination: ProjectLocation,
        authorization: CreateAuthorization,
    ) -> Self {
        Self {
            source,
            destination,
            authorization,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveCopyAsError {
    Path(PathFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    SaveCopyStateIndeterminate,
}

#[derive(Clone, Debug)]
pub struct SaveAsProjectRequest {
    expected_revision: u64,
    destination: ProjectLocation,
    authorization: SaveAsAuthorization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveAsAuthorization {
    CreateOnly,
    ReplaceConfirmed(PhysicalFileIdentity),
}

impl SaveAsProjectRequest {
    pub fn new(
        expected_revision: u64,
        destination: ProjectLocation,
        authorization: SaveAsAuthorization,
    ) -> Self {
        Self {
            expected_revision,
            destination,
            authorization,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaveAsProjectOutcome {
    pub previous_project_id: Uuid,
    pub project_id: Uuid,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveAsProjectError {
    StaleRevision { expected: u64, current: u64 },
    SameTarget,
    Path(PathFailure),
    DestinationConflict,
    ProjectInUse,
    IdentityIndeterminate,
    SaveAsStateIndeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveProjectOutcome {
    Saved { revision: u64 },
    AlreadyCurrent { revision: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveProjectError {
    StaleRevision { expected: u64, current: u64 },
    PersistedBaselineConflict,
    Path(PathFailure),
    SaveStateIndeterminate,
}

/// Opaque proof that one editable Project passed the identity-opening barrier.
///
/// Read-only loads never produce this value. Local state keyed by Project
/// identity must require it instead of accepting an ID parsed from a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIdentityAuthority {
    project_id: Uuid,
}

impl ProjectIdentityAuthority {
    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    fn authorized(project_id: Uuid) -> Self {
        Self { project_id }
    }
}

#[derive(Debug)]
pub struct EditableProject {
    core: ProjectCore,
    session: PersistentProjectSession,
    store: ProjectStore,
    identity_lease: ProjectIdentityLease,
    identity_authority: ProjectIdentityAuthority,
    photo_sources: HashMap<MediaId, HashMap<PathBuf, PhotoSourceMetadata>>,
    session_valid: bool,
}

#[derive(Clone, Debug)]
pub struct FrozenProjectRendering {
    snapshot: RenderSnapshot,
    sources: Vec<MediaRef>,
}

impl FrozenProjectRendering {
    pub fn into_export(
        self,
        sheet_ids: &[String],
    ) -> Result<(RenderSnapshot, Vec<MediaRef>), CoreError> {
        let problems = self.validate_export_sheets(sheet_ids)?;
        if !problems.is_empty() {
            return Err(CoreError::UnfilledLayoutPositions { problems });
        }
        let snapshot = self.snapshot;
        snapshot.export_units(sheet_ids, crate::ExportMode::Sheet)?;
        let referenced: HashSet<_> = snapshot
            .composition
            .sheets
            .iter()
            .filter(|sheet| sheet_ids.contains(&sheet.sheet_id))
            .flat_map(|sheet| sheet.referenced_media_ids())
            .collect();
        let sources: Vec<_> = self
            .sources
            .into_iter()
            .filter(|media| referenced.contains(&MediaId::from_uuid(media.id())))
            .collect();
        if sources.len() != referenced.len() {
            return Err(CoreError::InvalidSnapshot(
                "a composição congelada referencia uma fonte ausente".into(),
            ));
        }
        Ok((snapshot, sources))
    }

    pub fn validate_export_sheets(
        &self,
        sheet_ids: &[String],
    ) -> Result<Vec<crate::LayoutExportProblem>, CoreError> {
        for id in sheet_ids {
            if !self
                .snapshot
                .composition
                .sheets
                .iter()
                .any(|sheet| sheet.sheet_id == *id)
            {
                return Err(CoreError::SheetNotFound(id.clone()));
            }
        }
        Ok(self
            .snapshot
            .composition
            .sheets
            .iter()
            .filter(|sheet| sheet_ids.contains(&sheet.sheet_id))
            .flat_map(|sheet| {
                sheet
                    .frames
                    .iter()
                    .enumerate()
                    .filter(|(_, frame)| frame.photo.is_none())
                    .map(|(index, frame)| crate::LayoutExportProblem {
                        sheet_id: sheet.sheet_id.clone(),
                        sheet_number: sheet.number,
                        frame_id: frame.frame_id.clone(),
                        frame_number: index + 1,
                    })
            })
            .collect())
    }

    pub fn render_snapshot(&self) -> RenderSnapshotRef<'_> {
        RenderSnapshotRef::from_resolved(
            RenderSnapshotMetadata::from(&self.snapshot),
            &self.snapshot.composition,
        )
    }

    pub fn sources(&self) -> &[MediaRef] {
        &self.sources
    }
}

impl EditableProject {
    pub fn project_id(&self) -> Uuid {
        self.session.project_id()
    }

    pub fn identity_authority(&self) -> &ProjectIdentityAuthority {
        &self.identity_authority
    }

    pub fn revision(&self) -> u64 {
        self.session.revision()
    }

    pub fn saved_revision(&self) -> u64 {
        self.session.saved_revision()
    }

    pub fn project(&self) -> &ProjectDocument {
        self.session.project()
    }

    pub fn freeze_template(&self) -> Result<ProjectTemplate, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        Ok(ProjectTemplate {
            project: self.session.project().clone(),
        })
    }

    pub fn project_path(&self) -> &Path {
        self.store.location().project_path()
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.session.has_unsaved_changes()
    }

    pub fn can_undo(&self) -> bool {
        self.session_valid && self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session_valid && self.session.can_redo()
    }

    pub fn recovery_checkpoint(&self) -> Result<RecoveryCheckpoint, RecoveryCheckpointError> {
        if !self.session_valid {
            return Err(RecoveryCheckpointError::SessionUnavailable);
        }
        Ok(RecoveryCheckpoint {
            project_id: self.identity_authority.project_id,
            base_saved_revision: self.session.saved_revision(),
            creative_revision: self.session.current_revision(),
        })
    }

    pub fn restore_recovery(
        &mut self,
        checkpoint: RecoveryCheckpoint,
    ) -> Result<EditorProjection, RecoveryCheckpointError> {
        if !self.session_valid {
            return Err(RecoveryCheckpointError::SessionUnavailable);
        }
        if checkpoint.project_id != self.identity_authority.project_id {
            return Err(RecoveryCheckpointError::IdentityMismatch);
        }
        if checkpoint.base_saved_revision != self.session.saved_revision()
            || self.session.revision() != self.session.saved_revision()
        {
            return Err(RecoveryCheckpointError::BaselineMismatch);
        }
        self.session = PersistentProjectSession::from_recovery(
            checkpoint.creative_revision,
            checkpoint.base_saved_revision,
        );
        self.photo_sources.clear();
        Ok(self.projection())
    }

    /// Resolved editor view of the current productive Project document.
    pub fn projection(&self) -> EditorProjection {
        let project_name = project_name_from_path(self.project_path());
        persistent_projection::editor_projection(
            &self.session,
            self.session_valid,
            &project_name,
            &self.photo_sources,
        )
    }

    pub fn render_snapshot(&self) -> RenderSnapshot {
        persistent_projection::render_snapshot(
            self.project(),
            self.project_id(),
            &project_name_from_path(self.project_path()),
            self.revision(),
            &self.photo_sources,
        )
    }

    pub fn capture_custom_layout(
        &self,
        sheet_id: &str,
    ) -> Result<crate::LayoutDefinition, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let parsed =
            Uuid::parse_str(sheet_id).map_err(|_| CoreError::SheetNotFound(sheet_id.into()))?;
        Ok(self.project().current_layout(parsed)?.definition)
    }

    pub fn refresh_layout_catalog(
        &mut self,
        snapshot: crate::LayoutCatalogSnapshot,
    ) -> Result<bool, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        self.session.refresh_layout_catalog(snapshot)
    }

    pub fn query_layouts(&mut self, sheet_id: &str) -> Result<crate::LayoutQueryResult, CoreError> {
        self.query_layouts_with_frame_request(sheet_id, None)
    }

    pub fn query_layouts_with_frame_request(
        &mut self,
        sheet_id: &str,
        frame_request: Option<crate::LayoutFrameRequest>,
    ) -> Result<crate::LayoutQueryResult, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        self.session.query_layouts(sheet_id, frame_request)
    }

    pub fn preview_layout(
        &self,
        selection: &crate::LayoutSelection,
    ) -> Result<Vec<crate::ComposedFrame>, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let (sheet_id, patch) = self.session.checked_layout_patch(selection)?;
        let candidate = self.project().with_layout_preview(sheet_id, patch)?;
        let ids = patch
            .frame_ids()
            .iter()
            .chain(patch.placeholder_ids().iter())
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
        Ok(self.preview_frame_composition(candidate, &ids))
    }

    /// Composes a transient Frame with the same constraints and fill calculation
    /// as the committed gesture, without touching the Session or its History.
    pub fn preview_frame_geometry(
        &self,
        edit: &crate::FrameGeometryEdit,
    ) -> Result<crate::FrameGeometryPreview, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let (edits, snap) = self.project().frame_geometry_edit(edit)?;
        let edits = edits
            .into_iter()
            .map(|(id, rect)| (id.hyphenated().to_string(), rect))
            .collect::<Vec<_>>();
        let mut album = persistent_projection::album_snapshot(self.project(), &self.photo_sources);
        for frame in album.sheets.iter_mut().flat_map(|sheet| &mut sheet.frames) {
            if let Some((_, rect)) = edits.iter().find(|(id, _)| *id == frame.id) {
                frame.rect = (*rect).into();
            }
        }
        let frames = crate::composition::compose_album(&album)
            .sheets
            .into_iter()
            .flat_map(|sheet| sheet.frames)
            .filter(|frame| edits.iter().any(|(id, _)| *id == frame.frame_id))
            .collect();
        Ok(crate::FrameGeometryPreview { frames, snap })
    }

    /// Resolves a Frame-style draft through the productive document and composer.
    /// No revision, History entry or source file is changed by this preview.
    pub fn preview_frame_style(
        &self,
        edit: &crate::FrameStyleEdit,
    ) -> Result<Vec<crate::ComposedFrame>, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let candidate = self.project().with_frame_style(edit)?;
        Ok(self.preview_frame_composition(candidate, &edit.frame_ids))
    }

    /// Resolves absolute Photo Zoom without changing revision, History or originals.
    pub fn preview_photo_zoom(
        &self,
        edit: &crate::PhotoZoomEdit,
    ) -> Result<Vec<crate::ComposedFrame>, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let candidate = self.project().with_photo_zoom(edit)?;
        Ok(self.preview_frame_composition(candidate, &edit.frame_ids))
    }

    /// Resolves a Photo-angle draft through the productive document and composer.
    /// No revision, History entry or source file is changed by this preview.
    pub fn preview_photo_angle(
        &self,
        edit: &crate::PhotoAngleEdit,
    ) -> Result<Vec<crate::ComposedFrame>, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let candidate = self.project().with_photo_angle(edit)?;
        Ok(self.preview_frame_composition(candidate, &edit.frame_ids))
    }

    pub fn preview_decorative_drop(
        &self,
        request: &crate::DecorativeDropRequest,
    ) -> Result<Option<crate::DecorativeDropPreview>, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let Some(zone) = self.project().decorative_drop_zone(request)? else {
            return Ok(None);
        };
        let candidate = self.project().with_applied_decorative(
            &request.sheet_id,
            request.media_id,
            request.role,
            zone.scope,
        )?;
        let sheet = self
            .preview_composition(candidate)
            .sheets
            .into_iter()
            .find(|sheet| sheet.sheet_id == request.sheet_id)
            .expect("the validated target sheet was composed");
        Ok(Some(crate::DecorativeDropPreview {
            revision: self.revision(),
            role: request.role,
            scope: zone.scope,
            zone_rect: zone.rect,
            center_rect: zone.center,
            sheet,
        }))
    }

    fn preview_frame_composition(
        &self,
        candidate: ProjectDocument,
        frame_ids: &[String],
    ) -> Vec<crate::ComposedFrame> {
        self.preview_composition(candidate)
            .sheets
            .into_iter()
            .flat_map(|sheet| sheet.frames)
            .filter(|frame| frame_ids.contains(&frame.frame_id))
            .collect()
    }

    fn preview_composition(&self, candidate: ProjectDocument) -> crate::CompositionPlan {
        crate::composition::compose_album(&persistent_projection::album_snapshot(
            &candidate,
            &self.photo_sources,
        ))
    }

    /// Freezes one resolved composition and only the exact linked originals
    /// referenced at that creative Revision, without constructing editor state.
    pub fn freeze_rendering(&self) -> FrozenProjectRendering {
        let snapshot = self.render_snapshot();
        let referenced = snapshot
            .composition
            .sheets
            .iter()
            .flat_map(|sheet| sheet.referenced_media_ids())
            .collect::<HashSet<_>>();
        let sources = self
            .project()
            .media()
            .iter()
            .filter(|media| referenced.contains(&MediaId::from_uuid(media.id())))
            .cloned()
            .collect();

        FrozenProjectRendering { snapshot, sources }
    }

    pub fn apply_with_outcome(
        &mut self,
        intent: ProjectIntent,
    ) -> Result<ProjectMutationOutcome, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let sources = if matches!(&intent, ProjectIntent::SetAlbumInformation { .. }) {
            self.observed_photo_dimensions()
        } else {
            HashMap::new()
        };
        let intent_outcome = self.session.apply(intent, &sources)?;
        let affected_frame_id = intent_outcome
            .affected_frame_id
            .map(|frame_id| frame_id.hyphenated().to_string());
        let affected_sheet_id = intent_outcome
            .affected_sheet_id
            .map(|sheet_id| sheet_id.hyphenated().to_string());
        Ok(ProjectMutationOutcome {
            projection: self.projection(),
            affected_frame_id,
            affected_sheet_id,
            affected_frame_ids: intent_outcome.affected_frame_ids.map(|ids| {
                ids.into_iter()
                    .map(|id| id.hyphenated().to_string())
                    .collect()
            }),
        })
    }

    pub fn apply(&mut self, intent: ProjectIntent) -> Result<EditorProjection, CoreError> {
        self.apply_with_outcome(intent)
            .map(|outcome| outcome.projection)
    }

    pub fn import_photo(&mut self, command: ImportPhoto) -> Result<ImportPhotoOutcome, CoreError> {
        let outcome = self.import_photos(vec![command])?;
        Ok(ImportPhotoOutcome {
            projection: outcome.projection,
            media_id: outcome.media_ids[0],
            disposition: if outcome.imported_count == 0 {
                ImportPhotoDisposition::Existing
            } else {
                ImportPhotoDisposition::Imported
            },
        })
    }

    /// Commits all new links in one History entry; reselections only refresh runtime metadata.
    pub fn import_photos(
        &mut self,
        commands: Vec<ImportPhoto>,
    ) -> Result<ImportPhotosOutcome, CoreError> {
        self.import_media(crate::MediaKind::Photo, commands)
    }

    pub fn import_media(
        &mut self,
        kind: crate::MediaKind,
        commands: Vec<crate::ImportMedia>,
    ) -> Result<ImportPhotosOutcome, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        let mut known_paths = self
            .project()
            .media()
            .iter()
            .filter(|media| media.kind() == kind)
            .map(|media| (media.path().to_path_buf(), MediaId::from_uuid(media.id())))
            .collect::<HashMap<_, _>>();
        let mut new_links = Vec::new();
        let mut observations = Vec::new();
        let mut media_ids = Vec::new();
        let mut selected = HashSet::new();
        for command in commands {
            let media_id = if let Some(media_id) = known_paths.get(&command.path) {
                *media_id
            } else {
                if command.source_metadata.is_none() {
                    return Err(CoreError::InvalidProject(
                        "O vínculo da imagem selecionada não está mais no Projeto.".into(),
                    ));
                }
                let media_id = MediaId::from_uuid(Uuid::new_v4());
                known_paths.insert(command.path.clone(), media_id);
                new_links.push((media_id.into_uuid(), command.path.clone()));
                media_id
            };
            if selected.insert(media_id) {
                media_ids.push(media_id);
            }
            if let Some(metadata) = command.source_metadata {
                observations.push((media_id, command.path, metadata));
            }
        }
        let imported_count = new_links.len();
        if !new_links.is_empty() {
            self.session.import_media(kind, new_links)?;
        }
        for (media_id, path, metadata) in observations {
            self.photo_sources
                .entry(media_id)
                .or_default()
                .insert(path, metadata);
        }
        Ok(ImportPhotosOutcome {
            projection: self.projection(),
            media_ids,
            imported_count,
        })
    }

    pub fn observe_photo_source(
        &mut self,
        media_id: MediaId,
        metadata: PhotoSourceMetadata,
    ) -> Result<(), CoreError> {
        let source_path = self
            .project()
            .media()
            .iter()
            .find(|media| {
                media.id() == media_id.into_uuid() && media.kind() == crate::MediaKind::Photo
            })
            .map(|media| media.path().to_path_buf())
            .ok_or_else(|| CoreError::MediaNotFound(media_id.to_string()))?;
        self.photo_sources
            .entry(media_id)
            .or_default()
            .insert(source_path, metadata);
        Ok(())
    }

    pub fn photo_drop_target(
        &self,
        sheet_id: &str,
        x_um: i64,
        y_um: i64,
    ) -> Result<PhotoDropTarget, CoreError> {
        let parsed = Uuid::parse_str(sheet_id)
            .ok()
            .filter(|parsed| parsed.hyphenated().to_string() == sheet_id)
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.to_owned()))?;
        self.project()
            .photo_drop_target(parsed, x_um, y_um)
            .map_err(|()| CoreError::SheetNotFound(sheet_id.to_owned()))
    }

    pub fn relink_media(&mut self, command: RelinkMedia) -> Result<EditorProjection, CoreError> {
        if !self.session_valid {
            return Err(CoreError::EditableSessionInvalidated);
        }
        self.session.relink_media(command)?;
        Ok(self.projection())
    }

    pub fn validate_album_information(
        &self,
        information: &AlbumInformation,
    ) -> AlbumInformationValidation {
        self.session
            .validate_album_information(information, &self.observed_photo_dimensions())
    }

    fn observed_photo_dimensions(&self) -> crate::project_document::PhotoDimensions {
        self.project()
            .media()
            .iter()
            .filter_map(|media| {
                let source = self
                    .photo_sources
                    .get(&MediaId::from_uuid(media.id()))?
                    .get(media.path())?;
                Some((
                    media.id(),
                    (source.source_width_px(), source.source_height_px()),
                ))
            })
            .collect()
    }

    pub fn undo(&mut self) -> Option<EditorProjection> {
        if !self.session_valid {
            return None;
        }
        self.session.undo()?;
        Some(self.projection())
    }

    pub fn redo(&mut self) -> Option<EditorProjection> {
        if !self.session_valid {
            return None;
        }
        self.session.redo()?;
        Some(self.projection())
    }

    pub fn save(&mut self, expected_revision: u64) -> Result<SaveProjectOutcome, SaveProjectError> {
        if !self.session_valid {
            return Err(SaveProjectError::SaveStateIndeterminate);
        }
        let current = self.revision();
        if expected_revision != current {
            return Err(SaveProjectError::StaleRevision {
                expected: expected_revision,
                current,
            });
        }
        if !self.session.requires_save() {
            return Ok(SaveProjectOutcome::AlreadyCurrent { revision: current });
        }
        let candidate = self.session.current_revision();
        match self.store.save(candidate, &self.identity_lease) {
            SaveStoreResult::Saved(receipt) => {
                if self.session.confirm_saved(receipt.candidate()).is_err() {
                    self.invalidate_session();
                    return Err(SaveProjectError::SaveStateIndeterminate);
                }
                Ok(SaveProjectOutcome::Saved {
                    revision: receipt.candidate().revision,
                })
            }
            SaveStoreResult::NotSaved(SaveStoreError::PersistedBaselineConflict) => {
                Err(SaveProjectError::PersistedBaselineConflict)
            }
            SaveStoreResult::NotSaved(SaveStoreError::Path(error)) => {
                Err(SaveProjectError::Path(error))
            }
            SaveStoreResult::StateIndeterminate => {
                self.invalidate_session();
                Err(SaveProjectError::SaveStateIndeterminate)
            }
        }
    }

    pub fn save_as(
        &mut self,
        request: SaveAsProjectRequest,
    ) -> Result<SaveAsProjectOutcome, SaveAsProjectError> {
        self.save_as_with_transition(request, |_, _| Ok(()))
    }

    /// Publishes a Save As candidate, then lets the Host stage identity-scoped
    /// local authority before this editable Session adopts it.
    ///
    /// A failed transition keeps the previous Session, store, lease and
    /// authority. Because the destination publication may already be complete,
    /// callers receive the fail-closed indeterminate terminal.
    pub fn save_as_with_transition(
        &mut self,
        request: SaveAsProjectRequest,
        transition: impl FnOnce(&ProjectIdentityAuthority, SaveAsProjectOutcome) -> Result<(), ()>,
    ) -> Result<SaveAsProjectOutcome, SaveAsProjectError> {
        if !self.session_valid {
            return Err(SaveAsProjectError::SaveAsStateIndeterminate);
        }
        let SaveAsProjectRequest {
            expected_revision,
            destination,
            authorization,
        } = request;
        let current = self.revision();
        if expected_revision != current {
            return Err(SaveAsProjectError::StaleRevision {
                expected: expected_revision,
                current,
            });
        }
        if !self.store.location_still_matches_baseline() {
            return Err(SaveAsProjectError::IdentityIndeterminate);
        }
        let prepared_destination = destination
            .prepare_file_destination()
            .map_err(SaveAsProjectError::Path)?;
        match prepared_destination.resolve_existing() {
            Ok(Some(target)) => {
                match self.store.compare_physical(&target) {
                    PhysicalIdentityEvidence::Same => return Err(SaveAsProjectError::SameTarget),
                    PhysicalIdentityEvidence::Different => {}
                    PhysicalIdentityEvidence::Indeterminate => {
                        return Err(SaveAsProjectError::IdentityIndeterminate);
                    }
                }
                if let SaveAsAuthorization::ReplaceConfirmed(confirmed) = authorization {
                    match target.physical_identity() {
                        Some(current) if current == confirmed => {}
                        Some(_) => return Err(SaveAsProjectError::DestinationConflict),
                        None => return Err(SaveAsProjectError::IdentityIndeterminate),
                    }
                }
            }
            Ok(None) => {
                if matches!(authorization, SaveAsAuthorization::ReplaceConfirmed(_)) {
                    return Err(SaveAsProjectError::DestinationConflict);
                }
            }
            Err(error) => {
                return Err(SaveAsProjectError::Path(project_store::map_path_failure(
                    error,
                )));
            }
        }

        let source_physical_identity = self
            .store
            .physical_identity()
            .ok_or(SaveAsProjectError::IdentityIndeterminate)?;
        let previous_project_id = self.project_id();
        let project_id = Uuid::new_v4();
        let current_revision = self.session.current_revision();
        let candidate = ProjectRevision::new(
            project_id,
            current_revision.revision,
            current_revision.project,
        );
        let PublishedNewProject {
            store,
            identity_lease,
            identity_authority,
        } = publish_new_project(
            &self.core,
            &candidate,
            NewProjectTarget::SaveAs {
                location: destination,
                authorization,
                source: &self.store,
                source_identity: source_physical_identity,
            },
        )
        .map_err(|error| match error {
            NewPublicationError::Store(error) => map_save_as_store_error(error),
            NewPublicationError::BaselineChanged
            | NewPublicationError::RegistryUnavailable
            | NewPublicationError::BindingUnavailable => {
                SaveAsProjectError::SaveAsStateIndeterminate
            }
        })?;
        let previous_session = self.session.clone();
        if self.session.adopt_saved_as(&candidate).is_err() {
            return Err(SaveAsProjectError::SaveAsStateIndeterminate);
        }

        let outcome = SaveAsProjectOutcome {
            previous_project_id,
            project_id,
            revision: current,
        };
        if transition(&identity_authority, outcome).is_err() {
            self.session = previous_session;
            return Err(SaveAsProjectError::SaveAsStateIndeterminate);
        }

        let previous_store = std::mem::replace(&mut self.store, store);
        let previous_lease = std::mem::replace(&mut self.identity_lease, identity_lease);
        let previous_authority =
            std::mem::replace(&mut self.identity_authority, identity_authority);
        drop((previous_store, previous_lease, previous_authority));

        Ok(outcome)
    }

    fn invalidate_session(&mut self) {
        self.session_valid = false;
        self.store.invalidate();
    }
}

impl ProjectCore {
    #[cfg(windows)]
    pub fn inspect_creation_destination(
        &self,
        location: &ProjectLocation,
    ) -> Result<Option<PhysicalFileIdentity>, CreateProjectError> {
        use myalbuns_paths::{ProjectFileLock, ProjectFileLockError};
        let destination = location
            .prepare_file_destination()
            .map_err(CreateProjectError::Path)?;
        let Some(target) = destination
            .resolve_existing()
            .map_err(|error| CreateProjectError::Path(project_store::map_path_failure(error)))?
        else {
            return Ok(None);
        };
        let identity = target
            .physical_identity()
            .ok_or(CreateProjectError::IdentityIndeterminate)?;
        let lock =
            ProjectFileLock::try_acquire(target.operational_path()).map_err(
                |error| match error {
                    ProjectFileLockError::Conflict => CreateProjectError::ProjectInUse,
                    ProjectFileLockError::Unavailable { .. } => {
                        CreateProjectError::Path(PathFailure::IoFailure)
                    }
                },
            )?;
        if lock.compare_physical(&target) != PhysicalIdentityEvidence::Same {
            return Err(CreateProjectError::IdentityIndeterminate);
        }
        let bytes = lock
            .read_to_string()
            .map_err(|_| CreateProjectError::Path(PathFailure::IoFailure))?;
        if let Ok(revision) = project_store::decode(bytes.as_bytes()) {
            let root = self
                .identity_lease_root()
                .ok_or(CreateProjectError::IdentityIndeterminate)?;
            match ProjectIdentityLease::observe(root, revision.project_id, Some(identity)) {
                Ok(IdentityLeaseObservation::Inactive) => {}
                Ok(_) => return Err(CreateProjectError::ProjectInUse),
                Err(_) => return Err(CreateProjectError::IdentityIndeterminate),
            }
        }
        Ok(Some(identity))
    }

    pub fn load_persisted_revision(
        &self,
        request: project_store::LoadProjectRequest,
    ) -> Result<LoadedProjectRevision, project_store::LoadProjectError> {
        let loaded = project_store::load(request)?;
        authorize_loaded_identity(self, &loaded)?;
        Ok(LoadedProjectRevision {
            revision: loaded.revision,
            project_path: loaded.project_path,
            content_sha256: loaded.content_sha256,
        })
    }

    pub fn create_editable(
        &self,
        request: CreateProjectRequest,
    ) -> Result<EditableProject, CreateProjectError> {
        let CreateProjectRequest {
            location,
            initial_project,
            authorization,
        } = request;
        let project = initial_project
            .into_project()
            .map_err(|_| CreateProjectError::InvalidInitialProject)?;
        self.create_document(location, project, authorization)
    }

    /// Publishes the complete frozen model and deferred Photo links in one write.
    /// Original inspection belongs to project opening and Cache preparation, not generation.
    /// The source Session is never adopted, saved or edited by this operation.
    pub fn create_from_template(
        &self,
        template: &ProjectTemplate,
        location: ProjectLocation,
        authorization: CreateAuthorization,
        photo_paths: Vec<PathBuf>,
    ) -> Result<EditableProject, CreateProjectError> {
        let mut known = template
            .project
            .media()
            .iter()
            .filter(|media| media.kind() == crate::MediaKind::Photo)
            .map(|media| media.path().to_path_buf())
            .collect::<HashSet<_>>();
        let mut links = Vec::new();
        for path in photo_paths {
            if known.insert(path.clone()) {
                links.push((Uuid::new_v4(), path));
            }
        }
        let project = if links.is_empty() {
            template.project.clone()
        } else {
            template
                .project
                .with_imported_media(crate::MediaKind::Photo, links)
                .map_err(|()| CreateProjectError::InvalidInitialProject)?
        };
        self.create_document(location, project, authorization)
    }

    fn create_document(
        &self,
        location: ProjectLocation,
        project: ProjectDocument,
        authorization: CreateAuthorization,
    ) -> Result<EditableProject, CreateProjectError> {
        let revision = ProjectRevision::new(Uuid::new_v4(), 0, project);
        let PublishedNewProject {
            store,
            identity_lease,
            identity_authority,
        } = publish_new_project(
            self,
            &revision,
            NewProjectTarget::Create {
                location,
                authorization,
            },
        )
        .map_err(|error| match error {
            NewPublicationError::Store(error) => map_create_store_error(error),
            NewPublicationError::BaselineChanged | NewPublicationError::BindingUnavailable => {
                CreateProjectError::IdentityIndeterminate
            }
            NewPublicationError::RegistryUnavailable => {
                CreateProjectError::CreateStateIndeterminate
            }
        })?;
        Ok(EditableProject {
            core: self.clone(),
            session: PersistentProjectSession::from_persisted(revision),
            store,
            identity_lease,
            identity_authority,
            photo_sources: HashMap::new(),
            session_valid: true,
        })
    }

    pub fn open_editable(
        &self,
        request: OpenProjectRequest,
    ) -> Result<EditableProject, OpenProjectError> {
        let lease_root = self
            .identity_lease_root()
            .ok_or(OpenProjectError::Path(PathFailure::IoFailure))?;
        let opened = match project_store::open_editable(request.location, lease_root) {
            Ok(opened) => opened,
            Err(OpenStoreError::ProjectInUse {
                project_id,
                physical_identity,
            }) => {
                return Err(map_active_identity_observation(
                    lease_root,
                    project_id,
                    physical_identity,
                ));
            }
            Err(error) => return Err(map_open_store_error(error)),
        };
        let identity_lease =
            match ProjectIdentityLease::acquire(lease_root, opened.revision.project_id) {
                Ok(lease) => lease,
                Err(IdentityLeaseError::Conflict) => {
                    return match ProjectIdentityLease::observe(
                        lease_root,
                        opened.revision.project_id,
                        opened.store.physical_identity(),
                    ) {
                        Ok(IdentityLeaseObservation::SamePhysicalTarget { owner_process }) => {
                            Err(OpenProjectError::FocusExisting {
                                project_id: opened.revision.project_id,
                                owner_process,
                            })
                        }
                        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                            promote_external_copy(self, opened, None)
                        }
                        Ok(
                            IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending,
                        ) => Err(OpenProjectError::ProjectInUse),
                        Err(_) => Err(OpenProjectError::IdentityIndeterminate),
                    };
                }
                Err(IdentityLeaseError::Unavailable) => {
                    return Err(OpenProjectError::Path(PathFailure::IoFailure));
                }
            };
        if !opened.store.location_still_matches_baseline() {
            return Err(OpenProjectError::IdentityIndeterminate);
        }
        match authorize_identity_candidate(
            self,
            opened.revision.project_id,
            opened.store.location().project_path(),
            IdentityCandidateTarget::Editable(&opened.store),
        ) {
            Ok(()) => {}
            Err(IdentityCandidateError::ExternalCopy) => {
                return promote_external_copy(self, opened, Some(identity_lease));
            }
            Err(IdentityCandidateError::Indeterminate) => {
                return Err(OpenProjectError::IdentityIndeterminate);
            }
        }
        let identity_lease = bind_identity_target(identity_lease, &opened.store)
            .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
        let identity_authority = ProjectIdentityAuthority::authorized(identity_lease.project_id());
        Ok(EditableProject {
            core: self.clone(),
            session: PersistentProjectSession::from_persisted(opened.revision),
            store: opened.store,
            identity_lease,
            identity_authority,
            photo_sources: HashMap::new(),
            session_valid: true,
        })
    }
}

fn identity_registry(core: &ProjectCore) -> Result<ProjectIdentityRegistry, ()> {
    core.identity_registry_root()
        .map(|root| ProjectIdentityRegistry::new(root.to_path_buf()))
        .ok_or(())
}

fn authorize_loaded_identity(
    core: &ProjectCore,
    loaded: &project_store::LoadedStoredRevision,
) -> Result<(), project_store::LoadProjectError> {
    let Some(lease_root) = core.identity_lease_root() else {
        return Ok(());
    };
    let project_id = loaded.revision.project_id;
    match ProjectIdentityLease::observe(lease_root, project_id, loaded.physical_identity) {
        Ok(IdentityLeaseObservation::SamePhysicalTarget { .. }) => return Ok(()),
        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
            return Err(project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution);
        }
        Ok(IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending) => {}
        Err(_) => {
            return Err(project_store::LoadProjectError::IdentityIndeterminate);
        }
    }
    let lease = match ProjectIdentityLease::acquire(lease_root, project_id) {
        Ok(lease) => lease,
        Err(IdentityLeaseError::Conflict) => {
            return match ProjectIdentityLease::observe(
                lease_root,
                project_id,
                loaded.physical_identity,
            ) {
                Ok(IdentityLeaseObservation::SamePhysicalTarget { .. }) => Ok(()),
                Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => {
                    Err(project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution)
                }
                _ => Err(project_store::LoadProjectError::IdentityIndeterminate),
            };
        }
        Err(IdentityLeaseError::Unavailable) => {
            return Err(project_store::LoadProjectError::IdentityIndeterminate);
        }
    };
    loaded
        .physical_identity
        .ok_or(project_store::LoadProjectError::IdentityIndeterminate)?;
    let current_candidate = loaded
        .root_bindings
        .resolve_existing(&loaded.project_path, ExpectedObject::RegularFile)
        .map_err(|_| project_store::LoadProjectError::IdentityIndeterminate)?;
    if loaded.resolved_object.compare_physical(&current_candidate) != PhysicalIdentityEvidence::Same
    {
        lease.discard_unpublished();
        return Err(project_store::LoadProjectError::IdentityIndeterminate);
    }
    if let Err(error) = authorize_identity_candidate(
        core,
        project_id,
        &loaded.project_path,
        IdentityCandidateTarget::Loaded(&loaded.resolved_object),
    ) {
        lease.discard_unpublished();
        return Err(map_load_identity_error(error));
    }
    // The registry now holds durable evidence. A read-only observation creates
    // no Session; release its file lease before releasing publication arbitration.
    lease.discard_unpublished();
    Ok(())
}

fn publish_identity_location(
    core: &ProjectCore,
    project_id: Uuid,
    store: &ProjectStore,
) -> Result<(), ()> {
    identity_registry(core)?
        .publish(project_id, store.location().project_path())
        .map_err(|_| ())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityCandidateError {
    ExternalCopy,
    Indeterminate,
}

fn authorize_identity_candidate(
    core: &ProjectCore,
    project_id: Uuid,
    candidate_location: &Path,
    candidate_target: IdentityCandidateTarget<'_>,
) -> Result<(), IdentityCandidateError> {
    let registry = identity_registry(core).map_err(|()| IdentityCandidateError::Indeterminate)?;
    let previous_location = match registry
        .lookup(project_id)
        .map_err(|_| IdentityCandidateError::Indeterminate)?
    {
        IdentityRegistryLookup::Missing => {
            return registry
                .publish(project_id, candidate_location)
                .map_err(|_| IdentityCandidateError::Indeterminate);
        }
        IdentityRegistryLookup::Location(location) => location,
    };

    let mut context = OperationPathContext::new();
    context
        .capture(&previous_location)
        .map_err(|_| IdentityCandidateError::Indeterminate)?;
    let previous = project_store::read(&ProjectLocation::new(previous_location, context.freeze()));
    match previous {
        Ok(previous) if previous.revision.project_id == project_id => {
            match candidate_target.compare(&previous.resolved_object) {
                PhysicalIdentityEvidence::Same => Ok(()),
                PhysicalIdentityEvidence::Different => Err(IdentityCandidateError::ExternalCopy),
                PhysicalIdentityEvidence::Indeterminate => {
                    Err(IdentityCandidateError::Indeterminate)
                }
            }
        }
        Ok(_) => Err(IdentityCandidateError::Indeterminate),
        Err(project_store::DecodeFailure::Path(PathFailure::NotFound)) => registry
            .publish(project_id, candidate_location)
            .map_err(|_| IdentityCandidateError::Indeterminate),
        Err(_) => Err(IdentityCandidateError::Indeterminate),
    }
}

enum IdentityCandidateTarget<'a> {
    Loaded(&'a myalbuns_paths::ResolvedObject),
    Editable(&'a ProjectStore),
}

impl IdentityCandidateTarget<'_> {
    fn compare(&self, previous: &myalbuns_paths::ResolvedObject) -> PhysicalIdentityEvidence {
        match self {
            Self::Loaded(candidate) => previous.compare_physical(candidate),
            Self::Editable(candidate) => candidate.compare_physical(previous),
        }
    }
}

fn map_load_identity_error(error: IdentityCandidateError) -> project_store::LoadProjectError {
    match error {
        IdentityCandidateError::ExternalCopy => {
            project_store::LoadProjectError::ExternalCopyRequiresInteractiveResolution
        }
        IdentityCandidateError::Indeterminate => {
            project_store::LoadProjectError::IdentityIndeterminate
        }
    }
}

fn bind_identity_target(
    lease: PendingProjectIdentityLease,
    store: &ProjectStore,
) -> Result<ProjectIdentityLease, IdentityLeaseError> {
    let physical_identity = store
        .physical_identity()
        .ok_or(IdentityLeaseError::Unavailable)?;
    lease.bind_target(physical_identity)?;
    lease.into_published()
}

fn map_create_store_error(error: CreateStoreError) -> CreateProjectError {
    match error {
        CreateStoreError::Path(error) => CreateProjectError::Path(error),
        CreateStoreError::Document(_) => CreateProjectError::InvalidInitialProject,
        CreateStoreError::SameTarget | CreateStoreError::DestinationConflict => {
            CreateProjectError::DestinationConflict
        }
        CreateStoreError::ProjectInUse => CreateProjectError::ProjectInUse,
        CreateStoreError::IdentityIndeterminate => CreateProjectError::IdentityIndeterminate,
        CreateStoreError::StateIndeterminate => CreateProjectError::CreateStateIndeterminate,
    }
}

fn map_open_store_error(error: OpenStoreError) -> OpenProjectError {
    match error {
        OpenStoreError::Path(error) => OpenProjectError::Path(error),
        OpenStoreError::Document(error) => OpenProjectError::Document(error),
        OpenStoreError::ProjectInUse { .. } => OpenProjectError::ProjectInUse,
        OpenStoreError::IdentityIndeterminate => OpenProjectError::IdentityIndeterminate,
    }
}

fn map_save_copy_store_error(error: CreateStoreError) -> SaveCopyAsError {
    match error {
        CreateStoreError::Path(error) => SaveCopyAsError::Path(error),
        CreateStoreError::Document(_) | CreateStoreError::IdentityIndeterminate => {
            SaveCopyAsError::IdentityIndeterminate
        }
        CreateStoreError::SameTarget | CreateStoreError::DestinationConflict => {
            SaveCopyAsError::DestinationConflict
        }
        CreateStoreError::ProjectInUse => SaveCopyAsError::ProjectInUse,
        CreateStoreError::StateIndeterminate => SaveCopyAsError::SaveCopyStateIndeterminate,
    }
}

fn map_save_as_store_error(error: CreateStoreError) -> SaveAsProjectError {
    match error {
        CreateStoreError::Path(error) => SaveAsProjectError::Path(error),
        CreateStoreError::Document(_) | CreateStoreError::IdentityIndeterminate => {
            SaveAsProjectError::IdentityIndeterminate
        }
        CreateStoreError::SameTarget => SaveAsProjectError::SameTarget,
        CreateStoreError::DestinationConflict => SaveAsProjectError::DestinationConflict,
        CreateStoreError::ProjectInUse => SaveAsProjectError::ProjectInUse,
        CreateStoreError::StateIndeterminate => SaveAsProjectError::SaveAsStateIndeterminate,
    }
}

impl ProjectCore {
    pub fn save_copy_as(
        &self,
        request: SaveCopyAsRequest,
    ) -> Result<EditableProject, SaveCopyAsError> {
        let SaveCopyAsRequest {
            source,
            destination,
            authorization,
        } = request;
        if !source.store.location_still_matches_baseline() {
            return Err(SaveCopyAsError::IdentityIndeterminate);
        }
        let revision = ProjectRevision::new(
            Uuid::new_v4(),
            source.revision.revision,
            source.revision.project.clone(),
        );
        let PublishedNewProject {
            store,
            identity_lease,
            identity_authority,
        } = publish_new_project(
            self,
            &revision,
            NewProjectTarget::Create {
                location: destination,
                authorization,
            },
        )
        .map_err(|error| match error {
            NewPublicationError::Store(error) => map_save_copy_store_error(error),
            NewPublicationError::BaselineChanged => SaveCopyAsError::IdentityIndeterminate,
            NewPublicationError::RegistryUnavailable | NewPublicationError::BindingUnavailable => {
                SaveCopyAsError::SaveCopyStateIndeterminate
            }
        })?;
        Ok(EditableProject {
            core: self.clone(),
            session: PersistentProjectSession::from_persisted(revision),
            store,
            identity_lease,
            identity_authority,
            photo_sources: HashMap::new(),
            session_valid: true,
        })
    }
}

fn promote_external_copy(
    core: &ProjectCore,
    mut opened: project_store::OpenedProject,
    source_identity_guard: Option<PendingProjectIdentityLease>,
) -> Result<EditableProject, OpenProjectError> {
    let lease_root = core
        .identity_lease_root()
        .ok_or(OpenProjectError::IdentityIndeterminate)?;
    let project_id = Uuid::new_v4();
    let identity_lease = ProjectIdentityLease::acquire(lease_root, project_id)
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
    let revision = match opened.store.rewrite_identity(project_id, &identity_lease) {
        SaveStoreResult::Saved(receipt) => receipt.candidate().clone(),
        SaveStoreResult::NotSaved(SaveStoreError::Path(PathFailure::AccessDenied)) => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::ExternalCopyNotWritable(Box::new(
                external_copy_source(opened),
            )));
        }
        SaveStoreResult::NotSaved(SaveStoreError::Path(error)) => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::Path(error));
        }
        SaveStoreResult::NotSaved(SaveStoreError::PersistedBaselineConflict)
        | SaveStoreResult::StateIndeterminate => {
            identity_lease.discard_unpublished();
            return Err(OpenProjectError::IdentityIndeterminate);
        }
    };
    if publish_identity_location(core, project_id, &opened.store).is_err() {
        identity_lease.discard_unpublished();
        return Err(OpenProjectError::IdentityIndeterminate);
    }
    let identity_lease = identity_lease
        .into_published()
        .map_err(|_| OpenProjectError::IdentityIndeterminate)?;
    let identity_authority = ProjectIdentityAuthority::authorized(project_id);
    // The source's repeated Identidade remains reserved until the promoted
    // file, lease, registry and authority have all reached their terminal.
    drop(source_identity_guard);
    Ok(EditableProject {
        core: core.clone(),
        session: PersistentProjectSession::from_persisted(revision),
        store: opened.store,
        identity_lease,
        identity_authority,
        photo_sources: HashMap::new(),
        session_valid: true,
    })
}

fn external_copy_source(opened: project_store::OpenedProject) -> ExternalCopySource {
    ExternalCopySource {
        revision: opened.revision,
        store: opened.store,
    }
}

fn map_active_identity_observation(
    lease_root: &Path,
    project_id: Uuid,
    physical_identity: Option<myalbuns_paths::PhysicalFileIdentity>,
) -> OpenProjectError {
    match ProjectIdentityLease::observe(lease_root, project_id, physical_identity) {
        Ok(IdentityLeaseObservation::SamePhysicalTarget { owner_process }) => {
            OpenProjectError::FocusExisting {
                project_id,
                owner_process,
            }
        }
        Ok(IdentityLeaseObservation::DifferentPhysicalTarget) => OpenProjectError::ProjectInUse,
        Ok(IdentityLeaseObservation::Inactive | IdentityLeaseObservation::Pending) => {
            OpenProjectError::ProjectInUse
        }
        Err(_) => OpenProjectError::IdentityIndeterminate,
    }
}

#[cfg(test)]
mod save_tests {
    use myalbuns_paths::OperationPathContext;

    use super::{CreateAuthorization, CreateProjectRequest, OpenProjectRequest};
    use crate::{InitialProject, ProjectCore, ProjectLocation};

    fn location(path: &std::path::Path) -> ProjectLocation {
        let mut context = OperationPathContext::new();
        context
            .capture(path)
            .expect("the test Project root is captured");
        ProjectLocation::new(path.to_path_buf(), context.freeze())
    }

    #[test]
    fn projection_and_render_snapshot_derive_the_non_ascii_project_name_from_its_native_path() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let project_path = directory.path().join("Casamento da Júlia.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            directory.path().join("leases"),
            directory.path().join("identities"),
        );
        let project = core
            .create_editable(CreateProjectRequest::new(
                location(&project_path),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the named Project opens");

        assert_eq!(project.project_path(), project_path);
        assert_eq!(
            project.projection().state.project_name,
            "Casamento da Júlia"
        );
        assert_eq!(project.render_snapshot().project_name, "Casamento da Júlia");

        drop(project);
        let reopened = core
            .open_editable(OpenProjectRequest::new(location(&project_path)))
            .expect("the named Project reopens");
        assert_eq!(
            reopened.projection().state.project_name,
            "Casamento da Júlia"
        );
    }
}
