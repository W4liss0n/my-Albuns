mod composition;
mod model;
mod project;
mod sample_project;
mod session;
mod validation;

pub use composition::CompositionCore;
pub use model::{
    AlbumSnapshot, ComposedFrame, ComposedPhoto, ComposedSheet, CompositionPlan, CoreError,
    EditorProjection, EditorState, ExportResult, FrameSnapshot, Matrix2, MediaCatalogItem,
    MediaTransform, NumberRange, PHOTO_PAN_MAX, PHOTO_PAN_MIN, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN,
    PROJECT_SCHEMA_VERSION, PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot, ProjectIntent,
    RectUm, RenderSnapshot, SHEET_HEIGHT_UM, SHEET_WIDTH_UM, SheetRole, SheetSnapshot, SizeUm,
    VectorUm,
};
pub use project::{LoadedProjectRevision, ProjectCore};
pub use session::ProjectSession;

#[cfg(test)]
mod tests;
