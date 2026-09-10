use std::{fs, io, path::PathBuf, sync::Mutex};

use myalbuns_core::{
    CustomLayout, CustomLayoutId, LayoutCatalogSnapshot, LayoutDefinition, LayoutRules,
    LayoutScope, LayoutSurface, LayoutSurfaceKind, RectUm, SaveCustomLayoutResult,
};
use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};

use crate::local_store_io::write_atomically;
#[cfg(windows)]
use crate::local_store_io::{CrossProcessStoreGuard, store_mutex_name};

const SCHEMA_VERSION: u16 = 1;

/// Serializes global catalog writes across Project hosts and only publishes
/// complete revisions. Read failures never turn confirmed user content empty.
pub(crate) struct LayoutCatalogStore {
    confirmed: Mutex<Option<LayoutCatalogSnapshot>>,
    file: PathBuf,
    #[cfg(windows)]
    write_mutex_name: Vec<u16>,
}

impl LayoutCatalogStore {
    pub(crate) fn new(paths: &AppPaths) -> Self {
        Self {
            confirmed: Mutex::new(None),
            file: paths.layouts_dir().join("catalog.json"),
            #[cfg(windows)]
            write_mutex_name: store_mutex_name("Layouts", paths.roaming_root()),
        }
    }

    pub(crate) fn load(&self) -> io::Result<LayoutCatalogSnapshot> {
        let mut confirmed = self.confirmed.lock().map_err(|_| unavailable())?;
        let snapshot = self.read(confirmed.as_ref())?;
        *confirmed = Some(snapshot.clone());
        Ok(snapshot)
    }

    pub(crate) fn create(
        &self,
        definition: LayoutDefinition,
    ) -> io::Result<(SaveCustomLayoutResult, LayoutCatalogSnapshot)> {
        let mut confirmed = self.confirmed.lock().map_err(|_| unavailable())?;
        #[cfg(windows)]
        let _writer =
            CrossProcessStoreGuard::acquire(&self.write_mutex_name, "LayoutCatalogStore")?;
        let mut snapshot = self.read(confirmed.as_ref())?;
        if let Some(existing) = snapshot
            .entries
            .iter()
            .find(|item| LayoutRules::same_definition(&item.definition, &definition))
        {
            let result = SaveCustomLayoutResult {
                catalog_revision: snapshot.revision,
                layout_id: existing.id,
                created: false,
            };
            *confirmed = Some(snapshot.clone());
            return Ok((result, snapshot));
        }
        let id = CustomLayoutId::generate();
        snapshot.entries.push(CustomLayout { id, definition });
        advance_revision(&mut snapshot)?;
        self.publish(&snapshot)?;
        *confirmed = Some(snapshot.clone());
        Ok((
            SaveCustomLayoutResult {
                catalog_revision: snapshot.revision,
                layout_id: id,
                created: true,
            },
            snapshot,
        ))
    }

    pub(crate) fn delete(&self, id: CustomLayoutId) -> io::Result<LayoutCatalogSnapshot> {
        let mut confirmed = self.confirmed.lock().map_err(|_| unavailable())?;
        #[cfg(windows)]
        let _writer =
            CrossProcessStoreGuard::acquire(&self.write_mutex_name, "LayoutCatalogStore")?;
        let mut snapshot = self.read(confirmed.as_ref())?;
        if let Some(index) = snapshot.entries.iter().position(|item| item.id == id) {
            snapshot.entries.remove(index);
            advance_revision(&mut snapshot)?;
            self.publish(&snapshot)?;
        }
        *confirmed = Some(snapshot.clone());
        Ok(snapshot)
    }

