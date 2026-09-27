//! Projects of the old myAlbuns (a SQLite database with the same `.myalbuns`
//! extension). Opening one converts it in memory; nothing is written until
//! the first `Salvar`, which replaces the old file with the current format
//! (ADR 0012).
mod convert;
mod reader;

use std::path::Path;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{LayoutDefinition, project_document::ProjectRevision};

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Why an old Project could not be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LegacyFailure {
    /// The old program is writing the file: a SQLite journal is beside it.
    InUse,
    /// Formats 1.0 and 1.1, which the old program itself migrated on save.
    OldVersion,
    /// A Sheet arrangement the current Album cannot hold.
    UnsupportedStructure {
        sheet_number: Option<usize>,
    },
    Damaged,
}

/// Something the conversion changed or dropped. Recorded only in the
/// diagnostic log (decided on 26/09/2026); issues #133–#135 track the
/// features that would remove the most relevant ones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyConversionNote {
    SafetyInsideBleed,
    MissingImages { count: usize },
    ImagePathRejected { count: usize },
    FolderSkipped { name: String },
    FrameClipped { sheet_number: usize },
    FrameOutsideSheet { sheet_number: usize },
    MissingFrameImage { sheet_number: usize },
    PhotoZoomLimited { sheet_number: usize },
    PhotoAngleLimited { sheet_number: usize },
    PhotoFilterDropped { sheet_number: usize, filter: String },
    PhotoAdjustmentsDropped { sheet_number: usize },
    OverlayColorDropped,
    MissingDecorative,
    InvalidColor,
    LayoutLockDropped { sheet_number: usize },
    FavoriteLayoutsDropped { count: usize },
}

pub(crate) struct LegacyConversion {
    pub(crate) revision: ProjectRevision,
    pub(crate) notes: Vec<LegacyConversionNote>,
}

pub(crate) fn is_legacy_project(bytes: &[u8]) -> bool {
    bytes.starts_with(SQLITE_HEADER)
}

/// A non-empty rollback journal or write-ahead log means the old program has
/// the database open for writing; its bytes may not be complete yet.
pub(crate) fn has_active_journal(project_path: &Path) -> bool {
    ["-journal", "-wal"].iter().any(|suffix| {
        let mut sidecar = project_path.as_os_str().to_owned();
        sidecar.push(suffix);
        std::fs::metadata(&sidecar).is_ok_and(|metadata| metadata.len() > 0)
    })
}

/// Names one old file at one location. The pending Identity is kept under
/// this key until the first save, so reopening the same unsaved Project
/// finds its Recovery again, while an identical copy elsewhere gets its own.
pub(crate) fn identity_key(bytes: &[u8], project_path: &Path) -> String {
    let content = Sha256::digest(bytes);
    let location = project_path.to_string_lossy().to_lowercase();
    format!(
        "{:x}",
        Sha256::new()
            .chain_update(content)
            .chain_update(b"\n")
            .chain_update(location.as_bytes())
            .finalize()
    )
}

/// An Identity for a read-only load, stable for the same file and location.
pub(crate) fn derived_identity(key: &str) -> Uuid {
    convert::derived_uuid(key.as_bytes(), "project")
}

pub(crate) fn convert(
    bytes: &[u8],
    project_path: &Path,
    project_id: Uuid,
    exists: &dyn Fn(&Path) -> bool,
) -> Result<LegacyConversion, LegacyFailure> {
    let project = reader::read(bytes)?;
    let converted = convert::convert(
        project,
        &convert::ConversionContext {
            project_path,
            project_id,
            seed: Sha256::digest(bytes).into(),
            exists,
        },
    )?;
    Ok(LegacyConversion {
        revision: converted.revision,
        notes: converted.notes,
    })
}

/// Converts an old custom Layout (positions in percent of the whole Sheet)
/// for a Sheet with these dimensions. The Host uses it to bring the old
/// global library into the current catalog.
pub fn legacy_layout_definition(
    positions_percent: &[(f64, f64, f64, f64)],
    sheet_width_um: u64,
    sheet_height_um: u64,
) -> Option<LayoutDefinition> {
    let frames: Vec<reader::LegacyTemplateFrame> = positions_percent
        .iter()
        .map(|&(x, y, width, height)| reader::LegacyTemplateFrame {
            x,
            y,
            width,
            height,
        })
        .collect();
    convert::legacy_layout_definition(&frames, sheet_width_um, sheet_height_um)
}
