//! The public Project File (`.myalbuns`), schema version 1.
//!
//! One set of file DTOs maps directly to the domain in both directions. The
//! DTOs never reuse domain types, so refactoring the model cannot change the
//! file by accident. Optional values are omitted while they hold their default;
//! unknown fields are always rejected.
use std::{
    fmt,
    path::{Path, PathBuf},
};

use myalbuns_paths::validate_external_path;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{IgnoredAny, MapAccess, Visitor},
};
use serde_json::Value;
use uuid::{Uuid, Version};

use super::{DecodeFailure, DocumentFailure, PathFailure};
use crate::project_document::{
    ActiveSides, Background, BackgroundContent, DisplayUnit, DocumentSettings, FrameBorder,
    FrameBorderValues, FrameStyle, MAX_SAFE_INTEGER, MediaRef, Overlay, OverlayContent,
    ProjectDocument, ProjectFrame, ProjectPhoto, ProjectPhotoTransform, ProjectRect,
    ProjectRevision, ProjectSheet, Rgb, VisualDefaults, frame_border_width_is_valid,
    validate_project_state,
};
use crate::{
    FavoriteLayout, LayoutDefinition, LayoutFavoriteId, LayoutOrigin, LayoutParameters,
    LayoutPermission, LayoutScope, LayoutSettings, LayoutSurface, LayoutSurfaceKind, MediaFolder,
    MediaId, MediaKind, ProjectedBackgroundContent, ProjectedOverlayContent, RectUm, SheetVisual,
    SheetVisuals, SideVisual, StoredLayout, VisualMapping,
};

const DOCUMENT_TYPE: &str = "myalbuns.project";
pub(super) const SCHEMA_VERSION: u32 = 1;
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

pub(super) fn decode(bytes: &[u8]) -> Result<ProjectRevision, DecodeFailure> {
    if bytes.starts_with(UTF8_BOM) {
        return Err(invalid_document());
    }
    classify_header(bytes)?;
    let file: ProjectFile = serde_json::from_slice(bytes).map_err(|_| invalid_document())?;
    file.into_domain()
}

pub(super) fn encode(revision: &ProjectRevision) -> Result<Vec<u8>, DecodeFailure> {
    validate_project_state(&revision.project)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
    if revision.revision > MAX_SAFE_INTEGER {
        return Err(invalid_document());
    }
    to_bytes(&ProjectFile::from_domain(revision))
}

/// Gives a copy a new Identity while keeping every other byte of its content.
pub(super) fn rewrite_project_id(
    bytes: &[u8],
    project_id: Uuid,
) -> Result<(Vec<u8>, ProjectRevision), DecodeFailure> {
    decode(bytes)?;
    let mut file: ProjectFile = serde_json::from_slice(bytes).map_err(|_| invalid_document())?;
    file.project_id = project_id.hyphenated().to_string();
    let rewritten = to_bytes(&file)?;
    let candidate = decode(&rewritten)?;
    Ok((rewritten, candidate))
}

fn to_bytes(file: &ProjectFile) -> Result<Vec<u8>, DecodeFailure> {
    let mut bytes = serde_json::to_vec_pretty(file).map_err(|_| invalid_document())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn classify_header(bytes: &[u8]) -> Result<(), DecodeFailure> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let header = DocumentHeader::deserialize(&mut deserializer).map_err(|_| invalid_document())?;
    deserializer.end().map_err(|_| invalid_document())?;
    if header.document_type.as_deref() != Some(DOCUMENT_TYPE) {
        return Err(document_failure(DocumentFailure::InvalidDocumentType));
    }
    match header.schema_version {
        Some(SCHEMA_VERSION) => Ok(()),
        Some(0) => Err(document_failure(DocumentFailure::UnsupportedLegacySchema {
            version: 0,
        })),
        Some(version) => Err(document_failure(DocumentFailure::UnsupportedFutureSchema {
            version,
        })),
        None => Err(invalid_document()),
    }
}

fn document_failure(failure: DocumentFailure) -> DecodeFailure {
    DecodeFailure::Document(failure)
}

fn invalid_document() -> DecodeFailure {
    document_failure(DocumentFailure::InvalidProjectDocument)
}

fn invalid_state() -> DecodeFailure {
    document_failure(DocumentFailure::InvalidProjectState)
}

fn parse_uuid_v4(source: &str) -> Result<Uuid, DecodeFailure> {
    let parsed = Uuid::parse_str(source).map_err(|_| invalid_document())?;
    if parsed.get_version() != Some(Version::Random) || parsed.hyphenated().to_string() != source {
        return Err(invalid_document());
    }
    Ok(parsed)
}

