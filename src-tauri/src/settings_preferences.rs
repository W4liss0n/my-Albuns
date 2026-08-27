use std::{fs, io, path::PathBuf, sync::Mutex};

use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};
use tauri::State;

#[cfg(windows)]
use crate::preference_store_io::{CrossProcessPreferenceGuard, preference_mutex_name};
use crate::{
    ipc_contract::{
        ApplicationSettings, MediaPanelSettings, MediaPanelTabSettings, MediaPreferenceKind,
        MediaSortDirection, MediaUsageFilter, SettingsPreferenceChange,
    },
    preference_store_io::write_atomically,
};

const SCHEMA_VERSION: u16 = 1;

#[tauri::command]
pub(crate) fn application_settings(store: State<'_, SettingsStore>) -> ApplicationSettings {
    store.load()
}

#[tauri::command]
pub(crate) fn update_application_setting(
    store: State<'_, SettingsStore>,
    change: SettingsPreferenceChange,
) -> Result<ApplicationSettings, String> {
    store
        .update(change)
        .map_err(|_| "não foi possível atualizar as configurações do aplicativo".to_owned())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsEnvelope {
    schema_version: u16,
    media_panel: MediaPanelSettings,
}

pub(crate) struct SettingsStore {
    access: Mutex<()>,
    file: PathBuf,
    #[cfg(windows)]
    write_mutex_name: Vec<u16>,
}

impl SettingsStore {
    pub(crate) fn new(app_paths: &AppPaths) -> Self {
        #[cfg(windows)]
        let write_mutex_name = preference_mutex_name("Settings", app_paths.roaming_root());
        Self {
            access: Mutex::new(()),
            file: app_paths.settings_file(),
            #[cfg(windows)]
            write_mutex_name,
        }
    }

    pub(crate) fn load(&self) -> ApplicationSettings {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.load_unlocked()
    }

    pub(crate) fn update(
        &self,
        change: SettingsPreferenceChange,
    ) -> Result<ApplicationSettings, io::Error> {
        let _access = self
            .access
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[cfg(windows)]
        let _cross_process =
            CrossProcessPreferenceGuard::acquire(&self.write_mutex_name, "SettingsStore")?;
        let mut settings = self.load_unlocked();
        match change {
            SettingsPreferenceChange::MediaPanelSortDirection {
                media_kind,
                sort_direction,
            } => {
                media_panel_tab_mut(&mut settings.media_panel, media_kind).sort_direction =
                    sort_direction;
            }
            SettingsPreferenceChange::MediaPanelUsageFilter {
                media_kind,
                usage_filter,
            } => {
                media_panel_tab_mut(&mut settings.media_panel, media_kind).usage_filter =
                    usage_filter;
            }
        }
        self.publish(&settings)?;
        Ok(settings)
    }

    fn load_unlocked(&self) -> ApplicationSettings {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return default_settings(),
            Err(_) => return default_settings(),
        };
        let Ok(envelope) = serde_json::from_slice::<SettingsEnvelope>(&bytes) else {
            return default_settings();
        };
        if envelope.schema_version != SCHEMA_VERSION {
            return default_settings();
        }
        ApplicationSettings {
            media_panel: envelope.media_panel,
        }
    }

    fn publish(&self, settings: &ApplicationSettings) -> Result<(), io::Error> {
        let bytes = serde_json::to_vec_pretty(&SettingsEnvelope {
            schema_version: SCHEMA_VERSION,
            media_panel: settings.media_panel,
        })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_atomically(&self.file, &bytes, "settings.json")
    }
}

fn media_panel_tab_mut(
    settings: &mut MediaPanelSettings,
    media_kind: MediaPreferenceKind,
) -> &mut MediaPanelTabSettings {
    match media_kind {
        MediaPreferenceKind::Decorative => &mut settings.decorative,
        MediaPreferenceKind::Photo => &mut settings.photo,
    }
}

