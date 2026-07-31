mod composition;
mod model;
mod project;
mod session;
mod validation;

pub use model::{
    AlbumSnapshot, ComposedDecorative, ComposedFrame, ComposedPhoto, ComposedSheet,
    CompositionPlan, CoreError, EditorProjection, EditorState, FrameSnapshot, Matrix2,
    MediaCatalogItem, MediaKind, MediaTransform, MediaUsage, NormalizedPan, NumberRange,
    PhotoPlacement, PhotoPlacementPlan, PhotoSnapshot, ProjectIntent, RectUm, RenderSnapshot,
    SheetRole, SheetSnapshot, SizeUm, VectorUm,
};
pub use project::{EditableProject, LoadedProjectRevision, ProjectCore};

#[cfg(test)]
extern crate self as myalbuns_core;

#[cfg(test)]
#[path = "../../../tests/support/sample_project.rs"]
mod sample_project_fixture;

#[cfg(test)]
mod tests;
