use crate::{
    cache_activity_gate::CacheCancellation,
    cache_engine::{
        AuthorizedCacheNamespace, CacheDemandMediaUpdate, CacheDemandRevision, CacheEngine,
    },
    cache_previews::CachePreviewRegistry,
    imaging_processor::ImagingProcessor,
    media_runtime::{
        MediaBinding, MediaMonitor, MediaMonitorPoll, MediaResolutionProposal, MediaResolver,
        MediaRuntime,
    },
    project_host::ProjectHost,
};
use std::collections::HashSet;

use futures_util::{StreamExt, stream};
use myalbuns_core::MediaKind;
use myalbuns_logging::safe_log_identifier;
use tauri::Manager;

#[cfg(test)]
pub(crate) async fn refresh_project_media_with_capacity(
    host: &ProjectHost,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    bindings: &[MediaBinding],
    update: &crate::media_runtime::MediaRuntimeUpdate,
    roots: &myalbuns_paths::RootBindingPlan,
) -> Vec<String> {
    refresh_project_media(
        host,
        engine,
        processor,
        bindings,
        update,
        roots,
        &HashSet::new(),
    )
    .await
}

/// `decoded_next` names the media whose Original the caller hands to the
/// Processor right after. Their header is enough here: decoding them in the
/// Host too made a Project without Cache decode every photo twice, one at a
/// time, before its window could open. A full inspection still reserves the
/// whole Processor, so those remain one at a time.
async fn refresh_project_media(
    host: &ProjectHost,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    bindings: &[MediaBinding],
    update: &crate::media_runtime::MediaRuntimeUpdate,
    roots: &myalbuns_paths::RootBindingPlan,
    decoded_next: &HashSet<String>,
) -> Vec<String> {
    let mut results = stream::iter(
        changed_media_for_update(bindings, update)
            .into_iter()
            .enumerate(),
    )
    .map(|(index, binding)| async move {
        let result = if decoded_next.contains(&binding.media_id) {
            refresh_media_header(host, &binding, roots).await
        } else {
            refresh_media_source(host, engine, processor, &binding, roots).await
        };
        (index, binding, result)
    })
    .buffer_unordered(processor.cache_capacity().max(1))
    .collect::<Vec<_>>()
    .await;
    results.sort_unstable_by_key(|(index, _, _)| *index);
    results
        .into_iter()
        .filter_map(|(_, binding, result)| match result {
            Ok(()) => Some(binding.media_id),
            Err(error) => {
                tracing::info!(
                    target: "myalbuns.desktop",
                    media_id = safe_log_identifier(&binding.media_id),
                    error = %error,
                    event = "photo_source_refresh_deferred",
                );
                None
            }
        })
        .collect()
}

