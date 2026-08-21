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
use crate::MediaKind;
use crate::project_document::{
    ActiveSides, Background, BackgroundContent, DisplayUnit, DocumentSettings, FrameBorder,
    MAX_SAFE_INTEGER, MediaRef, Overlay, OverlayContent, ProjectDocument, ProjectFrame,
    ProjectPhoto, ProjectPhotoTransform, ProjectRect, ProjectRevision, ProjectSheet, Rgb,
    VisualDefaults, frame_border_width_is_valid, validate_project_state,
};

const DOCUMENT_TYPE: &str = "myalbuns.project";
const SCHEMA_VERSION_V1: u32 = 1;
pub(super) const SCHEMA_VERSION_V2: u32 = 2;
pub(super) const SCHEMA_VERSION_V3: u32 = 3;
const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

pub(super) struct DecodedProjectRevision {
    pub(super) revision: ProjectRevision,
    pub(super) source_schema_version: u32,
}

pub(super) fn decode(bytes: &[u8]) -> Result<DecodedProjectRevision, DecodeFailure> {
    if bytes.starts_with(UTF8_BOM) {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    let schema_version = classify_header(bytes)?;
    let document = match schema_version {
        SCHEMA_VERSION_V1 => {
            let document: ProjectDocumentV1 = serde_json::from_slice(bytes)
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
            migrate_v2_to_v3(migrate_v1_to_v2(document)?)?
        }
        SCHEMA_VERSION_V2 => {
            let document = serde_json::from_slice::<ProjectDocumentV2>(bytes)
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
            migrate_v2_to_v3(document)?
        }
        SCHEMA_VERSION_V3 => serde_json::from_slice::<ProjectDocumentV3>(bytes)
            .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?,
        _ => unreachable!("classify_header accepts only supported public schemas"),
    };
    let revision = map_document(document)?;
    Ok(DecodedProjectRevision {
        revision,
        source_schema_version: schema_version,
    })
}

pub(super) fn encode(revision: &ProjectRevision) -> Result<Vec<u8>, DecodeFailure> {
    validate_project_state(&revision.project)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
    if revision.revision > MAX_SAFE_INTEGER {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    let dto = ProjectDocumentV3::from_domain(revision)?;
    let mut bytes = serde_json::to_vec_pretty(&dto)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn rewrite_project_id(
    bytes: &[u8],
    project_id: Uuid,
) -> Result<(Vec<u8>, ProjectRevision), DecodeFailure> {
    let source = decode(bytes)?;
    let project_id = project_id.hyphenated().to_string();
    let mut rewritten = match source.source_schema_version {
        SCHEMA_VERSION_V1 => {
            let mut document: ProjectDocumentV1 = serde_json::from_slice(bytes)
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
            document.project_id = project_id;
            serde_json::to_vec_pretty(&document)
        }
        SCHEMA_VERSION_V2 => {
            let mut document: ProjectDocumentV2 = serde_json::from_slice(bytes)
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
            document.project_id = project_id;
            serde_json::to_vec_pretty(&document)
        }
        SCHEMA_VERSION_V3 => {
            let mut document: ProjectDocumentV3 = serde_json::from_slice(bytes)
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
            document.project_id = project_id;
            serde_json::to_vec_pretty(&document)
        }
        _ => unreachable!("decode accepts only supported public schemas"),
    }
    .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
    rewritten.push(b'\n');
    let candidate = decode(&rewritten)?.revision;
    Ok((rewritten, candidate))
}

fn classify_header(bytes: &[u8]) -> Result<u32, DecodeFailure> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let header = DocumentHeader::deserialize(&mut deserializer)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
    deserializer
        .end()
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;

    if header.document_type.as_deref() != Some(DOCUMENT_TYPE) {
        return Err(document_failure(DocumentFailure::InvalidDocumentType));
    }
    let Some(version) = header.schema_version else {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    };
    match version {
        SCHEMA_VERSION_V1 | SCHEMA_VERSION_V2 | SCHEMA_VERSION_V3 => Ok(version),
        0 => Err(document_failure(DocumentFailure::UnsupportedLegacySchema {
            version,
        })),
        version => Err(document_failure(DocumentFailure::UnsupportedFutureSchema {
            version,
        })),
    }
}

fn map_document(document: ProjectDocumentV3) -> Result<ProjectRevision, DecodeFailure> {
    if document.document_type != DOCUMENT_TYPE || document.schema_version != SCHEMA_VERSION_V3 {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    if document.revision > MAX_SAFE_INTEGER {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }

    let project_id = parse_uuid_v4(&document.project_id)?;
    let settings = map_settings(document.project.document)?;
    let visual_defaults = map_visual_defaults(document.project.visual_defaults)?;
    let media = document
        .project
        .media
        .into_iter()
        .map(map_media)
        .collect::<Result<Vec<_>, _>>()?;
    let sheets = document
        .project
        .sheets
        .into_iter()
        .map(map_sheet_v3)
        .collect::<Result<Vec<_>, _>>()?;
    let project = ProjectDocument::new(settings, visual_defaults, media, sheets);
    validate_project_state(&project)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectState))?;
    Ok(ProjectRevision::new(project_id, document.revision, project))
}

fn map_settings(settings: DocumentSettingsV1) -> Result<DocumentSettings, DecodeFailure> {
    if settings.sheet_width_um == 0
        || settings.sheet_width_um > MAX_SAFE_INTEGER
        || settings.sheet_height_um == 0
        || settings.sheet_height_um > MAX_SAFE_INTEGER
        || !(1..=1_200).contains(&settings.dpi)
        || settings.bleed_um > MAX_SAFE_INTEGER
        || settings.safety_um > MAX_SAFE_INTEGER
    {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    Ok(DocumentSettings::new(
        settings.display_unit.into(),
        settings.sheet_width_um,
        settings.sheet_height_um,
        settings.dpi,
        settings.bleed_um,
        settings.safety_um,
    ))
}

fn map_visual_defaults(defaults: VisualDefaultsV1) -> Result<VisualDefaults, DecodeFailure> {
    let background = match defaults.background {
        BackgroundV1::BothSides { both } => Background::BothSides {
            both: map_background_content(both)?,
        },
        BackgroundV1::PerSide { left, right } => Background::PerSide {
            left: map_background_content(left)?,
            right: map_background_content(right)?,
        },
    };
    let overlay = match defaults.overlay {
        OverlayV1::BothSides { both } => Overlay::BothSides {
            both: both.map(map_overlay_content).transpose()?,
        },
        OverlayV1::PerSide { left, right } => Overlay::PerSide {
            left: left.map(map_overlay_content).transpose()?,
            right: right.map(map_overlay_content).transpose()?,
        },
    };
    let frame_border = match defaults.frame_border {
        FrameBorderV1::None => FrameBorder::None,
        FrameBorderV1::Solid { rgb, width_um } => {
            if !frame_border_width_is_valid(width_um) {
                return Err(document_failure(DocumentFailure::InvalidProjectDocument));
            }
            FrameBorder::Solid {
                rgb: parse_rgb(&rgb)?,
                width_um,
            }
        }
    };
    Ok(VisualDefaults::new(background, overlay, frame_border))
}

fn map_background_content(
    content: BackgroundContentV1,
) -> Result<BackgroundContent, DecodeFailure> {
    match content {
        BackgroundContentV1::Color { rgb } => Ok(BackgroundContent::Color {
            rgb: parse_rgb(&rgb)?,
        }),
        BackgroundContentV1::Media { media_id } => Ok(BackgroundContent::Media {
            media_id: parse_uuid_v4(&media_id)?,
        }),
    }
}

fn map_overlay_content(content: OverlayContentV1) -> Result<OverlayContent, DecodeFailure> {
    match content {
        OverlayContentV1::Media { media_id } => Ok(OverlayContent::Media {
            media_id: parse_uuid_v4(&media_id)?,
        }),
    }
}

fn map_media(media: MediaRefV2) -> Result<MediaRef, DecodeFailure> {
    let path = decode_native_path(media.path)?;
    validate_external_path(&path).map_err(|_| DecodeFailure::Path(PathFailure::InvalidPath))?;
    let kind = match media.kind {
        MediaKindV2::Decorative => MediaKind::Decorative,
        MediaKindV2::Photo => MediaKind::Photo,
    };
    Ok(MediaRef::new(parse_uuid_v4(&media.id)?, kind, path))
}

fn map_sheet_v3(sheet: SheetV3) -> Result<ProjectSheet, DecodeFailure> {
    let frames = sheet
        .frames
        .into_iter()
        .map(map_frame_v3)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProjectSheet::with_frames(
        parse_uuid_v4(&sheet.id)?,
        sheet.active_sides.into(),
        frames,
    ))
}

fn map_frame_v3(frame: FrameV3) -> Result<ProjectFrame, DecodeFailure> {
    let photo = frame
        .photo
        .map(|photo| {
            Ok(ProjectPhoto::new(
                parse_uuid_v4(&photo.media_id)?,
                ProjectPhotoTransform::new(
                    photo.transform.pan_x,
                    photo.transform.pan_y,
                    photo.transform.user_zoom,
                )
                .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?,
            ))
        })
        .transpose()?;
    Ok(ProjectFrame::new(
        parse_uuid_v4(&frame.id)?,
        ProjectRect::new(
            frame.rect.x,
            frame.rect.y,
            frame.rect.width,
            frame.rect.height,
        ),
        photo,
    ))
}

fn parse_uuid_v4(source: &str) -> Result<Uuid, DecodeFailure> {
    let parsed = Uuid::parse_str(source)
        .map_err(|_| document_failure(DocumentFailure::InvalidProjectDocument))?;
    if parsed.get_version() != Some(Version::Random) || parsed.hyphenated().to_string() != source {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    Ok(parsed)
}

fn parse_rgb(source: &str) -> Result<Rgb, DecodeFailure> {
    Rgb::parse_canonical(source)
        .ok_or_else(|| document_failure(DocumentFailure::InvalidProjectDocument))
}

fn document_failure(failure: DocumentFailure) -> DecodeFailure {
    DecodeFailure::Document(failure)
}

#[cfg(windows)]
fn decode_native_path(path: NativePathV1) -> Result<PathBuf, DecodeFailure> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    match path {
        NativePathV1::WindowsUtf16 { units } => Ok(PathBuf::from(OsString::from_wide(&units))),
    }
}

#[cfg(not(windows))]
fn decode_native_path(_path: NativePathV1) -> Result<PathBuf, DecodeFailure> {
    Err(DecodeFailure::Path(PathFailure::InvalidPath))
}

#[cfg(windows)]
fn encode_native_path(path: &Path) -> NativePathV1 {
    use std::os::windows::ffi::OsStrExt;

    NativePathV1::WindowsUtf16 {
        units: path.as_os_str().encode_wide().collect(),
    }
}

#[cfg(not(windows))]
fn encode_native_path(_path: &Path) -> NativePathV1 {
    NativePathV1::WindowsUtf16 { units: Vec::new() }
}

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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDocumentV1 {
    document_type: String,
    schema_version: u32,
    project_id: String,
    revision: u64,
    project: ProjectPayloadV1,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDocumentV2 {
    document_type: String,
    schema_version: u32,
    project_id: String,
    revision: u64,
    project: ProjectPayloadV2,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDocumentV3 {
    document_type: String,
    schema_version: u32,
    project_id: String,
    revision: u64,
    project: ProjectPayloadV3,
}

impl ProjectDocumentV3 {
    fn from_domain(revision: &ProjectRevision) -> Result<Self, DecodeFailure> {
        Ok(Self {
            document_type: DOCUMENT_TYPE.into(),
            schema_version: SCHEMA_VERSION_V3,
            project_id: revision.project_id.hyphenated().to_string(),
            revision: revision.revision,
            project: ProjectPayloadV3::from_domain(&revision.project)?,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectPayloadV1 {
    document: DocumentSettingsV1,
    visual_defaults: VisualDefaultsV1,
    media: Vec<DecorativeMediaV1>,
    sheets: Vec<SheetV1>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectPayloadV2 {
    document: DocumentSettingsV1,
    visual_defaults: VisualDefaultsV1,
    media: Vec<MediaRefV2>,
    sheets: Vec<SheetV1>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectPayloadV3 {
    document: DocumentSettingsV1,
    visual_defaults: VisualDefaultsV1,
    media: Vec<MediaRefV2>,
    sheets: Vec<SheetV3>,
}

impl ProjectPayloadV3 {
    fn from_domain(project: &ProjectDocument) -> Result<Self, DecodeFailure> {
        Ok(Self {
            document: DocumentSettingsV1::from_domain(project.document()),
            visual_defaults: VisualDefaultsV1::from_domain(project.visual_defaults()),
            media: project
                .media()
                .iter()
                .map(MediaRefV2::from_domain)
                .collect(),
            sheets: project.sheets().iter().map(SheetV3::from_domain).collect(),
        })
    }
}

fn migrate_v1_to_v2(document: ProjectDocumentV1) -> Result<ProjectDocumentV2, DecodeFailure> {
    if document.document_type != DOCUMENT_TYPE || document.schema_version != SCHEMA_VERSION_V1 {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    let ProjectPayloadV1 {
        document: settings,
        visual_defaults,
        media,
        sheets,
    } = document.project;
    Ok(ProjectDocumentV2 {
        document_type: document.document_type,
        schema_version: SCHEMA_VERSION_V2,
        project_id: document.project_id,
        revision: document.revision,
        project: ProjectPayloadV2 {
            document: settings,
            visual_defaults,
            media: media.into_iter().map(MediaRefV2::from_v1).collect(),
            sheets,
        },
    })
}

fn migrate_v2_to_v3(document: ProjectDocumentV2) -> Result<ProjectDocumentV3, DecodeFailure> {
    if document.document_type != DOCUMENT_TYPE || document.schema_version != SCHEMA_VERSION_V2 {
        return Err(document_failure(DocumentFailure::InvalidProjectDocument));
    }
    let ProjectPayloadV2 {
        document: settings,
        visual_defaults,
        media,
        sheets,
    } = document.project;
    Ok(ProjectDocumentV3 {
        document_type: document.document_type,
        schema_version: SCHEMA_VERSION_V3,
        project_id: document.project_id,
        revision: document.revision,
        project: ProjectPayloadV3 {
            document: settings,
            visual_defaults,
            media,
            sheets: sheets.into_iter().map(SheetV3::from_v2).collect(),
        },
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentSettingsV1 {
    display_unit: DisplayUnitV1,
    sheet_width_um: u64,
    sheet_height_um: u64,
    dpi: u32,
    bleed_um: u64,
    safety_um: u64,
}

impl DocumentSettingsV1 {
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
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum DisplayUnitV1 {
    Mm,
    Cm,
    In,
}

impl From<DisplayUnitV1> for DisplayUnit {
    fn from(value: DisplayUnitV1) -> Self {
        match value {
            DisplayUnitV1::Mm => Self::Mm,
            DisplayUnitV1::Cm => Self::Cm,
            DisplayUnitV1::In => Self::In,
        }
    }
}

impl From<DisplayUnit> for DisplayUnitV1 {
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
struct VisualDefaultsV1 {
    background: BackgroundV1,
    overlay: OverlayV1,
    frame_border: FrameBorderV1,
}

impl VisualDefaultsV1 {
    fn from_domain(defaults: &VisualDefaults) -> Self {
        Self {
            background: BackgroundV1::from_domain(defaults.background()),
            overlay: OverlayV1::from_domain(defaults.overlay()),
            frame_border: FrameBorderV1::from_domain(defaults.frame_border()),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "scope", deny_unknown_fields)]
enum BackgroundV1 {
    #[serde(rename = "bothSides")]
    BothSides { both: BackgroundContentV1 },
    #[serde(rename = "perSide")]
    PerSide {
        left: BackgroundContentV1,
        right: BackgroundContentV1,
    },
}

impl BackgroundV1 {
    fn from_domain(background: &Background) -> Self {
        match background {
            Background::BothSides { both } => Self::BothSides {
                both: BackgroundContentV1::from_domain(both),
            },
            Background::PerSide { left, right } => Self::PerSide {
                left: BackgroundContentV1::from_domain(left),
                right: BackgroundContentV1::from_domain(right),
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum BackgroundContentV1 {
    Color {
        rgb: String,
    },
    Media {
        #[serde(rename = "mediaId")]
        media_id: String,
    },
}

impl BackgroundContentV1 {
    fn from_domain(content: &BackgroundContent) -> Self {
        match content {
            BackgroundContent::Color { rgb } => Self::Color {
                rgb: format_rgb(*rgb),
            },
            BackgroundContent::Media { media_id } => Self::Media {
                media_id: media_id.hyphenated().to_string(),
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "scope", deny_unknown_fields)]
enum OverlayV1 {
    #[serde(rename = "bothSides")]
    BothSides { both: Option<OverlayContentV1> },
    #[serde(rename = "perSide")]
    PerSide {
        left: Option<OverlayContentV1>,
        right: Option<OverlayContentV1>,
    },
}

impl OverlayV1 {
    fn from_domain(overlay: &Overlay) -> Self {
        match overlay {
            Overlay::BothSides { both } => Self::BothSides {
                both: both.as_ref().map(OverlayContentV1::from_domain),
            },
            Overlay::PerSide { left, right } => Self::PerSide {
                left: left.as_ref().map(OverlayContentV1::from_domain),
                right: right.as_ref().map(OverlayContentV1::from_domain),
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum OverlayContentV1 {
    Media {
        #[serde(rename = "mediaId")]
        media_id: String,
    },
}

impl OverlayContentV1 {
    fn from_domain(content: &OverlayContent) -> Self {
        match content {
            OverlayContent::Media { media_id } => Self::Media {
                media_id: media_id.hyphenated().to_string(),
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum FrameBorderV1 {
    None,
    Solid {
        rgb: String,
        #[serde(rename = "widthUm")]
        width_um: u64,
    },
}

impl FrameBorderV1 {
    fn from_domain(border: &FrameBorder) -> Self {
        match border {
            FrameBorder::None => Self::None,
            FrameBorder::Solid { rgb, width_um } => Self::Solid {
                rgb: format_rgb(*rgb),
                width_um: *width_um,
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecorativeMediaV1 {
    id: String,
    kind: MediaKindV1,
    path: NativePathV1,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaRefV2 {
    id: String,
    kind: MediaKindV2,
    path: NativePathV1,
}

impl MediaRefV2 {
    fn from_v1(media: DecorativeMediaV1) -> Self {
        let MediaKindV1::Decorative = media.kind;
        Self {
            id: media.id,
            kind: MediaKindV2::Decorative,
            path: media.path,
        }
    }

    fn from_domain(media: &MediaRef) -> Self {
        Self {
            id: media.id().hyphenated().to_string(),
            kind: match media.kind() {
                MediaKind::Photo => MediaKindV2::Photo,
                MediaKind::Decorative => MediaKindV2::Decorative,
            },
            path: encode_native_path(media.path()),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum MediaKindV1 {
    Decorative,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum MediaKindV2 {
    Photo,
    Decorative,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "encoding", deny_unknown_fields)]
enum NativePathV1 {
    #[serde(rename = "windowsUtf16")]
    WindowsUtf16 { units: Vec<u16> },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SheetV1 {
    id: String,
    active_sides: ActiveSidesV1,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SheetV3 {
    id: String,
    active_sides: ActiveSidesV1,
    frames: Vec<FrameV3>,
}

impl SheetV3 {
    fn from_v2(sheet: SheetV1) -> Self {
        Self {
            id: sheet.id,
            active_sides: sheet.active_sides,
            frames: Vec::new(),
        }
    }

    fn from_domain(sheet: &ProjectSheet) -> Self {
        Self {
            id: sheet.id().hyphenated().to_string(),
            active_sides: sheet.active_sides().into(),
            frames: sheet.frames().iter().map(FrameV3::from_domain).collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameV3 {
    id: String,
    rect: RectV3,
    photo: Option<PhotoV3>,
}

impl FrameV3 {
    fn from_domain(frame: &ProjectFrame) -> Self {
        Self {
            id: frame.id().hyphenated().to_string(),
            rect: RectV3::from_domain(frame.rect()),
            photo: frame.photo().map(PhotoV3::from_domain),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RectV3 {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

impl RectV3 {
    fn from_domain(rect: ProjectRect) -> Self {
        Self {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoV3 {
    media_id: String,
    transform: PhotoTransformV3,
}

impl PhotoV3 {
    fn from_domain(photo: &ProjectPhoto) -> Self {
        Self {
            media_id: photo.media_id().hyphenated().to_string(),
            transform: PhotoTransformV3::from_domain(photo.transform()),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoTransformV3 {
    pan_x: f32,
    pan_y: f32,
    user_zoom: f32,
}

impl PhotoTransformV3 {
    fn from_domain(transform: ProjectPhotoTransform) -> Self {
        Self {
            pan_x: transform.pan_x(),
            pan_y: transform.pan_y(),
            user_zoom: transform.user_zoom(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ActiveSidesV1 {
    Both,
    Left,
    Right,
}

impl From<ActiveSidesV1> for ActiveSides {
    fn from(value: ActiveSidesV1) -> Self {
        match value {
            ActiveSidesV1::Both => Self::Both,
            ActiveSidesV1::Left => Self::Left,
            ActiveSidesV1::Right => Self::Right,
        }
    }
}

impl From<ActiveSides> for ActiveSidesV1 {
    fn from(value: ActiveSides) -> Self {
        match value {
            ActiveSides::Both => Self::Both,
            ActiveSides::Left => Self::Left,
            ActiveSides::Right => Self::Right,
        }
    }
}

fn format_rgb(rgb: Rgb) -> String {
    rgb.canonical_hex()
}