    fn read(&self, confirmed: Option<&LayoutCatalogSnapshot>) -> io::Result<LayoutCatalogSnapshot> {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && confirmed.is_none_or(|previous| previous.revision == 0) =>
            {
                return Ok(LayoutCatalogSnapshot::default());
            }
            Err(error) => return Err(error),
        };
        let envelope: CatalogEnvelope = serde_json::from_slice(&bytes).map_err(invalid)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(invalid("unsupported Layout catalog schema"));
        }
        let snapshot = envelope.into_snapshot();
        if !snapshot.is_valid()
            || confirmed.is_some_and(|previous| {
                snapshot.revision < previous.revision
                    || snapshot.revision == previous.revision && snapshot != *previous
            })
        {
            return Err(invalid("invalid Layout catalog revision or content"));
        }
        Ok(snapshot)
    }

    fn publish(&self, snapshot: &LayoutCatalogSnapshot) -> io::Result<()> {
        if !snapshot.is_valid() {
            return Err(invalid("invalid Layout catalog"));
        }
        let bytes = serde_json::to_vec_pretty(&CatalogEnvelope::from_snapshot(snapshot))
            .map_err(invalid)?;
        write_atomically(&self.file, &bytes, "catalog.json")
    }
}

fn advance_revision(snapshot: &mut LayoutCatalogSnapshot) -> io::Result<()> {
    snapshot.revision = snapshot
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("catalog revision overflow"))?;
    if !snapshot.is_valid() {
        return Err(invalid("invalid Layout catalog"));
    }
    Ok(())
}

