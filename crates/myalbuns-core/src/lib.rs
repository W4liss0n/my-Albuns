mod composition;
mod frame_geometry;
mod model;
mod persistent_project;
mod persistent_projection;
mod persistent_session;
mod project_document;
mod project_recovery;
mod project_store;
mod validation;

pub use frame_geometry::{
    FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
};

pub use model::{
    AlbumSnapshot, ComposedBackground, ComposedColor, ComposedDecorative, ComposedFrame,
    ComposedOutputUnit, ComposedPhoto, ComposedSheet, CompositionPlan, CoreError, DocumentSnapshot,
    EditorProjection, EditorState, FrameSnapshot, FrameStackAction, ImportPhoto,
    ImportPhotoDisposition, ImportPhotoOutcome, ImportPhotosOutcome, Matrix2, MediaCatalogItem,
    MediaId, MediaKind, MediaTransform, MediaUsage, NormalizedPan, NumberRange, ParseMediaIdError,
    PhotoAngleEdit, PhotoDropTarget, PhotoOrientationAction, PhotoPlacement, PhotoPlacementMode,
    PhotoPlacementPlan, PhotoSnapshot, PhotoSourceMetadata, ProjectIntent, ProjectMutationOutcome,
    ProjectedActiveSides, ProjectedBackground, ProjectedBackgroundContent, ProjectedDisplayUnit,
    ProjectedFrameBorder, ProjectedOverlay, ProjectedOverlayContent, ProjectedVisualDefaults,
    RectUm, RelinkMedia, RenderSnapshot, RenderSnapshotRef, SheetInsertionPosition, SheetRole,
    SheetSnapshot, SizeUm, VectorUm,
};
pub use persistent_project::{
    CreateAuthorization, CreateProjectError, CreateProjectRequest, EditableProject,
    ExternalCopySource, FrozenProjectRendering, FrozenSheetRendering, LoadedProjectRevision,
    OpenProjectError, OpenProjectRequest, ProjectCore, ProjectIdentityAuthority,
    SaveAsAuthorization, SaveAsProjectError, SaveAsProjectOutcome, SaveAsProjectRequest,
    SaveCopyAsError, SaveCopyAsRequest, SaveProjectError, SaveProjectOutcome,
    project_name_from_path,
};
pub use project_document::{
    ActiveSides, AlbumInformation, AlbumInformationImpact, AlbumInformationValidation, Background,
    BackgroundContent, DisplayUnit, DocumentSettings, EndSheetFormat, FrameBorder,
    InitialBackground, InitialBackgroundContent, InitialFrameBorder, InitialOverlay,
    InitialOverlayContent, InitialProject, InitialProjectConfiguration,
    InitialProjectPersonalization, MediaRef, Overlay, OverlayContent,
    ProjectConfigurationValidationError, ProjectDocument, ProjectFrame, ProjectPhoto,
    ProjectPhotoTransform, ProjectRect, ProjectSheet, Rgb, VisualDefaults,
};
pub use project_recovery::{RecoveryCheckpoint, RecoveryCheckpointError};
pub use project_store::{
    DocumentFailure, LoadProjectError, LoadProjectRequest, PathFailure, ProjectLocation,
};
