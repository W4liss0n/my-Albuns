//! Converts one old myAlbuns Project into the current Project File.
//!
//! The result is written as a schema 1 document and decoded by the regular
//! reader, so it passes exactly the validation of a saved Project. Layout
//! locks and favorites are then restored through the domain. Anything the
//! current program cannot represent is dropped and recorded as a note for
//! the diagnostic log; the user is not asked about it.
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

use myalbuns_paths::validate_external_path;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    LegacyConversionNote as Note, LegacyFailure,
    reader::{
        LegacyFrame, LegacyImage, LegacyLayer, LegacyLayerSet, LegacyProject, LegacySheet,
        LegacyTemplateFrame,
    },
};
use crate::{
    FavoriteLayout, LayoutDefinition, LayoutFavoriteId, LayoutOrigin, LayoutRules, LayoutSurface,
    LayoutSurfaceKind, RectUm, StoredLayout,
    composition::photo_pan_basis,
    model::{MediaTransform, PHOTO_ZOOM_MAX, PHOTO_ZOOM_MIN},
    project_document::{MAX_SAFE_INTEGER, ProjectRevision},
    project_store::project_file,
};

const UM_PER_INCH: f64 = 25_400.0;
const POINTS_PER_INCH: f64 = 72.0;
const UM_PER_CM: f64 = 10_000.0;
const WHITE: &str = "#FFFFFF";
const MAX_FINE_ANGLE_TENTHS: i64 = 450;
const MAX_FOLDER_NAME_CHARS: usize = 80;

pub(super) struct Converted {
    pub(super) revision: ProjectRevision,
    pub(super) notes: Vec<Note>,
}

pub(super) struct ConversionContext<'a> {
    pub(super) project_path: &'a Path,
    pub(super) project_id: Uuid,
    pub(super) seed: [u8; 32],
    pub(super) exists: &'a dyn Fn(&Path) -> bool,
}

pub(super) fn convert(
    project: LegacyProject,
    context: &ConversionContext<'_>,
) -> Result<Converted, LegacyFailure> {
    let mut notes = Vec::new();
    let mut ids = Ids::new(context.seed);
    if !(1..=1_200).contains(&project.dpi) {
        return Err(LegacyFailure::Damaged);
    }
    let dpi = project.dpi as f64;
    let um_per_px = UM_PER_INCH / dpi;
    let sheet_width_um = sheet_axis_um(project.width_px, project.dpi, true)?;
    let sheet_height_um = sheet_axis_um(project.height_px, project.dpi, false)?;
    let sides = active_sides(&project.sheets)?;

    let bleed_um = cm_to_um(project.cut_margin_cm);
    if project.safe_margin_cm < project.cut_margin_cm {
        notes.push(Note::SafetyInsideBleed);
    }
    let safety_um = cm_to_um((project.safe_margin_cm - project.cut_margin_cm).max(0.0));

    let usage = MediaUsage::collect(&project);
    let media = MediaCatalog::build(&project.images, &usage, context, &mut ids, &mut notes);
    let folders = media_folders(&project, &media, &mut ids, &mut notes);

    let album_border = album_frame_border(&project);
    let album_background = album_sides(
        &project.defaults.default_background_type,
        background_content(
            project.defaults.default_background.as_ref(),
            &media,
            &mut notes,
        ),
    );
    let album_overlay = album_sides(
        &project.defaults.default_overlay_type,
        overlay_content(
            project.defaults.default_overlay.as_ref(),
            &media,
            &mut notes,
        ),
    );
    let frame_border_json = match &album_border {
        Some((rgb, width_um)) => json!({ "kind": "solid", "rgb": rgb, "widthUm": width_um }),
        None => json!({ "kind": "none" }),
    };

    let sheet_scale = SheetScale {
        um_per_px,
        width_um: sheet_width_um,
        height_um: sheet_height_um,
    };
    let mut sheet_ids = Vec::with_capacity(project.sheets.len());
    let mut sheets = Vec::with_capacity(project.sheets.len());
    for (index, (sheet, active)) in project.sheets.iter().zip(&sides).enumerate() {
        let sheet_id = ids.keep_or_derive(&sheet.id, &format!("sheet:{index}"));
        sheet_ids.push(sheet_id);
        let mut entry = Map::new();
        entry.insert("id".into(), json!(sheet_id.hyphenated().to_string()));
        entry.insert("activeSides".into(), json!(active.as_str()));
        let mut visuals = Map::new();
        if let Some(background) =
            sheet_background(&sheet.background, &album_background, &media, &mut notes)
        {
            visuals.insert("background".into(), background);
        }
        if let Some(overlay) = sheet_overlay(&sheet.overlay, &album_overlay, &media, &mut notes) {
            visuals.insert("overlay".into(), overlay);
        }
        if !visuals.is_empty() {
            entry.insert("visuals".into(), Value::Object(visuals));
        }
        let frames: Vec<Value> = sheet
            .frames
            .iter()
            .enumerate()
            .filter_map(|(frame_index, frame)| {
                convert_frame(
                    frame,
                    FramePlacement {
                        sheet_number: index + 1,
                        label: format!("frame:{index}:{frame_index}"),
                        active: *active,
                    },
                    &sheet_scale,
                    &project.images,
                    &media,
                    album_border.as_ref(),
                    &mut ids,
                    &mut notes,
                )
            })
            .collect();
        if !frames.is_empty() {
            entry.insert("frames".into(), Value::Array(frames));
        }
        sheets.push(Value::Object(entry));
    }

    let parameters = crate::LayoutParameters::default();
    let document = json!({
        "documentType": "myalbuns.project",
        "schemaVersion": 1,
        "projectId": context.project_id.hyphenated().to_string(),
        "revision": 0,
        "project": {
            "album": {
                "displayUnit": "cm",
                "sheetWidthUm": sheet_width_um,
                "sheetHeightUm": sheet_height_um,
                "dpi": project.dpi,
                "bleedUm": bleed_um,
                "safetyUm": safety_um,
            },
            "layoutSettings": {
                "permission": "pagesAndSheet",
                "marginUm": parameters.margin_um,
                "gapUm": parameters.gap_um,
                "minimumSideUm": parameters.minimum_side_um,
            },
            "visualDefaults": {
                "background": album_background.to_json(),
                "overlay": album_overlay.to_json(),
                "frameBorder": frame_border_json,
            },
            "media": media.to_json(),
            "mediaFolders": folders,
            "sheets": sheets,
        }
    });
    let bytes = serde_json::to_vec(&document).map_err(|_| LegacyFailure::Damaged)?;
    let mut revision = project_file::decode(&bytes).map_err(|_| LegacyFailure::Damaged)?;

    restore_layout_locks(&mut revision, &project.sheets, &sheet_ids, &mut notes);
    restore_favorites(
        &mut revision,
        &project,
        sheet_width_um,
        sheet_height_um,
        &mut ids,
        &mut notes,
    );
    Ok(Converted { revision, notes })
}