fn unavailable() -> io::Error {
    io::Error::other("LayoutCatalogStore is unavailable")
}
fn invalid(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

// This schema is independent of the Project file and the IPC representation.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogEnvelope {
    schema_version: u16,
    revision: u64,
    entries: Vec<CatalogEntryV1>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogEntryV1 {
    id: CustomLayoutId,
    definition: CatalogDefinitionV1,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDefinitionV1 {
    surface: CatalogSurfaceV1,
    scope: LayoutScope,
    positions: Vec<CatalogRectV1>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogSurfaceV1 {
    #[serde(rename = "type")]
    kind: LayoutSurfaceKind,
    width_um: i64,
    height_um: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CatalogRectV1 {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

impl CatalogEnvelope {
    fn from_snapshot(snapshot: &LayoutCatalogSnapshot) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: snapshot.revision,
            entries: snapshot
                .entries
                .iter()
                .map(|item| CatalogEntryV1 {
                    id: item.id,
                    definition: CatalogDefinitionV1 {
                        surface: CatalogSurfaceV1 {
                            kind: item.definition.surface.kind,
                            width_um: item.definition.surface.width_um,
                            height_um: item.definition.surface.height_um,
                        },
                        scope: item.definition.scope,
                        positions: item
                            .definition
                            .positions
                            .iter()
                            .map(|r| CatalogRectV1 {
                                x: r.x,
                                y: r.y,
                                width: r.width,
                                height: r.height,
                            })
                            .collect(),
                    },
                })
                .collect(),
        }
    }

    fn into_snapshot(self) -> LayoutCatalogSnapshot {
        LayoutCatalogSnapshot {
            revision: self.revision,
            entries: self
                .entries
                .into_iter()
                .map(|item| CustomLayout {
                    id: item.id,
                    definition: LayoutDefinition {
                        surface: LayoutSurface {
                            kind: item.definition.surface.kind,
                            width_um: item.definition.surface.width_um,
                            height_um: item.definition.surface.height_um,
                        },
                        scope: item.definition.scope,
                        positions: item
                            .definition
                            .positions
                            .into_iter()
                            .map(|r| RectUm {
                                x: r.x,
                                y: r.y,
                                width: r.width,
                                height: r.height,
                            })
                            .collect(),
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, AppPaths, LayoutCatalogStore) {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let store = LayoutCatalogStore::new(&paths);
        (root, paths, store)
    }

    fn definition() -> LayoutDefinition {
        LayoutRules::capture_custom(
            LayoutSurface {
                kind: LayoutSurfaceKind::SinglePage,
                width_um: 300_000,
                height_um: 240_000,
            },
            vec![
                RectUm {
                    x: 20_000,
                    y: 30_000,
                    width: 70_000,
                    height: 100_000,
                },
                RectUm {
                    x: 120_000,
                    y: 30_000,
                    width: 90_000,
                    height: 150_000,
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn creation_deduplication_and_deletion_survive_independent_hosts() {
        let (_root, paths, store) = fixture();
        assert_eq!(store.load().unwrap(), LayoutCatalogSnapshot::default());
        assert!(!paths.layouts_dir().exists());
        let (created, snapshot) = store.create(definition()).unwrap();
        assert!(created.created);
        assert_eq!(created.catalog_revision, 1);
        assert_eq!(LayoutCatalogStore::new(&paths).load().unwrap(), snapshot);
        let before = fs::read(&store.file).unwrap();
        let (duplicate, same) = store.create(definition()).unwrap();
        assert!(!duplicate.created);
        assert_eq!(duplicate.layout_id, created.layout_id);
        assert_eq!(same, snapshot);
        assert_eq!(fs::read(&store.file).unwrap(), before);
        let mut reordered = definition();
        reordered.positions.reverse();
        let (second, two) = LayoutCatalogStore::new(&paths).create(reordered).unwrap();
        assert!(second.created);
        assert_eq!(two.revision, 2);
        assert_eq!(store.load().unwrap(), two);
        let deleted = store.delete(created.layout_id).unwrap();
        assert_eq!(deleted.revision, 3);
        assert_eq!(deleted.entries.len(), 1);
        assert_eq!(deleted.entries[0].id, second.layout_id);
        assert_eq!(LayoutCatalogStore::new(&paths).load().unwrap(), deleted);
        assert_eq!(store.delete(created.layout_id).unwrap(), deleted);
    }

    #[test]
    fn corrupt_future_and_missing_catalogs_never_replace_confirmed_content() {
        let (_root, paths, store) = fixture();
        let (created, _) = store.create(definition()).unwrap();
        let before = fs::read(&store.file).unwrap();
        let mut invalid = vec![
            b"broken".to_vec(),
            br#"{"schemaVersion":99,"revision":2,"entries":[]}"#.to_vec(),
        ];
        for id in [
            "not-a-uuid",
            "550E8400-E29B-41D4-A716-446655440000",
            "00000000-0000-1000-8000-000000000001",
        ] {
            let mut document: serde_json::Value = serde_json::from_slice(&before).unwrap();
            document["entries"][0]["id"] = id.into();
            invalid.push(serde_json::to_vec(&document).unwrap());
            assert!(serde_json::from_value::<CustomLayoutId>(id.into()).is_err());
        }
        for bytes in invalid {
            fs::write(&store.file, &bytes).unwrap();
            assert!(store.load().is_err());
            assert!(store.create(definition()).is_err());
            assert!(store.delete(created.layout_id).is_err());
            assert_eq!(fs::read(&store.file).unwrap(), bytes);
            assert!(
                LayoutCatalogStore::new(&paths)
                    .create(definition())
                    .is_err()
            );
        }
        fs::remove_file(&store.file).unwrap();
        assert!(store.load().is_err());
        assert!(store.create(definition()).is_err());
        assert!(!store.file.exists());
        fs::write(&store.file, before).unwrap();
        assert_eq!(store.load().unwrap().entries[0].id, created.layout_id);
    }

    #[cfg(windows)]
    #[test]
    fn a_failed_replacement_keeps_the_last_confirmed_revision() {
        let (_root, _paths, store) = fixture();
        let (created, snapshot) = store.create(definition()).unwrap();
        let before = fs::read(&store.file).unwrap();
        let original_permission = fs::metadata(&store.file).unwrap().permissions();
        let mut permission = original_permission.clone();
        permission.set_readonly(true);
        fs::set_permissions(&store.file, permission).unwrap();
        assert!(store.delete(created.layout_id).is_err());
        assert_eq!(fs::read(&store.file).unwrap(), before);
        assert_eq!(store.load().unwrap(), snapshot);
        fs::set_permissions(&store.file, original_permission).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn concurrent_hosts_serialize_creations_without_losing_items() {
        let (_root, paths, _store) = fixture();
        std::thread::scope(|scope| {
            for index in 0..8 {
                let store = LayoutCatalogStore::new(&paths);
                scope.spawn(move || {
                    let mut layout = definition();
                    layout.positions[0].width += index;
                    store.create(layout).unwrap();
                });
            }
        });
        let snapshot = LayoutCatalogStore::new(&paths).load().unwrap();
        assert_eq!(snapshot.revision, 8);
        assert_eq!(snapshot.entries.len(), 8);
    }
}
