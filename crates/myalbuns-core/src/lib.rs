mod composition;
mod export_selection;
pub use export_selection::{ExportFormat, ExportMode, SelectedExportUnit};
mod frame_geometry;
mod frame_snap;
mod layouts;
mod media_folders;
mod model;
pub use media_folders::{MediaFolder, MediaFolderEdit};
mod persistent_project;
mod persistent_projection;
mod persistent_session;
mod project_document;
mod project_recovery;
mod project_store;
mod sheet_visuals;
mod validation;

pub use sheet_visuals::{
    DecorativeDropPreview, DecorativeDropRequest, DecorativeRole, DecorativeScope,
    EdgeConversionLoss, EdgeConversionSide, SheetVisual, SheetVisualChange, SheetVisuals,
    SideVisual, VisualMapping,
};

pub use frame_geometry::{
    FrameGeometryEdit, FrameGeometryGesture, FrameGeometryTarget, FrameResizeHandle,
};
pub use frame_snap::{
    FrameGeometryPreview, FrameSnapAxis, FrameSnapFeedback, FrameSnapGuide, FrameSnapKind,
    FrameSnapRequest,
};
pub use layouts::{
    CustomLayout, CustomLayoutId, FavoriteLayout, FrameOrientation, GeneratedLayout,
    LayoutCandidate, LayoutCatalogSnapshot, LayoutDefinition, LayoutExportProblem,
    LayoutFavoriteId, LayoutFrameRequest, LayoutGeneration, LayoutGenerationStatus, LayoutListing,
    LayoutOrigin, LayoutParameters, LayoutPatch, LayoutPermission, LayoutQuery, LayoutQueryResult,
    LayoutRules, LayoutScope, LayoutSelection, LayoutSettings, LayoutSources, LayoutSurface,
    LayoutSurfaceKind, SaveCustomLayoutResult, StoredLayout, generate_layouts,
};

pub use model::{
    AlbumSnapshot, ComposedBackground, ComposedColor, ComposedDecorative, ComposedFrame,
    ComposedOutputUnit, ComposedPhoto, ComposedSheet, CompositionPlan, CoreError, DocumentSnapshot,
    EditorProjection, EditorState, FrameSnapshot, FrameStackAction, FrameStyleChange,
    FrameStyleEdit, FrameStyleSource, ImportMedia, ImportPhoto, ImportPhotoDisposition,
    ImportPhotoOutcome, ImportPhotosOutcome, Matrix2, MediaCatalogItem, MediaId, MediaKind,
    MediaRemovalMode, MediaTransform, MediaUsage, MediaUsageBreakdown, NormalizedPan, NumberRange,
    ParseMediaIdError, PhotoAngleEdit, PhotoDropTarget, PhotoOrientationAction, PhotoPlacement,
    PhotoPlacementMode, PhotoPlacementPlan, PhotoSnapshot, PhotoSourceMetadata, PhotoZoomEdit,
    ProjectIntent, ProjectMutationOutcome, ProjectedActiveSides, ProjectedBackground,
    ProjectedBackgroundContent, ProjectedDisplayUnit, ProjectedFrameBorder, ProjectedFrameStyle,
    ProjectedOverlay, ProjectedOverlayContent, ProjectedVisualDefaults, RectUm, RelinkMedia,
    RenderSnapshot, RenderSnapshotRef, SheetInsertionPosition, SheetRole, SheetSnapshot,
    SheetStructureAvailability, SheetStructureProjection, SizeUm, VectorUm,
};
pub use persistent_project::{
    CreateAuthorization, CreateProjectError, CreateProjectRequest, EditableProject,
    ExternalCopySource, FrozenProjectRendering, LoadedProjectRevision, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectIdentityAuthority, ProjectTemplate,
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
