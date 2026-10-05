//! Runs the final-renderer corpus (design 0019) through the productive Core
//! and the real Processor, for the cases that describe a whole composition.
//!
//! Each creative state is written as a Project File and opened by the Core, so
//! the plan and the pixels compared here are the ones an Export produces. The
//! cases that only exist as a pixel-space unit, an envelope or an injected
//! fault have no productive entry point and stay with the reference oracle in
//! `myalbuns-core/tests/final_renderer_contract.rs`.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use image::{Rgba, RgbaImage};
use myalbuns_core::{
    ComposedBackground, ComposedPhoto, EditableProject, ExportMode, MediaId, OpenProjectRequest,
    PhotoSourceMetadata, ProjectCore, ProjectLocation, ProjectedFrameBorder, RectUm,
    RenderSnapshot, SelectedExportUnit,
};
use myalbuns_imaging_protocol::{
    AlbumRenderOutput, AlbumRenderRequest, IMAGING_PROTOCOL_VERSION, ImagingCommand,
    ImagingResponse, RenderFormat, RenderSource, decode_event_stream,
};
use myalbuns_paths::OperationPathContext;
use serde_json::{Value, json};

const Q32_ONE: f64 = 4_294_967_296.0;

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/final-renderer-cases-v1.json"
    ))
    .unwrap()
}

fn case(corpus: &Value, kind: &str, id: &str) -> Value {
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["kind"] == kind && case["data"]["id"] == id)
        .unwrap_or_else(|| panic!("the corpus has the {kind} case {id}"))["data"]
        .clone()
}

/// A corpus creative state opened as a real Project.
struct CorpusProject {
    root: tempfile::TempDir,
    project: EditableProject,
    /// Corpus Sheet ID to Project Sheet ID.
    sheet_ids: BTreeMap<String, String>,
    /// Corpus media ID to the Project media and its Original.
    media: BTreeMap<String, (MediaId, PathBuf)>,
}

impl CorpusProject {
    fn corpus_media_id(&self, media_id: MediaId) -> &str {
        self.media
            .iter()
            .find(|(_, (id, _))| *id == media_id)
            .map(|(name, _)| name.as_str())
            .expect("the plan references media of the corpus state")
    }
}

fn location(path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths.capture(path).unwrap();
    ProjectLocation::new(path.to_path_buf(), paths.freeze())
}

fn hex_pixel(pixel: &Rgba<u8>) -> String {
    format!(
        "{:02X}{:02X}{:02X}{:02X}",
        pixel[0], pixel[1], pixel[2], pixel[3]
    )
}

fn pixels(rows: &Value) -> RgbaImage {
    let rows = rows.as_array().unwrap();
    let width = rows[0].as_array().unwrap().len() as u32;
    RgbaImage::from_fn(width, rows.len() as u32, |x, y| {
        let hex = rows[y as usize][x as usize].as_str().unwrap();
        let channel =
            |index: usize| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        Rgba([channel(0), channel(1), channel(2), channel(3)])
    })
}

fn rect(value: &Value) -> RectUm {
    let [x, y, width, height]: [i64; 4] = serde_json::from_value(value.clone()).unwrap();
    RectUm {
        x,
        y,
        width,
        height,
    }
}

fn paint_document(paint: &Value, media: &BTreeMap<String, (MediaId, PathBuf)>) -> Value {
    match paint["kind"].as_str().unwrap() {
        "solid" => json!({ "kind": "color", "rgb": paint["data"]["rgb"] }),
        "image" => json!({
            "kind": "media",
            "mediaId": media[paint["data"]["mediaId"].as_str().unwrap()].0.to_string(),
        }),
        kind => panic!("the corpus paints with {kind}"),
    }
}

/// The corpus media that a Background or an Overlay of the state uses.
fn decorative_names(sheets: &[Value]) -> Vec<String> {
    let mut names = Vec::new();
    for sheet in sheets {
        for side in ["both", "left", "right"] {
            let background = &sheet["background"][side];
            if background["kind"] == "image" {
                names.push(background["data"]["mediaId"].as_str().unwrap().to_owned());
            }
            if let Some(name) = sheet["overlay"][side]["mediaId"].as_str() {
                names.push(name.to_owned());
            }
        }
    }
    names
}

