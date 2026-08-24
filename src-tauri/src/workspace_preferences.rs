use std::{collections::BTreeMap, fs, io, path::PathBuf, sync::Mutex};

use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(windows)]
use crate::preference_store_io::{CrossProcessPreferenceGuard, preference_mutex_name};

use crate::{
    ipc_contract::{
        MediaPreferenceKind, MediaThumbnailSizes, WorkspacePanelKind, WorkspacePanelPreference,
        WorkspacePanelPreferences, WorkspacePreferenceChange, WorkspacePreferences,
    },
    preference_store_io::write_atomically,
};

const SCHEMA_VERSION: u16 = 1;
const MIN_THUMBNAIL_SIZE: u16 = 58;
const MAX_THUMBNAIL_SIZE: u16 = 132;
const DEFAULT_THUMBNAIL_SIZE: u16 = 84;
const MIN_INSPECTOR_SIZE: u16 = 220;
const MAX_INSPECTOR_SIZE: u16 = 480;
const MIN_MEDIA_PANEL_SIZE: u16 = 120;
const MAX_MEDIA_PANEL_SIZE: u16 = 360;
const MAX_INSPECTOR_SECTIONS: usize = 64;

#[tauri::command]
pub(crate) fn workspace_preferences(
    store: State<'_, WorkspacePreferencesStore>,
) -> WorkspacePreferences {
    store.load()
}

#[tauri::command]
pub(crate) fn update_workspace_preference(
    store: State<'_, WorkspacePreferencesStore>,
    change: WorkspacePreferenceChange,
) -> Result<WorkspacePreferences, String> {
    store
        .update(change)
        .map_err(|_| "não foi possível atualizar as preferências da interface".to_owned())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspacePreferencesEnvelope {
    schema_version: u16,
    inspector_sections: BTreeMap<String, bool>,
    media_thumbnail_sizes: MediaThumbnailSizes,
    #[serde(default)]
    workspace_panels: WorkspacePanelPreferences,
}

pub(crate) struct WorkspacePreferencesStore {
    access: Mutex<()>,
    file: PathBuf,
    #[cfg(windows)]
    write_mutex_name: Vec<u16>,
}

impl WorkspacePreferencesStore {
    pub(crate) fn new(app_paths: &AppPaths) -> Self {
        #[cfg(windows)]
        let write_mutex_name =
            preference_mutex_name("WorkspacePreferences", app_paths.local_root());
        Self {
            access: Mutex::new(()),
            file: app_paths.workspace_preferences_file(),
            #[cfg(windows)]
            write_mutex_name,
        }
    }

    pub(crate) fn load(&self) -> WorkspacePreferences {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.load_unlocked()
    }

    pub(crate) fn update(
        &self,
        change: WorkspacePreferenceChange,
    ) -> Result<WorkspacePreferences, io::Error> {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[cfg(windows)]
        let _cross_process = CrossProcessPreferenceGuard::acquire(
            &self.write_mutex_name,
            "WorkspacePreferencesStore",
        )?;
        let mut preferences = self.load_unlocked();
        match change {
            WorkspacePreferenceChange::InspectorSection {
                preference_key,
                open,
            } => {
                if !valid_preference_key(&preference_key)
                    || (!preferences.inspector_sections.contains_key(&preference_key)
                        && preferences.inspector_sections.len() >= MAX_INSPECTOR_SECTIONS)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid Inspector preference key",
                    ));
                }
                preferences.inspector_sections.insert(preference_key, open);
            }
            WorkspacePreferenceChange::MediaThumbnailSize { media_kind, size } => {
                let size = size.clamp(MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE);
                match media_kind {
                    MediaPreferenceKind::Decorative => {
                        preferences.media_thumbnail_sizes.decorative = size;
                    }
                    MediaPreferenceKind::Photo => {
                        preferences.media_thumbnail_sizes.photo = size;
                    }
                }
            }
            WorkspacePreferenceChange::WorkspacePanelSize { panel, size } => {
                workspace_panel_mut(&mut preferences.workspace_panels, panel).size =
                    normalize_panel_size(panel, size);
            }
            WorkspacePreferenceChange::WorkspacePanelVisibility { panel, visible } => {
                workspace_panel_mut(&mut preferences.workspace_panels, panel).visible = visible;
            }
        }
        self.publish(&preferences)?;
        Ok(preferences)
    }

    fn load_unlocked(&self) -> WorkspacePreferences {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return default_preferences();
            }
            Err(_) => return default_preferences(),
        };
        let Ok(envelope) = serde_json::from_slice::<WorkspacePreferencesEnvelope>(&bytes) else {
            return default_preferences();
        };
        if envelope.schema_version != SCHEMA_VERSION
            || envelope.inspector_sections.len() > MAX_INSPECTOR_SECTIONS
            || !envelope
                .inspector_sections
                .keys()
                .all(|key| valid_preference_key(key))
        {
            return default_preferences();
        }
        WorkspacePreferences {
            inspector_sections: envelope.inspector_sections,
            media_thumbnail_sizes: MediaThumbnailSizes {
                decorative: envelope
                    .media_thumbnail_sizes
                    .decorative
                    .clamp(MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE),
                photo: envelope
                    .media_thumbnail_sizes
                    .photo
                    .clamp(MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE),
            },
            workspace_panels: WorkspacePanelPreferences {
                inspector: envelope.workspace_panels.inspector.map(|preference| {
                    WorkspacePanelPreference {
                        size: normalize_panel_size(WorkspacePanelKind::Inspector, preference.size),
                        visible: preference.visible,
                    }
                }),
                media: envelope
                    .workspace_panels
                    .media
                    .map(|preference| WorkspacePanelPreference {
                        size: normalize_panel_size(WorkspacePanelKind::Media, preference.size),
                        visible: preference.visible,
                    }),
            },
        }
    }

    fn publish(&self, preferences: &WorkspacePreferences) -> Result<(), io::Error> {
        let bytes = serde_json::to_vec_pretty(&WorkspacePreferencesEnvelope {
            schema_version: SCHEMA_VERSION,
            inspector_sections: preferences.inspector_sections.clone(),
            media_thumbnail_sizes: preferences.media_thumbnail_sizes,
            workspace_panels: preferences.workspace_panels,
        })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_atomically(&self.file, &bytes, "workspace-preferences.json")
    }
}

