use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader};
use serde::Deserialize;
use sha2::{Digest, Sha256};

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
    SourceNormalization(SourceNormalizationCaseV1),
    FormatAdapters(FormatAdaptersCaseV1),
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
    input: CompositionInputV1,
    expected_plan: ExpectedCompositionPlanV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompositionInputV1 {
    creative_state: CreativeStateV1,
    source_geometry_facts: Vec<SourceGeometryFactV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeStateV1 {
    revision: u64,
    dpi: u32,
    sheets: Vec<CreativeSheetV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeSheetV1 {
    sheet_id: String,
    number: u32,
    active_sides: ActiveSidesV1,
    width_um: u64,
    height_um: u64,
    backgrounds: Vec<CreativeDecorativeV1>,
    frames: Vec<CreativeFrameV1>,
    overlays: Vec<CreativeDecorativeV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ActiveSidesV1 {
    Both,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreativeDecorativeV1 {
    layer_id: String,
    scope: ScopeV1,
    rect_um: [u64; 4],
    paint: PaintV1,
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
enum PaintV1 {
    Solid(SolidPaintV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SolidPaintV1 {
    rgb: String,
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
    oriented_width_px: u32,
    oriented_height_px: u32,
    source_orientation: u8,
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
    paint: PaintV1,
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
    source_orientation: u8,
    transform: PhotoTransformV1,
    sampler: String,
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
    input: CanonicalRasterInputV1,
    expected_raster: CanonicalRasterV1,
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
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum ProjectedLayerV1 {
    Base(ProjectedBaseV1),
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
    border_width_px: u32,
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JpegNormalizationV1 {
    encoded_width_px: u32,
    encoded_height_px: u32,
    orientation: u8,
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
    orientation_metadata_ignored: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormatAdaptersCaseV1 {
    id: String,
    canonical_unit_id: String,
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
    decoded_comparison: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PdfExpectationV1 {
    extension: String,
    page_count: u32,
    media_box_points: [u32; 2],
    crop_box_points: [u32; 2],
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
    failed_target_evidence: TargetEvidenceV1,
    preparation_cleanup: PreparationCleanupV1,
    orphan_cleanup_runs: bool,
    full_export_recommended: bool,
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
        GoldenCaseV1::SourceNormalization(case) => &case.id,
        GoldenCaseV1::FormatAdapters(case) => &case.id,
        GoldenCaseV1::OutputNames(case) => &case.id,
        GoldenCaseV1::Operational(case) => &case.id,
    }
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

fn quantize_q32(value: f64) -> i64 {
    assert!(value.is_finite());
    (value * Q32_ONE as f64).round() as i64
}

fn q32_string(value: f64) -> String {
    quantize_q32(value).to_string()
}

fn planned_affine(
    frame_rect: [u64; 4],
    border_width_um: u64,
    transform: &PhotoTransformV1,
    source_width: u32,
    source_height: u32,
) -> AffineQ32V1 {
    let inner = inset_rect(frame_rect, border_width_um);
    let x = inner[0] as f64;
    let y = inner[1] as f64;
    let width = inner[2] as f64;
    let height = inner[3] as f64;
    let source_width = f64::from(source_width);
    let source_height = f64::from(source_height);

    let angle_degrees = f64::from(transform.quarter_turns_ccw) * 90.0
        + f64::from(transform.fine_angle_tenths) / 10.0;
    let radians = angle_degrees.to_radians();
    let cosine = quantize_q32(radians.cos()) as f64 / Q32_ONE as f64;
    let sine = quantize_q32(radians.sin()) as f64 / Q32_ONE as f64;
    let required_x = cosine.abs() * width + sine.abs() * height;
    let required_y = sine.abs() * width + cosine.abs() * height;
    let base_scale = (required_x / source_width).max(required_y / source_height);
    let scale = base_scale * f64::from(transform.user_zoom_millionths) / 1_000_000.0;
    assert!(scale.is_finite() && scale > 0.0);

    let overflow_x = (source_width * scale - required_x).max(0.0) / 2.0;
    let overflow_y = (source_height * scale - required_y).max(0.0) / 2.0;
    let pan_x = f64::from(transform.pan_x_millionths) / 1_000_000.0 * overflow_x;
    let pan_y = f64::from(transform.pan_y_millionths) / 1_000_000.0 * overflow_y;
    let center_x = x + width / 2.0 + cosine * pan_x + sine * pan_y;
    let center_y = y + height / 2.0 - sine * pan_x + cosine * pan_y;

    // Inverse of scale * M * R, where M is the post-rotation horizontal mirror.
    let mirror_sign = if transform.mirror_horizontal {
        -1.0
    } else {
        1.0
    };
    let xx = cosine * mirror_sign / scale;
    let xy = -sine / scale;
    let yx = sine * mirror_sign / scale;
    let yy = cosine / scale;
    let tx = source_width / 2.0 - xx * center_x - xy * center_y;
    let ty = source_height / 2.0 - yx * center_x - yy * center_y;

    AffineQ32V1 {
        xx: q32_string(xx),
        xy: q32_string(xy),
        tx: q32_string(tx),
        yx: q32_string(yx),
        yy: q32_string(yy),
        ty: q32_string(ty),
    }
}

fn inset_rect(rect: [u64; 4], inset: u64) -> [u64; 4] {
    assert!(rect[2] > 0 && rect[3] > 0);
    assert!(inset <= rect[2] / 2 && inset <= rect[3] / 2);
    [
        rect[0] + inset,
        rect[1] + inset,
        rect[2] - inset * 2,
        rect[3] - inset * 2,
    ]
}

fn border_fill_rects(rect: [u64; 4], border: u64) -> Vec<[u64; 4]> {
    if border == 0 {
        return Vec::new();
    }
    let inner = inset_rect(rect, border);
    vec![
        [rect[0], rect[1], rect[2], border],
        [rect[0], rect[1] + rect[3] - border, rect[2], border],
        [rect[0], inner[1], border, inner[3]],
        [rect[0] + rect[2] - border, inner[1], border, inner[3]],
    ]
}

fn reference_plan(input: &CompositionInputV1, sampler: &str) -> ExpectedCompositionPlanV1 {
    assert!(input.creative_state.dpi > 0);
    let facts = input
        .source_geometry_facts
        .iter()
        .map(|fact| (fact.media_id.as_str(), fact))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(facts.len(), input.source_geometry_facts.len());

    let mut referenced_media_ids = BTreeSet::new();
    let sheets = input
        .creative_state
        .sheets
        .iter()
        .map(|sheet| {
            assert_eq!(sheet.active_sides, ActiveSidesV1::Both);
            assert!(sheet.number > 0 && sheet.width_um > 0 && sheet.height_um > 0);
            assert_eq!(sheet.width_um % 2, 0, "the golden spread has equal pages");
            let surface = [0, 0, sheet.width_um, sheet.height_um];
            let mut ordered_layers = vec![PlannedLayerV1::Base(PlannedBaseV1 {
                layer_id: "base".to_owned(),
                rect_um: surface,
                rgb: "#FFFFFF".to_owned(),
            })];

            ordered_layers.extend(sheet.backgrounds.iter().map(|decorative| {
                validate_rect_inside(decorative.rect_um, surface);
                validate_paint(&decorative.paint);
                PlannedLayerV1::Background(PlannedDecorativeV1 {
                    layer_id: decorative.layer_id.clone(),
                    scope: decorative.scope,
                    rect_um: decorative.rect_um,
                    paint: decorative.paint.clone(),
                })
            }));

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
                        source_orientation: fact.source_orientation,
                        transform: frame.photo.transform.clone(),
                        sampler: sampler.to_owned(),
                        source_from_physical_q32: planned_affine(
                            frame.rect_um,
                            frame.style.border_width_um,
                            &frame.photo.transform,
                            fact.oriented_width_px,
                            fact.oriented_height_px,
                        ),
                    },
                })));
            }

            ordered_layers.extend(sheet.overlays.iter().map(|decorative| {
                validate_rect_inside(decorative.rect_um, surface);
                validate_paint(&decorative.paint);
                PlannedLayerV1::Overlay(PlannedDecorativeV1 {
                    layer_id: decorative.layer_id.clone(),
                    scope: decorative.scope,
                    rect_um: decorative.rect_um,
                    paint: decorative.paint.clone(),
                })
            }));

            let page_width = sheet.width_um / 2;
            let dpi = input.creative_state.dpi;
            let output_units = vec![
                ExpectedOutputUnitV1 {
                    unit_id: format!("{}:spread", sheet.sheet_id),
                    mode: ExportModeV1::PerSheet,
                    logical_index: sheet.number,
                    physical_source_rect_um: surface,
                    normalized_origin_um: [0, 0],
                    width_px: raster_edge(sheet.width_um, dpi),
                    height_px: raster_edge(sheet.height_um, dpi),
                },
                ExpectedOutputUnitV1 {
                    unit_id: format!("{}:left", sheet.sheet_id),
                    mode: ExportModeV1::PerPage,
                    logical_index: sheet.number * 2 - 1,
                    physical_source_rect_um: [0, 0, page_width, sheet.height_um],
                    normalized_origin_um: [0, 0],
                    width_px: raster_edge(page_width, dpi),
                    height_px: raster_edge(sheet.height_um, dpi),
                },
                ExpectedOutputUnitV1 {
                    unit_id: format!("{}:right", sheet.sheet_id),
                    mode: ExportModeV1::PerPage,
                    logical_index: sheet.number * 2,
                    physical_source_rect_um: [page_width, 0, page_width, sheet.height_um],
                    normalized_origin_um: [0, 0],
                    width_px: raster_edge(page_width, dpi),
                    height_px: raster_edge(sheet.height_um, dpi),
                },
            ];

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

fn validate_paint(paint: &PaintV1) {
    match paint {
        PaintV1::Solid(solid) => validate_rgb(&solid.rgb),
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
                assert!(frame.border_width_px * 2 <= frame.frame_rect_px[2]);
                assert!(frame.border_width_px * 2 <= frame.frame_rect_px[3]);
                let expected_clip = [
                    frame.frame_rect_px[0] + frame.border_width_px,
                    frame.frame_rect_px[1] + frame.border_width_px,
                    frame.frame_rect_px[2] - frame.border_width_px * 2,
                    frame.frame_rect_px[3] - frame.border_width_px * 2,
                ];
                assert_eq!(frame.photo_clip_rect_px, expected_clip);
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
                        } else {
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
            "jpeg-exif-orientation-6",
            "minimum-three-digit-output-indices",
            "odd-width-independent-pages",
            "png-alpha-normalization",
            "second-promotion-proven-untouched",
            "source-changes-during-capture",
            "source-identity-indeterminate",
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
        BTreeSet::from(["jpeg-orientation-6-2x1", "png-rgba-2x2"])
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
            GoldenCaseV1::Composition(case) => Some(case),
            _ => None,
        })
        .expect("the composition case exists");
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
}

#[test]
fn composed_unit_produces_the_exact_fractional_alpha_raster() {
    let corpus = corpus();
    let case = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::CanonicalRaster(case) => Some(case),
            _ => None,
        })
        .expect("the canonical raster case exists");
    let actual = render_reference(&case.input);
    assert_eq!(actual, case.expected_raster);
    assert!(
        actual
            .rgba_rows
            .iter()
            .flatten()
            .all(|pixel| pixel.ends_with("FF"))
    );

    let photo = case
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
    let frame = case.input.unit.layers.iter().find_map(|layer| match layer {
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
}

#[test]
fn encoded_sources_fix_real_exif_and_png_alpha_normalization() {
    let corpus = corpus();
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
                assert_eq!(expected.orientation, descriptor.exif_orientation);
                let reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg);
                let mut decoder = reader.into_decoder().expect("the JPEG decoder opens");
                assert_eq!(
                    decoder.dimensions(),
                    (expected.encoded_width_px, expected.encoded_height_px)
                );
                let orientation = decoder.orientation().expect("EXIF orientation is readable");
                assert_eq!(orientation.to_exif(), expected.orientation);
                let mut oriented =
                    DynamicImage::from_decoder(decoder).expect("the JPEG pixels decode");
                oriented.apply_orientation(orientation);
                assert_eq!(
                    oriented.dimensions(),
                    (expected.oriented_width_px, expected.oriented_height_px)
                );
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
                assert_eq!(expected.width_px, descriptor.encoded_width_px);
                assert_eq!(expected.height_px, descriptor.encoded_height_px);
                let rgba = image::load_from_memory_with_format(bytes, ImageFormat::Png)
                    .expect("the PNG pixels decode")
                    .to_rgba8();
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
    let raster_case = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::CanonicalRaster(case) => Some(case),
            _ => None,
        })
        .expect("the canonical unit exists");
    let formats = corpus
        .cases
        .iter()
        .find_map(|case| match case {
            GoldenCaseV1::FormatAdapters(case) => Some(case),
            _ => None,
        })
        .expect("the format oracle exists");
    assert_eq!(formats.canonical_unit_id, raster_case.input.unit.unit_id);
    let expected = &formats.expected;
    assert_eq!(expected.jpeg.extension, ".jpg");
    assert_eq!(expected.jpeg.quality_input, 100);
    assert_eq!(expected.jpeg.process, "baseline-sof0");
    assert_eq!(expected.jpeg.subsampling, "4:4:4");
    assert_eq!(expected.jpeg.components, 3);
    assert_eq!(expected.jpeg.dpi, raster_case.input.unit.dpi);
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
    assert_eq!(expected.png.decoded_comparison, "exact-canonical-rgb");

    assert_eq!(expected.pdf.extension, ".pdf");
    assert_eq!(expected.pdf.page_count, 1);
    assert_eq!(expected.pdf.media_box_points, expected.pdf.crop_box_points);
    let rect = raster_case.input.unit.physical_source_rect_um;
    assert_eq!(
        expected.pdf.media_box_points,
        [
            (rect[2] * 72 / MICROMETERS_PER_INCH) as u32,
            (rect[3] * 72 / MICROMETERS_PER_INCH) as u32,
        ]
    );
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
    assert_eq!(operational.len(), 7);
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
            assert!(expected.attempted_count > 0);
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
                    assert_eq!(expected.confirmed_promoted_count, expected.attempted_count)
                }
                TargetEvidenceV1::UntouchedByAttempt | TargetEvidenceV1::Indeterminate => {
                    assert_eq!(
                        expected.confirmed_promoted_count + 1,
                        expected.attempted_count
                    )
                }
                TargetEvidenceV1::NotApplicable => unreachable!(),
            }
        }
        PublicationStateV1::AllCandidatesConfirmed => {
            assert_eq!(expected.terminal, OperationalTerminalV1::Completed);
            assert_eq!(expected.attempted_count, context.total_file_count);
            assert_eq!(expected.confirmed_promoted_count, context.total_file_count);
            assert_eq!(
                expected.failed_target_evidence,
                TargetEvidenceV1::NotApplicable
            );
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
        }
        FaultInjectionV1::SourceIdentityIndeterminate(fault) => {
            assert_eq!(case.owner_issue, 13);
            assert!(!fault.media_id.is_empty());
            assert_eq!(
                expected.terminal,
                OperationalTerminalV1::SourceIdentityIndeterminate
            );
        }
        FaultInjectionV1::AtomicProbeUnsupported(fault) => {
            assert_eq!(case.owner_issue, 35);
            assert_eq!(fault.operation, "replace-confirmed");
            assert_eq!(
                expected.terminal,
                OperationalTerminalV1::AtomicReplacementUnsupported
            );
        }
        FaultInjectionV1::PromotionFails(fault) => {
            assert_eq!(case.owner_issue, 35);
            assert!(!fault.failed_output.is_empty());
            assert_eq!(fault.attempt_number, expected.attempted_count);
            assert_eq!(fault.target_evidence, expected.failed_target_evidence);
        }
        FaultInjectionV1::None(_) => {
            assert_eq!(case.owner_issue, 38);
            assert_eq!(expected.terminal, OperationalTerminalV1::Completed);
        }
    }
}