// ---------------------------------------------------------------------------
// Physical measures

/// The old Sheet size was pixels at the Project DPI, shown to the user in
/// centimetres with one decimal. Picks the roundest micrometre value (whole
/// millimetres first) whose canonical raster is the same pixel count, so
/// 5811 px at 300 DPI becomes 49,2 cm instead of 49,1998 cm; the full Sheet
/// width must also be even.
fn sheet_axis_um(pixels: f64, dpi: i64, even: bool) -> Result<u64, LegacyFailure> {
    if !pixels.is_finite() || pixels < 1.0 {
        return Err(LegacyFailure::Damaged);
    }
    let pixels = pixels.round() as i128;
    let exact = pixels as f64 * UM_PER_INCH / dpi as f64;
    let finest = if even { 2 } else { 1 };
    [1_000, 100, 10, finest]
        .into_iter()
        .find_map(|step: i128| {
            let nearest = (exact / step as f64).round() as i128;
            let mut candidates: Vec<i128> =
                (-2..=2).map(|delta| (nearest + delta) * step).collect();
            candidates.sort_by(|a, b| {
                (*a as f64 - exact)
                    .abs()
                    .total_cmp(&(*b as f64 - exact).abs())
            });
            candidates.into_iter().find(|candidate| {
                *candidate > 0 && raster_pixels(*candidate, dpi as i128) == pixels
            })
        })
        .and_then(|candidate| u64::try_from(candidate).ok())
        .filter(|candidate| *candidate <= MAX_SAFE_INTEGER)
        .ok_or(LegacyFailure::Damaged)
}

/// Same rounding as the canonical raster of the current program.
fn raster_pixels(micrometers: i128, dpi: i128) -> i128 {
    (micrometers * dpi + 12_700) / 25_400
}

fn cm_to_um(centimetres: f64) -> u64 {
    if centimetres.is_finite() && centimetres > 0.0 {
        (centimetres * UM_PER_CM).round() as u64
    } else {
        0
    }
}