fn parse_rgb(source: &str) -> Result<Rgb, DecodeFailure> {
    Rgb::parse_canonical(source).ok_or_else(invalid_document)
}

fn id_text(id: Uuid) -> String {
    id.hyphenated().to_string()
}

fn is_false(value: &bool) -> bool {
    !*value
}

// ---------------------------------------------------------------------------
// Document

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectFile {
    document_type: String,
    schema_version: u32,
    project_id: String,
    revision: u64,
    project: ProjectDto,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDto {
    album: AlbumDto,
    layout_settings: LayoutSettingsDto,
    visual_defaults: VisualDefaultsDto,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    media: Vec<MediaDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    media_folders: Vec<MediaFolderDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    favorite_layouts: Vec<FavoriteLayoutDto>,
    sheets: Vec<SheetDto>,
}

impl ProjectFile {
    fn from_domain(revision: &ProjectRevision) -> Self {
        let project = &revision.project;
        Self {
            document_type: DOCUMENT_TYPE.into(),
            schema_version: SCHEMA_VERSION,
            project_id: id_text(revision.project_id),
            revision: revision.revision,
            project: ProjectDto {
                album: AlbumDto::from_domain(project.document()),
                layout_settings: LayoutSettingsDto::from_domain(project.layout_settings()),
                visual_defaults: VisualDefaultsDto::from_domain(project.visual_defaults()),
                media: project.media().iter().map(MediaDto::from_domain).collect(),
                media_folders: project
                    .media_folders()
                    .iter()
                    .map(MediaFolderDto::from_domain)
                    .collect(),
                favorite_layouts: project
                    .favorite_layouts()
                    .iter()
                    .map(FavoriteLayoutDto::from_domain)
                    .collect(),
                sheets: project.sheets().iter().map(SheetDto::from_domain).collect(),
            },
        }
    }

    fn into_domain(self) -> Result<ProjectRevision, DecodeFailure> {
        if self.document_type != DOCUMENT_TYPE
            || self.schema_version != SCHEMA_VERSION
            || self.revision > MAX_SAFE_INTEGER
        {
            return Err(invalid_document());
        }
        let project_id = parse_uuid_v4(&self.project_id)?;
        let ProjectDto {
            album,
            layout_settings,
            visual_defaults,
            media,
            media_folders,
            favorite_layouts,
            sheets,
        } = self.project;

        let settings = album.into_domain()?;
        let visual_defaults = visual_defaults.into_domain()?;
        let media = media
            .into_iter()
            .map(MediaDto::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let mut last_layouts = Vec::with_capacity(sheets.len());
        let mut locks = Vec::with_capacity(sheets.len());
        let mut visuals = Vec::new();
        let mut domain_sheets = Vec::with_capacity(sheets.len());
        for sheet in sheets {
            let id = parse_uuid_v4(&sheet.id)?;
            last_layouts.push(
                sheet
                    .last_layout
                    .map(StoredLayoutDto::into_domain)
                    .transpose()?,
            );
            locks.push(sheet.layout_locked);
            if let Some(sheet_visuals) = sheet.visuals {
                visuals.push((id, sheet_visuals.into_domain()?));
            }
            let frames = sheet
                .frames
                .into_iter()
                .map(FrameDto::into_domain)
                .collect::<Result<Vec<_>, _>>()?;
            domain_sheets.push(ProjectSheet::with_frames(
                id,
                sheet.active_sides.into(),
                frames,
            ));
        }
        let favorites = favorite_layouts
            .into_iter()
            .map(FavoriteLayoutDto::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let folders = media_folders
            .into_iter()
            .map(MediaFolderDto::into_domain)
            .collect::<Result<Vec<_>, _>>()?;

        let project = ProjectDocument::new(settings, visual_defaults, media, domain_sheets);
        validate_project_state(&project).map_err(|_| invalid_state())?;
        let project = project
            .restore_layout_state(layout_settings.into_domain(), last_layouts)
            .and_then(|project| project.restore_layout_locks(locks))
            .and_then(|project| project.restore_favorite_layouts(favorites))
            .and_then(|project| project.restore_sheet_visuals(visuals))
            .and_then(|project| project.restore_media_folders(folders))
            .map_err(|_| invalid_state())?;
        Ok(ProjectRevision::new(project_id, self.revision, project))
    }
}

// ---------------------------------------------------------------------------
// Album and Layout settings

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AlbumDto {
    display_unit: DisplayUnitDto,
    sheet_width_um: u64,
    sheet_height_um: u64,
    dpi: u32,
    bleed_um: u64,
    safety_um: u64,
}

impl AlbumDto {
    fn from_domain(settings: &DocumentSettings) -> Self {
        Self {
            display_unit: settings.display_unit().into(),
            sheet_width_um: settings.sheet_width_um(),
            sheet_height_um: settings.sheet_height_um(),
            dpi: settings.dpi(),
            bleed_um: settings.bleed_um(),
            safety_um: settings.safety_um(),
        }
    }

    fn into_domain(self) -> Result<DocumentSettings, DecodeFailure> {
        if self.sheet_width_um == 0
            || self.sheet_width_um > MAX_SAFE_INTEGER
            || self.sheet_height_um == 0
            || self.sheet_height_um > MAX_SAFE_INTEGER
            || !(1..=1_200).contains(&self.dpi)
            || self.bleed_um > MAX_SAFE_INTEGER
            || self.safety_um > MAX_SAFE_INTEGER
        {
            return Err(invalid_document());
        }
        Ok(DocumentSettings::new(
            self.display_unit.into(),
            self.sheet_width_um,
            self.sheet_height_um,
            self.dpi,
            self.bleed_um,
            self.safety_um,
        ))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DisplayUnitDto {
    Mm,
    Cm,
    In,
}

impl From<DisplayUnitDto> for DisplayUnit {
    fn from(value: DisplayUnitDto) -> Self {
        match value {
            DisplayUnitDto::Mm => Self::Mm,
            DisplayUnitDto::Cm => Self::Cm,
            DisplayUnitDto::In => Self::In,
        }
    }
}

impl From<DisplayUnit> for DisplayUnitDto {
    fn from(value: DisplayUnit) -> Self {
        match value {
            DisplayUnit::Mm => Self::Mm,
            DisplayUnit::Cm => Self::Cm,
            DisplayUnit::In => Self::In,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutSettingsDto {
    permission: LayoutPermissionDto,
    margin_um: i64,
    gap_um: i64,
    minimum_side_um: i64,
}

impl LayoutSettingsDto {
    fn from_domain(settings: &LayoutSettings) -> Self {
        Self {
            permission: settings.permission.into(),
            margin_um: settings.parameters.margin_um,
            gap_um: settings.parameters.gap_um,
            minimum_side_um: settings.parameters.minimum_side_um,
        }
    }

    fn into_domain(self) -> LayoutSettings {
        LayoutSettings {
            permission: self.permission.into(),
            parameters: LayoutParameters {
                margin_um: self.margin_um,
                gap_um: self.gap_um,
                minimum_side_um: self.minimum_side_um,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum LayoutPermissionDto {
    PagesOnly,
    PagesAndSheet,
}

impl From<LayoutPermission> for LayoutPermissionDto {
    fn from(value: LayoutPermission) -> Self {
        match value {
            LayoutPermission::PagesOnly => Self::PagesOnly,
            LayoutPermission::PagesAndSheet => Self::PagesAndSheet,
        }
    }
}

impl From<LayoutPermissionDto> for LayoutPermission {
    fn from(value: LayoutPermissionDto) -> Self {
        match value {
            LayoutPermissionDto::PagesOnly => Self::PagesOnly,
            LayoutPermissionDto::PagesAndSheet => Self::PagesAndSheet,
        }
    }
}

// ---------------------------------------------------------------------------
// Visual defaults and per-Sheet visuals

/// The same content on both sides, or one content per side.
#[derive(Deserialize, Serialize)]
#[serde(tag = "sides", rename_all = "camelCase", deny_unknown_fields)]
enum SidesDto<T> {
    Both { both: T },
    PerSide { left: T, right: T },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum BackgroundContentDto {
    Color { rgb: String },
    Media { media_id: String },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum OverlayContentDto {
    None,
    Media { media_id: String },
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum FrameBorderDto {
    None,
    Solid { rgb: String, width_um: u64 },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VisualDefaultsDto {
    background: SidesDto<BackgroundContentDto>,
    overlay: SidesDto<OverlayContentDto>,
    frame_border: FrameBorderDto,
}

impl VisualDefaultsDto {
    fn from_domain(defaults: &VisualDefaults) -> Self {
        let background = |content: &BackgroundContent| match content {
            BackgroundContent::Color { rgb } => BackgroundContentDto::Color {
                rgb: rgb.canonical_hex(),
            },
            BackgroundContent::Media { media_id } => BackgroundContentDto::Media {
                media_id: id_text(*media_id),
            },
        };
        let overlay = |content: &Option<OverlayContent>| match content {
            None => OverlayContentDto::None,
            Some(OverlayContent::Media { media_id }) => OverlayContentDto::Media {
                media_id: id_text(*media_id),
            },
        };
        Self {
            background: match defaults.background() {
                Background::BothSides { both } => SidesDto::Both {
                    both: background(both),
                },
                Background::PerSide { left, right } => SidesDto::PerSide {
                    left: background(left),
                    right: background(right),
                },
            },
            overlay: match defaults.overlay() {
                Overlay::BothSides { both } => SidesDto::Both {
                    both: overlay(both),
                },
                Overlay::PerSide { left, right } => SidesDto::PerSide {
                    left: overlay(left),
                    right: overlay(right),
                },
            },
            frame_border: match defaults.frame_border() {
                FrameBorder::None => FrameBorderDto::None,
                FrameBorder::Solid { rgb, width_um } => FrameBorderDto::Solid {
                    rgb: rgb.canonical_hex(),
                    width_um: *width_um,
                },
            },
        }
    }

    fn into_domain(self) -> Result<VisualDefaults, DecodeFailure> {
        let background = |content: BackgroundContentDto| match content {
            BackgroundContentDto::Color { rgb } => Ok(BackgroundContent::Color {
                rgb: parse_rgb(&rgb)?,
            }),
            BackgroundContentDto::Media { media_id } => Ok(BackgroundContent::Media {
                media_id: parse_uuid_v4(&media_id)?,
            }),
        };
        let overlay = |content: OverlayContentDto| match content {
            OverlayContentDto::None => Ok(None),
            OverlayContentDto::Media { media_id } => Ok(Some(OverlayContent::Media {
                media_id: parse_uuid_v4(&media_id)?,
            })),
        };
        let background = match self.background {
            SidesDto::Both { both } => Background::BothSides {
                both: background(both)?,
            },
            SidesDto::PerSide { left, right } => Background::PerSide {
                left: background(left)?,
                right: background(right)?,
            },
        };
        let overlay = match self.overlay {
            SidesDto::Both { both } => Overlay::BothSides {
                both: overlay(both)?,
            },
            SidesDto::PerSide { left, right } => Overlay::PerSide {
                left: overlay(left)?,
                right: overlay(right)?,
            },
        };
        let frame_border = match self.frame_border {
            FrameBorderDto::None => FrameBorder::None,
            FrameBorderDto::Solid { rgb, width_um } => {
                if !frame_border_width_is_valid(width_um) {
                    return Err(invalid_document());
                }
                FrameBorder::Solid {
                    rgb: parse_rgb(&rgb)?,
                    width_um,
                }
            }
        };
        Ok(VisualDefaults::new(background, overlay, frame_border))
    }
}

/// Per-Sheet visuals. An absent role, or an absent side, inherits the Album.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SheetVisualsDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<SheetSidesDto<BackgroundContentDto>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<SheetSidesDto<OverlayContentDto>>,
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "sides",
    rename_all = "camelCase",
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de>")
)]
enum SheetSidesDto<T> {
    Both {
        both: T,
    },
    PerSide {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        left: Option<SideContentDto<T>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        right: Option<SideContentDto<T>>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de>")
)]
struct SideContentDto<T> {
    content: T,
    /// `bothSides` when this side keeps half of a content that covered the
    /// whole Sheet before the sides were split.
    #[serde(default, skip_serializing_if = "MappingDto::is_side")]
    mapping: MappingDto,
}

#[derive(Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum MappingDto {
    #[default]
    Side,
    BothSides,
}

impl MappingDto {
    fn is_side(&self) -> bool {
        *self == Self::Side
    }
}

impl From<VisualMapping> for MappingDto {
    fn from(value: VisualMapping) -> Self {
        match value {
            VisualMapping::Side => Self::Side,
            VisualMapping::BothSides => Self::BothSides,
        }
    }
}

impl From<MappingDto> for VisualMapping {
    fn from(value: MappingDto) -> Self {
        match value {
            MappingDto::Side => Self::Side,
            MappingDto::BothSides => Self::BothSides,
        }
    }
}

impl SheetVisualsDto {
    fn from_domain(visuals: &SheetVisuals) -> Option<Self> {
        if visuals.is_default() {
            return None;
        }
        Some(Self {
            background: sheet_sides_from_domain(&visuals.background, background_projected),
            overlay: sheet_sides_from_domain(&visuals.overlay, overlay_projected),
        })
    }

    fn into_domain(self) -> Result<SheetVisuals, DecodeFailure> {
        Ok(SheetVisuals {
            background: sheet_sides_into_domain(self.background, background_domain)?,
            overlay: sheet_sides_into_domain(self.overlay, overlay_domain)?,
        })
    }
}

fn sheet_sides_from_domain<T, D>(
    visual: &SheetVisual<T>,
    map: impl Fn(&T) -> D,
) -> Option<SheetSidesDto<D>> {
    let side = |side: &SideVisual<T>| match side {
        SideVisual::Default => None,
        SideVisual::Custom { content, mapping } => Some(SideContentDto {
            content: map(content),
            mapping: (*mapping).into(),
        }),
    };
    match visual {
        SheetVisual::Default => None,
        SheetVisual::BothSides { content } => Some(SheetSidesDto::Both { both: map(content) }),
        SheetVisual::PerSide { left, right } => Some(SheetSidesDto::PerSide {
            left: side(left),
            right: side(right),
        }),
    }
}

fn sheet_sides_into_domain<T, D>(
    sides: Option<SheetSidesDto<D>>,
    map: impl Fn(D) -> Result<T, DecodeFailure>,
) -> Result<SheetVisual<T>, DecodeFailure> {
    let side = |side: Option<SideContentDto<D>>| -> Result<SideVisual<T>, DecodeFailure> {
        Ok(match side {
            None => SideVisual::Default,
            Some(side) => SideVisual::Custom {
                content: map(side.content)?,
                mapping: side.mapping.into(),
            },
        })
    };
    Ok(match sides {
        None => SheetVisual::Default,
        Some(SheetSidesDto::Both { both }) => SheetVisual::BothSides {
            content: map(both)?,
        },
        Some(SheetSidesDto::PerSide { left, right }) => {
            // An all-inherited split is not a distinct state; the domain
            // collapses it to the Album default, so the file must not carry it.
            if left.is_none() && right.is_none() {
                return Err(invalid_document());
            }
            SheetVisual::PerSide {
                left: side(left)?,
                right: side(right)?,
            }
        }
    })
}

fn background_projected(content: &ProjectedBackgroundContent) -> BackgroundContentDto {
    match content {
        ProjectedBackgroundContent::Color { rgb } => {
            BackgroundContentDto::Color { rgb: rgb.clone() }
        }
        ProjectedBackgroundContent::Media { media_id } => BackgroundContentDto::Media {
            media_id: media_id.to_string(),
        },
    }
}

fn overlay_projected(content: &Option<ProjectedOverlayContent>) -> OverlayContentDto {
    match content {
        None => OverlayContentDto::None,
        Some(ProjectedOverlayContent::Media { media_id }) => OverlayContentDto::Media {
            media_id: media_id.to_string(),
        },
    }
}

fn background_domain(
    content: BackgroundContentDto,
) -> Result<ProjectedBackgroundContent, DecodeFailure> {
    Ok(match content {
        BackgroundContentDto::Color { rgb } => ProjectedBackgroundContent::Color {
            rgb: parse_rgb(&rgb)?.canonical_hex(),
        },
        BackgroundContentDto::Media { media_id } => ProjectedBackgroundContent::Media {
            media_id: MediaId::from_uuid(parse_uuid_v4(&media_id)?),
        },
    })
}

fn overlay_domain(
    content: OverlayContentDto,
) -> Result<Option<ProjectedOverlayContent>, DecodeFailure> {
    Ok(match content {
        OverlayContentDto::None => None,
        OverlayContentDto::Media { media_id } => Some(ProjectedOverlayContent::Media {
            media_id: MediaId::from_uuid(parse_uuid_v4(&media_id)?),
        }),
    })
}

// ---------------------------------------------------------------------------
// Media

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaDto {
    id: String,
    kind: MediaKindDto,
    path: PathDto,
}

impl MediaDto {
    fn from_domain(media: &MediaRef) -> Self {
        Self {
            id: id_text(media.id()),
            kind: media.kind().into(),
            path: PathDto::from_path(media.path()),
        }
    }

    fn into_domain(self) -> Result<MediaRef, DecodeFailure> {
        let path = self.path.into_path()?;
        validate_external_path(&path).map_err(|_| DecodeFailure::Path(PathFailure::InvalidPath))?;
        Ok(MediaRef::new(
            parse_uuid_v4(&self.id)?,
            self.kind.into(),
            path,
        ))
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum MediaKindDto {
    Photo,
    Decorative,
}

impl From<MediaKind> for MediaKindDto {
    fn from(value: MediaKind) -> Self {
        match value {
            MediaKind::Photo => Self::Photo,
            MediaKind::Decorative => Self::Decorative,
        }
    }
}

impl From<MediaKindDto> for MediaKind {
    fn from(value: MediaKindDto) -> Self {
        match value {
            MediaKindDto::Photo => Self::Photo,
            MediaKindDto::Decorative => Self::Decorative,
        }
    }
}

/// A Windows path is text whenever it is well-formed UTF-16. Only a name with
/// unpaired surrogates keeps its exact code units, so every path stays
/// reversible (ADR 0007).
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum PathDto {
    Text(String),
    Units(PathUnitsDto),
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathUnitsDto {
    windows_utf16: Vec<u16>,
}

impl PathDto {
    #[cfg(windows)]
    fn from_path(path: &Path) -> Self {
        use std::os::windows::ffi::OsStrExt;

        match path.to_str() {
            Some(text) => Self::Text(text.to_owned()),
            None => Self::Units(PathUnitsDto {
                windows_utf16: path.as_os_str().encode_wide().collect(),
            }),
        }
    }

    #[cfg(not(windows))]
    fn from_path(path: &Path) -> Self {
        Self::Text(path.to_string_lossy().into_owned())
    }

    #[cfg(windows)]
    fn into_path(self) -> Result<PathBuf, DecodeFailure> {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};

        match self {
            Self::Text(text) => Ok(PathBuf::from(text)),
            // Well-formed units must be written as text, so this form is kept
            // to exactly one encoding per path.
            Self::Units(units) if String::from_utf16(&units.windows_utf16).is_ok() => {
                Err(invalid_document())
            }
            Self::Units(units) => Ok(PathBuf::from(OsString::from_wide(&units.windows_utf16))),
        }
    }

    #[cfg(not(windows))]
    fn into_path(self) -> Result<PathBuf, DecodeFailure> {
        Err(DecodeFailure::Path(PathFailure::InvalidPath))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaFolderDto {
    id: String,
    kind: MediaKindDto,
    name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    media_ids: Vec<String>,
}

impl MediaFolderDto {
    fn from_domain(folder: &MediaFolder) -> Self {
        Self {
            id: folder.id.clone(),
            kind: folder.kind.into(),
            name: folder.name.clone(),
            media_ids: folder.media_ids.iter().map(ToString::to_string).collect(),
        }
    }

    fn into_domain(self) -> Result<MediaFolder, DecodeFailure> {
        Ok(MediaFolder {
            id: self.id,
            kind: self.kind.into(),
            name: self.name,
            media_ids: self
                .media_ids
                .into_iter()
                .map(|id| parse_uuid_v4(&id).map(MediaId::from_uuid))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Sheets, Frames and Photos

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SheetDto {
    id: String,
    active_sides: ActiveSidesDto,
    #[serde(default, skip_serializing_if = "is_false")]
    layout_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_layout: Option<StoredLayoutDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    visuals: Option<SheetVisualsDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    frames: Vec<FrameDto>,
}

impl SheetDto {
    fn from_domain(sheet: &ProjectSheet) -> Self {
        Self {
            id: id_text(sheet.id()),
            active_sides: sheet.active_sides().into(),
            layout_locked: sheet.layout_locked(),
            last_layout: sheet.last_layout().map(StoredLayoutDto::from_domain),
            visuals: SheetVisualsDto::from_domain(sheet.visuals()),
            frames: sheet.frames().iter().map(FrameDto::from_domain).collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ActiveSidesDto {
    Both,
    Left,
    Right,
}

impl From<ActiveSidesDto> for ActiveSides {
    fn from(value: ActiveSidesDto) -> Self {
        match value {
            ActiveSidesDto::Both => Self::Both,
            ActiveSidesDto::Left => Self::Left,
            ActiveSidesDto::Right => Self::Right,
        }
    }
}

impl From<ActiveSides> for ActiveSidesDto {
    fn from(value: ActiveSides) -> Self {
        match value {
            ActiveSides::Both => Self::Both,
            ActiveSides::Left => Self::Left,
            ActiveSides::Right => Self::Right,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameDto {
    id: String,
    x_um: u64,
    y_um: u64,
    width_um: u64,
    height_um: u64,
    /// Absent while the Frame follows the Album style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    style: Option<FrameStyleDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    photo: Option<PhotoDto>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameStyleDto {
    border_rgb: String,
    border_width_um: u64,
    opacity_percent: u8,
}

impl FrameDto {
    fn from_domain(frame: &ProjectFrame) -> Self {
        let rect = frame.rect();
        Self {
            id: id_text(frame.id()),
            x_um: rect.x(),
            y_um: rect.y(),
            width_um: rect.width(),
            height_um: rect.height(),
            style: match frame.style() {
                FrameStyle::Album => None,
                FrameStyle::Custom {
                    border,
                    opacity_percent,
                } => Some(FrameStyleDto {
                    border_rgb: border.rgb.canonical_hex(),
                    border_width_um: border.width_um,
                    opacity_percent: *opacity_percent,
                }),
            },
            photo: frame.photo().map(PhotoDto::from_domain),
        }
    }

    fn into_domain(self) -> Result<ProjectFrame, DecodeFailure> {
        let style = match self.style {
            None => FrameStyle::Album,
            Some(style) => {
                if style.opacity_percent > 100 || style.border_width_um > MAX_SAFE_INTEGER {
                    return Err(invalid_document());
                }
                FrameStyle::Custom {
                    border: FrameBorderValues {
                        rgb: parse_rgb(&style.border_rgb)?,
                        width_um: style.border_width_um,
                    },
                    opacity_percent: style.opacity_percent,
                }
            }
        };
        let photo = self.photo.map(PhotoDto::into_domain).transpose()?;
        Ok(ProjectFrame::new(
            parse_uuid_v4(&self.id)?,
            ProjectRect::new(self.x_um, self.y_um, self.width_um, self.height_um),
            photo,
        )
        .with_style(style))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoDto {
    media_id: String,
    #[serde(default, skip_serializing_if = "TransformDto::is_neutral")]
    transform: TransformDto,
}

/// Each adjustment is omitted while it keeps its neutral value.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransformDto {
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pan_x: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pan_y: f32,
    #[serde(default = "neutral_zoom", skip_serializing_if = "is_neutral_zoom")]
    user_zoom: f32,
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    quarter_turns: i8,
    #[serde(default, skip_serializing_if = "is_false")]
    mirror_x: bool,
    #[serde(default, skip_serializing_if = "is_zero_i16")]
    angle_tenths: i16,
    #[serde(default, skip_serializing_if = "is_false")]
    black_and_white: bool,
}

impl Default for TransformDto {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            user_zoom: neutral_zoom(),
            quarter_turns: 0,
            mirror_x: false,
            angle_tenths: 0,
            black_and_white: false,
        }
    }
}

fn neutral_zoom() -> f32 {
    1.0
}

fn is_zero_f32(value: &f32) -> bool {
    *value == 0.0
}

fn is_neutral_zoom(value: &f32) -> bool {
    *value == neutral_zoom()
}

fn is_zero_i8(value: &i8) -> bool {
    *value == 0
}

fn is_zero_i16(value: &i16) -> bool {
    *value == 0
}

impl TransformDto {
    fn is_neutral(&self) -> bool {
        is_zero_f32(&self.pan_x)
            && is_zero_f32(&self.pan_y)
            && is_neutral_zoom(&self.user_zoom)
            && self.quarter_turns == 0
            && !self.mirror_x
            && self.angle_tenths == 0
            && !self.black_and_white
    }
}

impl PhotoDto {
    fn from_domain(photo: &ProjectPhoto) -> Self {
        let transform = photo.transform();
        Self {
            media_id: id_text(photo.media_id()),
            transform: TransformDto {
                pan_x: transform.pan_x(),
                pan_y: transform.pan_y(),
                user_zoom: transform.user_zoom(),
                quarter_turns: transform.quarter_turns(),
                mirror_x: transform.mirror_x(),
                angle_tenths: transform.angle_tenths(),
                black_and_white: transform.black_and_white(),
            },
        }
    }

    fn into_domain(self) -> Result<ProjectPhoto, DecodeFailure> {
        let t = self.transform;
        let transform = ProjectPhotoTransform::new(t.pan_x, t.pan_y, t.user_zoom)
            .and_then(|transform| transform.with_orientation(t.quarter_turns, t.mirror_x))
            .and_then(|transform| transform.with_fine_rotation(t.angle_tenths))
            .map_err(|_| invalid_document())?
            .with_black_and_white(t.black_and_white);
        Ok(ProjectPhoto::new(parse_uuid_v4(&self.media_id)?, transform))
    }
}

// ---------------------------------------------------------------------------
// Layouts

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredLayoutDto {
    origin: LayoutOriginDto,
    scope: LayoutScopeDto,
    surface: LayoutSurfaceDto,
    positions: Vec<LayoutRectDto>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutSurfaceDto {
    kind: LayoutSurfaceKindDto,
    width_um: i64,
    height_um: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutRectDto {
    x_um: i64,
    y_um: i64,
    width_um: i64,
    height_um: i64,
}

impl StoredLayoutDto {
    fn from_domain(layout: &StoredLayout) -> Self {
        let definition = &layout.definition;
        Self {
            origin: layout.origin.into(),
            scope: definition.scope.into(),
            surface: LayoutSurfaceDto {
                kind: definition.surface.kind.into(),
                width_um: definition.surface.width_um,
                height_um: definition.surface.height_um,
            },
            positions: definition
                .positions
                .iter()
                .map(|rect| LayoutRectDto {
                    x_um: rect.x,
                    y_um: rect.y,
                    width_um: rect.width,
                    height_um: rect.height,
                })
                .collect(),
        }
    }

    fn into_domain(self) -> Result<StoredLayout, DecodeFailure> {
        // Layout positions share the Frame range: never negative.
        if self
            .positions
            .iter()
            .any(|rect| rect.x_um < 0 || rect.y_um < 0 || rect.width_um < 0 || rect.height_um < 0)
        {
            return Err(invalid_document());
        }
        Ok(StoredLayout {
            origin: self.origin.into(),
            definition: LayoutDefinition {
                surface: LayoutSurface {
                    kind: self.surface.kind.into(),
                    width_um: self.surface.width_um,
                    height_um: self.surface.height_um,
                },
                scope: self.scope.into(),
                positions: self
                    .positions
                    .into_iter()
                    .map(|rect| RectUm {
                        x: rect.x_um,
                        y: rect.y_um,
                        width: rect.width_um,
                        height: rect.height_um,
                    })
                    .collect(),
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FavoriteLayoutDto {
    id: String,
    order: u64,
    layout: StoredLayoutDto,
}

impl FavoriteLayoutDto {
    fn from_domain(favorite: &FavoriteLayout) -> Self {
        Self {
            id: favorite.id.to_string(),
            order: favorite.order,
            layout: StoredLayoutDto::from_domain(&favorite.layout),
        }
    }

    fn into_domain(self) -> Result<FavoriteLayout, DecodeFailure> {
        Ok(FavoriteLayout {
            id: LayoutFavoriteId::from_uuid(parse_uuid_v4(&self.id)?)
                .ok_or_else(invalid_document)?,
            order: self.order,
            layout: self.layout.into_domain()?,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum LayoutOriginDto {
    Automatic,
    Custom,
}

impl From<LayoutOrigin> for LayoutOriginDto {
    fn from(value: LayoutOrigin) -> Self {
        match value {
            LayoutOrigin::Automatic => Self::Automatic,
            LayoutOrigin::Custom => Self::Custom,
        }
    }
}

impl From<LayoutOriginDto> for LayoutOrigin {
    fn from(value: LayoutOriginDto) -> Self {
        match value {
            LayoutOriginDto::Automatic => Self::Automatic,
            LayoutOriginDto::Custom => Self::Custom,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum LayoutScopeDto {
    Page,
    Sheet,
}

impl From<LayoutScope> for LayoutScopeDto {
    fn from(value: LayoutScope) -> Self {
        match value {
            LayoutScope::Page => Self::Page,
            LayoutScope::Sheet => Self::Sheet,
        }
    }
}

impl From<LayoutScopeDto> for LayoutScope {
    fn from(value: LayoutScopeDto) -> Self {
        match value {
            LayoutScopeDto::Page => Self::Page,
            LayoutScopeDto::Sheet => Self::Sheet,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum LayoutSurfaceKindDto {
    SinglePage,
    DoubleSheet,
}

impl From<LayoutSurfaceKind> for LayoutSurfaceKindDto {
    fn from(value: LayoutSurfaceKind) -> Self {
        match value {
            LayoutSurfaceKind::SinglePage => Self::SinglePage,
            LayoutSurfaceKind::DoubleSheet => Self::DoubleSheet,
        }
    }
}

impl From<LayoutSurfaceKindDto> for LayoutSurfaceKind {
    fn from(value: LayoutSurfaceKindDto) -> Self {
        match value {
            LayoutSurfaceKindDto::SinglePage => Self::SinglePage,
            LayoutSurfaceKindDto::DoubleSheet => Self::DoubleSheet,
        }
    }
}

// ---------------------------------------------------------------------------
// Header

#[derive(Default)]
struct DocumentHeader {
    document_type: Option<String>,
    schema_version: Option<u32>,
}

impl<'de> Deserialize<'de> for DocumentHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(DocumentHeaderVisitor)
    }
}

struct DocumentHeaderVisitor;

impl<'de> Visitor<'de> for DocumentHeaderVisitor {
    type Value = DocumentHeader;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object containing a MyAlbuns document header")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut header = DocumentHeader::default();
        let mut saw_document_type = false;
        let mut saw_schema_version = false;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "documentType" => {
                    if saw_document_type {
                        return Err(serde::de::Error::duplicate_field("documentType"));
                    }
                    saw_document_type = true;
                    header.document_type = match map.next_value::<Value>()? {
                        Value::String(value) => Some(value),
                        _ => None,
                    };
                }
                "schemaVersion" => {
                    if saw_schema_version {
                        return Err(serde::de::Error::duplicate_field("schemaVersion"));
                    }
                    saw_schema_version = true;
                    header.schema_version = match map.next_value::<Value>()? {
                        Value::Number(value) => {
                            value.as_u64().and_then(|value| value.try_into().ok())
                        }
                        _ => None,
                    };
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(header)
    }
}
