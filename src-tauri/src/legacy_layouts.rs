//! Custom Layouts of the old myAlbuns (`%APPDATA%\MyAlbuns\layouts`).
//!
//! The old library stored positions in percent of the whole Sheet, while the
//! current catalog only offers a Layout on a surface of the same proportion.
//! So each converted Project brings the old custom Layouts with its own Sheet
//! size, when its first save replaces the old file (ADR 0012). Only Layouts
//! the user created (`author: "user"`) are kept; the imported libraries came
//! with the old program.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use myalbuns_core::{LayoutCatalogSnapshot, legacy_layout_definition};
use serde::Deserialize;

use crate::layout_catalog_store::LayoutCatalogStore;

const USER_AUTHOR: &str = "user";

#[derive(Deserialize)]
struct OldTemplate {
    #[serde(default)]
    metadata: OldMetadata,
    #[serde(default)]
    frames: Vec<OldFrame>,
}

#[derive(Default, Deserialize)]
struct OldMetadata {
    #[serde(default)]
    author: String,
}

#[derive(Deserialize)]
struct OldFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// The templates folder of the old program on this machine.
pub(crate) fn old_library_templates() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|roaming| {
            PathBuf::from(roaming)
                .join("MyAlbuns")
                .join("layouts")
                .join("templates")
        })
        .filter(|path| path.is_dir())
}

/// Adds the old custom Layouts for a Sheet of this size. Returns the catalog
/// after the last addition, or `None` when nothing was added.
pub(crate) fn import_custom_layouts(
    store: &LayoutCatalogStore,
    templates: &Path,
    sheet_width_um: u64,
    sheet_height_um: u64,
) -> io::Result<Option<LayoutCatalogSnapshot>> {
    let mut entries: Vec<PathBuf> = fs::read_dir(templates)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    entries.sort();
    let mut latest = None;
    for path in entries {
        let Ok(template) = fs::read(&path)
            .map_err(|_| ())
            .and_then(|bytes| serde_json::from_slice::<OldTemplate>(&bytes).map_err(|_| ()))
        else {
            continue;
        };
        if template.metadata.author != USER_AUTHOR {
            continue;
        }
        let positions: Vec<_> = template
            .frames
            .iter()
            .map(|frame| (frame.x, frame.y, frame.width, frame.height))
            .collect();
        let Some(definition) =
            legacy_layout_definition(&positions, sheet_width_um, sheet_height_um)
        else {
            continue;
        };
        let (result, snapshot) = store.create(definition)?;
        if result.created {
            latest = Some(snapshot);
        }
    }
    Ok(latest)
}

#[cfg(test)]
mod tests {
    use myalbuns_paths::AppPaths;

    use super::*;

    fn template(author: &str, frames: &str) -> String {
        format!(
            r#"{{"metadata": {{"id": "x", "name": "Layout", "author": "{author}"}}, "frames": {frames}}}"#
        )
    }

    #[test]
    fn only_layouts_created_by_the_user_enter_the_catalog_once_per_proportion() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let store = LayoutCatalogStore::new(&paths);
        let templates = root.path().join("templates");
        fs::create_dir_all(&templates).unwrap();
        let two = r#"[{"x": 0, "y": 0, "width": 50, "height": 100},
            {"x": 50, "y": 0, "width": 50, "height": 100}]"#;
        fs::write(templates.join("custom.json"), template("user", two)).unwrap();
        fs::write(templates.join("shipped.json"), template("imported", two)).unwrap();
        fs::write(templates.join("broken.json"), b"{").unwrap();

        let imported = import_custom_layouts(&store, &templates, 491_998, 305_985)
            .unwrap()
            .expect("the custom Layout is added");
        assert_eq!(imported.entries.len(), 1);

        assert!(
            import_custom_layouts(&store, &templates, 491_998, 305_985)
                .unwrap()
                .is_none(),
            "the same proportion is not added twice"
        );
        let other = import_custom_layouts(&store, &templates, 600_000, 300_000)
            .unwrap()
            .expect("another proportion gets its own copy");
        assert_eq!(other.entries.len(), 2);
    }
}