async fn refresh_media_header(
    host: &ProjectHost,
    binding: &MediaBinding,
    roots: &myalbuns_paths::RootBindingPlan,
) -> Result<(), String> {
    let host = host.clone();
    let binding = binding.clone();
    let roots = roots.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let observation = MediaResolver.observe_in_plan(&roots, &binding);
        if host.adopt_imported_photo_inspection(&binding, &observation) {
            return Ok(());
        }
        let metadata = MediaResolver.inspect_media_header_in_plan(&binding, &roots)?;
        if !observation.same_source(&MediaResolver.observe_in_plan(&roots, &binding)) {
            return Err("A origem mudou durante a inspeção da imagem.".to_owned());
        }
        confirm_inspected_binding(&host, &binding, metadata)
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn refresh_media_source(
    host: &ProjectHost,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    binding: &MediaBinding,
    roots: &myalbuns_paths::RootBindingPlan,
) -> Result<(), String> {
    use crate::imaging_processor::{ImageMemoryEstimate, ProcessorAdmissionFailure};

    let cancellation = CacheCancellation::default();
    loop {
        let admission =
            match crate::image_work_admission::ImageWorkAdmission::begin(engine, &cancellation)
                .await
            {
                Ok(admission) => admission,
                Err(error) => break Err(error.to_string()),
            };
        let observing_host = host.clone();
        let observing_binding = binding.clone();
        let observing_roots = roots.clone();
        let observation = tauri::async_runtime::spawn_blocking(move || {
            let observation = MediaResolver.observe_in_plan(&observing_roots, &observing_binding);
            if observing_host.adopt_imported_photo_inspection(&observing_binding, &observation) {
                return None;
            }
            let estimate = ImageMemoryEstimate::in_plan(
                &observing_roots,
                [observing_binding.logical_path.as_path()],
            );
            Some((observation, estimate))
        })
        .await;
        let (observation, estimate) = match observation {
            Ok(Some(value)) => value,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error.to_string()),
        };
        let lease = match admission
            .reserve(
                processor,
                estimate,
                crate::image_work_admission::ImageWorkKind::Inspection,
            )
            .await
        {
            Ok(lease) => lease,
            Err(ProcessorAdmissionFailure::Cancelled) => continue,
            Err(error) => break Err(error.to_string()),
        };
        let inspecting_host = host.clone();
        let inspecting_binding = binding.clone();
        let inspecting_roots = roots.clone();
        // Drain a started decoder before releasing either reservation. The
        // caller has already released its observation permit, so Export can
        // pause a resource wait without holding a nested activity alive.
        let inspected = tauri::async_runtime::spawn_blocking(move || {
            let metadata = MediaResolver
                .inspect_media_binding_in_plan(&inspecting_binding, &inspecting_roots)?;
            let current = MediaResolver.observe_in_plan(&inspecting_roots, &inspecting_binding);
            if !observation.same_source(&current) {
                return Err("A origem mudou durante a inspeção da imagem.".to_owned());
            }
            confirm_inspected_binding(&inspecting_host, &inspecting_binding, metadata)
        })
        .await;
        drop(lease);
        break inspected
            .map_err(|error| error.to_string())
            .and_then(|value| value);
    }
}

fn confirm_inspected_binding(
    host: &ProjectHost,
    binding: &MediaBinding,
    metadata: myalbuns_core::PhotoSourceMetadata,
) -> Result<(), String> {
    if binding.kind == MediaKind::Photo {
        host.observe_photo_source(binding, metadata)
    } else if host.authorized_media_catalog()?.bindings.contains(binding) {
        Ok(())
    } else {
        Err("A referência do Decorativo mudou durante a inspeção.".into())
    }
}

pub(crate) fn changed_media_for_update(
    bindings: &[MediaBinding],
    update: &crate::media_runtime::MediaRuntimeUpdate,
) -> Vec<MediaBinding> {
    let changed = update
        .changed_media_ids()
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    bindings
        .iter()
        .filter(|binding| changed.contains(binding.media_id.as_str()))
        .cloned()
        .collect()
}

pub(crate) struct ConfirmedMediaUpdate {
    pub(crate) poll: MediaMonitorPoll,
    pub(crate) refreshed_media_ids: Vec<String>,
    pub(crate) cache_update: Option<CacheDemandMediaUpdate>,
}

/// Coordinates observations and cache epochs before exposing a confirmed result.
pub(crate) struct MediaConfirmation<'a> {
    host: &'a ProjectHost,
    engine: &'a CacheEngine,
    processor: &'a ImagingProcessor,
    runtime: &'a MediaRuntime,
    monitor: &'a MediaMonitor,
    registry: &'a CachePreviewRegistry,
    namespace: &'a AuthorizedCacheNamespace,
    decoded_next: HashSet<String>,
}

impl<'a> MediaConfirmation<'a> {
    pub(crate) fn for_app(
        app: &'a tauri::AppHandle,
        namespace: &'a AuthorizedCacheNamespace,
    ) -> Self {
        Self {
            host: app.state::<ProjectHost>().inner(),
            engine: app.state::<CacheEngine>().inner(),
            processor: app.state::<ImagingProcessor>().inner(),
            runtime: app.state::<MediaRuntime>().inner(),
            monitor: app.state::<MediaMonitor>().inner(),
            registry: app.state::<CachePreviewRegistry>().inner(),
            namespace,
            decoded_next: HashSet::new(),
        }
    }