fn workspace_panel_mut(
    preferences: &mut WorkspacePanelPreferences,
    panel: WorkspacePanelKind,
) -> &mut WorkspacePanelPreference {
    let target = match panel {
        WorkspacePanelKind::Inspector => &mut preferences.inspector,
        WorkspacePanelKind::Media => &mut preferences.media,
    };
    target.get_or_insert_with(|| default_panel_preference(panel))
}

fn default_panel_preference(panel: WorkspacePanelKind) -> WorkspacePanelPreference {
    WorkspacePanelPreference {
        size: match panel {
            WorkspacePanelKind::Inspector => 310,
            WorkspacePanelKind::Media => 202,
        },
        visible: true,
    }
}

fn default_preferences() -> WorkspacePreferences {
    WorkspacePreferences {
        inspector_sections: BTreeMap::new(),
        media_thumbnail_sizes: MediaThumbnailSizes {
            decorative: DEFAULT_THUMBNAIL_SIZE,
            photo: DEFAULT_THUMBNAIL_SIZE,
        },
        workspace_panels: WorkspacePanelPreferences {
            inspector: None,
            media: None,
        },
    }
}

fn normalize_panel_size(panel: WorkspacePanelKind, size: u16) -> u16 {
    match panel {
        WorkspacePanelKind::Inspector => size.clamp(MIN_INSPECTOR_SIZE, MAX_INSPECTOR_SIZE),
        WorkspacePanelKind::Media => size.clamp(MIN_MEDIA_PANEL_SIZE, MAX_MEDIA_PANEL_SIZE),
    }
}

