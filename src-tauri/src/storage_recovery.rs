//! Native failure-scoped cache recovery. The UI receives an opaque attempt ID,
//! never a path capable of choosing which volume may be cleaned.
use crate::{cache_service::CacheService, ipc_contract::StorageRecovery};
use myalbuns_paths::StorageVolume;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tauri::{Emitter, Manager, State};

struct PausedStorage {
    id: String,
    volume: Option<StorageVolume>,
    cleanup_attempted: bool,
    export: Option<Box<crate::export_pipeline::AlbumExportRecovery>>,
}

#[derive(Clone, Default)]
pub(crate) struct StorageRecoveries(Arc<Mutex<HashMap<String, PausedStorage>>>, Arc<AtomicUsize>);

struct CleaningGuard<'a>(&'a AtomicUsize);
impl Drop for CleaningGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl StorageRecoveries {
    pub(crate) fn retain_export(
        &self,
        volume: Option<StorageVolume>,
        export: Option<Box<crate::export_pipeline::AlbumExportRecovery>>,
    ) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                "export".into(),
                PausedStorage {
                    id: uuid::Uuid::new_v4().to_string(),
                    volume,
                    cleanup_attempted: false,
                    export,
                },
            );
    }

    pub(crate) fn take_export(
        &self,
        id: &str,
    ) -> Result<Option<Box<crate::export_pipeline::AlbumExportRecovery>>, String> {
        let mut entries = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !entries.get("export").is_some_and(|paused| paused.id == id) {
            return Err("Esta exportação pausada não está mais disponível.".into());
        }
        Ok(entries.remove("export").and_then(|paused| paused.export))
    }
    pub(crate) fn is_cleaning(&self) -> bool {
        self.1.load(Ordering::Acquire) > 0
    }
    pub(crate) fn is_paused(&self, owner: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(owner)
    }
    pub(crate) fn pause(&self, owner: &str, volume: Option<StorageVolume>) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                owner.into(),
                PausedStorage {
                    id: uuid::Uuid::new_v4().to_string(),
                    volume,
                    cleanup_attempted: false,
                    export: None,
                },
            );
    }

    pub(crate) fn finish(&self, owner: &str) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(owner);
    }

    fn status(&self, owner: &str, cache: &CacheService) -> Option<StorageRecovery> {
        let entries = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let paused = entries.get(owner)?;
        let can_clear_cache = !paused.cleanup_attempted
            && paused.volume.as_ref().is_some_and(|volume| {
                cache
                    .recover_storage(volume, false)
                    .is_ok_and(|bytes| bytes > 0)
            });
        Some(StorageRecovery {
            id: paused.id.clone(),
            can_clear_cache,
        })
    }

    fn clear(&self, id: &str, cache: &CacheService) -> Result<bool, String> {
        self.1.fetch_add(1, Ordering::AcqRel);
        let _cleaning = CleaningGuard(&self.1);
        // Serializes cleanup with replacement/retirement of the failed attempt.
        let mut entries = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(paused) = entries
            .values_mut()
            .find(|entry| entry.id == id && !entry.cleanup_attempted)
        else {
            return Ok(false);
        };
        paused.cleanup_attempted = true;
        let Some(volume) = paused.volume.as_ref() else {
            return Ok(false);
        };
        cache
            .recover_storage(volume, true)
            .map(|bytes| bytes > 0)
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn pause_cache(app: &tauri::AppHandle) {
    let recoveries = app.state::<StorageRecoveries>();
    if !recoveries.is_paused("cache") {
        let namespace = app
            .state::<crate::cache_service::ActiveCacheNamespace>()
            .namespace();
        recoveries.pause("cache", StorageVolume::containing(namespace.paths().root()));
    }
    let _ = app.emit(
        "myalbuns://cache-processor-warning",
        crate::ipc_contract::CacheProcessorWarning {
            state: crate::ipc_contract::CacheProcessorState::StorageFull,
            message: "Libere espaço para continuar preparando as imagens.".into(),
        },
    );
}

#[tauri::command]
pub(crate) async fn resume_cache_images(
    app: tauri::AppHandle,
    on_progress: tauri::ipc::Channel<crate::ipc_contract::ImageProcessingProgress>,
) -> Result<bool, String> {
    let _operation = crate::project_ui_operations::begin(&app)?;
    let recoveries = app.state::<StorageRecoveries>();
    if !recoveries.is_paused("cache") {
        return Ok(true);
    }
    let catalog = app
        .state::<crate::project_host::ProjectHost>()
        .authorized_media_catalog()?;
    recoveries.finish("cache");
    let mut processing = crate::image_processing::ImageProcessingBatch::new(
        catalog.bindings.len() as u32,
        |progress| {
            let _ = on_progress.send(progress);
        },
    );
    processing.prepare_all(&app, catalog.bindings).await;
    Ok(!recoveries.is_paused("cache"))
}

#[tauri::command]
pub(crate) async fn storage_recovery_status(
    owner: String,
    recoveries: State<'_, StorageRecoveries>,
    cache: State<'_, CacheService>,
) -> Result<Option<StorageRecovery>, String> {
    let recoveries = recoveries.inner().clone();
    let cache = cache.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recoveries.status(&owner, &cache))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn clear_storage_recovery_cache(
    id: String,
    recoveries: State<'_, StorageRecoveries>,
    cache: State<'_, CacheService>,
) -> Result<bool, String> {
    let recoveries = recoveries.inner().clone();
    let cache = cache.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recoveries.clear(&id, &cache))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn discard_export_recovery(
    id: String,
    window: tauri::WebviewWindow,
    recoveries: State<'_, StorageRecoveries>,
) -> Result<(), String> {
    if window.label() != crate::product_runtime::PROJECT_WINDOW_LABEL {
        return Err("A Exportação pertence à Janela do Projeto.".into());
    }
    let recoveries = recoveries.inner().clone();
    tauri::async_runtime::spawn_blocking(move || recoveries.take_export(&id).map(drop))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext};

    #[test]
    fn storage_recovery_requires_the_current_failure_and_immediately_reclaimable_cache() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("roaming")).unwrap();
        std::fs::create_dir_all(root.path().join("local")).unwrap();
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let file = root.path().join("closed.myalbuns");
        let mut bindings = OperationPathContext::new();
        bindings.capture(&file).unwrap();
        let project = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(file, bindings.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let cache = CacheService::new(paths);
        let reservation = cache
            .reserve_namespace(project.identity_authority())
            .unwrap();
        let cache_path = reservation.namespace().paths().clone();
        std::fs::write(cache_path.media_directory().join("cached.bin"), [1; 16]).unwrap();
        let recoveries = StorageRecoveries::default();
        let volume = StorageVolume::containing(root.path());
        recoveries.pause("export", volume.clone());
        assert!(!recoveries.status("export", &cache).unwrap().can_clear_cache);
        drop(reservation);
        let first = recoveries.status("export", &cache).unwrap();
        assert!(first.can_clear_cache);
        recoveries.pause("export", None);
        assert!(
            !recoveries.clear(&first.id, &cache).unwrap(),
            "an obsolete attempt cannot clean"
        );
        assert!(
            !recoveries.status("export", &cache).unwrap().can_clear_cache,
            "unknown volumes do not authorize local cleanup"
        );
        assert!(cache_path.root().exists());
        recoveries.pause("export", volume);
        let current = recoveries.status("export", &cache).unwrap();
        assert!(recoveries.clear(&current.id, &cache).unwrap());
        assert!(!cache_path.root().exists());
        assert!(!recoveries.clear(&current.id, &cache).unwrap());
        assert!(!recoveries.status("export", &cache).unwrap().can_clear_cache);
        recoveries.finish("export");
        assert!(recoveries.status("export", &cache).is_none());
    }
}