    /// Media the caller prepares through the Processor right after this
    /// confirmation; see `refresh_project_media`.
    pub(crate) fn decoded_next(mut self, media_ids: impl IntoIterator<Item = String>) -> Self {
        self.decoded_next = media_ids.into_iter().collect();
        self
    }

    pub(crate) async fn confirm(
        &self,
        bindings: &[MediaBinding],
        roots: &myalbuns_paths::RootBindingPlan,
        prepared: Option<MediaResolutionProposal>,
        demand: Option<&mut CacheDemandRevision>,
    ) -> Result<ConfirmedMediaUpdate, String> {
        let runtime = self.runtime;
        let engine = self.engine;
        let refreshed_media_ids = if let Some(proposal) = prepared.as_ref() {
            refresh_project_media(
                self.host,
                engine,
                self.processor,
                bindings,
                &proposal.inspection_update(runtime),
                roots,
                &self.decoded_next,
            )
            .await
        } else {
            Vec::new()
        };
        let permit = engine
            .begin_cancellable_work(CacheCancellation::default())
            .await;
        let monitor = self.monitor.clone();
        let runtime = self.runtime.clone();
        let bindings = bindings.to_vec();
        let roots = roots.clone();
        let readable = refreshed_media_ids.clone();
        let poll = tauri::async_runtime::spawn_blocking(move || {
            prepared.map_or_else(
                || MediaMonitorPoll::unchanged(&runtime),
                |proposal| {
                    monitor.commit_prepared(&runtime, proposal, &bindings, &roots, &readable)
                },
            )
        })
        .await
        .map_err(|error| error.to_string())?;
        let cache_update = poll.update().and_then(|update| {
            let registry = self.registry;
            match demand {
                Some(_)
                    if update.changed_media_ids().is_empty()
                        && update.invalidated_media_ids().is_empty() =>
                {
                    None
                }
                Some(demand) => {
                    Some(engine.apply_demand_media_update(self.namespace, registry, demand, update))
                }
                None => {
                    engine.apply_monitor_media_update(self.namespace, registry, update);
                    None
                }
            }
        });
        drop(permit);
        Ok(ConfirmedMediaUpdate {
            poll,
            refreshed_media_ids,
            cache_update,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, ImportPhoto, InitialProject,
        PhotoSourceMetadata, ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext};
    use std::time::Duration;

    #[test]
    fn media_decoded_next_by_the_processor_are_confirmed_from_their_header() {
        let root = tempfile::tempdir().unwrap();
        let photo = root.path().join("Foto.png");
        let project_path = root.path().join("Projeto.myalbuns");
        RgbImage::from_pixel(17, 11, Rgb([50, 60, 70]))
            .save_with_format(&photo, ImageFormat::Png)
            .unwrap();
        // A readable header over a damaged body: only a full decode notices it.
        let mut bytes = std::fs::read(&photo).unwrap();
        let data = bytes.windows(4).position(|chunk| chunk == b"IDAT").unwrap() + 4;
        bytes[data + 2] ^= 0xFF;
        std::fs::write(&photo, bytes).unwrap();
        let mut context = OperationPathContext::new();
        context.capture(&project_path).unwrap();
        context.capture(&photo).unwrap();
        let roots = context.freeze();
        let mut project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, roots.clone()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        project
            .import_photo(ImportPhoto::new(
                photo,
                PhotoSourceMetadata::new(
                    1,
                    1,
                    ["#102030".into(), "#405060".into(), "#708090".into()],
                )
                .unwrap(),
            ))
            .unwrap();
        let host = ProjectHost::new(project);
        let bindings = host.authorized_media_catalog().unwrap().bindings;
        let engine = CacheEngine::default();
        let processor = ImagingProcessor::default();
        let monitor = MediaMonitor::default();
        let runtime = MediaRuntime::default();
        monitor.prepare_in_plan(&runtime, &bindings, &roots);
        let update = monitor
            .prepare_in_plan(&runtime, &bindings, &roots)
            .unwrap()
            .inspection_update(&runtime);
        let media_id = bindings[0].media_id.clone();

        tauri::async_runtime::block_on(async {
            let inspected = refresh_project_media(
                &host,
                &engine,
                &processor,
                &bindings,
                &update,
                &roots,
                &HashSet::new(),
            )
            .await;
            assert!(inspected.is_empty(), "a full inspection rejects the body");

            let confirmed = refresh_project_media(
                &host,
                &engine,
                &processor,
                &bindings,
                &update,
                &roots,
                &HashSet::from([media_id.clone()]),
            )
            .await;
            assert_eq!(confirmed, [media_id]);
        });
        assert_eq!(
            host.projection().unwrap().state.album.media[0].source_width_px,
            Some(17)
        );
    }

    #[test]
    fn confirmation_returns_after_cache_invalidation_and_preserves_demand_ownership() {
        for mode in ["monitor", "current-demand", "superseded-demand"] {
            let root = tempfile::tempdir().unwrap();
            let photo = root.path().join("Foto.jpg");
            let project_path = root.path().join("Projeto.myalbuns");
            RgbImage::from_pixel(17, 11, Rgb([50, 60, 70]))
                .save_with_format(&photo, ImageFormat::Jpeg)
                .unwrap();
            let mut context = OperationPathContext::new();
            context.capture(&project_path).unwrap();
            context.capture(&photo).unwrap();
            let roots = context.freeze();
            let mut project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(project_path, roots.clone()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            project
                .import_photo(ImportPhoto::new(
                    photo,
                    PhotoSourceMetadata::new(
                        1,
                        1,
                        ["#102030".into(), "#405060".into(), "#708090".into()],
                    )
                    .unwrap(),
                ))
                .unwrap();
            let app_paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            let namespace =
                AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority()).unwrap();
            let host = ProjectHost::new(project);
            let bindings = host.authorized_media_catalog().unwrap().bindings;
            let engine = CacheEngine::default();
            let processor = ImagingProcessor::default();
            let monitor = MediaMonitor::default();
            let runtime = MediaRuntime::default();
            let registry = CachePreviewRegistry::new("project");
            let mut demand = engine.reconcile_preview_demand(
                &registry,
                namespace.project_id(),
                1,
                [bindings[0].media_id.as_str()],
            );
            let old_epoch = demand.clone();
            if mode == "superseded-demand" {
                engine.reconcile_preview_demand(
                    &registry,
                    namespace.project_id(),
                    2,
                    [bindings[0].media_id.as_str()],
                );
            }
            monitor.prepare_in_plan(&runtime, &bindings, &roots);
            let prepared = monitor
                .prepare_in_plan(&runtime, &bindings, &roots)
                .unwrap();
            let owner = MediaConfirmation {
                host: &host,
                engine: &engine,
                processor: &processor,
                monitor: &monitor,
                runtime: &runtime,
                registry: &registry,
                namespace: &namespace,
                decoded_next: HashSet::new(),
            };
            tauri::async_runtime::block_on(async {
                let confirmed = owner
                    .confirm(
                        &bindings,
                        &roots,
                        Some(prepared),
                        (mode != "monitor").then_some(&mut demand),
                    )
                    .await
                    .unwrap();
                assert!(
                    confirmed
                        .poll
                        .update()
                        .unwrap()
                        .changed_media_ids()
                        .contains(&bindings[0].media_id)
                );
                assert_eq!(
                    confirmed.refreshed_media_ids,
                    [bindings[0].media_id.clone()]
                );
                assert_eq!(
                    host.projection().unwrap().state.album.media[0].source_width_px,
                    Some(17)
                );
                assert!(!engine.demand_is_current(&old_epoch));
                if let Some(update) = confirmed.cache_update {
                    assert_eq!(update.demand_can_resume(), mode == "current-demand");
                    assert_eq!(update.retry_required(), mode == "superseded-demand");
                } else {
                    assert_eq!(mode, "monitor");
                }
                assert_eq!(engine.demand_is_current(&demand), mode == "current-demand");
                // A caller can retain the result while Export takes exclusivity.
                let _pause = tokio::time::timeout(Duration::from_secs(5), engine.pause())
                    .await
                    .unwrap();
                assert!(confirmed.poll.update().is_some());
            });
        }
    }
}
