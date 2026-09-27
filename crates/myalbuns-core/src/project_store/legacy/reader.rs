//! Reads the SQLite database written by the old myAlbuns (formats 2.0–2.2).
//!
//! The database is deserialized from the exact bytes already read by the
//! store, so SQLite never opens the shared file itself. Values keep the old
//! program's units: pixels at the Project DPI, centimetres for margins and
//! points for Frame borders.
use std::collections::BTreeMap;

use rusqlite::{Connection, MAIN_DB, OptionalExtension};
use serde::Deserialize;
use serde_json::Value;

use super::LegacyFailure;

const CANONICAL_FORMATS: [&str; 3] = ["2.0", "2.1", "2.2"];
const OLD_FORMATS: [&str; 2] = ["1.0", "1.1"];
const CANONICAL_MODEL_VERSION: i64 = 2;

#[derive(Debug)]
pub(super) struct LegacyProject {
    pub(super) width_px: f64,
    pub(super) height_px: f64,
    pub(super) dpi: i64,
    pub(super) cut_margin_cm: f64,
    pub(super) safe_margin_cm: f64,
    pub(super) defaults: LegacyDefaults,
    pub(super) image_folders: Vec<String>,
    pub(super) favorites: LegacyFavorites,
    pub(super) images: Vec<LegacyImage>,
    pub(super) sheets: Vec<LegacySheet>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct LegacyDefaults {
    pub(super) default_background: Option<LegacyLayer>,
    pub(super) default_background_type: String,
    pub(super) default_overlay: Option<LegacyLayer>,
    pub(super) default_overlay_type: String,
    pub(super) default_frame_border_enabled: bool,
    pub(super) default_frame_border_width: f64,
    pub(super) default_frame_border_color: String,
}

/// One Background or Overlay value: a color or an image of the Project.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(super) struct LegacyLayer {
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct LegacyFavorites {
    pub(super) favorite_ids: Vec<String>,
    pub(super) favorite_templates: BTreeMap<String, LegacyTemplate>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LegacyTemplate {
    pub(crate) frames: Vec<LegacyTemplateFrame>,
}

/// A Layout position in percent of the whole Sheet.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LegacyTemplateFrame {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug)]
pub(super) struct LegacyImage {
    pub(super) id: String,
    pub(super) data: LegacyImageData,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct LegacyImageData {
    pub(super) source_relative_path: Option<String>,
    pub(super) original_path: String,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) is_resource: bool,
    pub(super) folder: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LegacySheet {
    #[serde(default)]
    pub(super) id: String,
    pub(super) left_page: LegacyPage,
    pub(super) right_page: LegacyPage,
    #[serde(default)]
    pub(super) frames: Vec<LegacyFrame>,
    #[serde(default)]
    pub(super) background: LegacyLayerSet,
    #[serde(default)]
    pub(super) overlay: LegacyLayerSet,
    #[serde(default)]
    pub(super) locked_layout_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LegacyPage {
    #[serde(default = "enabled")]
    pub(super) enabled: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct LegacyLayerSet {
    pub(super) mode: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) spread: Option<LegacyLayer>,
    pub(super) left: Option<LegacyLayer>,
    pub(super) right: Option<LegacyLayer>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LegacyFrame {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) image_id: Option<String>,
    #[serde(default)]
    pub(super) image_size: Option<(u32, u32)>,
    pub(super) geometry: LegacyGeometry,
    #[serde(default)]
    pub(super) border: LegacyBorder,
    #[serde(default)]
    pub(super) transform: LegacyTransform,
    #[serde(default)]
    pub(super) adjustments: LegacyAdjustments,
    #[serde(default)]
    pub(super) filter: Option<String>,
    #[serde(default = "opaque")]
    pub(super) opacity: f64,
}

fn opaque() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(super) struct LegacyGeometry {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub(super) struct LegacyBorder {
    pub(super) width: f64,
    pub(super) color: String,
}

impl Default for LegacyBorder {
    fn default() -> Self {
        Self {
            width: 0.0,
            color: "#ffffff".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub(super) struct LegacyTransform {
    pub(super) scale: f64,
    pub(super) pan_x: f64,
    pub(super) pan_y: f64,
    pub(super) rotation: f64,
    pub(super) angle_offset: f64,
    pub(super) flip_horizontal: bool,
}

impl Default for LegacyTransform {
    fn default() -> Self {
        Self {
            scale: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            angle_offset: 0.0,
            flip_horizontal: false,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct LegacyAdjustments {
    pub(super) brightness: f64,
    pub(super) contrast: f64,
    pub(super) saturation: f64,
}

impl LegacyAdjustments {
    pub(super) fn is_neutral(&self) -> bool {
        self.brightness == 0.0 && self.contrast == 0.0 && self.saturation == 0.0
    }
}

pub(super) fn read(bytes: &[u8]) -> Result<LegacyProject, LegacyFailure> {
    let connection = deserialize(bytes)?;
    // Older formats have other columns; recognize them before the full query.
    let format: Option<String> = connection
        .query_row("SELECT format_version FROM project_metadata", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|_| read_failure(&connection))?;
    if format
        .as_deref()
        .is_some_and(|format| OLD_FORMATS.contains(&format))
    {
        return Err(LegacyFailure::OldVersion);
    }
    let metadata = connection
        .query_row(
            "SELECT format_version, width, height, dpi, cut_margin, safe_margin, \
             default_settings, image_folders, layout_favorites_state, \
             canonical_model_version FROM project_metadata",
            [],
            |row| {
                Ok(MetadataRow {
                    format_version: row.get(0)?,
                    width: row.get(1)?,
                    height: row.get(2)?,
                    dpi: row.get(3)?,
                    cut_margin: row.get(4)?,
                    safe_margin: row.get(5)?,
                    default_settings: row.get(6)?,
                    image_folders: row.get(7)?,
                    layout_favorites_state: row.get(8)?,
                    canonical_model_version: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|_| read_failure(&connection))?
        .ok_or(LegacyFailure::Damaged)?;
    let format = metadata.format_version.as_str();
    if OLD_FORMATS.contains(&format) {
        return Err(LegacyFailure::OldVersion);
    }
    if !CANONICAL_FORMATS.contains(&format)
        || metadata.canonical_model_version != Some(CANONICAL_MODEL_VERSION)
    {
        return Err(LegacyFailure::Damaged);
    }

    Ok(LegacyProject {
        width_px: metadata.width,
        height_px: metadata.height,
        dpi: metadata.dpi,
        cut_margin_cm: metadata.cut_margin.unwrap_or(0.0),
        safe_margin_cm: metadata.safe_margin.unwrap_or(0.0),
        defaults: parse_optional_json(metadata.default_settings.as_deref())?.unwrap_or_default(),
        image_folders: parse_optional_json(metadata.image_folders.as_deref())?.unwrap_or_default(),
        // The old program also ignored unreadable favorites.
        favorites: parse_optional_json(metadata.layout_favorites_state.as_deref())
            .ok()
            .flatten()
            .unwrap_or_default(),
        images: read_images(&connection)?,
        sheets: read_sheets(&connection)?,
    })
}

struct MetadataRow {
    format_version: String,
    width: f64,
    height: f64,
    dpi: i64,
    cut_margin: Option<f64>,
    safe_margin: Option<f64>,
    default_settings: Option<String>,
    image_folders: Option<String>,
    layout_favorites_state: Option<String>,
    canonical_model_version: Option<i64>,
}

fn deserialize(bytes: &[u8]) -> Result<Connection, LegacyFailure> {
    let mut connection = Connection::open_in_memory().map_err(|_| LegacyFailure::Damaged)?;
    connection
        .deserialize_read_exact(MAIN_DB, bytes, bytes.len(), true)
        .map_err(|_| LegacyFailure::Damaged)?;
    let integrity: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|_| LegacyFailure::Damaged)?;
    if integrity != "ok" {
        return Err(LegacyFailure::Damaged);
    }
    Ok(connection)
}

fn read_images(connection: &Connection) -> Result<Vec<LegacyImage>, LegacyFailure> {
    let mut statement = connection
        .prepare("SELECT image_id, data FROM images ORDER BY rowid")
        .map_err(|_| read_failure(connection))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| LegacyFailure::Damaged)?;
    let mut images = Vec::new();
    for row in rows {
        let (id, data) = row.map_err(|_| LegacyFailure::Damaged)?;
        images.push(LegacyImage {
            id,
            data: serde_json::from_str(&data).map_err(|_| LegacyFailure::Damaged)?,
        });
    }
    Ok(images)
}

fn read_sheets(connection: &Connection) -> Result<Vec<LegacySheet>, LegacyFailure> {
    let mut statement = connection
        .prepare("SELECT data FROM laminas ORDER BY index_position")
        .map_err(|_| read_failure(connection))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| LegacyFailure::Damaged)?;
    let mut sheets = Vec::new();
    for row in rows {
        let data = row.map_err(|_| LegacyFailure::Damaged)?;
        sheets.push(serde_json::from_str(&data).map_err(|_| LegacyFailure::Damaged)?);
    }
    Ok(sheets)
}

fn parse_optional_json<T: for<'de> Deserialize<'de>>(
    source: Option<&str>,
) -> Result<Option<T>, LegacyFailure> {
    match source.map(str::trim) {
        None | Some("") => Ok(None),
        Some(source) => {
            let value: Value = serde_json::from_str(source).map_err(|_| LegacyFailure::Damaged)?;
            if value.is_null() {
                return Ok(None);
            }
            serde_json::from_value(value)
                .map(Some)
                .map_err(|_| LegacyFailure::Damaged)
        }
    }
}

/// A schema without the canonical tables belongs to a format this reader
/// does not know; everything else is damage.
fn read_failure(connection: &Connection) -> LegacyFailure {
    let has_tables = ["project_metadata", "images", "laminas"]
        .iter()
        .all(|table| {
            connection
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |_| Ok(()),
                )
                .is_ok()
        });
    if has_tables {
        LegacyFailure::Damaged
    } else {
        LegacyFailure::OldVersion
    }
}
