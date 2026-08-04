mod composition;
mod model;
mod persistent_project;
mod persistent_projection;
mod persistent_session;
mod project;
mod project_document;
mod project_store;
mod session;
mod validation;

pub use model::{
    AlbumSnapshot, ComposedDecorative, ComposedFrame, ComposedPhoto, ComposedSheet,
    CompositionPlan, CoreError, DocumentSnapshot, EditorProjection, EditorState, FrameSnapshot,
    Matrix2, MediaCatalogItem, MediaKind, MediaTransform, MediaUsage, NormalizedPan, NumberRange,
    PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot, ProjectIntent, ProjectedActiveSides,
    ProjectedDisplayUnit, RectUm, RenderSnapshot, SheetRole, SheetSnapshot, SizeUm, VectorUm,
};
pub use persistent_project::{
    CreateAuthorization, CreateProjectError, CreateProjectRequest, EditableProject,
    LoadedProjectRevision, OpenProjectError, OpenProjectRequest,
};
pub use project::{
    EditableProject as DemoEditableProject, LoadedProjectRevision as DemoLoadedProjectRevision,
    ProjectCore,
};
pub use project_document::{
    ActiveSides, Background, BackgroundContent, DecorativeMedia, DisplayUnit, DocumentSettings,
    EndSheetFormat, FrameBorder, InitialProject, InitialProjectConfiguration,
    InitialProjectValidationError, Overlay, OverlayContent, ProjectDocument, ProjectSheet, Rgb,
    VisualDefaults,
};
pub use project_store::{
    DocumentFailure, LoadProjectError, LoadProjectRequest, PathFailure, ProjectLocation,
};

#[cfg(test)]
extern crate self as myalbuns_core;

#[cfg(test)]
#[path = "../../../tests/support/sample_project.rs"]
mod sample_project_fixture;

#[cfg(test)]
mod tests;