fn default_settings() -> ApplicationSettings {
    let tab = MediaPanelTabSettings {
        sort_direction: MediaSortDirection::Ascending,
        usage_filter: MediaUsageFilter::All,
    };
    ApplicationSettings {
        media_panel: MediaPanelSettings {
            decorative: tab,
            photo: tab,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, thread};

    use myalbuns_paths::AppPaths;

    use crate::ipc_contract::{
        MediaPreferenceKind, MediaSortDirection, MediaUsageFilter, SettingsPreferenceChange,
    };

    use super::{SettingsStore, default_settings};

    fn store() -> (tempfile::TempDir, AppPaths, SettingsStore) {
        let root = tempfile::tempdir().expect("temporary application data root");
        let paths = AppPaths::from_roots(&root.path().join("Roaming"), &root.path().join("Local"));
        let store = SettingsStore::new(&paths);
        (root, paths, store)
    }

    #[test]
    fn absent_or_future_settings_use_stable_defaults_without_materializing_the_file() {
        let (_root, paths, store) = store();

        assert_eq!(store.load(), default_settings());
        assert!(!paths.settings_file().exists());

        fs::create_dir_all(paths.settings_file().parent().unwrap()).unwrap();
        fs::write(
            paths.settings_file(),
            br#"{"schemaVersion":99,"mediaPanel":{"decorative":{"sortDirection":"descending","usageFilter":"used"},"photo":{"sortDirection":"descending","usageFilter":"unused"}}}"#,
        )
        .unwrap();
        assert_eq!(store.load(), default_settings());
    }

    #[test]
    fn sorting_and_usage_filter_persist_independently_per_tab_in_roaming_settings() {
        let (_root, paths, first_host) = store();

        first_host
            .update(SettingsPreferenceChange::MediaPanelSortDirection {
                media_kind: MediaPreferenceKind::Photo,
                sort_direction: MediaSortDirection::Descending,
            })
            .expect("photo sorting persists");
        first_host
            .update(SettingsPreferenceChange::MediaPanelUsageFilter {
                media_kind: MediaPreferenceKind::Photo,
                usage_filter: MediaUsageFilter::Used,
            })
            .expect("photo usage filter persists");

        let loaded = SettingsStore::new(&paths).load();
        assert_eq!(
            loaded.media_panel.photo.sort_direction,
            MediaSortDirection::Descending
        );
        assert_eq!(
            loaded.media_panel.photo.usage_filter,
            MediaUsageFilter::Used
        );
        assert_eq!(
            loaded.media_panel.decorative,
            default_settings().media_panel.decorative
        );
        assert!(paths.settings_file().starts_with(paths.roaming_root()));
    }

    #[test]
    fn independent_hosts_do_not_lose_independent_fields_on_the_same_tab() {
        let (_root, paths, first_host) = store();
        let first_host = Arc::new(first_host);
        let second_host = Arc::new(SettingsStore::new(&paths));

        thread::scope(|scope| {
            scope.spawn({
                let first_host = Arc::clone(&first_host);
                move || {
                    first_host
                        .update(SettingsPreferenceChange::MediaPanelSortDirection {
                            media_kind: MediaPreferenceKind::Photo,
                            sort_direction: MediaSortDirection::Descending,
                        })
                        .expect("photo sorting persists");
                }
            });
            scope.spawn(move || {
                second_host
                    .update(SettingsPreferenceChange::MediaPanelUsageFilter {
                        media_kind: MediaPreferenceKind::Photo,
                        usage_filter: MediaUsageFilter::Unused,
                    })
                    .expect("photo usage filter persists");
            });
        });

        let loaded = SettingsStore::new(&paths).load();
        assert_eq!(
            loaded.media_panel.photo.sort_direction,
            MediaSortDirection::Descending
        );
        assert_eq!(
            loaded.media_panel.photo.usage_filter,
            MediaUsageFilter::Unused
        );
    }
}
