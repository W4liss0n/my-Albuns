use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};
use uuid::{Uuid, Version};

const MICROMETERS_PER_INCH: u64 = 25_400;
const Q32_ONE: i128 = 1_i128 << 32;
const Q32_HALF: i128 = 1_i128 << 31;
const Q16_ONE: i128 = 1_i128 << 16;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CorpusV1 {
    schema_version: u32,
    contract: String,
    algorithms: AlgorithmsV1,
    assets: Vec<AssetV1>,
    cases: Vec<GoldenCaseV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AlgorithmsV1 {
    fixed_point: String,
    sampler: String,
    alpha_composite: String,
    raster_edge: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetV1 {
    id: String,
    relative_path: String,
    encoding: AssetEncodingV1,
    decoded_byte_length: usize,
    sha256: String,
    descriptor: AssetDescriptorV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum AssetEncodingV1 {
    Hex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum AssetDescriptorV1 {
    Jpeg(JpegDescriptorV1),
    Png(PngDescriptorV1),
    Tiff(TiffDescriptorV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JpegDescriptorV1 {
    encoded_width_px: u32,
    encoded_height_px: u32,
    exif_orientation: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PngDescriptorV1 {
    encoded_width_px: u32,
    encoded_height_px: u32,
    has_alpha: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TiffDescriptorV1 {
    width_px: u32,
    height_px: u32,
    bits_per_sample: Vec<u16>,
    samples_per_pixel: u16,
    photometric: u16,
    extra_samples: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum GoldenCaseV1 {
    RasterGeometry(RasterGeometryCaseV1),
    Composition(CompositionCaseV1),
    CanonicalRaster(CanonicalRasterCaseV1),
    ImagingEnvelope(ImagingEnvelopeCaseV1),
    SourceNormalization(SourceNormalizationCaseV1),
    FormatAdapters(Box<FormatAdaptersCaseV1>),
    OutputNames(OutputNamesCaseV1),
    Operational(OperationalCaseV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RasterGeometryCaseV1 {
    id: String,
    input: RasterGeometryInputV1,
    expected: RasterGeometryExpectedV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RasterGeometryInputV1 {
    sheet_width_um: u64,
    sheet_height_um: u64,
    page_width_um: u64,
    dpi: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RasterGeometryExpectedV1 {
    sheet_width_px: u32,
    sheet_height_px: u32,
    center_edge_px: u32,
    left_interval_px: [u32; 2],
    right_interval_px: [u32; 2],
    independent_page_width_px: u32,
    independent_page_height_px: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompositionCaseV1 {
    id: String,
    adapter_registrations: Vec<ContractAdapterV1>,
    input: CompositionInputV1,
    expected_plan: ExpectedCompositionPlanV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ContractAdapterV1 {
    CompositionCore,
    Canvas,
    ExportPipeline,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompositionInputV1 {
    creative_state: CreativeStateV1,
    source_geometry_facts: Vec<SourceGeometryFactV1>,
    source_observations: Vec<SourceObservationV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeStateV1 {
    revision: u64,
    dpi: u32,
    media_refs: Vec<MediaRefV1>,
    sheets: Vec<CreativeSheetV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaRefV1 {
    media_id: String,
    native_path: NativePathV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeSheetV1 {
    sheet_id: String,
    number: u32,
    active_sides: ActiveSidesV1,
    width_um: u64,
    height_um: u64,
    background: CreativeBackgroundV1,
    frames: Vec<CreativeFrameV1>,
    overlay: CreativeOverlayV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ActiveSidesV1 {
    Both,
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
enum CreativeBackgroundV1 {
    Both {
        both: CreativePaintV1,
    },
    PerSide {
        left: CreativePaintV1,
        right: CreativePaintV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
enum CreativeOverlayV1 {
    Both {
        both: Option<ImagePaintV1>,
    },
    PerSide {
        left: Option<ImagePaintV1>,
        right: Option<ImagePaintV1>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ScopeV1 {
    Left,
    Right,
    Both,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum CreativePaintV1 {
    Solid(SolidPaintV1),
    Image(ImagePaintV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SolidPaintV1 {
    rgb: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagePaintV1 {
    media_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeFrameV1 {
    frame_id: String,
    z_index: u32,
    rect_um: [u64; 4],
    photo: PhotoUseV1,
    style: FrameStyleV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoUseV1 {
    media_id: String,
    transform: PhotoTransformV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhotoTransformV1 {
    quarter_turns_ccw: i8,
    fine_angle_tenths: i16,
    mirror_horizontal: bool,
    user_zoom_millionths: u32,
    pan_x_millionths: i32,
    pan_y_millionths: i32,
    black_and_white: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameStyleV1 {
    border_width_um: u64,
    border_rgb: String,
    opacity_percent: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceGeometryFactV1 {
    media_id: String,
    native_path: NativePathV1,
    oriented_width_px: u32,
    oriented_height_px: u32,
    applied_orientation: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceObservationV1 {
    media_id: String,
    native_path: NativePathV1,
    detected_format: DetectedFormatV1,
    source_variant: SourceVariantV1,
    encoded_width_px: u32,
    encoded_height_px: u32,
    oriented_width_px: u32,
    oriented_height_px: u32,
    embedded_orientation: u8,
    applied_orientation: u8,
    color_profile: ColorProfileObservationV1,
    sha256_full_file_v1: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum DetectedFormatV1 {
    Jpeg,
    Png,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum SourceVariantV1 {
    JpegBaselineRgb8,
    PngStaticRgba8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ColorProfileObservationV1 {
    AbsentAssumeSrgb,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativePathV1 {
    windows_utf16: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedCompositionPlanV1 {
    plan_version: u32,
    revision: u64,
    dpi: u32,
    referenced_media_ids: Vec<String>,
    sheets: Vec<ExpectedSheetPlanV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedSheetPlanV1 {
    sheet_id: String,
    surface_rect_um: [u64; 4],
    ordered_layers: Vec<PlannedLayerV1>,
    output_units: Vec<ExpectedOutputUnitV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum PlannedLayerV1 {
    Base(PlannedBaseV1),
    Background(PlannedDecorativeV1),
    FrameGroup(Box<PlannedFrameGroupV1>),
    Overlay(PlannedDecorativeV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedBaseV1 {
    layer_id: String,
    rect_um: [u64; 4],
    rgb: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedDecorativeV1 {
    layer_id: String,
    scope: ScopeV1,
    rect_um: [u64; 4],
    paint: PlannedPaintV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum PlannedPaintV1 {
    Solid(SolidPaintV1),
    Image(PlannedImagePaintV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedImagePaintV1 {
    media_id: String,
    oriented_width_px: u32,
    oriented_height_px: u32,
    applied_orientation: u8,
    source_from_physical_q32: AffineQ32V1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedFrameGroupV1 {
    layer_id: String,
    frame_id: String,
    z_index: u32,
    frame_rect_um: [u64; 4],
    photo_clip_rect_um: [u64; 4],
    border_fill_rects_um: Vec<[u64; 4]>,
    border_rgb: String,
    group_opacity_byte: u8,
    photo: PlannedPhotoV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlannedPhotoV1 {
    media_id: String,
    oriented_width_px: u32,
    oriented_height_px: u32,
    applied_orientation: u8,
    transform: PhotoTransformV1,
    sampler: String,
    physical_from_source_q32: AffineQ32V1,
    source_from_physical_q32: AffineQ32V1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AffineQ32V1 {
    xx: String,
    xy: String,
    tx: String,
    yx: String,
    yy: String,
    ty: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedOutputUnitV1 {
    unit_id: String,
    mode: ExportModeV1,
    logical_index: u32,
    physical_source_rect_um: [u64; 4],
    normalized_origin_um: [u64; 2],
    width_px: u32,
    height_px: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ExportModeV1 {
    PerSheet,
    PerPage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalRasterCaseV1 {
    id: String,
    origin: RasterOriginV1,
    input: CanonicalRasterInputV1,
    expected_raster: CanonicalRasterV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum RasterOriginV1 {
    Standalone,
    CompositionFrame(CompositionFrameLinkV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompositionFrameLinkV1 {
    composition_case_id: String,
    unit_id: String,
    frame_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalRasterInputV1 {
    unit: ComposedOutputUnitV1,
    normalized_sources: Vec<NormalizedSourceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComposedOutputUnitV1 {
    unit_id: String,
    width_px: u32,
    height_px: u32,
    dpi: u32,
    physical_source_rect_um: [u64; 4],
    layers: Vec<ProjectedLayerV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingEnvelopeCaseV1 {
    id: String,
    composition_case_id: String,
    canonical_raster_case_id: String,
    expected_envelope: ImagingRenderEnvelopeV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingRenderEnvelopeV1 {
    protocol_version: u32,
    correlation: ImagingCorrelationV1,
    attempt: ImagingAttemptV1,
    revision: u64,
    dpi: u32,
    format_options: ImagingFormatOptionsV1,
    preparation: ImagingPreparationV1,
    units: Vec<ComposedOutputUnitV1>,
    sources: Vec<ImagingSourceV1>,
    root_binding_plan: RootBindingPlanV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingCorrelationV1 {
    request_id: String,
    project_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingAttemptV1 {
    #[serde(deserialize_with = "deserialize_canonical_uuid_v4")]
    attempt_id: String,
    cancellation_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "format", rename_all = "kebab-case", deny_unknown_fields)]
enum ImagingFormatOptionsV1 {
    Jpeg {
        #[serde(deserialize_with = "deserialize_jpeg_quality")]
        quality: u8,
    },
    Png {},
    Pdf {},
}

fn deserialize_canonical_uuid_v4<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let parsed = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
    if parsed.get_version() != Some(Version::Random) || parsed.hyphenated().to_string() != value {
        return Err(serde::de::Error::custom(
            "attemptId must be a canonical lowercase hyphenated UUID v4",
        ));
    }
    Ok(value)
}

fn deserialize_jpeg_quality<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let quality = u8::deserialize(deserializer)?;
    if !(1..=100).contains(&quality) {
        return Err(serde::de::Error::custom(
            "JPEG quality must be in the inclusive range 1..=100",
        ));
    }
    Ok(quality)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingPreparationV1 {
    destination_directory: NativePathV1,
    attempt_directory: NativePathV1,
    output_path: NativePathV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImagingSourceV1 {
    media_ref: MediaRefV1,
    source_observation: SourceObservationV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootBindingPlanV1 {
    bindings: Vec<RootBindingV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootBindingV1 {
    kind: PathRootKindV1,
    logical_root: NativePathV1,
    operational_root: NativePathV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
enum PathRootKindV1 {
    Disk,
    Unc,
    VerbatimDisk,
    VerbatimUnc,
    Posix,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum ProjectedLayerV1 {
    Base(ProjectedBaseV1),
    Solid(ProjectedSolidV1),
    Image(ProjectedImageV1),
    FrameGroup(ProjectedFrameGroupV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectedBaseV1 {
    rgb: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectedSolidV1 {
    layer_id: String,
    clip_rect_px: [u32; 4],
    rgb: String,
    opacity_byte: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectedImageV1 {
    layer_id: String,
    source_id: String,
    clip_rect_px: [u32; 4],
    source_from_destination_q32: AffineQ32V1,
    opacity_byte: u8,
    black_and_white: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectedFrameGroupV1 {
    layer_id: String,
    source_id: String,
    frame_rect_px: [u32; 4],
    photo_clip_rect_px: [u32; 4],
    source_from_destination_q32: AffineQ32V1,
    border_fill_rects_px: Vec<[u32; 4]>,
    border_rgb: String,
    group_opacity_byte: u8,
    black_and_white: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NormalizedSourceV1 {
    source_id: String,
    width_px: u32,
    height_px: u32,
    rgba_rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalRasterV1 {
    width_px: u32,
    height_px: u32,
    dpi: u32,
    color_space: String,
    rgba_rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceNormalizationCaseV1 {
    id: String,
    asset_id: String,
    verification_owner_issue: u32,
    expected: NormalizedSourceExpectationV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum NormalizedSourceExpectationV1 {
    JpegExif(JpegNormalizationV1),
    PngAlpha(PngNormalizationV1),
    TiffAlpha(TiffNormalizationV1),
    TiffRejected(TiffRejectionV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JpegNormalizationV1 {
    encoded_width_px: u32,
    encoded_height_px: u32,
    embedded_orientation: u8,
    applied_orientation: u8,
    oriented_width_px: u32,
    oriented_height_px: u32,
    apply_exactly_once: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PngNormalizationV1 {
    width_px: u32,
    height_px: u32,
    rgba_rows: Vec<Vec<String>>,
    linked_composition_case_id: String,
    linked_raster_case_id: String,
    linked_media_id: String,
    embedded_orientation: u8,
    applied_orientation: u8,
    orientation_metadata_ignored: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TiffNormalizationV1 {
    width_px: u32,
    height_px: u32,
    alpha_semantics: TiffAlphaSemanticsV1,
    rgba_rows: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum TiffAlphaSemanticsV1 {
    AssociatedAlpha,
    UnassociatedAlpha,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TiffRejectionV1 {
    error_code: NormalizationErrorV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum NormalizationErrorV1 {
    UnsupportedSourceVariant,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormatAdaptersCaseV1 {
    id: String,
    canonical_unit_ids: Vec<String>,
    expected: FormatExpectationsV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormatExpectationsV1 {
    jpeg: JpegExpectationV1,
    png: PngExpectationV1,
    pdf: PdfExpectationV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JpegExpectationV1 {
    extension: String,
    quality_input: u8,
    process: String,
    subsampling: String,
    components: u8,
    dpi: u32,
    icc: String,
    forbidden_metadata: Vec<String>,
    decoded_error_metric: String,
    decoded_max_channel_error: u8,
    decoded_mean_error_at_most: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PngExpectationV1 {
    extension: String,
    color_type: String,
    interlaced: bool,
    pixels_per_meter: u32,
    icc: String,
    forbidden_chunks: Vec<String>,
    decoded_comparison: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PdfExpectationV1 {
    extension: String,
    page_count: u32,
    page_order: Vec<String>,
    media_boxes_points: Vec<[String; 2]>,
    crop_boxes_points: Vec<[String; 2]>,
    embedded_raster_order: Vec<String>,
    placement: String,
    embedded_color_space: String,
    embedded_raster_comparison: String,
    lossless: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OutputNamesCaseV1 {
    id: String,
    project_name: String,
    extension: String,
    indices: Vec<u32>,
    expected_names: Vec<String>,
    rejected_index_strings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationalCaseV1 {
    id: String,
    owner_issue: u32,
    context: OperationalContextV1,
    injection: FaultInjectionV1,
    expected: OperationalExpectedV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationalContextV1 {
    total_file_count: u32,
    integral_export: bool,
    orphan_removal_confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum FaultInjectionV1 {
    SourceDigestChanges(SourceDigestChangesV1),
    SourceIdentityIndeterminate(SourceIdentityIndeterminateV1),
    AtomicProbeUnsupported(AtomicProbeUnsupportedV1),
    PromotionFails(PromotionFailsV1),
    None(NoFaultV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceDigestChangesV1 {
    media_id: String,
    before_sha256: String,
    after_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceIdentityIndeterminateV1 {
    media_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AtomicProbeUnsupportedV1 {
    operation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PromotionFailsV1 {
    failed_output: String,
    attempt_number: u32,
    target_evidence: TargetEvidenceV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct NoFaultV1 {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationalExpectedV1 {
    terminal: OperationalTerminalV1,
    publication_state: PublicationStateV1,
    attempted_count: u32,
    confirmed_promoted_count: u32,
    failed_output: FailedOutputV1,
    cause_code: CauseCodeV1,
    failed_target_evidence: TargetEvidenceV1,
    preparation_cleanup: PreparationCleanupV1,
    orphan_cleanup_runs: bool,
    full_export_recommended: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum FailedOutputV1 {
    NotApplicable,
    Output(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum CauseCodeV1 {
    NotApplicable,
    SourceChanged,
    SourceIdentityIndeterminate,
    AtomicReplacementUnsupported,
    AtomicPromotionFailed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum OperationalTerminalV1 {
    SourceChanged,
    SourceIdentityIndeterminate,
    AtomicReplacementUnsupported,
    PublicationFailed,
    PartialPublication,
    Completed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum PublicationStateV1 {
    NotStarted,
    UntouchedByAttempt,
    PossiblyMixed,
    AllCandidatesConfirmed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum TargetEvidenceV1 {
    NotApplicable,
    UntouchedByAttempt,
    CandidateAtFinal,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum PreparationCleanupV1 {
    RequiredWhenSafe,
}

fn corpus() -> CorpusV1 {
    let path = workspace_root().join("tests/fixtures/final-renderer-cases-v1.json");
    serde_json::from_slice(&fs::read(path).expect("the renderer corpus is readable"))
        .expect("the renderer corpus matches its closed v1 schema")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

fn case_id(case: &GoldenCaseV1) -> &str {
    match case {
        GoldenCaseV1::RasterGeometry(case) => &case.id,
        GoldenCaseV1::Composition(case) => &case.id,
        GoldenCaseV1::CanonicalRaster(case) => &case.id,
        GoldenCaseV1::ImagingEnvelope(case) => &case.id,
        GoldenCaseV1::SourceNormalization(case) => &case.id,
        GoldenCaseV1::FormatAdapters(case) => &case.id,
        GoldenCaseV1::OutputNames(case) => &case.id,
        GoldenCaseV1::Operational(case) => &case.id,
    }
}

fn composition_case<'a>(corpus: &'a CorpusV1, case_id: &str) -> &'a CompositionCaseV1 {
    corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::Composition(case) if case.id == case_id => Some(case),
            _ => None,
        })
        .expect("the named composition case exists")
}

fn registered_composition_case_ids(
    corpus: &CorpusV1,
    adapter: ContractAdapterV1,
) -> BTreeSet<&str> {
    corpus
        .cases
        .iter()
        .filter_map(|case| match case {
            GoldenCaseV1::Composition(case) if case.adapter_registrations.contains(&adapter) => {
                Some(case.id.as_str())
            }
            _ => None,
        })
        .collect()
}

fn raster_edge(micrometers: u64, dpi: u32) -> u32 {
    let numerator = u128::from(micrometers)
        .checked_mul(u128::from(dpi))
        .and_then(|value| value.checked_add(u128::from(MICROMETERS_PER_INCH / 2)))
        .expect("the golden geometry fits the checked raster formula");
    u32::try_from(numerator / u128::from(MICROMETERS_PER_INCH))
        .expect("the golden raster edge fits u32")
}

fn opacity_byte(percent: u8) -> u8 {
    assert!(percent <= 100, "opacity is a percentage");
    ((u16::from(percent) * 255 + 50) / 100) as u8
}

fn round_signed(numerator: i128, denominator: i128) -> i128 {
    assert!(denominator > 0);
    if numerator >= 0 {
        numerator
            .checked_add(denominator / 2)
            .expect("golden Q32.32 rounding does not overflow")
            / denominator
    } else {
        -round_signed(
            numerator
                .checked_neg()
                .expect("golden Q32.32 numerator is representable"),
            denominator,
        )
    }
}

fn q_int(value: i128) -> i128 {
    value
        .checked_mul(Q32_ONE)
        .expect("golden Q32.32 integer conversion does not overflow")
}

fn q_ratio(numerator: i128, denominator: i128) -> i128 {
    round_signed(
        numerator
            .checked_mul(Q32_ONE)
            .expect("golden Q32.32 ratio does not overflow"),
        denominator,
    )
}

fn q_mul(left: i128, right: i128) -> i128 {
    round_signed(
        left.checked_mul(right)
            .expect("golden Q32.32 multiplication does not overflow"),
        Q32_ONE,
    )
}

fn q_div(numerator: i128, denominator: i128) -> i128 {
    assert!(denominator > 0);
    round_signed(
        numerator
            .checked_mul(Q32_ONE)
            .expect("golden Q32.32 division does not overflow"),
        denominator,
    )
}

fn linear_q(a: i128, x: i128, b: i128, y: i128) -> i128 {
    let numerator = a
        .checked_mul(x)
        .and_then(|left| b.checked_mul(y).and_then(|right| left.checked_add(right)))
        .expect("golden Q32.32 linear combination does not overflow");
    round_signed(numerator, Q32_ONE)
}

fn q32_decimal(value: i128) -> String {
    i64::try_from(value)
        .expect("golden Q32.32 value fits the contract's i64")
        .to_string()
}

fn golden_trig_q32(angle_tenths: i32) -> (i128, i128) {
    match angle_tenths.rem_euclid(3_600) {
        0 => (Q32_ONE, 0),
        1_050 => (-1_111_619_334, 4_148_619_834),
        unsupported => panic!("the v1 corpus has no trig oracle for {unsupported} tenths"),
    }
}

fn planned_affines(
    frame_rect: [u64; 4],
    border_width_um: u64,
    transform: &PhotoTransformV1,
    source_width: u32,
    source_height: u32,
) -> (AffineQ32V1, AffineQ32V1) {
    let inner = inset_rect(frame_rect, border_width_um);
    let width = q_int(i128::from(inner[2]));
    let height = q_int(i128::from(inner[3]));
    let source_width = q_int(i128::from(source_width));
    let source_height = q_int(i128::from(source_height));
    let angle_tenths =
        i32::from(transform.quarter_turns_ccw) * 900 + i32::from(transform.fine_angle_tenths);
    let (cosine, sine) = golden_trig_q32(angle_tenths);
    let mirror = if transform.mirror_horizontal {
        -Q32_ONE
    } else {
        Q32_ONE
    };

    let required_x = linear_q(cosine.abs(), width, sine.abs(), height);
    let required_y = linear_q(sine.abs(), width, cosine.abs(), height);
    let base_scale = q_div(required_x, source_width).max(q_div(required_y, source_height));
    let scale = q_mul(
        base_scale,
        q_ratio(i128::from(transform.user_zoom_millionths), 1_000_000),
    );
    assert!(scale > 0);

    let overflow_x = q_div(
        q_mul(source_width, scale)
            .checked_sub(required_x)
            .expect("golden horizontal overflow is representable")
            .max(0),
        q_int(2),
    );
    let overflow_y = q_div(
        q_mul(source_height, scale)
            .checked_sub(required_y)
            .expect("golden vertical overflow is representable")
            .max(0),
        q_int(2),
    );
    let pan_x = q_mul(
        q_ratio(i128::from(transform.pan_x_millionths), 1_000_000),
        overflow_x,
    );
    let pan_y = q_mul(
        q_ratio(i128::from(transform.pan_y_millionths), 1_000_000),
        overflow_y,
    );
    let center_x = q_int(i128::from(inner[0])) + q_div(width, q_int(2));
    let center_y = q_int(i128::from(inner[1])) + q_div(height, q_int(2));
    let photo_center_x = center_x + linear_q(cosine, pan_x, sine, pan_y);
    let photo_center_y = center_y + linear_q(-sine, pan_x, cosine, pan_y);

    let direct_xx = q_mul(q_mul(mirror, cosine), scale);
    let direct_xy = q_mul(q_mul(mirror, sine), scale);
    let direct_yx = q_mul(-sine, scale);
    let direct_yy = q_mul(cosine, scale);
    let source_center_x = q_div(source_width, q_int(2));
    let source_center_y = q_div(source_height, q_int(2));
    let direct_tx =
        photo_center_x - linear_q(direct_xx, source_center_x, direct_xy, source_center_y);
    let direct_ty =
        photo_center_y - linear_q(direct_yx, source_center_x, direct_yy, source_center_y);

    let inverse_xx = q_div(q_mul(mirror, cosine), scale);
    let inverse_xy = q_div(-sine, scale);
    let inverse_yx = q_div(q_mul(mirror, sine), scale);
    let inverse_yy = q_div(cosine, scale);
    let inverse_tx =
        source_center_x - linear_q(inverse_xx, photo_center_x, inverse_xy, photo_center_y);
    let inverse_ty =
        source_center_y - linear_q(inverse_yx, photo_center_x, inverse_yy, photo_center_y);

    (
        AffineQ32V1 {
            xx: q32_decimal(direct_xx),
            xy: q32_decimal(direct_xy),
            tx: q32_decimal(direct_tx),
            yx: q32_decimal(direct_yx),
            yy: q32_decimal(direct_yy),
            ty: q32_decimal(direct_ty),
        },
        AffineQ32V1 {
            xx: q32_decimal(inverse_xx),
            xy: q32_decimal(inverse_xy),
            tx: q32_decimal(inverse_tx),
            yx: q32_decimal(inverse_yx),
            yy: q32_decimal(inverse_yy),
            ty: q32_decimal(inverse_ty),
        },
    )
}

fn project_source_from_destination(
    source_from_physical: &AffineQ32V1,
    physical_origin_um: [u64; 2],
    dpi: u32,
) -> AffineQ32V1 {
    let inverse_xx = i128::from(parse_q32(&source_from_physical.xx));
    let inverse_xy = i128::from(parse_q32(&source_from_physical.xy));
    let inverse_tx = i128::from(parse_q32(&source_from_physical.tx));
    let inverse_yx = i128::from(parse_q32(&source_from_physical.yx));
    let inverse_yy = i128::from(parse_q32(&source_from_physical.yy));
    let inverse_ty = i128::from(parse_q32(&source_from_physical.ty));
    let step = q_ratio(i128::from(MICROMETERS_PER_INCH), i128::from(dpi));
    let first_x = q_int(i128::from(physical_origin_um[0]))
        + q_ratio(i128::from(MICROMETERS_PER_INCH / 2), i128::from(dpi));
    let first_y = q_int(i128::from(physical_origin_um[1]))
        + q_ratio(i128::from(MICROMETERS_PER_INCH / 2), i128::from(dpi));

    AffineQ32V1 {
        xx: q32_decimal(q_mul(inverse_xx, step)),
        xy: q32_decimal(q_mul(inverse_xy, step)),
        tx: q32_decimal(inverse_tx + linear_q(inverse_xx, first_x, inverse_xy, first_y)),
        yx: q32_decimal(q_mul(inverse_yx, step)),
        yy: q32_decimal(q_mul(inverse_yy, step)),
        ty: q32_decimal(inverse_ty + linear_q(inverse_yx, first_x, inverse_yy, first_y)),
    }
}

fn decorative_source_from_physical(
    rect_um: [u64; 4],
    source_width_px: u32,
    source_height_px: u32,
) -> AffineQ32V1 {
    assert!(rect_um[2] > 0 && rect_um[3] > 0);
    assert!(source_width_px > 0 && source_height_px > 0);
    let xx = q_ratio(i128::from(source_width_px), i128::from(rect_um[2]));
    let yy = q_ratio(i128::from(source_height_px), i128::from(rect_um[3]));
    AffineQ32V1 {
        xx: q32_decimal(xx),
        xy: "0".to_owned(),
        tx: q32_decimal(-q_mul(xx, q_int(i128::from(rect_um[0])))),
        yx: "0".to_owned(),
        yy: q32_decimal(yy),
        ty: q32_decimal(-q_mul(yy, q_int(i128::from(rect_um[1])))),
    }
}

fn project_rect_to_unit(rect_um: [u64; 4], unit_rect_um: [u64; 4], dpi: u32) -> [u32; 4] {
    let rect_right = rect_um[0] + rect_um[2];
    let rect_bottom = rect_um[1] + rect_um[3];
    let unit_right = unit_rect_um[0] + unit_rect_um[2];
    let unit_bottom = unit_rect_um[1] + unit_rect_um[3];
    let left = rect_um[0].max(unit_rect_um[0]);
    let top = rect_um[1].max(unit_rect_um[1]);
    let right = rect_right.min(unit_right).max(left);
    let bottom = rect_bottom.min(unit_bottom).max(top);
    let left_px = raster_edge(left - unit_rect_um[0], dpi);
    let top_px = raster_edge(top - unit_rect_um[1], dpi);
    let right_px = raster_edge(right - unit_rect_um[0], dpi);
    let bottom_px = raster_edge(bottom - unit_rect_um[1], dpi);
    [left_px, top_px, right_px - left_px, bottom_px - top_px]
}

fn inset_rect(rect: [u64; 4], inset: u64) -> [u64; 4] {
    assert!(rect[2] > 0 && rect[3] > 0);
    let inset_x = inset.min(rect[2] / 2);
    let inset_y = inset.min(rect[3] / 2);
    [
        rect[0] + inset_x,
        rect[1] + inset_y,
        rect[2] - inset_x * 2,
        rect[3] - inset_y * 2,
    ]
}

fn border_fill_rects(rect: [u64; 4], border: u64) -> Vec<[u64; 4]> {
    if border == 0 {
        return Vec::new();
    }
    let inset_x = border.min(rect[2] / 2);
    let inset_y = border.min(rect[3] / 2);
    let inner_height = rect[3] - inset_y * 2;
    [
        [rect[0], rect[1], rect[2], inset_y],
        [rect[0], rect[1] + rect[3] - inset_y, rect[2], inset_y],
        [rect[0], rect[1] + inset_y, inset_x, inner_height],
        [
            rect[0] + rect[2] - inset_x,
            rect[1] + inset_y,
            inset_x,
            inner_height,
        ],
    ]
    .into_iter()
    .filter(|candidate| candidate[2] > 0 && candidate[3] > 0)
    .collect()
}

fn reference_plan(input: &CompositionInputV1, sampler: &str) -> ExpectedCompositionPlanV1 {
    assert!(input.creative_state.dpi > 0);
    let media_refs = input
        .creative_state
        .media_refs
        .iter()
        .map(|media_ref| (media_ref.media_id.as_str(), media_ref))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(media_refs.len(), input.creative_state.media_refs.len());
    let facts = input
        .source_geometry_facts
        .iter()
        .map(|fact| (fact.media_id.as_str(), fact))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(facts.len(), input.source_geometry_facts.len());
    let observations = input
        .source_observations
        .iter()
        .map(|observation| (observation.media_id.as_str(), observation))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(observations.len(), input.source_observations.len());
    assert_eq!(
        media_refs.keys().copied().collect::<BTreeSet<_>>(),
        facts.keys().copied().collect::<BTreeSet<_>>(),
        "every MediaRef has exactly one geometry fact"
    );
    assert_eq!(
        media_refs.keys().copied().collect::<BTreeSet<_>>(),
        observations.keys().copied().collect::<BTreeSet<_>>(),
        "every MediaRef has exactly one frozen source observation"
    );
    for (media_id, media_ref) in &media_refs {
        assert!(!media_ref.native_path.windows_utf16.is_empty());
        assert!(!media_ref.native_path.windows_utf16.contains(&0));
        assert_eq!(
            media_ref.native_path, facts[*media_id].native_path,
            "geometry facts belong to the same mediaId + NativePathDto"
        );
        let observation = observations[*media_id];
        let fact = facts[*media_id];
        assert_eq!(media_ref.native_path, observation.native_path);
        assert_eq!(fact.native_path, observation.native_path);
        assert_eq!(fact.oriented_width_px, observation.oriented_width_px);
        assert_eq!(fact.oriented_height_px, observation.oriented_height_px);
        assert_eq!(fact.applied_orientation, observation.applied_orientation);
        assert!((1..=8).contains(&observation.embedded_orientation));
        assert!((1..=8).contains(&observation.applied_orientation));
        assert!(observation.encoded_width_px > 0 && observation.encoded_height_px > 0);
        assert_eq!(
            observation.color_profile,
            ColorProfileObservationV1::AbsentAssumeSrgb
        );
        validate_sha256(&observation.sha256_full_file_v1);
        assert!(matches!(
            (observation.detected_format, observation.source_variant),
            (DetectedFormatV1::Jpeg, SourceVariantV1::JpegBaselineRgb8)
                | (DetectedFormatV1::Png, SourceVariantV1::PngStaticRgba8)
        ));
        match observation.detected_format {
            DetectedFormatV1::Jpeg => assert_eq!(
                observation.applied_orientation, observation.embedded_orientation,
                "JPEG applies its frozen EXIF Orientation exactly once"
            ),
            DetectedFormatV1::Png => assert_eq!(
                observation.applied_orientation, 1,
                "PNG eXIf is frozen but never projected as an image rotation"
            ),
        }
    }

    let mut referenced_media_ids = BTreeSet::new();
    let mut next_page_index = 1_u32;
    let sheets = input
        .creative_state
        .sheets
        .iter()
        .map(|sheet| {
            assert!(sheet.number > 0 && sheet.width_um > 0 && sheet.height_um > 0);
            assert_eq!(sheet.width_um % 2, 0, "the golden spread has equal pages");
            let page_width = sheet.width_um / 2;
            let local_page_rect = [0, 0, page_width, sheet.height_um];
            let surface = match sheet.active_sides {
                ActiveSidesV1::Both => [0, 0, sheet.width_um, sheet.height_um],
                ActiveSidesV1::Left | ActiveSidesV1::Right => local_page_rect,
            };
            let left_rect = local_page_rect;
            let right_rect = match sheet.active_sides {
                ActiveSidesV1::Both => [page_width, 0, page_width, sheet.height_um],
                ActiveSidesV1::Left | ActiveSidesV1::Right => local_page_rect,
            };
            let mut ordered_layers = vec![PlannedLayerV1::Base(PlannedBaseV1 {
                layer_id: "base".to_owned(),
                rect_um: surface,
                rgb: "#FFFFFF".to_owned(),
            })];

            match &sheet.background {
                CreativeBackgroundV1::Both { both } => {
                    ordered_layers.push(PlannedLayerV1::Background(plan_decorative(
                        "background:both",
                        ScopeV1::Both,
                        surface,
                        both,
                        &facts,
                        &mut referenced_media_ids,
                    )));
                }
                CreativeBackgroundV1::PerSide { left, right } => {
                    if matches!(
                        sheet.active_sides,
                        ActiveSidesV1::Both | ActiveSidesV1::Left
                    ) {
                        ordered_layers.push(PlannedLayerV1::Background(plan_decorative(
                            "background:left",
                            ScopeV1::Left,
                            left_rect,
                            left,
                            &facts,
                            &mut referenced_media_ids,
                        )));
                    }
                    if matches!(
                        sheet.active_sides,
                        ActiveSidesV1::Both | ActiveSidesV1::Right
                    ) {
                        ordered_layers.push(PlannedLayerV1::Background(plan_decorative(
                            "background:right",
                            ScopeV1::Right,
                            right_rect,
                            right,
                            &facts,
                            &mut referenced_media_ids,
                        )));
                    }
                }
            }

            let mut frames = sheet.frames.iter().collect::<Vec<_>>();
            frames.sort_by_key(|frame| (frame.z_index, frame.frame_id.as_str()));
            for frame in frames {
                validate_rect_inside(frame.rect_um, surface);
                validate_rgb(&frame.style.border_rgb);
                let fact = facts
                    .get(frame.photo.media_id.as_str())
                    .copied()
                    .expect("every Photo has immutable source geometry facts");
                referenced_media_ids.insert(frame.photo.media_id.clone());
                let clip = inset_rect(frame.rect_um, frame.style.border_width_um);
                let (physical_from_source_q32, source_from_physical_q32) = planned_affines(
                    frame.rect_um,
                    frame.style.border_width_um,
                    &frame.photo.transform,
                    fact.oriented_width_px,
                    fact.oriented_height_px,
                );
                ordered_layers.push(PlannedLayerV1::FrameGroup(Box::new(PlannedFrameGroupV1 {
                    layer_id: format!("frame:{}", frame.frame_id),
                    frame_id: frame.frame_id.clone(),
                    z_index: frame.z_index,
                    frame_rect_um: frame.rect_um,
                    photo_clip_rect_um: clip,
                    border_fill_rects_um: border_fill_rects(
                        frame.rect_um,
                        frame.style.border_width_um,
                    ),
                    border_rgb: frame.style.border_rgb.clone(),
                    group_opacity_byte: opacity_byte(frame.style.opacity_percent),
                    photo: PlannedPhotoV1 {
                        media_id: frame.photo.media_id.clone(),
                        oriented_width_px: fact.oriented_width_px,
                        oriented_height_px: fact.oriented_height_px,
                        applied_orientation: fact.applied_orientation,
                        transform: frame.photo.transform.clone(),
                        sampler: sampler.to_owned(),
                        physical_from_source_q32,
                        source_from_physical_q32,
                    },
                })));
            }

            let mut push_overlay =
                |layer_id: &str, scope: ScopeV1, rect_um: [u64; 4], image: &ImagePaintV1| {
                    let paint = CreativePaintV1::Image(image.clone());
                    ordered_layers.push(PlannedLayerV1::Overlay(plan_decorative(
                        layer_id,
                        scope,
                        rect_um,
                        &paint,
                        &facts,
                        &mut referenced_media_ids,
                    )));
                };
            match &sheet.overlay {
                CreativeOverlayV1::Both { both } => {
                    if let Some(image) = both {
                        push_overlay("overlay:both", ScopeV1::Both, surface, image);
                    }
                }
                CreativeOverlayV1::PerSide { left, right } => {
                    if matches!(
                        sheet.active_sides,
                        ActiveSidesV1::Both | ActiveSidesV1::Left
                    ) && let Some(image) = left
                    {
                        push_overlay("overlay:left", ScopeV1::Left, left_rect, image);
                    }
                    if matches!(
                        sheet.active_sides,
                        ActiveSidesV1::Both | ActiveSidesV1::Right
                    ) && let Some(image) = right
                    {
                        push_overlay("overlay:right", ScopeV1::Right, right_rect, image);
                    }
                }
            }

            let dpi = input.creative_state.dpi;
            let mut output_units = vec![ExpectedOutputUnitV1 {
                unit_id: format!("{}:spread", sheet.sheet_id),
                mode: ExportModeV1::PerSheet,
                logical_index: sheet.number,
                physical_source_rect_um: surface,
                normalized_origin_um: [0, 0],
                width_px: raster_edge(surface[2], dpi),
                height_px: raster_edge(surface[3], dpi),
            }];
            if matches!(
                sheet.active_sides,
                ActiveSidesV1::Both | ActiveSidesV1::Left
            ) {
                output_units.push(ExpectedOutputUnitV1 {
                    unit_id: format!("{}:left", sheet.sheet_id),
                    mode: ExportModeV1::PerPage,
                    logical_index: next_page_index,
                    physical_source_rect_um: left_rect,
                    normalized_origin_um: [0, 0],
                    width_px: raster_edge(page_width, dpi),
                    height_px: raster_edge(sheet.height_um, dpi),
                });
                next_page_index += 1;
            }
            if matches!(
                sheet.active_sides,
                ActiveSidesV1::Both | ActiveSidesV1::Right
            ) {
                output_units.push(ExpectedOutputUnitV1 {
                    unit_id: format!("{}:right", sheet.sheet_id),
                    mode: ExportModeV1::PerPage,
                    logical_index: next_page_index,
                    physical_source_rect_um: right_rect,
                    normalized_origin_um: [0, 0],
                    width_px: raster_edge(page_width, dpi),
                    height_px: raster_edge(sheet.height_um, dpi),
                });
                next_page_index += 1;
            }

            ExpectedSheetPlanV1 {
                sheet_id: sheet.sheet_id.clone(),
                surface_rect_um: surface,
                ordered_layers,
                output_units,
            }
        })
        .collect();

    ExpectedCompositionPlanV1 {
        plan_version: 1,
        revision: input.creative_state.revision,
        dpi: input.creative_state.dpi,
        referenced_media_ids: referenced_media_ids.into_iter().collect(),
        sheets,
    }
}

fn plan_decorative(
    layer_id: &str,
    scope: ScopeV1,
    rect_um: [u64; 4],
    paint: &CreativePaintV1,
    facts: &BTreeMap<&str, &SourceGeometryFactV1>,
    referenced_media_ids: &mut BTreeSet<String>,
) -> PlannedDecorativeV1 {
    PlannedDecorativeV1 {
        layer_id: layer_id.to_owned(),
        scope,
        rect_um,
        paint: plan_paint(paint, rect_um, facts, referenced_media_ids),
    }
}

fn plan_paint(
    paint: &CreativePaintV1,
    rect_um: [u64; 4],
    facts: &BTreeMap<&str, &SourceGeometryFactV1>,
    referenced_media_ids: &mut BTreeSet<String>,
) -> PlannedPaintV1 {
    match paint {
        CreativePaintV1::Solid(solid) => {
            validate_rgb(&solid.rgb);
            PlannedPaintV1::Solid(solid.clone())
        }
        CreativePaintV1::Image(image) => {
            let fact = facts
                .get(image.media_id.as_str())
                .copied()
                .expect("every decorative image has immutable source geometry facts");
            referenced_media_ids.insert(image.media_id.clone());
            PlannedPaintV1::Image(PlannedImagePaintV1 {
                media_id: image.media_id.clone(),
                oriented_width_px: fact.oriented_width_px,
                oriented_height_px: fact.oriented_height_px,
                applied_orientation: fact.applied_orientation,
                source_from_physical_q32: decorative_source_from_physical(
                    rect_um,
                    fact.oriented_width_px,
                    fact.oriented_height_px,
                ),
            })
        }
    }
}

fn validate_rect_inside(rect: [u64; 4], surface: [u64; 4]) {
    assert!(rect[2] > 0 && rect[3] > 0);
    assert!(rect[0] >= surface[0] && rect[1] >= surface[1]);
    assert!(rect[0] + rect[2] <= surface[0] + surface[2]);
    assert!(rect[1] + rect[3] <= surface[1] + surface[3]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pixel {
    rgb: [u8; 3],
    alpha: u8,
}

impl Pixel {
    const TRANSPARENT: Self = Self {
        rgb: [0, 0, 0],
        alpha: 0,
    };
}

#[derive(Clone, Debug)]
struct SourcePixels {
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
}

fn validate_rgb(value: &str) {
    assert_eq!(value.len(), 7);
    assert!(value.starts_with('#'));
    assert!(
        value[1..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
    );
}

fn parse_rgb(value: &str) -> [u8; 3] {
    validate_rgb(value);
    [
        u8::from_str_radix(&value[1..3], 16).expect("red is hexadecimal"),
        u8::from_str_radix(&value[3..5], 16).expect("green is hexadecimal"),
        u8::from_str_radix(&value[5..7], 16).expect("blue is hexadecimal"),
    ]
}

fn parse_rgba(value: &str) -> Pixel {
    assert_eq!(value.len(), 8);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
    );
    Pixel {
        rgb: [
            u8::from_str_radix(&value[0..2], 16).expect("red is hexadecimal"),
            u8::from_str_radix(&value[2..4], 16).expect("green is hexadecimal"),
            u8::from_str_radix(&value[4..6], 16).expect("blue is hexadecimal"),
        ],
        alpha: u8::from_str_radix(&value[6..8], 16).expect("alpha is hexadecimal"),
    }
}

fn rgba_hex(pixel: Pixel) -> String {
    format!(
        "{:02X}{:02X}{:02X}{:02X}",
        pixel.rgb[0], pixel.rgb[1], pixel.rgb[2], pixel.alpha
    )
}

fn sources(input: &[NormalizedSourceV1]) -> BTreeMap<&str, SourcePixels> {
    let mut result = BTreeMap::new();
    for source in input {
        assert!(source.width_px > 0 && source.height_px > 0);
        assert_eq!(source.rgba_rows.len(), source.height_px as usize);
        let mut pixels = Vec::with_capacity((source.width_px * source.height_px) as usize);
        for row in &source.rgba_rows {
            assert_eq!(row.len(), source.width_px as usize);
            pixels.extend(row.iter().map(|value| parse_rgba(value)));
        }
        assert!(
            result
                .insert(
                    source.source_id.as_str(),
                    SourcePixels {
                        width: source.width_px,
                        height: source.height_px,
                        pixels,
                    },
                )
                .is_none()
        );
    }
    result
}

fn parse_q32(value: &str) -> i64 {
    let parsed = value.parse::<i64>().expect("Q32.32 is a signed decimal");
    assert_eq!(parsed.to_string(), value, "Q32.32 decimal is canonical");
    parsed
}

fn affine_coordinate(matrix: &AffineQ32V1, x: u32, y: u32) -> (i128, i128) {
    let x = i128::from(x);
    let y = i128::from(y);
    let source_x = i128::from(parse_q32(&matrix.xx)) * x
        + i128::from(parse_q32(&matrix.xy)) * y
        + i128::from(parse_q32(&matrix.tx));
    let source_y = i128::from(parse_q32(&matrix.yx)) * x
        + i128::from(parse_q32(&matrix.yy)) * y
        + i128::from(parse_q32(&matrix.ty));
    (source_x, source_y)
}

fn q16_axis(source_edge_q32: i128) -> (i64, i128) {
    let centered = source_edge_q32 - Q32_HALF;
    let mut index = centered.div_euclid(Q32_ONE) as i64;
    let fraction_q32 = centered.rem_euclid(Q32_ONE);
    let mut fraction_q16 = (fraction_q32 + (1_i128 << 15)) >> 16;
    if fraction_q16 == Q16_ONE {
        index += 1;
        fraction_q16 = 0;
    }
    (index, fraction_q16)
}

fn sample_bilinear(source: &SourcePixels, source_x: i128, source_y: i128) -> Pixel {
    if source_x < 0
        || source_y < 0
        || source_x >= i128::from(source.width) * Q32_ONE
        || source_y >= i128::from(source.height) * Q32_ONE
    {
        return Pixel::TRANSPARENT;
    }

    let (x0, fx) = q16_axis(source_x);
    let (y0, fy) = q16_axis(source_y);
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let weights = [
        (Q16_ONE - fx) * (Q16_ONE - fy),
        fx * (Q16_ONE - fy),
        (Q16_ONE - fx) * fy,
        fx * fy,
    ];
    assert_eq!(weights.iter().sum::<i128>(), Q32_ONE);
    let coordinates = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
    let mut sum_alpha = 0_u128;
    let mut sum_premultiplied = [0_u128; 3];
    for ((x, y), weight) in coordinates.into_iter().zip(weights) {
        let x = x.clamp(0, i64::from(source.width) - 1) as u32;
        let y = y.clamp(0, i64::from(source.height) - 1) as u32;
        let pixel = source.pixels[(y * source.width + x) as usize];
        let weight = weight as u128;
        let alpha = u128::from(pixel.alpha);
        sum_alpha += alpha * weight;
        for (channel, sum) in sum_premultiplied.iter_mut().enumerate() {
            *sum += u128::from(pixel.rgb[channel]) * alpha * weight;
        }
    }

    if sum_alpha == 0 {
        return Pixel::TRANSPARENT;
    }
    let alpha = ((sum_alpha + (1_u128 << 31)) / (1_u128 << 32)) as u8;
    if alpha == 0 {
        return Pixel::TRANSPARENT;
    }
    let rgb = std::array::from_fn(|channel| {
        ((sum_premultiplied[channel] + sum_alpha / 2) / sum_alpha) as u8
    });
    Pixel { rgb, alpha }
}

fn grayscale(pixel: Pixel) -> Pixel {
    if pixel.alpha == 0 {
        return Pixel::TRANSPARENT;
    }
    let luminance = (54 * u16::from(pixel.rgb[0])
        + 183 * u16::from(pixel.rgb[1])
        + 19 * u16::from(pixel.rgb[2])
        + 128)
        / 256;
    Pixel {
        rgb: [luminance as u8; 3],
        alpha: pixel.alpha,
    }
}

fn apply_opacity(mut pixel: Pixel, opacity: u8) -> Pixel {
    pixel.alpha = ((u16::from(pixel.alpha) * u16::from(opacity) + 127) / 255) as u8;
    if pixel.alpha == 0 {
        pixel.rgb = [0, 0, 0];
    }
    pixel
}

fn source_over(source: Pixel, destination: Pixel) -> Pixel {
    let source_alpha = u32::from(source.alpha);
    let destination_alpha = u32::from(destination.alpha);
    let alpha_denominator = source_alpha * 255 + destination_alpha * (255 - source_alpha);
    if alpha_denominator == 0 {
        return Pixel::TRANSPARENT;
    }
    let alpha = ((alpha_denominator + 127) / 255) as u8;
    let rgb = std::array::from_fn(|channel| {
        let numerator = u32::from(source.rgb[channel]) * source_alpha * 255
            + u32::from(destination.rgb[channel]) * destination_alpha * (255 - source_alpha);
        ((numerator + alpha_denominator / 2) / alpha_denominator) as u8
    });
    Pixel { rgb, alpha }
}

fn contains_pixel(rect: [u32; 4], x: u32, y: u32) -> bool {
    x >= rect[0] && y >= rect[1] && x < rect[0] + rect[2] && y < rect[1] + rect[3]
}

fn render_reference(input: &CanonicalRasterInputV1) -> CanonicalRasterV1 {
    let unit = &input.unit;
    assert!(unit.width_px > 0 && unit.height_px > 0 && unit.dpi > 0);
    assert_eq!(
        raster_edge(unit.physical_source_rect_um[2], unit.dpi),
        unit.width_px
    );
    assert_eq!(
        raster_edge(unit.physical_source_rect_um[3], unit.dpi),
        unit.height_px
    );
    let sources = sources(&input.normalized_sources);
    let mut output = vec![Pixel::TRANSPARENT; (unit.width_px * unit.height_px) as usize];
    let mut saw_base = false;
    let mut layer_ids = BTreeSet::new();

    for layer in &unit.layers {
        match layer {
            ProjectedLayerV1::Base(base) => {
                assert!(!saw_base, "a unit has exactly one base");
                assert!(output.iter().all(|pixel| *pixel == Pixel::TRANSPARENT));
                saw_base = true;
                let rgb = parse_rgb(&base.rgb);
                output.fill(Pixel { rgb, alpha: 255 });
            }
            ProjectedLayerV1::Solid(solid) => {
                assert!(saw_base);
                assert!(layer_ids.insert(solid.layer_id.as_str()));
                let source = Pixel {
                    rgb: parse_rgb(&solid.rgb),
                    alpha: solid.opacity_byte,
                };
                for y in 0..unit.height_px {
                    for x in 0..unit.width_px {
                        if contains_pixel(solid.clip_rect_px, x, y) {
                            let index = (y * unit.width_px + x) as usize;
                            output[index] = source_over(source, output[index]);
                        }
                    }
                }
            }
            ProjectedLayerV1::Image(image) => {
                assert!(saw_base);
                assert!(layer_ids.insert(image.layer_id.as_str()));
                let source = sources
                    .get(image.source_id.as_str())
                    .expect("every projected image resolves a normalized source");
                for y in 0..unit.height_px {
                    for x in 0..unit.width_px {
                        if !contains_pixel(image.clip_rect_px, x, y) {
                            continue;
                        }
                        let (source_x, source_y) =
                            affine_coordinate(&image.source_from_destination_q32, x, y);
                        let mut sample = sample_bilinear(source, source_x, source_y);
                        if image.black_and_white {
                            sample = grayscale(sample);
                        }
                        sample = apply_opacity(sample, image.opacity_byte);
                        let index = (y * unit.width_px + x) as usize;
                        output[index] = source_over(sample, output[index]);
                    }
                }
            }
            ProjectedLayerV1::FrameGroup(frame) => {
                assert!(saw_base);
                assert!(layer_ids.insert(frame.layer_id.as_str()));
                for border_rect in &frame.border_fill_rects_px {
                    assert!(
                        border_rect[0] >= frame.frame_rect_px[0]
                            && border_rect[1] >= frame.frame_rect_px[1]
                            && border_rect[0] + border_rect[2]
                                <= frame.frame_rect_px[0] + frame.frame_rect_px[2]
                            && border_rect[1] + border_rect[3]
                                <= frame.frame_rect_px[1] + frame.frame_rect_px[3]
                    );
                }
                let source = sources
                    .get(frame.source_id.as_str())
                    .expect("every Frame resolves a normalized source");
                let border = Pixel {
                    rgb: parse_rgb(&frame.border_rgb),
                    alpha: 255,
                };
                for y in 0..unit.height_px {
                    for x in 0..unit.width_px {
                        if !contains_pixel(frame.frame_rect_px, x, y) {
                            continue;
                        }
                        let mut group = Pixel::TRANSPARENT;
                        if contains_pixel(frame.photo_clip_rect_px, x, y) {
                            let (source_x, source_y) =
                                affine_coordinate(&frame.source_from_destination_q32, x, y);
                            group = sample_bilinear(source, source_x, source_y);
                            if frame.black_and_white {
                                group = grayscale(group);
                            }
                        }
                        if frame
                            .border_fill_rects_px
                            .iter()
                            .any(|rect| contains_pixel(*rect, x, y))
                        {
                            group = source_over(border, group);
                        }
                        group = apply_opacity(group, frame.group_opacity_byte);
                        let index = (y * unit.width_px + x) as usize;
                        output[index] = source_over(group, output[index]);
                    }
                }
            }
        }
    }
    assert!(saw_base);
    assert!(output.iter().all(|pixel| pixel.alpha == 255));
    let rows = output
        .chunks(unit.width_px as usize)
        .map(|row| row.iter().copied().map(rgba_hex).collect())
        .collect();
    CanonicalRasterV1 {
        width_px: unit.width_px,
        height_px: unit.height_px,
        dpi: unit.dpi,
        color_space: "srgb2014".to_owned(),
        rgba_rows: rows,
    }
}

fn decode_hex_asset(asset: &AssetV1) -> Vec<u8> {
    assert_eq!(asset.encoding, AssetEncodingV1::Hex);
    let relative = Path::new(&asset.relative_path);
    assert!(!relative.is_absolute());
    assert!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    );
    let root = workspace_root();
    let path = root.join(relative);
    let canonical = path.canonicalize().expect("the golden asset exists");
    assert!(canonical.starts_with(&root));
    let encoded = fs::read_to_string(canonical).expect("the hexadecimal asset is readable");
    let compact = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    assert!(
        compact
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
    );
    assert_eq!(compact.len() % 2, 0);
    let (hexadecimal_pairs, remainder) = compact.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    let bytes = hexadecimal_pairs
        .iter()
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII hex"), 16)
                .expect("valid hexadecimal byte")
        })
        .collect::<Vec<_>>();
    assert_eq!(bytes.len(), asset.decoded_byte_length);
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(digest, asset.sha256);
    bytes
}

fn png_exif_orientation(bytes: &[u8]) -> Option<u16> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..8)? != PNG_SIGNATURE {
        return None;
    }
    let mut offset = 8_usize;
    while offset.checked_add(12)? <= bytes.len() {
        let length = u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?) as usize;
        let kind = bytes.get(offset + 4..offset + 8)?;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(length)?;
        let data = bytes.get(data_start..data_end)?;
        if kind == b"eXIf" {
            let little_endian = match data.get(..2)? {
                b"II" => true,
                b"MM" => false,
                _ => return None,
            };
            let read_u16 = |slice: &[u8]| {
                let value: [u8; 2] = slice.try_into().ok()?;
                Some(if little_endian {
                    u16::from_le_bytes(value)
                } else {
                    u16::from_be_bytes(value)
                })
            };
            let read_u32 = |slice: &[u8]| {
                let value: [u8; 4] = slice.try_into().ok()?;
                Some(if little_endian {
                    u32::from_le_bytes(value)
                } else {
                    u32::from_be_bytes(value)
                })
            };
            if read_u16(data.get(2..4)?)? != 42 {
                return None;
            }
            let ifd = usize::try_from(read_u32(data.get(4..8)?)?).ok()?;
            let count = usize::from(read_u16(data.get(ifd..ifd + 2)?)?);
            for entry in 0..count {
                let start = ifd.checked_add(2 + entry * 12)?;
                let item = data.get(start..start + 12)?;
                if read_u16(&item[0..2])? == 0x0112
                    && read_u16(&item[2..4])? == 3
                    && read_u32(&item[4..8])? == 1
                {
                    return read_u16(&item[8..10]);
                }
            }
            return None;
        }
        offset = data_end.checked_add(4)?;
    }
    None
}

#[derive(Debug, Eq, PartialEq)]
struct TiffProbe {
    width_px: u32,
    height_px: u32,
    bits_per_sample: Vec<u16>,
    samples_per_pixel: u16,
    photometric: u16,
    extra_samples: Vec<u16>,
    samples: Vec<u16>,
}

fn tiff_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("the TIFF SHORT is in bounds"),
    )
}

fn tiff_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("the TIFF LONG is in bounds"),
    )
}

fn probe_tiff(bytes: &[u8]) -> TiffProbe {
    assert_eq!(bytes.get(..4), Some(b"II*\0".as_slice()));
    let ifd_offset = usize::try_from(tiff_u32(bytes, 4)).expect("IFD offset fits usize");
    let entry_count = usize::from(tiff_u16(bytes, ifd_offset));
    let mut fields = BTreeMap::<u16, (u16, u32, usize)>::new();
    for index in 0..entry_count {
        let entry = ifd_offset + 2 + index * 12;
        let tag = tiff_u16(bytes, entry);
        let field_type = tiff_u16(bytes, entry + 2);
        let count = tiff_u32(bytes, entry + 4);
        assert!(fields.insert(tag, (field_type, count, entry + 8)).is_none());
    }
    let values = |tag: u16| -> Vec<u32> {
        let (field_type, count, value_field) = fields[&tag];
        let item_size = match field_type {
            3 => 2_usize,
            4 => 4_usize,
            _ => panic!("unsupported golden TIFF field type"),
        };
        let count = usize::try_from(count).expect("golden TIFF count fits usize");
        let byte_length = item_size * count;
        let start = if byte_length <= 4 {
            value_field
        } else {
            usize::try_from(tiff_u32(bytes, value_field)).expect("TIFF value offset fits usize")
        };
        (0..count)
            .map(|index| match field_type {
                3 => u32::from(tiff_u16(bytes, start + index * item_size)),
                4 => tiff_u32(bytes, start + index * item_size),
                _ => unreachable!(),
            })
            .collect()
    };
    assert_eq!(values(259), [1], "the fixture is uncompressed");
    assert_eq!(values(274), [1], "the fixture is top-left oriented");
    assert_eq!(values(284), [1], "the fixture is chunky");
    let width_px = values(256)[0];
    let height_px = values(257)[0];
    let bits_per_sample = values(258)
        .into_iter()
        .map(|value| u16::try_from(value).expect("bits fit SHORT"))
        .collect::<Vec<_>>();
    let samples_per_pixel = u16::try_from(values(277)[0]).expect("samples fit SHORT");
    let photometric = u16::try_from(values(262)[0]).expect("photometric fits SHORT");
    let extra_samples = if fields.contains_key(&338) {
        values(338)
            .into_iter()
            .map(|value| u16::try_from(value).expect("ExtraSamples fits SHORT"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let strip_offset = usize::try_from(values(273)[0]).expect("strip offset fits usize");
    let strip_byte_count = usize::try_from(values(279)[0]).expect("strip size fits usize");
    let strip = &bytes[strip_offset..strip_offset + strip_byte_count];
    assert!(
        bits_per_sample
            .iter()
            .all(|bits| *bits == bits_per_sample[0])
    );
    let samples = match bits_per_sample[0] {
        8 => strip.iter().copied().map(u16::from).collect(),
        16 => {
            let (samples, remainder) = strip.as_chunks::<2>();
            assert!(remainder.is_empty());
            samples.iter().copied().map(u16::from_le_bytes).collect()
        }
        _ => panic!("unsupported golden TIFF depth"),
    };
    TiffProbe {
        width_px,
        height_px,
        bits_per_sample,
        samples_per_pixel,
        photometric,
        extra_samples,
        samples,
    }
}

fn reduce_tiff_sample(value: u32, bits: u16) -> u8 {
    match bits {
        8 => u8::try_from(value).expect("8-bit TIFF sample fits u8"),
        16 => u8::try_from((value + 128) / 257).expect("reduced TIFF sample fits u8"),
        _ => panic!("unsupported golden TIFF depth"),
    }
}

fn normalize_tiff(probe: &TiffProbe) -> Result<Vec<Vec<String>>, NormalizationErrorV1> {
    let semantics = match probe.extra_samples.as_slice() {
        [1] => TiffAlphaSemanticsV1::AssociatedAlpha,
        [2] => TiffAlphaSemanticsV1::UnassociatedAlpha,
        _ => return Err(NormalizationErrorV1::UnsupportedSourceVariant),
    };
    let accepted_shape = (probe.photometric == 2 && probe.samples_per_pixel == 4)
        || (probe.photometric == 1 && probe.samples_per_pixel == 2);
    if !accepted_shape
        || probe.bits_per_sample.len() != usize::from(probe.samples_per_pixel)
        || !probe
            .bits_per_sample
            .iter()
            .all(|bits| matches!(bits, 8 | 16))
    {
        return Err(NormalizationErrorV1::UnsupportedSourceVariant);
    }
    let bits = probe.bits_per_sample[0];
    let maximum = if bits == 16 { 65_535_u32 } else { 255_u32 };
    let channels = usize::from(probe.samples_per_pixel);
    let rgba = probe
        .samples
        .chunks_exact(channels)
        .map(|samples| {
            let alpha = u32::from(samples[channels - 1]);
            let straight = |value: u16| -> u32 {
                let value = u32::from(value);
                if semantics == TiffAlphaSemanticsV1::UnassociatedAlpha {
                    value
                } else if alpha == 0 {
                    0
                } else {
                    assert!(value <= alpha, "associated color cannot exceed alpha");
                    (value * maximum + alpha / 2) / alpha
                }
            };
            let rgb = if probe.photometric == 2 {
                [
                    straight(samples[0]),
                    straight(samples[1]),
                    straight(samples[2]),
                ]
            } else {
                [straight(samples[0]); 3]
            };
            format!(
                "{:02X}{:02X}{:02X}{:02X}",
                reduce_tiff_sample(rgb[0], bits),
                reduce_tiff_sample(rgb[1], bits),
                reduce_tiff_sample(rgb[2], bits),
                reduce_tiff_sample(alpha, bits),
            )
        })
        .collect::<Vec<_>>();
    let width = usize::try_from(probe.width_px).expect("TIFF width fits usize");
    Ok(rgba.chunks(width).map(<[_]>::to_vec).collect())
}

fn points_decimal_6(micrometers: u64) -> String {
    let scaled = (u128::from(micrometers) * 72 * 1_000_000 + u128::from(MICROMETERS_PER_INCH / 2))
        / u128::from(MICROMETERS_PER_INCH);
    let integer = scaled / 1_000_000;
    let fraction = scaled % 1_000_000;
    if fraction == 0 {
        return integer.to_string();
    }
    let mut fraction = format!("{fraction:06}");
    while fraction.ends_with('0') {
        fraction.pop();
    }
    format!("{integer}.{fraction}")
}

fn validate_raster_origin(corpus: &CorpusV1, raster_case: &CanonicalRasterCaseV1) {
    let RasterOriginV1::CompositionFrame(link) = &raster_case.origin else {
        return;
    };
    let composition = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::Composition(case) if case.id == link.composition_case_id => Some(case),
            _ => None,
        })
        .expect("a linked raster names a composition case");
    let plan = reference_plan(&composition.input, &corpus.algorithms.sampler);
    assert_eq!(plan, composition.expected_plan);
    let planned_sheet = plan
        .sheets
        .iter()
        .find(|sheet| {
            sheet
                .output_units
                .iter()
                .any(|unit| unit.unit_id == link.unit_id)
        })
        .expect("a linked raster names a planned sheet");
    let planned_unit = planned_sheet
        .output_units
        .iter()
        .find(|unit| unit.unit_id == link.unit_id)
        .expect("a linked raster names a planned output unit");
    let raster_unit = &raster_case.input.unit;
    assert_eq!(raster_unit.unit_id, planned_unit.unit_id);
    assert_eq!(raster_unit.width_px, planned_unit.width_px);
    assert_eq!(raster_unit.height_px, planned_unit.height_px);
    assert_eq!(raster_unit.dpi, plan.dpi);
    assert_eq!(
        raster_unit.physical_source_rect_um,
        planned_unit.physical_source_rect_um
    );

    let planned_frame = plan
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.ordered_layers)
        .find_map(|layer| match layer {
            PlannedLayerV1::FrameGroup(frame) if frame.frame_id == link.frame_id => Some(frame),
            _ => None,
        })
        .expect("a linked raster names a planned Frame");
    let projected_frame = raster_unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::FrameGroup(frame) if frame.layer_id == planned_frame.layer_id => {
                Some(frame)
            }
            _ => None,
        })
        .expect("the linked Frame is present in the output unit");
    let origin = [
        planned_unit.physical_source_rect_um[0],
        planned_unit.physical_source_rect_um[1],
    ];
    assert_eq!(
        projected_frame.source_from_destination_q32,
        project_source_from_destination(
            &planned_frame.photo.source_from_physical_q32,
            origin,
            plan.dpi,
        )
    );
    assert_eq!(
        projected_frame.frame_rect_px,
        project_rect_to_unit(
            planned_frame.frame_rect_um,
            planned_unit.physical_source_rect_um,
            plan.dpi,
        )
    );
    assert_eq!(
        projected_frame.photo_clip_rect_px,
        project_rect_to_unit(
            planned_frame.photo_clip_rect_um,
            planned_unit.physical_source_rect_um,
            plan.dpi,
        )
    );
    let projected_borders = planned_frame
        .border_fill_rects_um
        .iter()
        .map(|rect| project_rect_to_unit(*rect, planned_unit.physical_source_rect_um, plan.dpi))
        .filter(|rect| rect[2] > 0 && rect[3] > 0)
        .collect::<Vec<_>>();
    assert_eq!(projected_frame.border_fill_rects_px, projected_borders);
    assert_eq!(projected_frame.source_id, planned_frame.photo.media_id);
    assert_eq!(projected_frame.border_rgb, planned_frame.border_rgb);
    assert_eq!(
        projected_frame.group_opacity_byte,
        planned_frame.group_opacity_byte
    );
    assert_eq!(
        projected_frame.black_and_white,
        planned_frame.photo.transform.black_and_white
    );

    let (planned_decorative, planned_image) = planned_sheet
        .ordered_layers
        .iter()
        .find_map(|layer| match layer {
            PlannedLayerV1::Background(decorative) | PlannedLayerV1::Overlay(decorative) => {
                match &decorative.paint {
                    PlannedPaintV1::Image(image) => Some((decorative, image)),
                    PlannedPaintV1::Solid(_) => None,
                }
            }
            _ => None,
        })
        .expect("the composition seam includes a Both-sides decorative image");
    assert_eq!(planned_decorative.scope, ScopeV1::Both);
    let projected_decorative = raster_unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::Image(image) if image.layer_id == planned_decorative.layer_id => {
                Some(image)
            }
            _ => None,
        })
        .expect("the Both-sides decorative image is projected into the Page");
    assert_eq!(projected_decorative.source_id, planned_image.media_id);
    assert_eq!(planned_image.applied_orientation, 1);
    assert_eq!(
        projected_decorative.clip_rect_px,
        project_rect_to_unit(
            planned_decorative.rect_um,
            planned_unit.physical_source_rect_um,
            plan.dpi,
        )
    );
    assert_eq!(
        projected_decorative.source_from_destination_q32,
        project_source_from_destination(&planned_image.source_from_physical_q32, origin, plan.dpi,),
        "Both-sides stretch remains continuous across the right Page origin"
    );

    let spread = planned_sheet
        .output_units
        .iter()
        .find(|unit| unit.mode == ExportModeV1::PerSheet)
        .expect("the linked sheet also exposes a spread unit");
    assert_eq!(spread.width_px, planned_unit.width_px * 2 + 1);
    let layer_order = raster_unit
        .layers
        .iter()
        .map(|layer| match layer {
            ProjectedLayerV1::Base(_) => "base",
            ProjectedLayerV1::Solid(layer) => layer.layer_id.as_str(),
            ProjectedLayerV1::Image(layer) => layer.layer_id.as_str(),
            ProjectedLayerV1::FrameGroup(layer) => layer.layer_id.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        layer_order,
        [
            "base",
            "background:both",
            "frame:00000000-0000-4000-8000-000000000003",
            "frame:00000000-0000-4000-8000-000000000002",
        ]
    );
}

#[test]
fn corpus_schema_ids_and_assets_are_closed_and_versioned() {
    let corpus = corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(
        corpus.contract,
        "docs/design/0019-contrato-do-renderizador-final.md"
    );
    assert!(workspace_root().join(&corpus.contract).is_file());
    assert_eq!(corpus.algorithms.fixed_point, "q32.32-round-half-away-v1");
    assert_eq!(corpus.algorithms.sampler, "bilinear-premultiplied-q16-v1");
    assert_eq!(
        corpus.algorithms.alpha_composite,
        "porter-duff-source-over-u8-half-up-v1"
    );
    assert_eq!(
        corpus.algorithms.raster_edge,
        "floor((micrometers*dpi+12700)/25400)"
    );

    let ids = corpus.cases.iter().map(case_id).collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), corpus.cases.len());
    assert_eq!(
        ids,
        BTreeSet::from([
            "atomic-probe-unsupported",
            "complete-publication-allows-orphan-cleanup",
            "first-promotion-indeterminate",
            "first-promotion-proven-untouched",
            "format-adapters-share-canonical-unit",
            "fractional-bilinear-alpha-border",
            "fractional-transform-z-order-and-page-units",
            "imaging-envelope-right-page",
            "jpeg-exif-orientation-6",
            "last-promotion-candidate-confirmed",
            "minimum-three-digit-output-indices",
            "odd-width-independent-pages",
            "png-alpha-normalization",
            "q16-half-up-carry",
            "q16-half-up-tie",
            "right-page-crossing-frame-q32",
            "second-promotion-proven-untouched",
            "single-active-edge-pages",
            "source-changes-during-capture",
            "source-identity-indeterminate",
            "tiff-associated-alpha-16",
            "tiff-four-samples-requires-extra-samples",
            "tiff-unassociated-gray-alpha-16",
        ])
    );

    let asset_ids = corpus
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(asset_ids.len(), corpus.assets.len());
    assert_eq!(
        asset_ids,
        BTreeSet::from([
            "jpeg-orientation-6-2x1",
            "png-rgba-2x2",
            "tiff-associated-rgba16-1x1",
            "tiff-missing-extra-rgba8-1x1",
            "tiff-unassociated-graya16-1x1",
        ])
    );
    for asset in &corpus.assets {
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
        decode_hex_asset(asset);
    }
}

#[test]
fn geometry_and_creative_state_produce_the_complete_expected_plan() {
    let corpus = corpus();
    let geometry = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::RasterGeometry(case) => Some(case),
            _ => None,
        })
        .expect("the geometry case exists");
    let input = &geometry.input;
    let expected = &geometry.expected;
    assert_eq!(
        raster_edge(input.sheet_width_um, input.dpi),
        expected.sheet_width_px
    );
    assert_eq!(
        raster_edge(input.sheet_height_um, input.dpi),
        expected.sheet_height_px
    );
    assert_eq!(
        raster_edge(input.page_width_um, input.dpi),
        expected.center_edge_px
    );
    assert_eq!(expected.left_interval_px, [0, expected.center_edge_px]);
    assert_eq!(
        expected.right_interval_px,
        [expected.center_edge_px, expected.sheet_width_px]
    );
    assert_eq!(
        expected.independent_page_width_px,
        raster_edge(input.page_width_um, input.dpi)
    );
    assert_eq!(
        expected.independent_page_height_px,
        raster_edge(input.sheet_height_um, input.dpi)
    );
    assert_eq!(
        expected.right_interval_px[1] - expected.right_interval_px[0],
        expected.independent_page_width_px + 1,
        "the right spread interval is not reused as an independent Page raster"
    );

    let composition = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::Composition(case)
                if case.id == "fractional-transform-z-order-and-page-units" =>
            {
                Some(case)
            }
            _ => None,
        })
        .expect("the composition case exists");
    assert_eq!(
        composition.adapter_registrations.as_slice(),
        &[
            ContractAdapterV1::CompositionCore,
            ContractAdapterV1::Canvas,
            ContractAdapterV1::ExportPipeline,
        ]
    );
    let actual = reference_plan(&composition.input, &corpus.algorithms.sampler);
    assert_eq!(actual, composition.expected_plan);

    let input_order = composition.input.creative_state.sheets[0]
        .frames
        .iter()
        .map(|frame| frame.frame_id.as_str())
        .collect::<Vec<_>>();
    let planned_order = actual.sheets[0]
        .ordered_layers
        .iter()
        .filter_map(|layer| match layer {
            PlannedLayerV1::FrameGroup(frame) => Some(frame.frame_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_ne!(
        input_order, planned_order,
        "the fixture exercises stable sorting"
    );
    assert_eq!(
        planned_order,
        [
            "00000000-0000-4000-8000-000000000003",
            "00000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-000000000002",
        ]
    );

    let active_edges = composition_case(&corpus, "single-active-edge-pages");
    assert_eq!(
        active_edges.adapter_registrations.as_slice(),
        &[
            ContractAdapterV1::CompositionCore,
            ContractAdapterV1::Canvas,
            ContractAdapterV1::ExportPipeline,
        ],
        "the same case ID is assigned to every future contract adapter"
    );
    let active_edges_plan = reference_plan(&active_edges.input, &corpus.algorithms.sampler);
    assert_eq!(active_edges_plan, active_edges.expected_plan);
    let page_units = active_edges_plan
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.output_units)
        .filter(|unit| unit.mode == ExportModeV1::PerPage)
        .map(|unit| {
            (
                unit.unit_id.as_str(),
                unit.logical_index,
                unit.physical_source_rect_um,
                unit.normalized_origin_um,
                unit.width_px,
                unit.height_px,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        page_units,
        [
            (
                "edge-initial:right",
                1,
                [0, 0, 25_400, 25_400],
                [0, 0],
                4,
                4,
            ),
            (
                "edge-internal:left",
                2,
                [0, 0, 25_400, 25_400],
                [0, 0],
                4,
                4,
            ),
            (
                "edge-internal:right",
                3,
                [25_400, 0, 25_400, 25_400],
                [0, 0],
                4,
                4,
            ),
            ("edge-final:left", 4, [0, 0, 25_400, 25_400], [0, 0], 4, 4,),
        ],
        "only active Pages become normalized, gapless contract units"
    );
}

#[test]
fn composition_case_ids_are_registered_for_every_contract_adapter() {
    let corpus = corpus();
    let expected = BTreeSet::from([
        "fractional-transform-z-order-and-page-units",
        "single-active-edge-pages",
    ]);
    for adapter in [
        ContractAdapterV1::CompositionCore,
        ContractAdapterV1::Canvas,
        ContractAdapterV1::ExportPipeline,
    ] {
        assert_eq!(registered_composition_case_ids(&corpus, adapter), expected);
    }
}

#[test]
fn imaging_envelope_is_closed_and_contains_only_the_selected_projection() {
    let corpus = corpus();
    let envelope_case = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::ImagingEnvelope(case) if case.id == "imaging-envelope-right-page" => {
                Some(case)
            }
            _ => None,
        })
        .expect("the closed Imaging envelope case exists");
    let composition = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::Composition(case) if case.id == envelope_case.composition_case_id => {
                Some(case)
            }
            _ => None,
        })
        .expect("the envelope names its authoritative composition case");
    let raster = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::CanonicalRaster(case)
                if case.id == envelope_case.canonical_raster_case_id =>
            {
                Some(case)
            }
            _ => None,
        })
        .expect("the envelope names its selected canonical output unit");
    let envelope = &envelope_case.expected_envelope;
    let RasterOriginV1::CompositionFrame(raster_origin) = &raster.origin else {
        panic!("the selected envelope unit must remain linked to a composition");
    };
    assert_eq!(
        raster_origin.composition_case_id,
        envelope_case.composition_case_id
    );

    assert_eq!(envelope.protocol_version, 1);
    assert_eq!(envelope.correlation.request_id, "render-contract-0001");
    assert_eq!(envelope.correlation.project_id, "project-golden");
    assert_eq!(
        envelope.attempt.attempt_id,
        "00000000-0000-4000-8000-000000000017"
    );
    assert_eq!(
        envelope.attempt.cancellation_id,
        "cancel-render-contract-0001"
    );
    assert_eq!(envelope.revision, composition.input.creative_state.revision);
    assert_eq!(envelope.dpi, composition.input.creative_state.dpi);
    let ImagingFormatOptionsV1::Jpeg { quality } = envelope.format_options else {
        panic!("the linked right-Page envelope is the JPEG quality oracle");
    };
    assert_eq!(quality, 87);
    assert!((1..=100).contains(&quality));
    assert_eq!(
        envelope.units.as_slice(),
        std::slice::from_ref(&raster.input.unit)
    );

    let destination_directory =
        String::from_utf16(&envelope.preparation.destination_directory.windows_utf16)
            .expect("the golden destination is valid UTF-16");
    let attempt_directory =
        String::from_utf16(&envelope.preparation.attempt_directory.windows_utf16)
            .expect("the golden attempt directory is valid UTF-16");
    let output_path = String::from_utf16(&envelope.preparation.output_path.windows_utf16)
        .expect("the golden prepared output is valid UTF-16");
    assert_eq!(destination_directory, r"C:\Export");
    assert_eq!(
        attempt_directory,
        format!(
            r"{destination_directory}\.myalbuns-export-{}.tmp",
            envelope.attempt.attempt_id
        )
    );
    assert_eq!(output_path, format!(r"{attempt_directory}\album_001.jpg"));

    let selected_source_ids = envelope.units[0]
        .layers
        .iter()
        .filter_map(|layer| match layer {
            ProjectedLayerV1::Image(layer) => Some(layer.source_id.as_str()),
            ProjectedLayerV1::FrameGroup(layer) => Some(layer.source_id.as_str()),
            ProjectedLayerV1::Base(_) | ProjectedLayerV1::Solid(_) => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        selected_source_ids,
        BTreeSet::from(["decorative-checker", "photo-blue", "photo-frac"])
    );
    let envelope_source_ids = envelope
        .sources
        .iter()
        .map(|source| source.media_ref.media_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(envelope_source_ids.len(), envelope.sources.len());
    assert_eq!(envelope_source_ids, selected_source_ids);
    assert!(!envelope_source_ids.contains("photo-opaque"));

    let media_refs = composition
        .input
        .creative_state
        .media_refs
        .iter()
        .map(|media_ref| (media_ref.media_id.as_str(), media_ref))
        .collect::<BTreeMap<_, _>>();
    let observations = composition
        .input
        .source_observations
        .iter()
        .map(|observation| (observation.media_id.as_str(), observation))
        .collect::<BTreeMap<_, _>>();
    for source in &envelope.sources {
        let media_id = source.media_ref.media_id.as_str();
        assert_eq!(source.media_ref, *media_refs[media_id]);
        assert_eq!(source.source_observation, *observations[media_id]);
        assert_eq!(source.source_observation.media_id, media_id);
        assert_eq!(
            source.media_ref.native_path,
            source.source_observation.native_path
        );
    }

    assert_eq!(envelope.root_binding_plan.bindings.len(), 1);
    assert_eq!(
        envelope.root_binding_plan.bindings[0],
        RootBindingV1 {
            kind: PathRootKindV1::Disk,
            logical_root: NativePathV1 {
                windows_utf16: vec![67, 58, 92],
            },
            operational_root: NativePathV1 {
                windows_utf16: vec![67, 58, 92],
            },
        }
    );
    let logical_roots = envelope
        .root_binding_plan
        .bindings
        .iter()
        .map(|binding| binding.logical_root.windows_utf16.as_slice())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        logical_roots.len(),
        envelope.root_binding_plan.bindings.len()
    );
    for source in &envelope.sources {
        assert!(envelope.root_binding_plan.bindings.iter().any(|binding| {
            !binding.logical_root.windows_utf16.is_empty()
                && !binding.operational_root.windows_utf16.is_empty()
                && source
                    .media_ref
                    .native_path
                    .windows_utf16
                    .starts_with(&binding.logical_root.windows_utf16)
        }));
    }
    for path in [
        &envelope.preparation.destination_directory,
        &envelope.preparation.attempt_directory,
        &envelope.preparation.output_path,
    ] {
        assert!(envelope.root_binding_plan.bindings.iter().any(|binding| {
            path.windows_utf16
                .starts_with(&binding.logical_root.windows_utf16)
        }));
    }

    for (wire, expected) in [
        ("disk", PathRootKindV1::Disk),
        ("unc", PathRootKindV1::Unc),
        ("verbatimDisk", PathRootKindV1::VerbatimDisk),
        ("verbatimUnc", PathRootKindV1::VerbatimUnc),
        ("posix", PathRootKindV1::Posix),
    ] {
        assert_eq!(
            serde_json::from_value::<PathRootKindV1>(serde_json::json!(wire))
                .expect("every canonical RootBinding kind deserializes"),
            expected
        );
    }
    for noncanonical in ["verbatim-disk", "verbatim-unc"] {
        assert!(serde_json::from_value::<PathRootKindV1>(serde_json::json!(noncanonical)).is_err());
    }

    let corpus_path = workspace_root().join("tests/fixtures/final-renderer-cases-v1.json");
    let raw_corpus: serde_json::Value =
        serde_json::from_slice(&fs::read(corpus_path).expect("the raw corpus is readable"))
            .expect("the raw corpus is valid JSON");
    let raw_envelope = raw_corpus["cases"]
        .as_array()
        .expect("cases is an array")
        .iter()
        .find(|case| case["kind"] == "imaging-envelope")
        .and_then(|case| case["data"].get("expectedEnvelope"))
        .expect("the raw Imaging envelope exists")
        .clone();
    for forbidden_field in [
        "renderSnapshot",
        "projectDocument",
        "compositionPlan",
        "session",
        "cache",
    ] {
        let mut mutated = raw_envelope.clone();
        mutated
            .as_object_mut()
            .expect("the envelope is an object")
            .insert(forbidden_field.to_owned(), serde_json::Value::Null);
        assert!(
            serde_json::from_value::<ImagingRenderEnvelopeV1>(mutated).is_err(),
            "the closed envelope rejects {forbidden_field}"
        );
    }
    for required_field in [
        "protocolVersion",
        "correlation",
        "attempt",
        "revision",
        "dpi",
        "formatOptions",
        "preparation",
        "units",
        "sources",
        "rootBindingPlan",
    ] {
        let mut mutated = raw_envelope.clone();
        mutated
            .as_object_mut()
            .expect("the envelope is an object")
            .remove(required_field);
        assert!(
            serde_json::from_value::<ImagingRenderEnvelopeV1>(mutated).is_err(),
            "the closed envelope requires {required_field}"
        );
    }

    for malformed_format in [
        serde_json::json!({ "format": "jpeg" }),
        serde_json::json!({ "format": "jpeg", "quality": 0 }),
        serde_json::json!({ "format": "jpeg", "quality": 101 }),
        serde_json::json!({ "format": "png", "quality": 87 }),
        serde_json::json!({ "format": "pdf", "quality": 87 }),
    ] {
        let mut mutated = raw_envelope.clone();
        mutated
            .as_object_mut()
            .expect("the envelope is an object")
            .insert("formatOptions".to_owned(), malformed_format.clone());
        assert!(
            serde_json::from_value::<ImagingRenderEnvelopeV1>(mutated).is_err(),
            "the closed format union rejects {malformed_format}"
        );
    }

    for noncanonical_attempt_id in [
        "00000000-0000-1000-8000-000000000017",
        "00000000000040008000000000000017",
        "00000000-0000-4000-8000-00000000001A",
    ] {
        let mut mutated = raw_envelope.clone();
        mutated["attempt"]["attemptId"] = serde_json::json!(noncanonical_attempt_id);
        assert!(serde_json::from_value::<ImagingRenderEnvelopeV1>(mutated).is_err());
    }
    assert_eq!(
        serde_json::from_value::<ImagingFormatOptionsV1>(serde_json::json!({
            "format": "png"
        }))
        .expect("PNG has no JPEG-only option"),
        ImagingFormatOptionsV1::Png {}
    );
    assert_eq!(
        serde_json::from_value::<ImagingFormatOptionsV1>(serde_json::json!({
            "format": "pdf"
        }))
        .expect("PDF has no JPEG-only option"),
        ImagingFormatOptionsV1::Pdf {}
    );

    for (container, required_nested_field) in [
        ("attempt", "cancellationId"),
        ("preparation", "attemptDirectory"),
    ] {
        let mut mutated = raw_envelope.clone();
        mutated[container]
            .as_object_mut()
            .expect("the nested envelope value is an object")
            .remove(required_nested_field);
        assert!(serde_json::from_value::<ImagingRenderEnvelopeV1>(mutated).is_err());
    }
}

#[test]
fn composed_units_produce_exact_fractional_and_page_projection_rasters() {
    let corpus = corpus();
    let raster_cases = corpus
        .cases
        .iter()
        .filter_map(|case| match case {
            GoldenCaseV1::CanonicalRaster(case) => Some(case),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(raster_cases.len(), 4);
    for case in &raster_cases {
        validate_raster_origin(&corpus, case);
        let actual = render_reference(&case.input);
        assert_eq!(actual, case.expected_raster);
        assert_eq!(actual.dpi, case.input.unit.dpi);
        assert_eq!(actual.color_space, "srgb2014");
        assert!(
            actual
                .rgba_rows
                .iter()
                .flatten()
                .all(|pixel| pixel.ends_with("FF"))
        );
    }

    let alpha_case = raster_cases
        .iter()
        .find(|case| case.id == "fractional-bilinear-alpha-border")
        .expect("the alpha-composition raster exists");
    let photo = alpha_case
        .input
        .normalized_sources
        .iter()
        .find(|source| source.source_id == "photo")
        .expect("the asymmetric Photo source exists");
    assert_eq!(
        photo
            .rgba_rows
            .iter()
            .flatten()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    let frame = alpha_case
        .input
        .unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::FrameGroup(frame) => Some(frame),
            _ => None,
        });
    let frame = frame.expect("the fractional Frame exists");
    assert_ne!(
        parse_q32(&frame.source_from_destination_q32.xx) % (Q32_ONE as i64),
        0,
        "the oracle distinguishes bilinear sampling from nearest-neighbor"
    );
    assert!((1..=254).contains(&frame.group_opacity_byte));

    let q16_case = raster_cases
        .iter()
        .find(|case| case.id == "q16-half-up-tie")
        .expect("the Q16 half-up tie raster exists");
    let q16_probe = q16_case
        .input
        .unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::Image(image) => Some(image),
            _ => None,
        });
    let q16_probe = q16_probe.expect("the Q16 probe layer exists");
    let tie_source = i128::from(parse_q32(&q16_probe.source_from_destination_q32.tx));
    let tie_fraction_q32 = (tie_source - Q32_HALF).rem_euclid(Q32_ONE);
    assert_eq!(tie_fraction_q32.div_euclid(Q16_ONE), 128);
    assert_eq!(tie_fraction_q32.rem_euclid(Q16_ONE), Q16_ONE / 2);
    let (_, tie_fraction) = q16_axis(tie_source);
    assert_eq!(tie_fraction, 129, "Q32.32 to Q16 ties round half-up");
    let carry_case = raster_cases
        .iter()
        .find(|case| case.id == "q16-half-up-carry")
        .expect("the Q16 carry raster exists");
    let carry_probe = carry_case
        .input
        .unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::Image(image) => Some(image),
            _ => None,
        });
    let carry_probe = carry_probe.expect("the Q16 carry probe layer exists");
    let carry_source = i128::from(parse_q32(&carry_probe.source_from_destination_q32.tx));
    let carry_fraction_q32 = (carry_source - Q32_HALF).rem_euclid(Q32_ONE);
    assert_eq!(carry_fraction_q32.div_euclid(Q16_ONE), Q16_ONE - 1);
    assert_eq!(carry_fraction_q32.rem_euclid(Q16_ONE), Q16_ONE / 2);
    assert_eq!(
        q16_axis(carry_source),
        (1, 0),
        "a rounded Q16 weight of 65,536 carries into the texel index"
    );

    let page_case = raster_cases
        .iter()
        .find(|case| case.id == "right-page-crossing-frame-q32")
        .expect("the linked right-Page raster exists");
    let crossing_frame = page_case
        .input
        .unit
        .layers
        .iter()
        .find_map(|layer| match layer {
            ProjectedLayerV1::FrameGroup(frame) if frame.black_and_white => Some(frame),
            _ => None,
        })
        .expect("the linked raster exercises Preto e branco");
    assert_ne!(parse_q32(&crossing_frame.source_from_destination_q32.xy), 0);
    assert_ne!(parse_q32(&crossing_frame.source_from_destination_q32.yx), 0);
    assert!(
        page_case
            .expected_raster
            .rgba_rows
            .iter()
            .flatten()
            .any(|pixel| pixel.as_str() != "FFFFFFFF")
    );
}

#[test]
fn encoded_sources_fix_real_orientation_alpha_and_tiff_normalization() {
    let corpus = corpus();
    let composition_inputs = corpus
        .cases
        .iter()
        .filter_map(|case| match case {
            GoldenCaseV1::Composition(case) => Some((case.id.as_str(), &case.input)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let asset_bytes = corpus
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), (asset, decode_hex_asset(asset))))
        .collect::<BTreeMap<_, _>>();
    let mut referenced_assets = BTreeSet::new();

    for case in &corpus.cases {
        let GoldenCaseV1::SourceNormalization(case) = case else {
            continue;
        };
        assert_eq!(case.verification_owner_issue, 35);
        referenced_assets.insert(case.asset_id.as_str());
        let (asset, bytes) = asset_bytes
            .get(case.asset_id.as_str())
            .expect("every normalization case resolves a versioned asset");
        match (&case.expected, &asset.descriptor) {
            (
                NormalizedSourceExpectationV1::JpegExif(expected),
                AssetDescriptorV1::Jpeg(descriptor),
            ) => {
                assert!(expected.apply_exactly_once);
                assert_eq!(expected.encoded_width_px, descriptor.encoded_width_px);
                assert_eq!(expected.encoded_height_px, descriptor.encoded_height_px);
                assert_eq!(expected.embedded_orientation, descriptor.exif_orientation);
                assert_eq!(expected.applied_orientation, expected.embedded_orientation);
                let reader = ImageReader::new(Cursor::new(bytes.as_slice()))
                    .with_guessed_format()
                    .expect("the JPEG signature is readable");
                assert_eq!(reader.format(), Some(ImageFormat::Jpeg));
                let mut decoder = reader.into_decoder().expect("the JPEG decoder opens");
                assert_eq!(
                    decoder.dimensions(),
                    (expected.encoded_width_px, expected.encoded_height_px)
                );
                let orientation = decoder.orientation().expect("EXIF orientation is readable");
                assert_eq!(orientation.to_exif(), expected.embedded_orientation);
                let encoded = DynamicImage::from_decoder(decoder).expect("the JPEG pixels decode");
                let encoded_rgb = encoded.to_rgb8();
                assert_ne!(
                    encoded_rgb.get_pixel(0, 0),
                    encoded_rgb.get_pixel(1, 0),
                    "the asymmetric JPEG makes Orientation direction observable"
                );
                let mut oriented = encoded;
                oriented.apply_orientation(orientation);
                assert_eq!(
                    oriented.dimensions(),
                    (expected.oriented_width_px, expected.oriented_height_px)
                );
                let oriented_rgb = oriented.to_rgb8();
                assert_eq!(oriented_rgb.get_pixel(0, 0), encoded_rgb.get_pixel(0, 0));
                assert_eq!(oriented_rgb.get_pixel(0, 1), encoded_rgb.get_pixel(1, 0));
                let mut applied_twice = oriented.clone();
                applied_twice.apply_orientation(orientation);
                assert_ne!(
                    applied_twice.dimensions(),
                    (expected.oriented_width_px, expected.oriented_height_px),
                    "Orientation is not applied a second time"
                );
            }
            (
                NormalizedSourceExpectationV1::PngAlpha(expected),
                AssetDescriptorV1::Png(descriptor),
            ) => {
                assert!(descriptor.has_alpha);
                assert!(expected.orientation_metadata_ignored);
                assert_eq!(expected.applied_orientation, 1);
                assert_eq!(
                    png_exif_orientation(bytes),
                    Some(expected.embedded_orientation.into())
                );
                assert_eq!(expected.width_px, descriptor.encoded_width_px);
                assert_eq!(expected.height_px, descriptor.encoded_height_px);
                let reader = ImageReader::new(Cursor::new(bytes.as_slice()))
                    .with_guessed_format()
                    .expect("the PNG signature is readable");
                assert_eq!(reader.format(), Some(ImageFormat::Png));
                let rgba = reader.decode().expect("the PNG pixels decode").to_rgba8();
                assert_eq!(rgba.dimensions(), (expected.width_px, expected.height_px));
                let rows = rgba
                    .rows()
                    .map(|row| {
                        row.map(|pixel| {
                            format!(
                                "{:02X}{:02X}{:02X}{:02X}",
                                pixel[0], pixel[1], pixel[2], pixel[3]
                            )
                        })
                        .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rows, expected.rgba_rows);

                let linked_input = composition_inputs
                    .get(expected.linked_composition_case_id.as_str())
                    .expect("the PNG normalization case links to a composition input");
                let observation = linked_input
                    .source_observations
                    .iter()
                    .find(|candidate| candidate.media_id == expected.linked_media_id)
                    .expect("the linked composition input freezes the PNG observation");
                assert_eq!(observation.detected_format, DetectedFormatV1::Png);
                assert_eq!(observation.source_variant, SourceVariantV1::PngStaticRgba8);
                assert_eq!(observation.encoded_width_px, expected.width_px);
                assert_eq!(observation.encoded_height_px, expected.height_px);
                assert_eq!(observation.oriented_width_px, expected.width_px);
                assert_eq!(observation.oriented_height_px, expected.height_px);
                assert_eq!(
                    observation.embedded_orientation,
                    expected.embedded_orientation
                );
                assert_eq!(
                    observation.applied_orientation,
                    expected.applied_orientation
                );
                assert_eq!(observation.sha256_full_file_v1, asset.sha256);
                let projected_fact = linked_input
                    .source_geometry_facts
                    .iter()
                    .find(|candidate| candidate.media_id == expected.linked_media_id)
                    .expect("the linked PNG observation projects one geometry fact");
                assert_eq!(
                    projected_fact.applied_orientation, expected.applied_orientation,
                    "CompositionCore receives the effective orientation, not PNG eXIf"
                );
                let linked_raster = corpus
                    .cases
                    .iter()
                    .find_map(|case| match case {
                        GoldenCaseV1::CanonicalRaster(case)
                            if case.id == expected.linked_raster_case_id =>
                        {
                            Some(case)
                        }
                        _ => None,
                    })
                    .expect("the PNG normalization case links to a canonical raster");
                let normalized_source = linked_raster
                    .input
                    .normalized_sources
                    .iter()
                    .find(|source| source.source_id == expected.linked_media_id)
                    .expect("the linked raster consumes the normalized PNG source");
                assert_eq!(normalized_source.width_px, expected.width_px);
                assert_eq!(normalized_source.height_px, expected.height_px);
                assert_eq!(normalized_source.rgba_rows, expected.rgba_rows);
            }
            (
                NormalizedSourceExpectationV1::TiffAlpha(expected),
                AssetDescriptorV1::Tiff(descriptor),
            ) => {
                let probe = probe_tiff(bytes);
                assert_eq!(probe.width_px, descriptor.width_px);
                assert_eq!(probe.height_px, descriptor.height_px);
                assert_eq!(probe.bits_per_sample, descriptor.bits_per_sample);
                assert_eq!(probe.samples_per_pixel, descriptor.samples_per_pixel);
                assert_eq!(probe.photometric, descriptor.photometric);
                assert_eq!(probe.extra_samples, descriptor.extra_samples);
                assert_eq!(probe.width_px, expected.width_px);
                assert_eq!(probe.height_px, expected.height_px);
                let semantics = match probe.extra_samples.as_slice() {
                    [1] => TiffAlphaSemanticsV1::AssociatedAlpha,
                    [2] => TiffAlphaSemanticsV1::UnassociatedAlpha,
                    _ => panic!("an accepted TIFF has exactly one declared alpha"),
                };
                assert_eq!(semantics, expected.alpha_semantics);
                assert_eq!(
                    normalize_tiff(&probe).expect("the TIFF alpha declaration is accepted"),
                    expected.rgba_rows
                );
            }
            (
                NormalizedSourceExpectationV1::TiffRejected(expected),
                AssetDescriptorV1::Tiff(descriptor),
            ) => {
                let probe = probe_tiff(bytes);
                assert_eq!(probe.width_px, descriptor.width_px);
                assert_eq!(probe.height_px, descriptor.height_px);
                assert_eq!(probe.bits_per_sample, descriptor.bits_per_sample);
                assert_eq!(probe.samples_per_pixel, descriptor.samples_per_pixel);
                assert_eq!(probe.photometric, descriptor.photometric);
                assert_eq!(probe.extra_samples, descriptor.extra_samples);
                assert_eq!(normalize_tiff(&probe), Err(expected.error_code));
            }
            _ => panic!("asset descriptor and normalization oracle disagree"),
        }
    }
    assert_eq!(
        referenced_assets,
        asset_bytes.keys().copied().collect::<BTreeSet<_>>()
    );
}

#[test]
fn format_names_and_operational_oracles_enforce_the_remaining_contract() {
    let corpus = corpus();
    let raster_cases = corpus
        .cases
        .iter()
        .filter_map(|case| match case {
            GoldenCaseV1::CanonicalRaster(case) => Some(case),
            _ => None,
        })
        .map(|case| (case.input.unit.unit_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let formats = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::FormatAdapters(case) => Some(case),
            _ => None,
        })
        .expect("the format oracle exists");
    assert_eq!(
        formats.canonical_unit_ids.len(),
        formats
            .canonical_unit_ids
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
    );
    assert_eq!(
        formats
            .canonical_unit_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["raster-alpha:spread", "q16-rounding:unit"]
    );
    let canonical_units = formats
        .canonical_unit_ids
        .iter()
        .map(|unit_id| raster_cases[unit_id.as_str()])
        .collect::<Vec<_>>();
    let expected = &formats.expected;
    assert_eq!(expected.jpeg.extension, ".jpg");
    assert_eq!(expected.jpeg.quality_input, 100);
    assert_eq!(expected.jpeg.process, "baseline-sof0");
    assert_eq!(expected.jpeg.subsampling, "4:4:4");
    assert_eq!(expected.jpeg.components, 3);
    assert!(
        canonical_units
            .iter()
            .all(|case| expected.jpeg.dpi == case.input.unit.dpi)
    );
    assert_eq!(expected.jpeg.icc, "sRGB2014.icc");
    assert_eq!(
        expected
            .jpeg
            .forbidden_metadata
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "EXIF",
            "GPS",
            "Orientation",
            "XMP",
            "comment",
            "date",
            "thumbnail",
        ])
    );
    assert_eq!(
        expected.jpeg.decoded_error_metric,
        "per-channel-absolute-error; max over all channels; mean=sum/(width*height*3)"
    );
    assert!(expected.jpeg.decoded_max_channel_error <= 8);
    assert!(expected.jpeg.decoded_mean_error_at_most <= 2);

    assert_eq!(expected.png.extension, ".png");
    assert_eq!(expected.png.color_type, "rgb8");
    assert!(!expected.png.interlaced);
    assert_eq!(
        expected.png.pixels_per_meter,
        (expected.jpeg.dpi * 10_000 + 127) / 254
    );
    assert_eq!(expected.png.icc, expected.jpeg.icc);
    assert_eq!(
        expected
            .png
            .forbidden_chunks
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "acTL", "fcTL", "fdAT", "eXIf", "tEXt", "zTXt", "iTXt", "tIME",
        ])
    );
    assert_eq!(expected.png.decoded_comparison, "exact-canonical-rgb");

    assert_eq!(expected.pdf.extension, ".pdf");
    assert_eq!(
        usize::try_from(expected.pdf.page_count).expect("page count fits usize"),
        canonical_units.len()
    );
    assert_eq!(expected.pdf.page_order, formats.canonical_unit_ids);
    assert_eq!(
        expected.pdf.embedded_raster_order,
        formats.canonical_unit_ids
    );
    assert_eq!(
        expected.pdf.media_boxes_points,
        expected.pdf.crop_boxes_points
    );
    let boxes = canonical_units
        .iter()
        .map(|case| {
            let rect = case.input.unit.physical_source_rect_um;
            [points_decimal_6(rect[2]), points_decimal_6(rect[3])]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.pdf.media_boxes_points, boxes,
        "PDF boxes use the physical unit dimensions rounded to six decimals"
    );
    assert!(
        boxes.iter().flatten().any(|value| value.contains('.')),
        "the multi-page PDF oracle includes a non-integral physical box"
    );
    assert_eq!(expected.pdf.placement, "cover-media-box-no-rotation");
    assert_eq!(expected.pdf.embedded_color_space, "ICCBased-sRGB2014");
    assert_eq!(
        expected.pdf.embedded_raster_comparison,
        "exact-canonical-rgb"
    );
    assert!(expected.pdf.lossless);

    let names = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::OutputNames(case) => Some(case),
            _ => None,
        })
        .expect("the output-name oracle exists");
    assert_eq!(names.indices.as_slice(), &[1, 9, 10, 998, 999, 1000, 1001]);
    assert_eq!(
        names
            .rejected_index_strings
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["000", "0001", "+001", "-001", "1"]
    );
    assert_eq!(names.indices.len(), names.expected_names.len());
    let formatted = names
        .indices
        .iter()
        .map(|index| {
            assert!(*index > 0);
            format!("{}_{index:03}{}", names.project_name, names.extension)
        })
        .collect::<Vec<_>>();
    assert_eq!(formatted, names.expected_names);
    for rejected in &names.rejected_index_strings {
        assert!(!canonical_index(rejected));
    }

    let operational = corpus
        .cases
        .iter()
        .filter_map(|case| match case {
            GoldenCaseV1::Operational(case) => Some(case),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(operational.len(), 8);
    for case in operational {
        validate_operational_case(case);
    }
}

fn canonical_index(value: &str) -> bool {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(index) = value.parse::<u32>() else {
        return false;
    };
    index > 0 && format!("{index:03}") == value
}

fn validate_sha256(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn validate_operational_case(case: &OperationalCaseV1) {
    let context = &case.context;
    let expected = &case.expected;
    assert!(context.total_file_count > 0);
    assert!(expected.confirmed_promoted_count <= expected.attempted_count);
    assert!(expected.attempted_count <= context.total_file_count);
    assert_eq!(
        expected.preparation_cleanup,
        PreparationCleanupV1::RequiredWhenSafe
    );
    assert_eq!(
        expected.full_export_recommended,
        expected.terminal == OperationalTerminalV1::PartialPublication
    );

    match expected.publication_state {
        PublicationStateV1::NotStarted => {
            assert_eq!(expected.attempted_count, 0);
            assert_eq!(expected.confirmed_promoted_count, 0);
            assert_eq!(
                expected.failed_target_evidence,
                TargetEvidenceV1::NotApplicable
            );
            assert!(matches!(
                expected.terminal,
                OperationalTerminalV1::SourceChanged
                    | OperationalTerminalV1::SourceIdentityIndeterminate
                    | OperationalTerminalV1::AtomicReplacementUnsupported
            ));
        }
        PublicationStateV1::UntouchedByAttempt => {
            assert_eq!(expected.terminal, OperationalTerminalV1::PublicationFailed);
            assert_eq!(expected.attempted_count, 1);
            assert_eq!(expected.confirmed_promoted_count, 0);
            assert_eq!(
                expected.failed_target_evidence,
                TargetEvidenceV1::UntouchedByAttempt
            );
        }
        PublicationStateV1::PossiblyMixed => {
            assert_eq!(expected.terminal, OperationalTerminalV1::PartialPublication);
            assert!(expected.attempted_count > 0);
            assert_ne!(
                expected.failed_target_evidence,
                TargetEvidenceV1::NotApplicable
            );
            match expected.failed_target_evidence {
                TargetEvidenceV1::CandidateAtFinal => {
                    assert_eq!(expected.confirmed_promoted_count, expected.attempted_count);
                    assert!(
                        expected.attempted_count < context.total_file_count,
                        "all candidates confirmed is not PossiblyMixed"
                    );
                }
                TargetEvidenceV1::UntouchedByAttempt => {
                    assert!(
                        expected.confirmed_promoted_count > 0,
                        "an untouched first target is UntouchedByAttempt, not PossiblyMixed"
                    );
                    assert_eq!(
                        expected.confirmed_promoted_count + 1,
                        expected.attempted_count
                    )
                }
                TargetEvidenceV1::Indeterminate => {
                    assert_eq!(
                        expected.confirmed_promoted_count + 1,
                        expected.attempted_count
                    )
                }
                TargetEvidenceV1::NotApplicable => unreachable!(),
            }
        }
        PublicationStateV1::AllCandidatesConfirmed => {
            assert_eq!(expected.attempted_count, context.total_file_count);
            assert_eq!(expected.confirmed_promoted_count, context.total_file_count);
            match expected.failed_target_evidence {
                TargetEvidenceV1::NotApplicable => {
                    assert_eq!(expected.terminal, OperationalTerminalV1::Completed)
                }
                TargetEvidenceV1::CandidateAtFinal => {
                    assert_eq!(expected.terminal, OperationalTerminalV1::PartialPublication)
                }
                TargetEvidenceV1::UntouchedByAttempt | TargetEvidenceV1::Indeterminate => {
                    panic!("all candidates require positive final evidence")
                }
            }
        }
    }

    assert_eq!(
        expected.orphan_cleanup_runs,
        expected.terminal == OperationalTerminalV1::Completed
            && context.integral_export
            && context.orphan_removal_confirmed
    );
    if expected.terminal != OperationalTerminalV1::Completed {
        assert!(!expected.orphan_cleanup_runs);
    }

    match &case.injection {
        FaultInjectionV1::SourceDigestChanges(fault) => {
            assert_eq!(case.owner_issue, 35);
            assert!(!fault.media_id.is_empty());
            validate_sha256(&fault.before_sha256);
            validate_sha256(&fault.after_sha256);
            assert_ne!(fault.before_sha256, fault.after_sha256);
            assert_eq!(expected.terminal, OperationalTerminalV1::SourceChanged);
            assert_eq!(expected.failed_output, FailedOutputV1::NotApplicable);
            assert_eq!(expected.cause_code, CauseCodeV1::SourceChanged);
        }
        FaultInjectionV1::SourceIdentityIndeterminate(fault) => {
            assert_eq!(case.owner_issue, 13);
            assert!(!fault.media_id.is_empty());
            assert_eq!(
                expected.terminal,
                OperationalTerminalV1::SourceIdentityIndeterminate
            );
            assert_eq!(expected.failed_output, FailedOutputV1::NotApplicable);
            assert_eq!(
                expected.cause_code,
                CauseCodeV1::SourceIdentityIndeterminate
            );
        }
        FaultInjectionV1::AtomicProbeUnsupported(fault) => {
            assert_eq!(case.owner_issue, 35);
            assert_eq!(fault.operation, "replace-confirmed");
            assert_eq!(
                expected.terminal,
                OperationalTerminalV1::AtomicReplacementUnsupported
            );
            assert_eq!(expected.failed_output, FailedOutputV1::NotApplicable);
            assert_eq!(
                expected.cause_code,
                CauseCodeV1::AtomicReplacementUnsupported
            );
        }
        FaultInjectionV1::PromotionFails(fault) => {
            assert_eq!(case.owner_issue, 35);
            assert!(!fault.failed_output.is_empty());
            assert_eq!(fault.attempt_number, expected.attempted_count);
            assert_eq!(fault.target_evidence, expected.failed_target_evidence);
            assert_eq!(
                expected.failed_output,
                FailedOutputV1::Output(fault.failed_output.clone())
            );
            assert_eq!(expected.cause_code, CauseCodeV1::AtomicPromotionFailed);
        }
        FaultInjectionV1::None(_) => {
            assert_eq!(case.owner_issue, 38);
            assert_eq!(expected.terminal, OperationalTerminalV1::Completed);
            assert_eq!(expected.failed_output, FailedOutputV1::NotApplicable);
            assert_eq!(expected.cause_code, CauseCodeV1::NotApplicable);
        }
    }
}
