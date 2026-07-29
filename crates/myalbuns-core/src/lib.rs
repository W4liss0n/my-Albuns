mod composition;
mod model;
mod project;
mod sample_project;
mod session;
mod validation;

pub use model::{
    AlbumSnapshot, ComposedFrame, ComposedPhoto, ComposedSheet, CompositionPlan, CoreError,
    EditorProjection, EditorState, ExportResult, FrameSnapshot, Matrix2, MediaCatalogItem,
    MediaTransform, NumberRange, PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot, ProjectIntent,
    RectUm, RenderSnapshot, SheetRole, SheetSnapshot, SizeUm, VectorUm,
};
pub use project::{LoadedProjectRevision, ProjectCore};
pub use session::ProjectSession;

#[cfg(test)]
mod tests;
