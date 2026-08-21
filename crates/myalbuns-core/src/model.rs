use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;
use ts_rs::TS;
use uuid::{Uuid, Version};

use crate::project_document::{DisplayUnit, DocumentSettings};

pub(crate) const RENDER_SNAPSHOT_SCHEMA_VERSION: u32 = 6;
pub(crate) const PHOTO_PAN_MIN: f32 = -1.0;
pub(crate) const PHOTO_PAN_MAX: f32 = 1.0;
pub(crate) const PHOTO_ZOOM_MIN: f32 = 1.0;
pub(crate) const PHOTO_ZOOM_MAX: f32 = 4.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MediaId(Uuid);

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Identidade de mídia inválida; esperado UUID v4 canônico")]
pub struct ParseMediaIdError;

impl MediaId {
    pub(crate) fn from_uuid(value: Uuid) -> Self {
        Self::try_from(value).expect("a identidade interna de mídia deve ser UUID v4")
    }

    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl TryFrom<Uuid> for MediaId {
    type Error = ParseMediaIdError;

    fn try_from(value: Uuid) -> Result<Self, Self::Error> {
        if value.get_version() != Some(Version::Random) {
            return Err(ParseMediaIdError);
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for MediaId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::str::FromStr for MediaId {
    type Err = ParseMediaIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = Uuid::parse_str(value).map_err(|_| ParseMediaIdError)?;
        if parsed.get_version() != Some(Version::Random) || parsed.hyphenated().to_string() != value
        {
            return Err(ParseMediaIdError);
        }
        Ok(Self(parsed))
    }
}

impl From<MediaId> for String {
    fn from(value: MediaId) -> Self {
        value.to_string()
    }
}

/// A creative command that changes one persisted media occurrence.
///
/// The native path deliberately has no serde or TypeScript representation, so
/// only the trusted Host can construct this command after `MediaResolver`
/// validates a user-selected file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelinkMedia {
    pub(crate) media_id: MediaId,
    pub(crate) replacement_path: PathBuf,
}

impl RelinkMedia {
    pub fn new(media_id: MediaId, replacement_path: PathBuf) -> Self {
        Self {
            media_id,
            replacement_path,
        }
    }
}

impl Serialize for MediaId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.hyphenated().to_string())
    }
}

impl<'de> Deserialize<'de> for MediaId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

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
/// A placement keeps the minimum Frame-filling scale separate from the
/// user's relative Zoom (`current_zoom == 1.0` means no adjustment).
pub struct PhotoPlacementPlan {
    pub base_fill_zoom: f64,
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
    #[ts(type = "string")]
    pub media_id: MediaId,
    pub transform: MediaTransform,
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
    Color {
        rgb: String,
    },
    Media {
        #[ts(type = "string")]
        media_id: MediaId,
    },
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
    Media {
        #[ts(type = "string")]
        media_id: MediaId,
    },
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
    pub width_um: i64,
    pub height_um: i64,
    pub frames: Vec<FrameSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaCatalogItem {
    #[ts(type = "string")]
    pub id: MediaId,
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
    #[ts(type = "string")]
    pub media_id: MediaId,
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
    #[ts(type = "string")]
    pub media_id: MediaId,
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
        #[ts(type = "string")]
        media_id: MediaId,
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
    pub fn referenced_media_ids(&self) -> impl Iterator<Item = MediaId> + '_ {
        self.backgrounds
            .iter()
            .filter_map(|background| match background {
                ComposedBackground::Color { .. } => None,
                ComposedBackground::Media { media_id, .. } => Some(*media_id),
            })
            .chain(
                self.frames
                    .iter()
                    .filter_map(|frame| frame.photo.as_ref())
                    .map(|photo| photo.media_id),
            )
            .chain(self.overlays.iter().map(|overlay| overlay.media_id))
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
    #[ts(type = "string")]
    pub media_id: MediaId,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EditorProjection {
    pub state: EditorState,
    pub composition: CompositionPlan,
    pub media_usage: Vec<MediaUsage>,
}

/// Runtime-only presentation facts observed or assigned for the linked
/// Original while its opaque preview is resolved.
///
/// This type deliberately has no serde representation. It can enrich a
/// projection, but can never become part of the Project document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhotoSourceMetadata {
    source_width_px: u32,
    source_height_px: u32,
    palette: [String; 3],
}

impl PhotoSourceMetadata {
    pub fn new(
        source_width_px: u32,
        source_height_px: u32,
        palette: [String; 3],
    ) -> Result<Self, CoreError> {
        if source_width_px == 0
            || source_height_px == 0
            || palette.iter().any(|color| !is_canonical_rgb(color))
        {
            return Err(CoreError::InvalidPhotoSourceMetadata);
        }
        Ok(Self {
            source_width_px,
            source_height_px,
            palette,
        })
    }