fn frame_document(frame: &Value, media: &BTreeMap<String, (MediaId, PathBuf)>) -> Value {
    let rect = rect(&frame["rectUm"]);
    let transform = &frame["photo"]["transform"];
    let millionths = |name: &str| transform[name].as_f64().unwrap() / 1_000_000.0;
    let mut document = json!({
        "id": frame["frameId"],
        "xUm": rect.x,
        "yUm": rect.y,
        "widthUm": rect.width,
        "heightUm": rect.height,
        "photo": {
            "mediaId": media[frame["photo"]["mediaId"].as_str().unwrap()].0.to_string(),
            "transform": {
                "panX": millionths("panXMillionths"),
                "panY": millionths("panYMillionths"),
                "userZoom": millionths("userZoomMillionths"),
                // The Project counts quarter turns the other way: one
                // `RotateCounterClockwise` stores three.
                "quarterTurns": (4 - transform["quarterTurnsCcw"].as_i64().unwrap()) % 4,
                "mirrorX": transform["mirrorHorizontal"],
                "angleTenths": transform["fineAngleTenths"],
                "blackAndWhite": transform["blackAndWhite"],
            },
        },
    });
    // A Frame without Border at full Opacity follows the Album style.
    let style = &frame["style"];
    if style["borderWidthUm"] != 0 || style["opacityPercent"] != 100 {
        document["style"] = json!({
            "borderRgb": style["borderRgb"],
            "borderWidthUm": style["borderWidthUm"],
            "opacityPercent": style["opacityPercent"],
        });
    }
    document
}

fn sheet_document(sheet: &Value, id: &str, media: &BTreeMap<String, (MediaId, PathBuf)>) -> Value {
    let background = &sheet["background"];
    let background = if background["scope"] == "both" {
        json!({ "sides": "both", "both": paint_document(&background["both"], media) })
    } else {
        json!({
            "sides": "perSide",
            "left": { "content": paint_document(&background["left"], media) },
            "right": { "content": paint_document(&background["right"], media) },
        })
    };
    let mut visuals = json!({ "background": background });
    let mut overlay = json!({ "sides": "perSide" });
    for side in ["left", "right"] {
        if let Some(name) = sheet["overlay"][side]["mediaId"].as_str() {
            overlay[side] = json!({
                "content": { "kind": "media", "mediaId": media[name].0.to_string() },
            });
        }
    }
    if overlay.as_object().unwrap().len() > 1 {
        visuals["overlay"] = overlay;
    }
    // A Project keeps its Visual Stack as the order of the Frames, so the
    // corpus order by `zIndex` and then `frameId` is applied while writing.
    let mut frames = sheet["frames"].as_array().unwrap().clone();
    frames.sort_by_key(|frame| {
        (
            frame["zIndex"].as_u64().unwrap(),
            frame["frameId"].as_str().unwrap().to_owned(),
        )
    });
    let mut document = json!({ "id": id, "activeSides": sheet["activeSides"], "visuals": visuals });
    if !frames.is_empty() {
        document["frames"] = frames
            .iter()
            .map(|frame| frame_document(frame, media))
            .collect();
    }
    document
}