/// Old borders were whole points. Rounded to a tenth of a millimetre, so the
/// panel shows 0,28 cm for 8 pt instead of 0,2822 cm; the exported border
/// keeps the same pixel width.
fn points_to_um(points: f64) -> u64 {
    const STEP_UM: f64 = 100.0;
    if points.is_finite() && points > 0.0 {
        let um = ((points * UM_PER_INCH / POINTS_PER_INCH) / STEP_UM).round() * STEP_UM;
        (um as u64).max(STEP_UM as u64)
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Sheet structure

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Active {
    Both,
    Left,
    Right,
}

impl Active {
    fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

fn active_sides(sheets: &[LegacySheet]) -> Result<Vec<Active>, LegacyFailure> {
    if sheets.len() < 2 {
        return Err(LegacyFailure::UnsupportedStructure { sheet_number: None });
    }
    let last = sheets.len() - 1;
    sheets
        .iter()
        .enumerate()
        .map(|(index, sheet)| {
            let active = match (sheet.left_page.enabled, sheet.right_page.enabled) {
                (true, true) => Some(Active::Both),
                (false, true) if index == 0 => Some(Active::Right),
                (true, false) if index == last => Some(Active::Left),
                _ => None,
            };
            active.ok_or(LegacyFailure::UnsupportedStructure {
                sheet_number: Some(index + 1),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Media

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Photo,
    Decorative,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Photo => "photo",
            Self::Decorative => "decorative",
        }
    }
}

/// Which old images each role needs: Frames need Photos, Background and
/// Overlay need Decoratives.
struct MediaUsage {
    photos: HashSet<String>,
    decoratives: HashSet<String>,
}

impl MediaUsage {
    fn collect(project: &LegacyProject) -> Self {
        let mut photos = HashSet::new();
        let mut decoratives = HashSet::new();
        let mut add_layer = |layer: Option<&LegacyLayer>| {
            if let Some(layer) = layer.filter(|layer| layer.kind == "image") {
                decoratives.insert(layer.value.clone());
            }
        };
        add_layer(project.defaults.default_background.as_ref());
        add_layer(project.defaults.default_overlay.as_ref());
        for sheet in &project.sheets {
            for set in [&sheet.background, &sheet.overlay] {
                if set.mode == "custom" {
                    add_layer(set.spread.as_ref());
                    add_layer(set.left.as_ref());
                    add_layer(set.right.as_ref());
                }
            }
            photos.extend(
                sheet
                    .frames
                    .iter()
                    .filter_map(|frame| frame.image_id.clone()),
            );
        }
        Self {
            photos,
            decoratives,
        }
    }

    fn kinds(&self, image: &LegacyImage) -> Vec<Kind> {
        let as_photo = self.photos.contains(&image.id);
        let as_decorative = self.decoratives.contains(&image.id);
        match (as_photo, as_decorative) {
            (false, false) if image.data.is_resource => vec![Kind::Decorative],
            (false, false) => vec![Kind::Photo],
            (true, false) => vec![Kind::Photo],
            (false, true) => vec![Kind::Decorative],
            (true, true) => vec![Kind::Photo, Kind::Decorative],
        }
    }
}

struct MediaEntry {
    id: Uuid,
    kind: Kind,
    path: PathBuf,
}

struct MediaCatalog {
    entries: Vec<MediaEntry>,
    by_old_id: HashMap<(String, Kind), Uuid>,
}

impl MediaCatalog {
    fn build(
        images: &[LegacyImage],
        usage: &MediaUsage,
        context: &ConversionContext<'_>,
        ids: &mut Ids,
        notes: &mut Vec<Note>,
    ) -> Self {
        let project_directory = context
            .project_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut entries = Vec::new();
        let mut by_old_id = HashMap::new();
        let mut by_path: HashMap<(Kind, String), Uuid> = HashMap::new();
        let mut missing = 0;
        let mut rejected = 0;
        for image in images {
            let Some((path, exists)) =
                resolve_image_path(&project_directory, image, context.exists)
            else {
                rejected += 1;
                continue;
            };
            if !exists {
                missing += 1;
            }
            for kind in usage.kinds(image) {
                let key = (kind, path.to_string_lossy().to_lowercase());
                let id = *by_path.entry(key).or_insert_with(|| {
                    let id = ids.derive(&format!("media:{}:{}", image.id, kind.as_str()));
                    entries.push(MediaEntry {
                        id,
                        kind,
                        path: path.clone(),
                    });
                    id
                });
                by_old_id.insert((image.id.clone(), kind), id);
            }
        }
        if missing > 0 {
            notes.push(Note::MissingImages { count: missing });
        }
        if rejected > 0 {
            notes.push(Note::ImagePathRejected { count: rejected });
        }
        Self { entries, by_old_id }
    }

    fn get(&self, old_id: &str, kind: Kind) -> Option<Uuid> {
        self.by_old_id.get(&(old_id.to_owned(), kind)).copied()
    }

    fn to_json(&self) -> Value {
        Value::Array(
            self.entries
                .iter()
                .map(|entry| {
                    json!({
                        "id": entry.id.hyphenated().to_string(),
                        "kind": entry.kind.as_str(),
                        "path": entry.path.to_string_lossy(),
                    })
                })
                .collect(),
        )
    }
}

/// The old program looked for an image beside the Project first, then at the
/// path where it was imported. Returns the first candidate that exists, or
/// the first acceptable one so the Project can show the image as missing.
fn resolve_image_path(
    project_directory: &Path,
    image: &LegacyImage,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<(PathBuf, bool)> {
    let relative = image
        .data
        .source_relative_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(|path| lexical_normal(&project_directory.join(windows_separators(path))));
    let original = Some(image.data.original_path.as_str())
        .filter(|path| !path.trim().is_empty())
        .map(|path| lexical_normal(Path::new(&windows_separators(path))));
    let candidates: Vec<PathBuf> = [relative, original]
        .into_iter()
        .flatten()
        .filter(|path| validate_external_path(path).is_ok())
        .collect();
    candidates
        .iter()
        .find(|path| exists(path))
        .map(|path| (path.clone(), true))
        .or_else(|| candidates.first().map(|path| (path.clone(), false)))
}

fn windows_separators(path: &str) -> String {
    path.replace('/', "\\")
}

/// Removes `.` and resolves `..` without touching the disk; `..` never climbs
/// above the drive or share root.
fn lexical_normal(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(parts.last(), Some(Component::Normal(_))) {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    parts.iter().map(|part| part.as_os_str()).collect()
}

fn media_folders(
    project: &LegacyProject,
    media: &MediaCatalog,
    ids: &mut Ids,
    notes: &mut Vec<Note>,
) -> Value {
    let mut names: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let declared = project.image_folders.iter();
    let used = project.images.iter().map(|image| &image.data.folder);
    for name in declared.chain(used) {
        let name = name.trim();
        if !name.is_empty() && seen.insert(name.to_lowercase()) {
            names.push(name.to_owned());
        }
    }

    let mut folders = Vec::new();
    let mut taken_names: HashSet<(Kind, String)> = HashSet::new();
    let mut assigned: HashSet<Uuid> = HashSet::new();
    for name in names {
        if !folder_name_is_valid(&name) {
            notes.push(Note::FolderSkipped { name });
            continue;
        }
        let mut members: HashMap<Kind, Vec<Uuid>> = HashMap::new();
        for image in project
            .images
            .iter()
            .filter(|image| image.data.folder.trim() == name)
        {
            for kind in [Kind::Photo, Kind::Decorative] {
                if let Some(id) = media.get(&image.id, kind)
                    && assigned.insert(id)
                {
                    members.entry(kind).or_default().push(id);
                }
            }
        }
        let kinds: Vec<Kind> = if members.is_empty() {
            vec![Kind::Photo]
        } else {
            [Kind::Photo, Kind::Decorative]
                .into_iter()
                .filter(|kind| members.contains_key(kind))
                .collect()
        };
        for kind in kinds {
            if !taken_names.insert((kind, name.to_lowercase())) {
                continue;
            }
            let id = ids.derive(&format!("folder:{}:{}", kind.as_str(), name));
            let mut folder = Map::new();
            folder.insert("id".into(), json!(id.hyphenated().to_string()));
            folder.insert("kind".into(), json!(kind.as_str()));
            folder.insert("name".into(), json!(name));
            if let Some(members) = members.get(&kind) {
                folder.insert(
                    "mediaIds".into(),
                    Value::Array(
                        members
                            .iter()
                            .map(|id| json!(id.hyphenated().to_string()))
                            .collect(),
                    ),
                );
            }
            folders.push(Value::Object(folder));
        }
    }
    Value::Array(folders)
}

/// Same rules as a folder named in the current program (design 0035).
fn folder_name_is_valid(name: &str) -> bool {
    name.chars().count() <= MAX_FOLDER_NAME_CHARS
        && !name.chars().any(char::is_control)
        && !["todas", "ausentes"].contains(&name.to_lowercase().as_str())
}

// ---------------------------------------------------------------------------
// Background, Overlay and Frame border

#[derive(Clone, Debug, PartialEq)]
enum Sides {
    Both(Value),
    PerSide(Value, Value),
}

impl Sides {
    fn to_json(&self) -> Value {
        match self {
            Self::Both(both) => json!({ "sides": "both", "both": both }),
            Self::PerSide(left, right) => {
                json!({ "sides": "perSide", "left": left, "right": right })
            }
        }
    }

    /// The value a side of a Sheet inherits when it omits its own. A whole
    /// Sheet image is not the same as that image on one side, so only a
    /// color spans both sides.
    fn side(&self, left: bool) -> Option<&Value> {
        match self {
            Self::PerSide(value, _) if left => Some(value),
            Self::PerSide(_, value) => Some(value),
            Self::Both(value) if value["kind"] == "color" => Some(value),
            Self::Both(_) => None,
        }
    }
}

fn album_sides(kind: &str, content: Value) -> Sides {
    if kind == "individual" {
        Sides::PerSide(content.clone(), content)
    } else {
        Sides::Both(content)
    }
}

fn background_content(
    layer: Option<&LegacyLayer>,
    media: &MediaCatalog,
    notes: &mut Vec<Note>,
) -> Value {
    let white = || json!({ "kind": "color", "rgb": WHITE });
    let Some(layer) = layer else {
        return white();
    };
    match layer.kind.as_str() {
        "color" => match canonical_rgb(&layer.value) {
            Some(rgb) => json!({ "kind": "color", "rgb": rgb }),
            None => {
                notes.push(Note::InvalidColor);
                white()
            }
        },
        "image" => match media.get(&layer.value, Kind::Decorative) {
            Some(id) => json!({ "kind": "media", "mediaId": id.hyphenated().to_string() }),
            None => {
                notes.push(Note::MissingDecorative);
                white()
            }
        },
        _ => {
            notes.push(Note::InvalidColor);
            white()
        }
    }
}

fn overlay_content(
    layer: Option<&LegacyLayer>,
    media: &MediaCatalog,
    notes: &mut Vec<Note>,
) -> Value {
    let none = || json!({ "kind": "none" });
    let Some(layer) = layer else {
        return none();
    };
    if layer.kind != "image" {
        notes.push(Note::OverlayColorDropped);
        return none();
    }
    match media.get(&layer.value, Kind::Decorative) {
        Some(id) => json!({ "kind": "media", "mediaId": id.hyphenated().to_string() }),
        None => {
            notes.push(Note::MissingDecorative);
            none()
        }
    }
}

fn sheet_background(
    set: &LegacyLayerSet,
    album: &Sides,
    media: &MediaCatalog,
    notes: &mut Vec<Note>,
) -> Option<Value> {
    sheet_layer(set, album, |layer| background_content(layer, media, notes))
}

fn sheet_overlay(
    set: &LegacyLayerSet,
    album: &Sides,
    media: &MediaCatalog,
    notes: &mut Vec<Note>,
) -> Option<Value> {
    sheet_layer(set, album, |layer| overlay_content(layer, media, notes))
}

/// A Sheet keeps only what differs from the Album: a Sheet that followed the
/// Album default inherits, and so does each side equal to the Album side.
fn sheet_layer(
    set: &LegacyLayerSet,
    album: &Sides,
    mut content: impl FnMut(Option<&LegacyLayer>) -> Value,
) -> Option<Value> {
    if set.mode != "custom" {
        return None;
    }
    let own = if set.kind == "individual" {
        Sides::PerSide(content(set.left.as_ref()), content(set.right.as_ref()))
    } else {
        Sides::Both(content(set.spread.as_ref()))
    };
    match own {
        Sides::Both(value) => (Sides::Both(value.clone()) != *album)
            .then(|| json!({ "sides": "both", "both": value })),
        Sides::PerSide(left, right) => {
            let mut sides = Map::new();
            sides.insert("sides".into(), json!("perSide"));
            if album.side(true) != Some(&left) {
                sides.insert("left".into(), json!({ "content": left }));
            }
            if album.side(false) != Some(&right) {
                sides.insert("right".into(), json!({ "content": right }));
            }
            (sides.len() > 1).then_some(Value::Object(sides))
        }
    }
}

fn album_frame_border(project: &LegacyProject) -> Option<(String, u64)> {
    let defaults = &project.defaults;
    let width_um = points_to_um(defaults.default_frame_border_width);
    (defaults.default_frame_border_enabled && width_um > 0).then(|| {
        (
            canonical_rgb(&defaults.default_frame_border_color).unwrap_or_else(|| WHITE.into()),
            width_um,
        )
    })
}

fn canonical_rgb(source: &str) -> Option<String> {
    let hex = source.trim().strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", hex.to_ascii_uppercase()))
}

// ---------------------------------------------------------------------------
// Frames and Photos

struct SheetScale {
    um_per_px: f64,
    width_um: u64,
    height_um: u64,
}

struct FramePlacement {
    sheet_number: usize,
    label: String,
    active: Active,
}

#[allow(clippy::too_many_arguments)]
fn convert_frame(
    frame: &LegacyFrame,
    placement: FramePlacement,
    scale: &SheetScale,
    images: &[LegacyImage],
    media: &MediaCatalog,
    album_border: Option<&(String, u64)>,
    ids: &mut Ids,
    notes: &mut Vec<Note>,
) -> Option<Value> {
    let sheet_number = placement.sheet_number;
    let geometry = frame.geometry;
    let k = scale.um_per_px;
    let (surface_x, surface_width) = match placement.active {
        Active::Both => (0.0, scale.width_um as f64),
        Active::Left => (0.0, (scale.width_um / 2) as f64),
        Active::Right => ((scale.width_um / 2) as f64, (scale.width_um / 2) as f64),
    };
    let surface_height = scale.height_um as f64;
    let left = geometry.x * k - surface_x;
    let top = geometry.y * k;
    let right = (geometry.x + geometry.width) * k - surface_x;
    let bottom = (geometry.y + geometry.height) * k;
    if ![left, top, right, bottom]
        .iter()
        .all(|value| value.is_finite())
    {
        notes.push(Note::FrameOutsideSheet { sheet_number });
        return None;
    }
    let x0 = left.max(0.0).round();
    let y0 = top.max(0.0).round();
    let x1 = right.min(surface_width).round();
    let y1 = bottom.min(surface_height).round();
    if x1 - x0 < 1.0 || y1 - y0 < 1.0 {
        notes.push(Note::FrameOutsideSheet { sheet_number });
        return None;
    }
    if left < -0.5 || top < -0.5 || right > surface_width + 0.5 || bottom > surface_height + 0.5 {
        notes.push(Note::FrameClipped { sheet_number });
    }
    let rect = RectUm {
        x: x0 as i64,
        y: y0 as i64,
        width: (x1 - x0) as i64,
        height: (y1 - y0) as i64,
    };

    let id = ids.keep_or_derive(&frame.id, &placement.label);
    let mut entry = Map::new();
    entry.insert("id".into(), json!(id.hyphenated().to_string()));
    entry.insert("xUm".into(), json!(rect.x));
    entry.insert("yUm".into(), json!(rect.y));
    entry.insert("widthUm".into(), json!(rect.width));
    entry.insert("heightUm".into(), json!(rect.height));
    if let Some(style) = frame_style(frame, album_border) {
        entry.insert("style".into(), style);
    }
    if let Some(image_id) = frame.image_id.as_deref() {
        match media.get(image_id, Kind::Photo) {
            Some(media_id) => {
                let source = frame.image_size.or_else(|| {
                    images
                        .iter()
                        .find(|image| image.id == image_id)
                        .map(|image| (image.data.width, image.data.height))
                });
                let transform = photo_transform(
                    frame,
                    &rect,
                    PhotoGeometry {
                        um_per_px: k,
                        surface_x,
                        source,
                        sheet_number,
                    },
                    notes,
                );
                entry.insert(
                    "photo".into(),
                    json!({ "mediaId": media_id.hyphenated().to_string(), "transform": transform }),
                );
            }
            None => notes.push(Note::MissingFrameImage { sheet_number }),
        }
    }
    Some(Value::Object(entry))
}

/// The old border was already the effective one of each Frame. A Frame
/// follows the Album only when it looks exactly like the Album default.
fn frame_style(frame: &LegacyFrame, album_border: Option<&(String, u64)>) -> Option<Value> {
    let width_um = points_to_um(frame.border.width);
    let own_rgb = canonical_rgb(&frame.border.color);
    let own = (width_um > 0).then(|| {
        (
            own_rgb
                .clone()
                .or_else(|| album_border.map(|(rgb, _)| rgb.clone()))
                .unwrap_or_else(|| WHITE.into()),
            width_um,
        )
    });
    let opacity = if frame.opacity.is_finite() {
        frame.opacity
    } else {
        1.0
    };
    let opacity_percent = (opacity * 100.0).round().clamp(0.0, 100.0) as u8;
    if own.as_ref() == album_border && opacity_percent == 100 {
        return None;
    }
    let rgb = own
        .as_ref()
        .map(|(rgb, _)| rgb.clone())
        .or(own_rgb)
        .or_else(|| album_border.map(|(rgb, _)| rgb.clone()))
        .unwrap_or_else(|| WHITE.into());
    Some(json!({
        "borderRgb": rgb,
        "borderWidthUm": own.map_or(0, |(_, width)| width),
        "opacityPercent": opacity_percent,
    }))
}

struct PhotoGeometry {
    um_per_px: f64,
    surface_x: f64,
    source: Option<(u32, u32)>,
    sheet_number: usize,
}

/// Keeps the Photo where the old program showed it: the same displayed size
/// and the same displayed center, expressed in the current Pan and Zoom.
fn photo_transform(
    frame: &LegacyFrame,
    rect: &RectUm,
    geometry: PhotoGeometry,
    notes: &mut Vec<Note>,
) -> Value {
    let sheet_number = geometry.sheet_number;
    let old = &frame.transform;
    let quarter_turns = (((old.rotation / 90.0).round() as i64).rem_euclid(4)) as i8;
    let requested_tenths = -(old.angle_offset * 10.0).round() as i64;
    let angle_tenths = requested_tenths.clamp(-MAX_FINE_ANGLE_TENTHS, MAX_FINE_ANGLE_TENTHS) as i16;
    if i64::from(angle_tenths) != requested_tenths {
        notes.push(Note::PhotoAngleLimited { sheet_number });
    }
    let black_and_white = match frame.filter.as_deref().map(str::trim) {
        None | Some("") | Some("none") => false,
        Some("pb") => true,
        Some(other) => {
            notes.push(Note::PhotoFilterDropped {
                sheet_number,
                filter: other.to_owned(),
            });
            false
        }
    };
    if !frame.adjustments.is_neutral() {
        notes.push(Note::PhotoAdjustmentsDropped { sheet_number });
    }
    let old_scale = if old.scale.is_finite() && old.scale > 0.0 {
        old.scale
    } else {
        1.0
    };

    let mut transform = MediaTransform {
        pan_x: 0.0,
        pan_y: 0.0,
        user_zoom: PHOTO_ZOOM_MIN,
        quarter_turns,
        fine_rotation_degrees: f32::from(angle_tenths) / 10.0,
        mirror_x: old.flip_horizontal,
        black_and_white,
    };
    let (pan_x, pan_y, zoom) = match geometry.source.filter(|(w, h)| *w > 0 && *h > 0) {
        Some(source) => {
            let old_fill = old_fill_scale(frame, source, old.rotation + old.angle_offset);
            let displayed_scale_um = old_fill * old_scale * geometry.um_per_px;
            let fill = photo_pan_basis(rect, &transform, source).fill_scale;
            let requested_zoom = displayed_scale_um / fill;
            let zoom = requested_zoom.clamp(f64::from(PHOTO_ZOOM_MIN), f64::from(PHOTO_ZOOM_MAX));
            if requested_zoom > f64::from(PHOTO_ZOOM_MAX) + ROUNDING {
                notes.push(Note::PhotoZoomLimited { sheet_number });
            }
            transform.user_zoom = zoom as f32;
            let basis = photo_pan_basis(rect, &transform, source);
            let k = geometry.um_per_px;
            let g = frame.geometry;
            let old_center_x = (g.x + g.width / 2.0 + old.pan_x) * k - geometry.surface_x;
            let old_center_y = (g.y + g.height / 2.0 + old.pan_y) * k;
            let dx = old_center_x - (rect.x as f64 + rect.width as f64 / 2.0);
            let dy = old_center_y - (rect.y as f64 + rect.height as f64 / 2.0);
            let pan = |axis: &crate::VectorUm, span: f64| {
                if span > 0.0 {
                    ((dx * axis.x + dy * axis.y) / (span / 2.0)).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            };
            (
                pan(&basis.horizontal, basis.horizontal_span),
                pan(&basis.vertical, basis.vertical_span),
                zoom,
            )
        }
        None => {
            if old_scale > f64::from(PHOTO_ZOOM_MAX) + ROUNDING {
                notes.push(Note::PhotoZoomLimited { sheet_number });
            }
            (
                0.0,
                0.0,
                old_scale.clamp(f64::from(PHOTO_ZOOM_MIN), f64::from(PHOTO_ZOOM_MAX)),
            )
        }
    };
    json!({
        "panX": micro(pan_x),
        "panY": micro(pan_y),
        "userZoom": micro(zoom),
        "quarterTurns": quarter_turns,
        "mirrorX": old.flip_horizontal,
        "angleTenths": angle_tenths,
        "blackAndWhite": black_and_white,
    })
}

/// `compute_fill_scale` of the old program: the smallest scale that covers
/// the old Frame at the old total rotation, in pixels per source pixel.
fn old_fill_scale(frame: &LegacyFrame, source: (u32, u32), angle_degrees: f64) -> f64 {
    let radians = angle_degrees.to_radians();
    let (sine, cosine) = (radians.sin().abs(), radians.cos().abs());
    let (width, height) = (frame.geometry.width, frame.geometry.height);
    let scale_width = (width * cosine + height * sine) / f64::from(source.0);
    let scale_height = (width * sine + height * cosine) / f64::from(source.1);
    scale_width.max(scale_height)
}

/// Rounding error of Pan and Zoom converted through micrometre Frames.
const ROUNDING: f64 = 1e-4;

/// The file keeps six decimals of Pan and Zoom. Values within a rounding
/// error of a limit or of neutral snap to it.
fn micro(value: f64) -> f64 {
    let snapped = [-1.0, 0.0, 1.0]
        .into_iter()
        .find(|target: &f64| (value - target).abs() < ROUNDING)
        .unwrap_or(value);
    (snapped * 1_000_000.0).round() / 1_000_000.0
}

// ---------------------------------------------------------------------------
// Layout locks and favorites

fn restore_layout_locks(
    revision: &mut ProjectRevision,
    sheets: &[LegacySheet],
    sheet_ids: &[Uuid],
    notes: &mut Vec<Note>,
) {
    if sheets.iter().all(|sheet| sheet.locked_layout_id.is_none()) {
        return;
    }
    let project = &revision.project;
    let mut lasts = Vec::with_capacity(sheets.len());
    let mut locks = Vec::with_capacity(sheets.len());
    for (index, (sheet, id)) in sheets.iter().zip(sheet_ids).enumerate() {
        let current = project
            .sheets()
            .iter()
            .find(|candidate| candidate.id() == *id);
        let locked = sheet.locked_layout_id.is_some()
            && current.is_some_and(|sheet| !sheet.frames().is_empty());
        let last = locked
            .then(|| project.current_layout(*id).ok())
            .flatten()
            .map(|layout| StoredLayout {
                definition: layout.definition,
                origin: LayoutOrigin::Custom,
            });
        if sheet.locked_layout_id.is_some() && last.is_none() {
            notes.push(Note::LayoutLockDropped {
                sheet_number: index + 1,
            });
        }
        locks.push(last.is_some());
        lasts.push(last);
    }
    let settings = project.layout_settings().clone();
    match project
        .clone()
        .restore_layout_state(settings, lasts)
        .and_then(|project| project.restore_layout_locks(locks))
    {
        Ok(project) => revision.project = project,
        Err(()) => notes.push(Note::LayoutLockDropped { sheet_number: 0 }),
    }
}

fn restore_favorites(
    revision: &mut ProjectRevision,
    project: &LegacyProject,
    sheet_width_um: u64,
    sheet_height_um: u64,
    ids: &mut Ids,
    notes: &mut Vec<Note>,
) {
    let mut favorites: Vec<FavoriteLayout> = Vec::new();
    let mut skipped = 0;
    for favorite_id in &project.favorites.favorite_ids {
        let definition = project
            .favorites
            .favorite_templates
            .get(favorite_id)
            .and_then(|template| {
                legacy_layout_definition(&template.frames, sheet_width_um, sheet_height_um)
            });
        let Some(definition) = definition else {
            skipped += 1;
            continue;
        };
        let layout = StoredLayout {
            definition,
            origin: LayoutOrigin::Custom,
        };
        if favorites
            .iter()
            .any(|item| LayoutRules::same_definition(&item.layout.definition, &layout.definition))
        {
            continue;
        }
        let Some(id) = LayoutFavoriteId::from_uuid(ids.derive(&format!("favorite:{favorite_id}")))
        else {
            skipped += 1;
            continue;
        };
        favorites.push(FavoriteLayout {
            id,
            order: favorites.len() as u64,
            layout,
        });
    }
    if !favorites.is_empty() {
        let count = favorites.len();
        match revision.project.clone().restore_favorite_layouts(favorites) {
            Ok(project) => revision.project = project,
            Err(()) => skipped += count,
        }
    }
    if skipped > 0 {
        notes.push(Note::FavoriteLayoutsDropped { count: skipped });
    }
}

/// Turns an old Layout, in percent of the whole Sheet, into a custom Layout
/// of a Sheet with these dimensions.
pub(crate) fn legacy_layout_definition(
    frames: &[LegacyTemplateFrame],
    sheet_width_um: u64,
    sheet_height_um: u64,
) -> Option<LayoutDefinition> {
    let width = sheet_width_um as f64;
    let height = sheet_height_um as f64;
    let positions: Vec<RectUm> = frames
        .iter()
        .filter_map(|frame| {
            let x0 = (frame.x / 100.0 * width).clamp(0.0, width).round();
            let y0 = (frame.y / 100.0 * height).clamp(0.0, height).round();
            let x1 = ((frame.x + frame.width) / 100.0 * width)
                .clamp(0.0, width)
                .round();
            let y1 = ((frame.y + frame.height) / 100.0 * height)
                .clamp(0.0, height)
                .round();
            (x1 - x0 >= 1.0 && y1 - y0 >= 1.0).then_some(RectUm {
                x: x0 as i64,
                y: y0 as i64,
                width: (x1 - x0) as i64,
                height: (y1 - y0) as i64,
            })
        })
        .collect();
    if positions.is_empty() || positions.len() != frames.len() {
        return None;
    }
    LayoutRules::capture_custom(
        LayoutSurface {
            kind: LayoutSurfaceKind::DoubleSheet,
            width_um: sheet_width_um as i64,
            height_um: sheet_height_um as i64,
        },
        positions,
    )
    .ok()
}

// ---------------------------------------------------------------------------
// Identities

/// Identifiers derived from the old file's content, so reading the same
/// bytes twice yields the same document. Old identifiers that are already
/// canonical UUID v4 are kept.
struct Ids {
    seed: [u8; 32],
    used: HashSet<Uuid>,
}

impl Ids {
    fn new(seed: [u8; 32]) -> Self {
        Self {
            seed,
            used: HashSet::new(),
        }
    }

    fn keep_or_derive(&mut self, old: &str, label: &str) -> Uuid {
        match Uuid::parse_str(old) {
            Ok(id)
                if id.get_version_num() == 4
                    && id.hyphenated().to_string() == old
                    && self.used.insert(id) =>
            {
                id
            }
            _ => self.derive(label),
        }
    }

    fn derive(&mut self, label: &str) -> Uuid {
        let mut attempt = 0_u32;
        loop {
            let id = derived_uuid(&self.seed, &format!("{label}#{attempt}"));
            if self.used.insert(id) {
                return id;
            }
            attempt += 1;
        }
    }
}

pub(super) fn derived_uuid(seed: &[u8], label: &str) -> Uuid {
    let digest = Sha256::new()
        .chain_update(seed)
        .chain_update(b"\n")
        .chain_update(label.as_bytes())
        .finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_store::legacy::reader::{
        LegacyAdjustments, LegacyBorder, LegacyGeometry, LegacyTransform,
    };

    const DPI: f64 = 300.0;

    fn old_frame(transform: LegacyTransform) -> LegacyFrame {
        LegacyFrame {
            id: String::new(),
            image_id: Some("img".into()),
            image_size: Some((4000, 2667)),
            geometry: LegacyGeometry {
                x: 400.0,
                y: 300.0,
                width: 1200.0,
                height: 1600.0,
            },
            border: LegacyBorder::default(),
            transform,
            adjustments: LegacyAdjustments::default(),
            filter: None,
            opacity: 1.0,
        }
    }

    /// `clamp_pan` of the old program: a saved Pan never uncovered the Frame.
    fn clamp_old_pan(frame: &mut LegacyFrame) {
        let t = &frame.transform;
        let source = frame.image_size.unwrap();
        let total = t.rotation + t.angle_offset;
        let scale = old_fill_scale(frame, source, total) * t.scale;
        let angle = if t.flip_horizontal { -total } else { total }.to_radians();
        let (sine, cosine) = (angle.sin(), angle.cos());
        let (width, height) = (frame.geometry.width, frame.geometry.height);
        let half_width = (width * cosine.abs() + height * sine.abs()) / 2.0;
        let half_height = (width * sine.abs() + height * cosine.abs()) / 2.0;
        let limit_x = (f64::from(source.0) * scale / 2.0 - half_width).max(0.0);
        let limit_y = (f64::from(source.1) * scale / 2.0 - half_height).max(0.0);
        let local_x = (t.pan_x * cosine + t.pan_y * sine).clamp(-limit_x, limit_x);
        let local_y = (-t.pan_x * sine + t.pan_y * cosine).clamp(-limit_y, limit_y);
        frame.transform.pan_x = local_x * cosine - local_y * sine;
        frame.transform.pan_y = local_x * sine + local_y * cosine;
    }

    /// Center and size of the displayed Photo in the old program, in µm of
    /// the Sheet (`rotation_geometry.py` and `frame_renderer.py`).
    fn old_display(frame: &LegacyFrame) -> ((f64, f64), (f64, f64)) {
        let k = UM_PER_INCH / DPI;
        let t = &frame.transform;
        let g = frame.geometry;
        let source = frame.image_size.unwrap();
        let scale = old_fill_scale(frame, source, t.rotation + t.angle_offset) * t.scale * k;
        (
            (
                (g.x + g.width / 2.0 + t.pan_x) * k,
                (g.y + g.height / 2.0 + t.pan_y) * k,
            ),
            (f64::from(source.0) * scale, f64::from(source.1) * scale),
        )
    }

    fn new_display(frame: &LegacyFrame) -> ((f64, f64), (f64, f64)) {
        let k = UM_PER_INCH / DPI;
        let g = frame.geometry;
        let rect = RectUm {
            x: (g.x * k).round() as i64,
            y: (g.y * k).round() as i64,
            width: ((g.x + g.width) * k).round() as i64 - (g.x * k).round() as i64,
            height: ((g.y + g.height) * k).round() as i64 - (g.y * k).round() as i64,
        };
        let value = photo_transform(
            frame,
            &rect,
            PhotoGeometry {
                um_per_px: k,
                surface_x: 0.0,
                source: frame.image_size,
                sheet_number: 1,
            },
            &mut Vec::new(),
        );
        let number = |key: &str| value[key].as_f64().unwrap();
        let transform = MediaTransform {
            pan_x: number("panX") as f32,
            pan_y: number("panY") as f32,
            user_zoom: number("userZoom") as f32,
            quarter_turns: value["quarterTurns"].as_i64().unwrap() as i8,
            fine_rotation_degrees: value["angleTenths"].as_i64().unwrap() as f32 / 10.0,
            mirror_x: value["mirrorX"].as_bool().unwrap(),
            black_and_white: false,
        };
        let source = frame.image_size.unwrap();
        let basis = photo_pan_basis(&rect, &transform, source);
        let pan_x = f64::from(transform.pan_x) * basis.horizontal_span / 2.0;
        let pan_y = f64::from(transform.pan_y) * basis.vertical_span / 2.0;
        let scale = basis.fill_scale * f64::from(transform.user_zoom);
        (
            (
                rect.x as f64
                    + rect.width as f64 / 2.0
                    + basis.horizontal.x * pan_x
                    + basis.vertical.x * pan_y,
                rect.y as f64
                    + rect.height as f64 / 2.0
                    + basis.horizontal.y * pan_x
                    + basis.vertical.y * pan_y,
            ),
            (f64::from(source.0) * scale, f64::from(source.1) * scale),
        )
    }

    #[test]
    fn the_photo_keeps_its_displayed_center_and_size() {
        for rotation in [0.0, 90.0, 180.0, 270.0] {
            for angle_offset in [0.0, 7.3, -12.0] {
                for flip_horizontal in [false, true] {
                    for scale in [1.0, 2.5] {
                        let mut frame = old_frame(LegacyTransform {
                            scale,
                            pan_x: 600.0,
                            pan_y: -450.0,
                            rotation,
                            angle_offset,
                            flip_horizontal,
                        });
                        clamp_old_pan(&mut frame);
                        let (old_center, old_size) = old_display(&frame);
                        let (new_center, new_size) = new_display(&frame);
                        let case = format!(
                            "rotation {rotation}, angle {angle_offset}, flip {flip_horizontal}, scale {scale}"
                        );
                        // One pixel at 300 DPI is 84.7 µm.
                        assert!(
                            (old_center.0 - new_center.0).abs() < 85.0,
                            "{case}: {old_center:?} {new_center:?}"
                        );
                        assert!(
                            (old_center.1 - new_center.1).abs() < 85.0,
                            "{case}: {old_center:?} {new_center:?}"
                        );
                        assert!(
                            (old_size.0 - new_size.0).abs() / old_size.0 < 1e-3,
                            "{case}: {old_size:?} {new_size:?}"
                        );
                        assert!(
                            (old_size.1 - new_size.1).abs() / old_size.1 < 1e-3,
                            "{case}: {old_size:?} {new_size:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sheet_measures_keep_the_old_pixel_count() {
        // 49,2 × 30,6 cm, as the old program showed them.
        assert_eq!(sheet_axis_um(5811.0, 300, true), Ok(492_000));
        assert_eq!(sheet_axis_um(3614.0, 300, false), Ok(306_000));
        assert_eq!(sheet_axis_um(5669.0, 300, true), Ok(480_000));
        assert_eq!(sheet_axis_um(7204.0, 300, true), Ok(609_900));
        assert_eq!(sheet_axis_um(2657.0, 300, false), Ok(225_000));
        // Without a whole millimetre for this pixel count, a finer value is used.
        let fine = sheet_axis_um(1.0, 1_200, true).unwrap();
        assert_eq!(raster_pixels(i128::from(fine), 1_200), 1);
        assert_eq!(sheet_axis_um(0.0, 300, true), Err(LegacyFailure::Damaged));
    }
}