    pub const fn source_width_px(&self) -> u32 {
        self.source_width_px
    }

    pub const fn source_height_px(&self) -> u32 {
        self.source_height_px
    }

    pub fn palette(&self) -> &[String; 3] {
        &self.palette
    }
}

fn is_canonical_rgb(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(byte))
}

/// Trusted native import command. The Host constructs it only after the
/// selected JPEG has passed path and codec inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportPhoto {
    pub(crate) path: PathBuf,
    pub(crate) source_metadata: PhotoSourceMetadata,
}

impl ImportPhoto {
    pub fn new(path: PathBuf, source_metadata: PhotoSourceMetadata) -> Self {
        Self {
            path,
            source_metadata,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PhotoPlacementMode {
    Normal,
    Edit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMutationOutcome {
    pub projection: EditorProjection,
    pub affected_frame_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportPhotoOutcome {
    pub projection: EditorProjection,
    pub media_id: MediaId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(tag = "kind")]
pub enum PhotoDropTarget {
    Frame { frame_id: String },
    Sheet { sheet_id: String },
    Invalid,
}

/// Borrowed rendering envelope over one already resolved CompositionPlan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderSnapshotRef<'a> {
    pub schema_version: u32,
    pub project_id: &'a str,
    pub project_name: &'a str,
    pub revision: u64,
    pub dpi: u32,
    pub unit: &'static str,
    pub composition: &'a CompositionPlan,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RenderSnapshotMetadata<'a> {
    project_id: &'a str,
    project_name: &'a str,
    revision: u64,
    dpi: u32,
}

impl<'a> From<&'a EditorState> for RenderSnapshotMetadata<'a> {
    fn from(state: &'a EditorState) -> Self {
        Self {
            project_id: &state.project_id,
            project_name: &state.project_name,
            revision: state.revision,
            dpi: state.document.dpi,
        }
    }
}

impl<'a> RenderSnapshotRef<'a> {
    pub(crate) fn from_resolved(
        metadata: RenderSnapshotMetadata<'a>,
        composition: &'a CompositionPlan,
    ) -> Self {
        Self {
            schema_version: RENDER_SNAPSHOT_SCHEMA_VERSION,
            project_id: metadata.project_id,
            project_name: metadata.project_name,
            revision: metadata.revision,
            dpi: metadata.dpi,
            unit: "micrometers",
            composition,
        }
    }
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
    pub(crate) fn from_resolved(
        metadata: RenderSnapshotMetadata<'_>,
        composition: CompositionPlan,
    ) -> Self {
        Self {
            schema_version: RENDER_SNAPSHOT_SCHEMA_VERSION,
            project_id: metadata.project_id.to_owned(),
            project_name: metadata.project_name.to_owned(),
            revision: metadata.revision,
            dpi: metadata.dpi,
            unit: "micrometers".into(),
            composition,
        }
    }

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
    SetDpi {
        dpi: u32,
    },
    TransformPhoto {
        frame_id: String,
        delta_pan_x: f32,
        delta_pan_y: f32,
        delta_zoom: f32,
    },
    AddPhoto {
        sheet_id: String,
        #[ts(type = "string")]
        media_id: MediaId,
        mode: PhotoPlacementMode,
    },
    DropPhoto {
        sheet_id: String,
        #[ts(type = "string")]
        media_id: MediaId,
        x_um: i64,
        y_um: i64,
        mode: PhotoPlacementMode,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    #[error("A Sessão editável do Projeto foi invalidada e precisa ser reaberta")]
    EditableSessionInvalidated,
    #[error("O DPI {0} não é válido para as dimensões atuais do Projeto")]
    InvalidDpi(u32),
    #[error("A Sessão do Projeto esgotou o intervalo seguro de Revisões")]
    RevisionSpaceExhausted,
    #[error("A intenção não é compatível com o Documento de Projeto v1")]
    UnsupportedProjectIntent,
    #[error("Os metadados observados da Foto não são válidos")]
    InvalidPhotoSourceMetadata,
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