/// Writes a corpus `input` as a Project File and opens it through the Core.
/// `sources` gives the pixels of the Originals that the case renders; the
/// others are a single opaque pixel.
fn open_corpus_project(input: &Value, sources: &BTreeMap<String, RgbaImage>) -> CorpusProject {
    let state = &input["creativeState"];
    let sheets = state["sheets"].as_array().unwrap();
    let decoratives = decorative_names(sheets);
    let root = tempfile::tempdir().unwrap();

    let mut media = BTreeMap::new();
    let mut media_documents = Vec::new();
    for (index, reference) in state["mediaRefs"].as_array().unwrap().iter().enumerate() {
        let name = reference["mediaId"].as_str().unwrap();
        let id = format!("00000000-0000-4000-a000-{:012}", index + 1);
        let path = root.path().join(format!("{name}.png"));
        sources
            .get(name)
            .cloned()
            .unwrap_or_else(|| RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255])))
            .save(&path)
            .unwrap();
        let kind = if decoratives.iter().any(|decorative| decorative == name) {
            "decorative"
        } else {
            "photo"
        };
        media_documents.push(json!({ "id": id, "kind": kind, "path": path.to_str().unwrap() }));
        media.insert(name.to_owned(), (id.parse::<MediaId>().unwrap(), path));
    }

    let mut sheet_ids = BTreeMap::new();
    let mut sheet_documents = Vec::new();
    for (index, sheet) in sheets.iter().enumerate() {
        let id = format!("00000000-0000-4000-9000-{:012}", index + 1);
        sheet_documents.push(sheet_document(sheet, &id, &media));
        sheet_ids.insert(sheet["sheetId"].as_str().unwrap().to_owned(), id);
    }
    if sheet_documents.len() < 2 {
        // An Album has at least two Sheets; this one is never selected.
        sheet_documents.push(json!({
            "id": "00000000-0000-4000-9000-999999999999",
            "activeSides": "both",
        }));
    }

    let document = json!({
        "documentType": "myalbuns.project",
        "schemaVersion": 1,
        "projectId": "550e8400-e29b-41d4-a716-446655440000",
        "revision": state["revision"],
        "project": {
            "album": {
                "displayUnit": "mm",
                "sheetWidthUm": sheets[0]["widthUm"],
                "sheetHeightUm": sheets[0]["heightUm"],
                "dpi": state["dpi"],
                "bleedUm": 0,
                "safetyUm": 0,
            },
            "layoutSettings": {
                "permission": "pagesAndSheet",
                "marginUm": 15000,
                "gapUm": 5000,
                "minimumSideUm": 20000,
            },
            "visualDefaults": {
                "background": { "sides": "both", "both": { "kind": "color", "rgb": "#FFFFFF" } },
                "overlay": { "sides": "both", "both": { "kind": "none" } },
                "frameBorder": { "kind": "none" },
            },
            "media": media_documents,
            "sheets": sheet_documents,
        },
    });
    let path = root.path().join("Corpus.myalbuns");
    std::fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .open_editable(OpenProjectRequest::new(location(&path)))
        .unwrap_or_else(|error| panic!("the corpus state opens as a Project: {error:?}"));
    for fact in input["sourceGeometryFacts"].as_array().unwrap() {
        let name = fact["mediaId"].as_str().unwrap();
        if decoratives.iter().any(|decorative| decorative == name) {
            continue;
        }
        let side = |field: &str| u32::try_from(fact[field].as_u64().unwrap()).unwrap();
        project
            .observe_photo_source(
                media[name].0,
                PhotoSourceMetadata::new(
                    side("orientedWidthPx"),
                    side("orientedHeightPx"),
                    ["#000000", "#000000", "#000000"].map(String::from),
                )
                .unwrap(),
            )
            .unwrap();
    }
    CorpusProject {
        root,
        project,
        sheet_ids,
        media,
    }
}