fn valid_preference_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs, sync::Arc, thread};

    use myalbuns_paths::AppPaths;

    use crate::ipc_contract::{
        MediaPreferenceKind, WorkspacePanelKind, WorkspacePanelPreference,
        WorkspacePreferenceChange,
    };

    use super::{WorkspacePreferencesStore, default_preferences};

    fn store() -> (tempfile::TempDir, AppPaths, WorkspacePreferencesStore) {
        let root = tempfile::tempdir().expect("temporary application data root");
        let paths = AppPaths::from_roots(root.path(), root.path(), root.path());
        let store = WorkspacePreferencesStore::new(&paths);
        (root, paths, store)
    }

    #[test]
    fn an_absent_state_file_uses_stable_defaults_without_materializing_it() {
        let (_root, paths, store) = store();

        assert_eq!(store.load(), default_preferences());
        assert!(!paths.workspace_preferences_file().exists());
    }

    #[test]
    fn updates_are_visible_to_independent_project_hosts_through_the_shared_state_file() {
        let (_root, paths, first_host) = store();

        first_host
            .update(WorkspacePreferenceChange::InspectorSection {
                preference_key: "album.design".into(),
                open: true,
            })
            .expect("the Inspector preference persists");
        first_host
            .update(WorkspacePreferenceChange::MediaThumbnailSize {
                media_kind: MediaPreferenceKind::Photo,
                size: 124,
            })
            .expect("the per-tab size persists");

        let second_host = WorkspacePreferencesStore::new(&paths);
        let loaded = second_host.load();
        assert_eq!(
            loaded.inspector_sections,
            BTreeMap::from([("album.design".into(), true)])
        );
        assert_eq!(loaded.media_thumbnail_sizes.photo, 124);
        assert_eq!(loaded.media_thumbnail_sizes.decorative, 84);
    }

    #[test]
    fn independent_hosts_serialize_read_modify_write_updates() {
        let (_root, paths, first_host) = store();
        let first_host = Arc::new(first_host);
        let second_host = Arc::new(WorkspacePreferencesStore::new(&paths));

        thread::scope(|scope| {
            for (prefix, host) in [("first", first_host), ("second", second_host)] {
                scope.spawn(move || {
                    for index in 0..24 {
                        host.update(WorkspacePreferenceChange::InspectorSection {
                            preference_key: format!("album.{prefix}.{index}"),
                            open: index % 2 == 0,
                        })
                        .expect("the shared State writer remains available");
                        thread::yield_now();
                    }
                });
            }
        });

        let stored = WorkspacePreferencesStore::new(&paths).load();
        assert_eq!(stored.inspector_sections.len(), 48);
        assert!(stored.inspector_sections.contains_key("album.first.23"));
        assert!(stored.inspector_sections.contains_key("album.second.23"));
    }

    #[test]
    fn invalid_or_future_state_falls_back_without_leaking_partial_values() {
        let (_root, paths, store) = store();
        let file = paths.workspace_preferences_file();
        fs::create_dir_all(file.parent().expect("State parent")).expect("State is writable");
        fs::write(
            &file,
            br#"{"schemaVersion":99,"inspectorSections":{"album.design":true},"mediaThumbnailSizes":{"decorative":110,"photo":124}}"#,
        )
        .expect("future state fixture is writable");

        assert_eq!(store.load(), default_preferences());
    }

    #[test]
    fn size_updates_are_clamped_and_invalid_section_keys_are_rejected() {
        let (_root, paths, store) = store();

        let updated = store
            .update(WorkspacePreferenceChange::MediaThumbnailSize {
                media_kind: MediaPreferenceKind::Decorative,
                size: u16::MAX,
            })
            .expect("out-of-range UI state is normalized");
        assert_eq!(updated.media_thumbnail_sizes.decorative, 132);

        assert!(
            store
                .update(WorkspacePreferenceChange::InspectorSection {
                    preference_key: "../outside".into(),
                    open: true,
                })
                .is_err()
        );
        assert_eq!(WorkspacePreferencesStore::new(&paths).load(), updated);
    }

    #[test]
    fn absent_panel_state_remains_distinguishable_until_legacy_migration_publishes_it() {
        let (_root, paths, store) = store();

        let initial = store.load();
        assert_eq!(initial.workspace_panels.inspector, None);
        assert_eq!(initial.workspace_panels.media, None);

        let inspector = store
            .update(WorkspacePreferenceChange::WorkspacePanelSize {
                panel: WorkspacePanelKind::Inspector,
                size: 350,
            })
            .expect("the migrated Inspector state persists");
        assert_eq!(
            inspector.workspace_panels.inspector,
            Some(WorkspacePanelPreference {
                size: 350,
                visible: true,
            })
        );
        assert_eq!(inspector.workspace_panels.media, None);

        let reloaded = WorkspacePreferencesStore::new(&paths).load();
        assert_eq!(reloaded.workspace_panels, inspector.workspace_panels);
    }

    #[test]
    fn panel_geometry_is_clamped_without_changing_visibility() {
        let (_root, _paths, store) = store();

        store
            .update(WorkspacePreferenceChange::WorkspacePanelVisibility {
                panel: WorkspacePanelKind::Inspector,
                visible: false,
            })
            .expect("the Inspector visibility persists");
        let inspector = store
            .update(WorkspacePreferenceChange::WorkspacePanelSize {
                panel: WorkspacePanelKind::Inspector,
                size: 1,
            })
            .expect("the Inspector state persists");
        assert_eq!(inspector.workspace_panels.inspector.unwrap().size, 220);
        assert!(!inspector.workspace_panels.inspector.unwrap().visible);

        let media = store
            .update(WorkspacePreferenceChange::WorkspacePanelSize {
                panel: WorkspacePanelKind::Media,
                size: u16::MAX,
            })
            .expect("the media state persists");
        assert_eq!(media.workspace_panels.media.unwrap().size, 360);
        assert!(media.workspace_panels.media.unwrap().visible);
    }

    #[test]
    fn independent_hosts_do_not_lose_size_or_visibility_on_the_same_panel() {
        let (_root, paths, first_host) = store();
        let first_host = Arc::new(first_host);
        let second_host = Arc::new(WorkspacePreferencesStore::new(&paths));

        thread::scope(|scope| {
            scope.spawn({
                let first_host = Arc::clone(&first_host);
                move || {
                    first_host
                        .update(WorkspacePreferenceChange::WorkspacePanelSize {
                            panel: WorkspacePanelKind::Inspector,
                            size: 350,
                        })
                        .expect("the Inspector size persists");
                }
            });
            scope.spawn(move || {
                second_host
                    .update(WorkspacePreferenceChange::WorkspacePanelVisibility {
                        panel: WorkspacePanelKind::Inspector,
                        visible: false,
                    })
                    .expect("the Inspector visibility persists");
            });
        });

        assert_eq!(
            WorkspacePreferencesStore::new(&paths)
                .load()
                .workspace_panels
                .inspector,
            Some(WorkspacePanelPreference {
                size: 350,
                visible: false,
            })
        );
    }
}
