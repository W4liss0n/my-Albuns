use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub(crate) const PROJECT_DOCUMENT_SCHEMA_VERSION: u32 = 3;
pub(crate) const RENDER_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
pub(crate) const PHOTO_PAN_MIN: f32 = -1.0;
pub(crate) const PHOTO_PAN_MAX: f32 = 1.0;
pub(crate) const PHOTO_ZOOM_MIN: f32 = 1.0;
pub(crate) const PHOTO_ZOOM_MAX: f32 = 4.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RectUm {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VectorUm {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedPan {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SizeUm {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NumberRange {
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Matrix2 {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhotoPlacement {
    pub center: VectorUm,
    pub size: SizeUm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhotoPlacementPlan {
    pub current_pan: NormalizedPan,
    pub current_zoom: f64,
    pub pan_range: NumberRange,
    pub zoom_range: NumberRange,
    pub current: PhotoPlacement,
    pub pan_origin: VectorUm,
    pub pan_to_center: Matrix2,
    pub pan_to_center_per_zoom: Matrix2,
    pub size_per_zoom: SizeUm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaTransform {
    pub pan_x: f32,
    pub pan_y: f32,
    pub user_zoom: f32,
    pub quarter_turns: i8,
    pub fine_rotation_degrees: f32,
    pub mirror_x: bool,
}

impl Default for MediaTransform {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            user_zoom: 1.0,
            quarter_turns: 0,
            fine_rotation_degrees: 0.0,
            mirror_x: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSnapshot {
    pub media_id: String,
    pub transform: MediaTransform,
}

impl PhotoSnapshot {
    pub(crate) fn for_media(media_id: String) -> Self {
        Self {
            media_id,
            transform: MediaTransform::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FrameSnapshot {
    pub id: String,
    pub rect: RectUm,
    pub z_index: u32,
    pub photo: Option<PhotoSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SheetRole {
    Initial,
    Internal,
    Final,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Photo,
    Decorative,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SheetSnapshot {
    pub id: String,
    pub number: usize,
    pub role: SheetRole,
    pub width_um: i64,
    pub height_um: i64,
    pub frames: Vec<FrameSnapshot>,
    pub overlay_media_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaCatalogItem {
    pub id: String,
    pub kind: MediaKind,
    pub name: String,
    pub source_width_px: u32,
    pub source_height_px: u32,
    pub palette: [String; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSnapshot {
    pub sheets: Vec<SheetSnapshot>,
    pub media: Vec<MediaCatalogItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    pub project_id: String,
    pub project_name: String,
    pub album: AlbumSnapshot,
    pub revision: u64,
    pub saved_revision: u64,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ComposedPhoto {
    pub media_id: String,
    pub name: String,
    pub draw_rect: RectUm,
    pub placement: PhotoPlacementPlan,
    pub rotation_degrees: f32,
    pub mirror_x: bool,
    pub palette: [String; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ComposedFrame {
    pub frame_id: String,
    pub clip_rect: RectUm,
    pub z_index: u32,
    pub photo: Option<ComposedPhoto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ComposedDecorative {
    pub media_id: String,
    pub name: String,
    pub draw_rect: RectUm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ComposedSheet {
    pub sheet_id: String,
    pub number: usize,
    pub width_um: i64,
    pub height_um: i64,
    pub overlay: Option<ComposedDecorative>,
    pub frames: Vec<ComposedFrame>,
}

impl ComposedSheet {
    pub fn referenced_media_ids(&self) -> impl Iterator<Item = &str> {
        self.frames
            .iter()
            .filter_map(|frame| frame.photo.as_ref())
            .map(|photo| photo.media_id.as_str())
            .chain(self.overlay.iter().map(|overlay| overlay.media_id.as_str()))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CompositionPlan {
    pub sheets: Vec<ComposedSheet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaUsage {
    pub media_id: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EditorProjection {
    pub state: EditorState,
    pub composition: CompositionPlan,
    pub media_usage: Vec<MediaUsage>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSnapshot {
    pub schema_version: u32,
    pub project_id: String,
    pub project_name: String,
    pub revision: u64,
    pub unit: String,
    pub composition: CompositionPlan,
}

impl RenderSnapshot {
    pub fn validate(&self) -> Result<(), CoreError> {
        crate::validation::validate_render_snapshot(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectIntent {
    TransformPhoto {
        frame_id: String,
        delta_pan_x: f32,
        delta_pan_y: f32,
        delta_zoom: f32,
    },
    FillLeftmostPlaceholder {
        sheet_id: String,
        media_id: String,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    #[error("Frame não encontrado: {0}")]
    FrameNotFound(String),
    #[error("O Frame não contém uma Foto: {0}")]
    FrameHasNoPhoto(String),
    #[error("Lâmina não encontrada: {0}")]
    SheetNotFound(String),
    #[error("Foto não encontrada no Projeto: {0}")]
    MediaNotFound(String),
    #[error("A Lâmina não possui Frame placeholder: {0}")]
    PlaceholderNotFound(String),
    #[error("Documento de Projeto inválido: {0}")]
    InvalidProject(String),
    #[error("Snapshot de renderização inválido: {0}")]
    InvalidSnapshot(String),
    #[error("Versão de documento não suportada: {0}")]
    UnsupportedSchema(u32),
    #[error("A revisão salva confirmada ({confirmed}) não corresponde à revisão atual ({current})")]
    SavedRevisionMismatch { current: u64, confirmed: u64 },
    #[error("O Projeto já possui uma sessão editável aberta: {project_id}")]
    EditableSessionAlreadyOpen { project_id: String },
}