/// Renders each unit to its own PNG with the real Processor and returns the
/// rasters in the same order.
fn render_units(
    fixture: &CorpusProject,
    snapshot: &RenderSnapshot,
    units: &[SelectedExportUnit],
    label: &str,
) -> Vec<RgbaImage> {
    let outputs: Vec<_> = units
        .iter()
        .map(|unit| AlbumRenderOutput {
            prepared_path: fixture
                .root
                .path()
                .join(format!("{label}-{}.png", unit.index))
                .into(),
            units: vec![unit.clone()],
        })
        .collect();
    let referenced: Vec<_> = snapshot
        .composition
        .sheets
        .iter()
        .flat_map(|sheet| sheet.referenced_media_ids())
        .collect();
    let mut paths = OperationPathContext::new();
    paths.capture(fixture.root.path()).unwrap();
    let request = AlbumRenderRequest {
        protocol_version: IMAGING_PROTOCOL_VERSION,
        request_id: "final-renderer-corpus".into(),
        snapshot: snapshot.clone(),
        format: RenderFormat::Png,
        outputs: outputs.clone(),
        sources: fixture
            .media
            .values()
            .filter(|(id, _)| referenced.contains(id))
            .map(|(id, path)| RenderSource::new(*id, path.clone()).unwrap())
            .collect(),
        root_bindings: paths.freeze(),
    };
    request.validate().unwrap();
    let result = super::invoke_imaging_command(&ImagingCommand::RenderAlbum(request), None);
    let (_, response) = decode_event_stream(&result.stdout).unwrap();
    let ImagingResponse::AlbumCompleted { completion, .. } = response else {
        panic!(
            "unexpected response: {response:?}; {}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    assert_eq!(completion.outputs.len(), outputs.len());
    outputs
        .iter()
        .zip(&completion.outputs)
        .map(|(output, completed)| {
            let raster = image::open(output.prepared_path.as_path())
                .unwrap()
                .to_rgba8();
            assert_eq!(
                raster.dimensions(),
                (completed.width_px, completed.height_px)
            );
            raster
        })
        .collect()
}

/// Where the Processor puts the four corners of the oriented source, in
/// micrometres of the Sheet: it draws the Photo across `draw_rect`, turned
/// by `rotation_degrees` around its centre and mirrored before that.
fn drawn_corners(photo: &ComposedPhoto) -> [[f64; 2]; 4] {
    let draw = &photo.draw_rect;
    let (width, height) = (draw.width as f64, draw.height as f64);
    let center = [draw.x as f64 + width / 2.0, draw.y as f64 + height / 2.0];
    let (sine, cosine) = f64::from(photo.rotation_degrees).to_radians().sin_cos();
    [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]].map(|[horizontal, vertical]| {
        let along = (horizontal - 0.5) * width;
        let across = (vertical - 0.5) * height;
        let delta_x = cosine * along - sine * across;
        let delta_y = sine * along + cosine * across;
        [
            center[0] + if photo.mirror_x { -delta_x } else { delta_x },
            center[1] + delta_y,
        ]
    })
}

/// The same corners through the `physicalFromSourceQ32` matrix of the corpus.
fn planned_corners(photo: &Value) -> [[f64; 2]; 4] {
    let matrix = &photo["physicalFromSourceQ32"];
    let q32 = |name: &str| matrix[name].as_str().unwrap().parse::<f64>().unwrap() / Q32_ONE;
    let width = photo["orientedWidthPx"].as_f64().unwrap();
    let height = photo["orientedHeightPx"].as_f64().unwrap();
    [[0.0, 0.0], [width, 0.0], [0.0, height], [width, height]].map(|[x, y]| {
        [
            q32("xx") * x + q32("xy") * y + q32("tx"),
            q32("yx") * x + q32("yy") * y + q32("ty"),
        ]
    })
}

fn assert_photo_is_placed_as_planned(photo: &ComposedPhoto, planned: &Value, frame_id: &str) {
    // The Core rounds the drawn rectangle to whole micrometres.
    const TOLERANCE_UM: f64 = 2.0;
    for (drawn, planned) in drawn_corners(photo).iter().zip(planned_corners(planned)) {
        assert!(
            (drawn[0] - planned[0]).abs() <= TOLERANCE_UM
                && (drawn[1] - planned[1]).abs() <= TOLERANCE_UM,
            "{frame_id}: a source corner is drawn at {drawn:?} and planned at {planned:?}"
        );
    }
}

/// Compares everything the productive plan states with the `expectedPlan` of
/// a composition case. A Photo is compared when `photo_is_compared` accepts
/// its Frame layer.
fn assert_plan(
    case: &Value,
    fixture: &CorpusProject,
    snapshot: &RenderSnapshot,
    photo_is_compared: impl Fn(&Value) -> bool,
) {
    let expected = &case["expectedPlan"];
    assert_eq!(snapshot.revision, expected["revision"].as_u64().unwrap());
    assert_eq!(u64::from(snapshot.dpi), expected["dpi"].as_u64().unwrap());
    let mut referenced: Vec<&str> = snapshot
        .composition
        .sheets
        .iter()
        .flat_map(|sheet| sheet.referenced_media_ids())
        .map(|id| fixture.corpus_media_id(id))
        .collect();
    referenced.sort_unstable();
    referenced.dedup();
    assert_eq!(json!(referenced), expected["referencedMediaIds"]);

    for expected_sheet in expected["sheets"].as_array().unwrap() {
        let name = expected_sheet["sheetId"].as_str().unwrap();
        let sheet = snapshot
            .composition
            .sheets
            .iter()
            .find(|sheet| sheet.sheet_id == fixture.sheet_ids[name])
            .unwrap();
        let surface = RectUm {
            x: 0,
            y: 0,
            width: sheet.width_um,
            height: sheet.height_um,
        };
        assert_eq!(surface, rect(&expected_sheet["surfaceRectUm"]), "{name}");

        // The Processor paints Backgrounds, then Frames, then Overlays, each
        // list in its own order: the corpus layers must come the same way.
        let mut backgrounds = sheet.backgrounds.iter();
        let mut frames = sheet.frames.iter();
        let mut overlays = sheet.overlays.iter();
        let mut stage = 0;
        for layer in expected_sheet["orderedLayers"].as_array().unwrap() {
            let data = &layer["data"];
            let layer_id = data["layerId"].as_str().unwrap();
            let kind = layer["kind"].as_str().unwrap();
            let layer_stage = ["base", "background", "frame-group", "overlay"]
                .iter()
                .position(|known| *known == kind)
                .unwrap_or_else(|| panic!("the corpus has a {kind} layer"));
            assert!(
                layer_stage >= stage,
                "{name}: {layer_id} is out of the stack order"
            );
            stage = layer_stage;
            match kind {
                "base" => {
                    assert_eq!(json!(sheet.base.rgb), data["rgb"], "{name}");
                    assert_eq!(sheet.base.draw_rect, rect(&data["rectUm"]), "{name}");
                }
                "background" => match backgrounds.next().expect(layer_id) {
                    ComposedBackground::Color { rgb, draw_rect } => {
                        assert_eq!(data["paint"]["kind"], "solid", "{layer_id}");
                        assert_eq!(json!(rgb), data["paint"]["data"]["rgb"], "{layer_id}");
                        assert_eq!(*draw_rect, rect(&data["rectUm"]), "{layer_id}");
                    }
                    ComposedBackground::Media {
                        media_id,
                        draw_rect,
                        clip_rect,
                        ..
                    } => {
                        assert_eq!(data["paint"]["kind"], "image", "{layer_id}");
                        assert_eq!(
                            fixture.corpus_media_id(*media_id),
                            data["paint"]["data"]["mediaId"],
                            "{layer_id}"
                        );
                        // The image is stretched over the whole layer rectangle.
                        assert_eq!(*draw_rect, rect(&data["rectUm"]), "{layer_id}");
                        assert_eq!(*clip_rect, None, "{layer_id}");
                    }
                },
                "frame-group" => {
                    let frame = frames.next().expect(layer_id);
                    assert_eq!(json!(frame.frame_id), data["frameId"]);
                    assert_eq!(frame.clip_rect, rect(&data["frameRectUm"]), "{layer_id}");
                    let ring: Vec<_> = data["borderFillRectsUm"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(rect)
                        .collect();
                    assert_eq!(frame.border_fill_rects, ring, "{layer_id}");
                    match &frame.border {
                        ProjectedFrameBorder::Solid { rgb, .. } => {
                            assert_eq!(json!(rgb), data["borderRgb"], "{layer_id}")
                        }
                        ProjectedFrameBorder::None => assert!(ring.is_empty(), "{layer_id}"),
                    }
                    assert_eq!(
                        u64::from(frame.opacity_byte),
                        data["groupOpacityByte"].as_u64().unwrap(),
                        "{layer_id}"
                    );
                    let photo = frame.photo.as_ref().expect(layer_id);
                    let planned = &data["photo"];
                    assert_eq!(
                        fixture.corpus_media_id(photo.media_id),
                        planned["mediaId"],
                        "{layer_id}"
                    );
                    assert_eq!(
                        json!(photo.black_and_white),
                        planned["transform"]["blackAndWhite"],
                        "{layer_id}"
                    );
                    assert_eq!(
                        json!(photo.mirror_x),
                        planned["transform"]["mirrorHorizontal"],
                        "{layer_id}"
                    );
                    if photo_is_compared(data) {
                        assert_photo_is_placed_as_planned(photo, planned, layer_id);
                    }
                }
                _ => {
                    let overlay = overlays.next().expect(layer_id);
                    assert_eq!(
                        fixture.corpus_media_id(overlay.media_id),
                        data["paint"]["data"]["mediaId"],
                        "{layer_id}"
                    );
                    assert_eq!(overlay.draw_rect, rect(&data["rectUm"]), "{layer_id}");
                    assert_eq!(overlay.clip_rect, None, "{layer_id}");
                }
            }
        }
        assert!(
            backgrounds.next().is_none() && frames.next().is_none() && overlays.next().is_none(),
            "{name}: the productive plan has a layer the corpus does not expect"
        );
    }
}

/// The Frame layers whose Photo fills the whole Frame: without a Border, the
/// area the corpus fits the Photo to is the Frame itself.
fn without_border(frame_layer: &Value) -> bool {
    frame_layer["photoClipRectUm"] == frame_layer["frameRectUm"]
}

const COMPOSITION_CASES: [&str; 2] = [
    "fractional-transform-z-order-and-page-units",
    "single-active-edge-pages",
];

#[test]
fn corpus_compositions_are_planned_by_the_core_as_the_contract_expects() {
    let corpus = corpus();
    for id in COMPOSITION_CASES {
        let case = case(&corpus, "composition", id);
        let fixture = open_corpus_project(&case["input"], &BTreeMap::new());
        assert_plan(
            &case,
            &fixture,
            &fixture.project.render_snapshot(),
            without_border,
        );
    }
}

#[test]
#[ignore = "documents a suspected defect: the Core fits a bordered Photo to the whole Frame (design 0024), while design 0019 and its corpus fit it to the area inside the Border"]
fn corpus_photos_inside_a_border_are_placed_as_the_contract_expects() {
    let corpus = corpus();
    let case = case(&corpus, "composition", COMPOSITION_CASES[0]);
    let fixture = open_corpus_project(&case["input"], &BTreeMap::new());
    assert_plan(&case, &fixture, &fixture.project.render_snapshot(), |_| {
        true
    });
}

#[test]
fn corpus_output_units_are_selected_and_rasterized_at_the_contract_sizes() {
    let corpus = corpus();
    for id in COMPOSITION_CASES {
        let case = case(&corpus, "composition", id);
        let fixture = open_corpus_project(&case["input"], &BTreeMap::new());
        let snapshot = fixture.project.render_snapshot();
        let sheet_ids: Vec<String> = fixture.sheet_ids.values().cloned().collect();
        for (mode, corpus_mode) in [
            (ExportMode::Sheet, "per-sheet"),
            (ExportMode::Page, "per-page"),
        ] {
            let expected: Vec<&Value> = case["expectedPlan"]["sheets"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|sheet| sheet["outputUnits"].as_array().unwrap())
                .filter(|unit| unit["mode"] == corpus_mode)
                .collect();
            let units = snapshot.export_units(&sheet_ids, mode).unwrap();
            assert_eq!(units.len(), expected.len(), "{id} {corpus_mode}");
            let rasters = render_units(&fixture, &snapshot, &units, corpus_mode);
            for ((unit, raster), expected) in units.iter().zip(rasters).zip(expected) {
                let unit_id = expected["unitId"].as_str().unwrap();
                let (sheet, _) = unit_id.split_once(':').unwrap();
                assert_eq!(unit.sheet_id, fixture.sheet_ids[sheet], "{unit_id}");
                assert_eq!(json!(unit.index), expected["logicalIndex"], "{unit_id}");
                assert_eq!(
                    unit.viewport,
                    rect(&expected["physicalSourceRectUm"]),
                    "{unit_id}"
                );
                assert_eq!(
                    json!(raster.dimensions()),
                    json!([expected["widthPx"], expected["heightPx"]]),
                    "{unit_id}"
                );
            }
        }
    }
}

#[test]
fn the_odd_raster_width_of_the_corpus_gives_the_right_page_the_extra_pixel() {
    let corpus = corpus();
    let case = case(&corpus, "raster-geometry", "odd-width-independent-pages");
    let (input, expected) = (&case["input"], &case["expected"]);
    assert_eq!(
        input["sheetWidthUm"].as_u64().unwrap(),
        2 * input["pageWidthUm"].as_u64().unwrap()
    );
    let (left, right) = ("FF0000", "0000FF");
    let fixture = open_corpus_project(
        &json!({
            "creativeState": {
                "revision": 1,
                "dpi": input["dpi"],
                "mediaRefs": [],
                "sheets": [{
                    "sheetId": "odd-width",
                    "activeSides": "both",
                    "widthUm": input["sheetWidthUm"],
                    "heightUm": input["sheetHeightUm"],
                    "background": {
                        "scope": "per-side",
                        "left": { "kind": "solid", "data": { "rgb": format!("#{left}") } },
                        "right": { "kind": "solid", "data": { "rgb": format!("#{right}") } },
                    },
                    "frames": [],
                    "overlay": { "scope": "per-side", "left": null, "right": null },
                }],
            },
            "sourceGeometryFacts": [],
        }),
        &BTreeMap::new(),
    );
    let snapshot = fixture.project.render_snapshot();
    let sheet_ids: Vec<String> = fixture.sheet_ids.values().cloned().collect();
    let pixel = |value: &Value| u32::try_from(value.as_u64().unwrap()).unwrap();
    let color = |raster: &RgbaImage, x: u32| hex_pixel(raster.get_pixel(x, raster.height() / 2));

    let units = snapshot
        .export_units(&sheet_ids, ExportMode::Sheet)
        .unwrap();
    let [spread] = &render_units(&fixture, &snapshot, &units, "sheet")[..] else {
        panic!("one Sheet is one unit");
    };
    assert_eq!(
        spread.dimensions(),
        (
            pixel(&expected["sheetWidthPx"]),
            pixel(&expected["sheetHeightPx"])
        )
    );
    let center = pixel(&expected["centerEdgePx"]);
    assert_eq!(expected["leftIntervalPx"], json!([0, center]));
    assert_eq!(expected["rightIntervalPx"], json!([center, spread.width()]));
    for (x, side) in [
        (0, left),
        (center - 1, left),
        (center, right),
        (spread.width() - 1, right),
    ] {
        assert_eq!(color(spread, x), format!("{side}FF"), "column {x}");
    }

    let units = snapshot.export_units(&sheet_ids, ExportMode::Page).unwrap();
    let pages = render_units(&fixture, &snapshot, &units, "page");
    assert_eq!(pages.len(), 2);
    for (page, side) in pages.iter().zip([left, right]) {
        assert_eq!(
            page.dimensions(),
            (
                pixel(&expected["independentPageWidthPx"]),
                pixel(&expected["independentPageHeightPx"])
            )
        );
        for x in [0, page.width() - 1] {
            assert_eq!(color(page, x), format!("{side}FF"), "column {x}");
        }
    }
}

#[test]
fn black_and_white_is_the_integer_luminance_of_the_contract_on_every_channel() {
    // Uniform quadrants: away from their seams the sampler has nothing to blend.
    let colors = [[240, 16, 16], [16, 180, 32], [16, 32, 240], [240, 220, 16]];
    let source = RgbaImage::from_fn(80, 80, |x, y| {
        let [r, g, b] = colors[usize::from(x >= 40) + 2 * usize::from(y >= 40)];
        Rgba([r, g, b, 255])
    });
    let transform = |black_and_white: bool| {
        json!({
            "quarterTurnsCcw": 0, "fineAngleTenths": 0, "mirrorHorizontal": false,
            "userZoomMillionths": 1_000_000, "panXMillionths": 0, "panYMillionths": 0,
            "blackAndWhite": black_and_white,
        })
    };
    let style = json!({ "borderWidthUm": 0, "borderRgb": "#000000", "opacityPercent": 100 });
    let fixture = open_corpus_project(
        &json!({
            "creativeState": {
                "revision": 1,
                "dpi": 100,
                "mediaRefs": [{ "mediaId": "quadrants" }],
                "sheets": [{
                    "sheetId": "effects",
                    "activeSides": "both",
                    "widthUm": 50_800,
                    "heightUm": 25_400,
                    "background": {
                        "scope": "both",
                        "both": { "kind": "solid", "data": { "rgb": "#FFFFFF" } },
                    },
                    "frames": [
                        {
                            "frameId": "00000000-0000-4000-8000-000000000001",
                            "zIndex": 1,
                            "rectUm": [0, 0, 25_400, 25_400],
                            "photo": { "mediaId": "quadrants", "transform": transform(true) },
                            "style": style,
                        },
                        {
                            "frameId": "00000000-0000-4000-8000-000000000002",
                            "zIndex": 2,
                            "rectUm": [25_400, 0, 25_400, 25_400],
                            "photo": { "mediaId": "quadrants", "transform": transform(false) },
                            "style": style,
                        },
                    ],
                    "overlay": { "scope": "per-side", "left": null, "right": null },
                }],
            },
            "sourceGeometryFacts": [
                { "mediaId": "quadrants", "orientedWidthPx": 80, "orientedHeightPx": 80 },
            ],
        }),
        &BTreeMap::from([("quadrants".to_owned(), source)]),
    );
    let snapshot = fixture.project.render_snapshot();
    let sheet_ids: Vec<String> = fixture.sheet_ids.values().cloned().collect();
    let units = snapshot
        .export_units(&sheet_ids, ExportMode::Sheet)
        .unwrap();
    let [raster] = &render_units(&fixture, &snapshot, &units, "effects")[..] else {
        panic!("one Sheet is one unit");
    };
    assert_eq!(raster.dimensions(), (200, 100));

    // Worked examples of `floor((54 r + 183 g + 19 b + 128) / 256)`.
    for (((x, y), [r, g, b]), worked) in [(25, 25), (75, 25), (25, 75), (75, 75)]
        .into_iter()
        .zip(colors)
        .zip([63_u8, 134, 44, 209])
    {
        let luminance = (54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b) + 128) / 256;
        assert_eq!(luminance, u32::from(worked));
        assert_eq!(
            raster.get_pixel(x, y).0,
            [worked, worked, worked, 255],
            "black and white at {x},{y}"
        );
        assert_eq!(
            raster.get_pixel(x + 100, y).0,
            [r, g, b, 255],
            "the other occurrence keeps its colors at {x},{y}"
        );
    }
}

#[test]
#[ignore = "documents a suspected defect: the Processor samples with straight-alpha floating-point bilinear between the first and last texel and fits a bordered Photo to the whole Frame, while design 0019 fixes premultiplied Q16 bilinear at pixel centres inside the Border; every pixel of the corpus raster differs"]
fn the_corpus_right_page_is_rendered_pixel_for_pixel() {
    let corpus = corpus();
    let raster_case = case(&corpus, "canonical-raster", "right-page-crossing-frame-q32");
    let origin = &raster_case["origin"]["data"];
    let composition = case(
        &corpus,
        "composition",
        origin["compositionCaseId"].as_str().unwrap(),
    );
    let sources = raster_case["input"]["normalizedSources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| {
            (
                source["sourceId"].as_str().unwrap().to_owned(),
                pixels(&source["rgbaRows"]),
            )
        })
        .collect();
    let fixture = open_corpus_project(&composition["input"], &sources);
    let snapshot = fixture.project.render_snapshot();
    let sheet_ids: Vec<String> = fixture.sheet_ids.values().cloned().collect();
    let units = snapshot.export_units(&sheet_ids, ExportMode::Page).unwrap();
    let viewport = rect(&raster_case["input"]["unit"]["physicalSourceRectUm"]);
    let unit = units
        .iter()
        .position(|unit| unit.viewport == viewport)
        .expect("the right Page is one of the units");
    let rendered = &render_units(&fixture, &snapshot, &units, "right-page")[unit];

    let rows: Vec<Vec<String>> = rendered
        .rows()
        .map(|row| row.map(hex_pixel).collect())
        .collect();
    assert_eq!(json!(rows), raster_case["expectedRaster"]["rgbaRows"]);
}
