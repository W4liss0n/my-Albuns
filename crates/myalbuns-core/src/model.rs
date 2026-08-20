use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::project_document::{
    AlbumInformation, DisplayUnit, DocumentSettings, ProjectConfigurationValidationError,
};

pub(crate) const PROJECT_DOCUMENT_SCHEMA_VERSION: u32 = 4;
pub(crate) const RENDER_SNAPSHOT_SCHEMA_VERSION: u32 = 6;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectedDisplayUnit {
    Mm,
    Cm,
    In,
}

impl ProjectedDisplayUnit {
    fn from_domain(display_unit: DisplayUnit) -> Self {
        match display_unit {
            DisplayUnit::Mm => Self::Mm,
            DisplayUnit::Cm => Self::Cm,
            DisplayUnit::In => Self::In,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProjectedActiveSides {
    Both,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Photo,
    Decorative,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectedBackgroundContent {
    Color { rgb: String },
    Media { media_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "scope")]
pub enum ProjectedBackground {
    BothSides {
        both: ProjectedBackgroundContent,
    },
    PerSide {
        left: ProjectedBackgroundContent,
        right: ProjectedBackgroundContent,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectedOverlayContent {
    Media { media_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "scope")]
pub enum ProjectedOverlay {
    BothSides {
        both: Option<ProjectedOverlayContent>,
    },
    PerSide {
        left: Option<ProjectedOverlayContent>,
        right: Option<ProjectedOverlayContent>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ProjectedFrameBorder {
    None,
    Solid { rgb: String, width_um: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedVisualDefaults {
    pub background: ProjectedBackground,
    pub overlay: ProjectedOverlay,
    pub frame_border: ProjectedFrameBorder,
}

impl Default for ProjectedVisualDefaults {
    fn default() -> Self {
        Self {
            background: ProjectedBackground::BothSides {
                both: ProjectedBackgroundContent::Color {
                    rgb: "#FFFFFF".into(),
                },
            },
            overlay: ProjectedOverlay::BothSides { both: None },
            frame_border: ProjectedFrameBorder::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SheetSnapshot {
    pub id: String,
    pub number: usize,
    pub role: SheetRole,
    pub active_sides: ProjectedActiveSides,
    pub page_numbers: Vec<usize>,
    pub width_um: i64,
    pub height_um: i64,
    pub frames: Vec<FrameSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaCatalogItem {
    pub id: String,
    pub kind: MediaKind,
    pub name: String,
    pub source_width_px: Option<u32>,
    pub source_height_px: Option<u32>,
    pub palette: Option<[String; 3]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSnapshot {
    pub sheets: Vec<SheetSnapshot>,
    pub media: Vec<MediaCatalogItem>,
    pub visual_defaults: ProjectedVisualDefaults,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSnapshot {
    pub display_unit: ProjectedDisplayUnit,
    pub sheet_width_um: u64,
    pub sheet_height_um: u64,
    pub dpi: u32,
    pub bleed_um: u64,
    pub safety_um: u64,
}

impl DocumentSnapshot {
    pub(crate) fn from_settings(settings: &DocumentSettings) -> Self {
        Self {
            display_unit: ProjectedDisplayUnit::from_domain(settings.display_unit()),
            sheet_width_um: settings.sheet_width_um(),
            sheet_height_um: settings.sheet_height_um(),
            dpi: settings.dpi(),
            bleed_um: settings.bleed_um(),
            safety_um: settings.safety_um(),
        }
    }

    pub(crate) fn neutral() -> Self {
        Self::from_settings(&DocumentSettings::neutral())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    pub project_id: String,
    pub project_name: String,
    pub document: DocumentSnapshot,
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
    pub border_fill_rects: Vec<RectUm>,
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
pub struct ComposedColor {
    pub rgb: String,
    pub draw_rect: RectUm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum ComposedBackground {
    Color {
        rgb: String,
        draw_rect: RectUm,
    },
    Media {
        media_id: String,
        name: String,
        draw_rect: RectUm,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ComposedSheet {
    pub sheet_id: String,
    pub number: usize,
    pub active_sides: ProjectedActiveSides,
    pub width_um: i64,
    pub height_um: i64,
    pub base: ComposedColor,
    pub backgrounds: Vec<ComposedBackground>,
    pub frames: Vec<ComposedFrame>,
    pub overlays: Vec<ComposedDecorative>,
}

impl ComposedSheet {
    pub fn referenced_media_ids(&self) -> impl Iterator<Item = &str> {
        self.backgrounds
            .iter()
            .filter_map(|background| match background {
                ComposedBackground::Color { .. } => None,
                ComposedBackground::Media { media_id, .. } => Some(media_id.as_str()),
            })
            .chain(
                self.frames
                    .iter()
                    .filter_map(|frame| frame.photo.as_ref())
                    .map(|photo| photo.media_id.as_str()),
            )
            .chain(
                self.overlays
                    .iter()
                    .map(|overlay| overlay.media_id.as_str()),
            )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CompositionPlan {
    pub frame_border: ProjectedFrameBorder,
    pub sheets: Vec<ComposedSheet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposedOutputUnit {
    pub frame_border: ProjectedFrameBorder,
    pub sheet: ComposedSheet,
}

impl ComposedOutputUnit {
    pub fn validate(&self) -> Result<(), CoreError> {
        crate::validation::validate_composed_output_unit(self)
    }
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
    pub dpi: u32,
    pub unit: String,
    pub composition: CompositionPlan,
}

impl RenderSnapshot {
    pub fn validate(&self) -> Result<(), CoreError> {
        crate::validation::validate_render_snapshot(self)
    }

    pub fn output_unit(&self, sheet_id: &str) -> Result<ComposedOutputUnit, CoreError> {
        let sheet = self
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .cloned()
            .ok_or_else(|| CoreError::SheetNotFound(sheet_id.to_owned()))?;
        Ok(ComposedOutputUnit {
            frame_border: self.composition.frame_border.clone(),
            sheet,
        })
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
    SetAlbumInformation {
        information: AlbumInformation,
    },
    SetDpi {
        dpi: u32,
    },
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
    #[error("A Sessão editável do Projeto foi invalidada e precisa ser reaberta")]
    EditableSessionInvalidated,
    #[error("O DPI {0} não é válido para as dimensões atuais do Projeto")]
    InvalidDpi(u32),
    #[error("As Informações do Álbum não são válidas")]
    InvalidAlbumInformation(Vec<ProjectConfigurationValidationError>),
    #[error("A Sessão do Projeto esgotou o intervalo seguro de Revisões")]
    RevisionSpaceExhausted,
    #[error("A intenção não é compatível com o Documento de Projeto v1")]
    UnsupportedProjectIntent,
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
