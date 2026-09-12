use futures_util::{StreamExt, stream};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        self, CacheEngine, CacheFailure, CacheFailureStage, CacheFlightClaim, CacheWork,
        PendingCachePublication,
    },
    cache_service::ActiveCacheNamespace,
    imaging_processor::{
        ImageMemoryEstimate, ImagingProcessor, InvocationContext, InvocationFailureStage,
        ProcessorAdmissionFailure, TauriImagingTransport,
    },
    logging::LoggingState,
    media_runtime::MediaBinding,
    project_host::ProjectHost,
};
use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::{AppPaths, OperationPathContext, RootBindingPlan};

/// Before a Project identity exists, a selected decorative keeps its validated
/// encoded preview only for the lifetime of the provisional selection.
pub(crate) fn prepare_provisional_image(
    file: std::fs::File,
) -> Result<crate::opaque_image_protocol::ImagePayload, crate::opaque_image_protocol::ImageReadError>
{
    use crate::opaque_image_protocol::{ImageReadError, read_image};
    let preview = read_image(file, true)?;
    image::ImageReader::new(std::io::Cursor::new(&preview.body))
        .with_guessed_format()
        .map_err(|_| ImageReadError::ReadFailed)?
        .decode()
        .map_err(|_| ImageReadError::UnsupportedImage)?;
    Ok(preview)
}

pub(crate) async fn prepare_changed_images(
    app: &AppHandle,
    previous_bindings: &[MediaBinding],
    publish: impl FnMut(crate::ipc_contract::ImageProcessingProgress),
) -> Result<(), String> {
    let catalog = app.state::<ProjectHost>().authorized_media_catalog()?;
    let bindings = images_requiring_preparation(previous_bindings, &catalog.bindings);
    if !bindings.is_empty() {
        let mut batch = ImageProcessingBatch::new(bindings.len() as u32, publish);
        batch.prepare_all(app, bindings).await;
    }
    Ok(())
}

fn images_requiring_preparation(
    previous_bindings: &[MediaBinding],
    current_bindings: &[MediaBinding],
) -> Vec<MediaBinding> {
    // Placement changes composition, not the imported source. Canvas demand
    // owns any missing preview; creative mutations must not wait for Cache.
    if previous_bindings == current_bindings {
        return Vec::new();
    }
    current_bindings
        .iter()
        .filter(|binding| !previous_bindings.contains(binding))
        .cloned()
        .collect()
}

pub(crate) struct ImageProcessingBatch<F: FnMut(crate::ipc_contract::ImageProcessingProgress)> {
    completed: u32,
    total: u32,
    publish: F,
    operation_problem: Option<String>,
}

#[derive(Debug)]
enum ProcessingFailure {
    File(String),
    Operation(String),
}

impl<F: FnMut(crate::ipc_contract::ImageProcessingProgress)> ImageProcessingBatch<F> {
    pub(crate) fn new(total: u32, mut publish: F) -> Self {
        publish(crate::ipc_contract::ImageProcessingProgress {
            completed_files: 0,
            total_files: total,
            problem: None,
            operation_problem: None,
        });
        Self {
            completed: 0,
            total,
            publish,
            operation_problem: None,
        }
    }

    pub(crate) async fn prepare(&mut self, app: &AppHandle, binding: &MediaBinding) {
        self.prepare_all(app, vec![binding.clone()]).await;
    }

    pub(crate) async fn prepare_all(&mut self, app: &AppHandle, bindings: Vec<MediaBinding>) {
        self.prepare_all_with_plan(app, bindings, None).await;
    }

    pub(crate) async fn prepare_all_in_plan(
        &mut self,
        app: &AppHandle,
        bindings: Vec<MediaBinding>,
        roots: RootBindingPlan,
    ) {
        self.prepare_all_with_plan(app, bindings, Some(roots)).await;
    }

    async fn prepare_all_with_plan(
        &mut self,
        app: &AppHandle,
        bindings: Vec<MediaBinding>,
        roots: Option<RootBindingPlan>,
    ) {
        if bindings.is_empty() {
            return;
        }
        let attempt = synchronize_processing_sources(app, roots, bindings.clone()).await;
        let engine = app.state::<CacheEngine>();
        let mut owners = Vec::new();
        let mut waiters = Vec::new();
        let permit = engine
            .begin_cancellable_work(CacheCancellation::default())
            .await;
        let mut claimed = std::collections::HashSet::new();
        for binding in bindings {
            let work = attempt.as_ref().map_err(Clone::clone).and_then(|attempt| {
                if !app
                    .state::<ProjectHost>()
                    .is_current_project(&attempt.project_id)
                {
                    return Err("O Projeto mudou durante o processamento.".to_string());
                }
                attempt
                    .works
                    .get(&binding.media_id)
                    .cloned()
                    .ok_or_else(|| {
                        "A referência da imagem mudou durante o processamento.".to_string()
                    })
            });
            let work = match work {
                Ok(work) => work,
                Err(error) => {
                    self.prepare_with(
                        &binding,
                        std::future::ready(Err(ProcessingFailure::File(error))),
                    )
                    .await;
                    continue;
                }
            };
            if !claimed.insert(binding.media_id.clone()) {
                // Callers normally supply a unique catalog. A repeated occurrence
                // must not occupy a waiter slot owned by this same batch.
                self.prepare_with(&binding, std::future::ready(Ok(())))
                    .await;
                continue;
            }
            match engine.claim_for_processing(&work) {
                CacheFlightClaim::Owner(owner) => owners.push((binding, work, owner)),
                CacheFlightClaim::Waiter(waiter) => waiters.push((binding, waiter)),
            }
        }
        drop(permit);
        let resource_interrupted = std::sync::atomic::AtomicBool::new(false);
        let preparation = stream::iter(owners)
            .map(|(binding, work, owner)| {
                let resource_interrupted = &resource_interrupted;
                async move {
                    let result = if resource_interrupted.load(std::sync::atomic::Ordering::Acquire)
                    {
                        Err(CacheFailure::new(
                            CacheFailureStage::MemoryPressure,
                            ProcessorAdmissionFailure::MemoryPressure.to_string(),
                        ))
                    } else {
                        prepare_owned_cache(
                            app,
                            app.state::<LoggingState>().inner(),
                            app.state::<AppPaths>().inner(),
                            app.state::<CacheEngine>().inner(),
                            app.state::<ImagingProcessor>().inner(),
                            work,
                            owner.cancellation(),
                        )
                        .await
                    };
                    if result
                        .as_ref()
                        .is_err_and(|failure| failure.stage == CacheFailureStage::MemoryPressure)
                    {
                        resource_interrupted.store(true, std::sync::atomic::Ordering::Release);
                    }
                    (binding, owner, result)
                }
            })
            .buffer_unordered(app.state::<ImagingProcessor>().cache_capacity());
        tokio::pin!(preparation);
        let mut prepared = Vec::new();
        let mut publications = Vec::new();
        while let Some((binding, owner, result)) = preparation.next().await {
            match result {
                Ok(pending) => {
                    prepared.push((binding, owner));
                    publications.push(pending);
                }
                Err(failure) => {
                    let result = owner.complete(Err(failure));
                    self.prepare_with(&binding, std::future::ready(processing_result(result)))
                        .await;
                }
            }
        }
        if !publications.is_empty() {
            let _permit = engine
                .begin_cancellable_work(CacheCancellation::default())
                .await;
            let publish_app = app.clone();
            let project_id = attempt
                .as_ref()
                .expect("prepared work belongs to a captured attempt")
                .project_id
                .clone();
            let results = tauri::async_runtime::spawn_blocking(move || {
                if !publish_app
                    .state::<ProjectHost>()
                    .is_current_project(&project_id)
                {
                    return publications
                        .iter()
                        .map(|_| {
                            Err(CacheFailure::new(
                                CacheFailureStage::Cancelled,
                                "O Projeto mudou antes da publicação do Cache.",
                            ))
                        })
                        .collect();
                }
                publish_app
                    .state::<CacheEngine>()
                    .publish_prepared_batch(publications)
            })
            .await;
            let results = results.unwrap_or_else(|_| {
                prepared
                    .iter()
                    .map(|_| {
                        Err(CacheFailure::new(
                            CacheFailureStage::PublishIndex,
                            "Não foi possível publicar o lote de Cache.",
                        ))
                    })
                    .collect()
            });
            for ((binding, owner), result) in prepared.into_iter().zip(results) {
                self.prepare_with(
                    &binding,
                    std::future::ready(processing_result(owner.complete(result))),
                )
                .await;
            }
        }
        // Publish our owners before joining foreign flights, preventing cycles
        // between overlapping foreground batches and viewport demands.
        for (binding, waiter) in waiters {
            self.prepare_with(&binding, async { processing_result(waiter.wait().await) })
                .await;
        }
    }

    #[cfg(test)]
    async fn prepare_all_with<Fut: std::future::Future<Output = Result<(), String>>>(
        &mut self,
        bindings: Vec<MediaBinding>,
        concurrency: usize,
        preparation: impl Fn(MediaBinding) -> Fut,
    ) {
        let pending = stream::iter(bindings)
            .map(|binding| {
                let future = preparation(binding.clone());
                async move { (binding, future.await) }
            })
            .buffer_unordered(concurrency);
        tokio::pin!(pending);
        while let Some((binding, result)) = pending.next().await {
            self.prepare_with(
                &binding,
                std::future::ready(result.map_err(ProcessingFailure::File)),
            )
            .await;
        }
    }

    async fn prepare_with(
        &mut self,
        binding: &MediaBinding,
        preparation: impl std::future::Future<Output = Result<(), ProcessingFailure>>,
    ) {
        let problem = match preparation.await {
            Ok(()) => None,
            Err(ProcessingFailure::Operation(reason)) => {
                if self.operation_problem.is_none() {
                    self.operation_problem = Some(reason);
                }
                self.publish_progress(None);
                return;
            }
            Err(ProcessingFailure::File(reason)) => {
                Some(crate::ipc_contract::ImageProcessingProblem {
                    file_name: binding
                        .logical_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    reason,
                })
            }
        };
        self.complete(problem);
    }

    pub(crate) fn complete(
        &mut self,
        problem: Option<crate::ipc_contract::ImageProcessingProblem>,
    ) {
        self.completed += 1;
        self.publish_progress(problem);
    }

    pub(crate) fn report_problem(&mut self, problem: crate::ipc_contract::ImageProcessingProblem) {
        self.publish_progress(Some(problem));
    }

    fn publish_progress(&mut self, problem: Option<crate::ipc_contract::ImageProcessingProblem>) {
        (self.publish)(crate::ipc_contract::ImageProcessingProgress {
            completed_files: self.completed,
            total_files: self.total,
            problem,
            operation_problem: self.operation_problem.clone(),
        });
    }
}

struct ProcessingAttempt {
    project_id: String,
    works: std::collections::HashMap<String, CacheWork>,
}

async fn synchronize_processing_sources(
    app: &AppHandle,
    roots: Option<RootBindingPlan>,
    selected: Vec<MediaBinding>,
) -> Result<ProcessingAttempt, String> {
    let engine = app.state::<CacheEngine>();
    let _permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let processing_app = app.clone();
    let (catalog, namespace, roots, prepared) = tauri::async_runtime::spawn_blocking(move || {
        let host = processing_app.state::<ProjectHost>();
        let catalog = host.authorized_media_catalog()?;
        let namespace = processing_app.state::<ActiveCacheNamespace>().namespace();
        if namespace.project_id() != catalog.project_id {
            return Err("O Projeto mudou durante o processamento das imagens.".into());
        }
        let roots = roots.unwrap_or_else(|| {
            let mut context = OperationPathContext::new();
            for path in std::iter::once(namespace.paths().root()).chain(
                catalog
                    .bindings
                    .iter()
                    .map(|binding| binding.logical_path.as_path()),
            ) {
                let _ = context.capture(path);
            }
            context.freeze()
        });
        processing_app
            .state::<CacheEngine>()
            .retain_prepared_catalog(&catalog.project_id, &catalog.bindings);
        let monitor = processing_app.state::<crate::media_runtime::MediaMonitor>();
        let runtime = processing_app.state::<crate::media_runtime::MediaRuntime>();
        monitor.prepare_in_plan(&runtime, &catalog.bindings, &roots);
        let prepared = monitor.prepare_in_plan(&runtime, &catalog.bindings, &roots);
        Ok::<_, String>((catalog, namespace, roots, prepared))
    })
    .await
    .map_err(|_| "Não foi possível inspecionar as imagens do Projeto.".to_string())??;
    drop(_permit);
    let confirmed =
        crate::product_runtime::confirm_prepared_media(app, &catalog.bindings, &roots, prepared)
            .await?;
    if let Some(update) = confirmed.poll.update() {
        engine.apply_monitor_media_update(
            &namespace,
            app.state::<crate::cache_previews::CachePreviewRegistry>()
                .inner(),
            update,
        );
        if !update.changed_media_ids().is_empty()
            && let Some(window) =
                app.get_webview_window(crate::product_runtime::PROJECT_WINDOW_LABEL)
        {
            window
                .emit(
                    crate::product_runtime::LINKED_MEDIA_CHANGED_EVENT,
                    crate::ipc_contract::LinkedMediaChanged {
                        media_ids: update.changed_media_ids().to_vec(),
                    },
                )
                .map_err(|_| "Não foi possível atualizar as imagens do Projeto.".to_string())?;
        }
    }
    drop(confirmed);
    let _permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let processing_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let authorized = catalog
            .bindings
            .iter()
            .map(|binding| (binding.media_id.clone(), binding.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let sources = selected
            .iter()
            .filter(|binding| authorized.get(&binding.media_id) == Some(*binding))
            .map(|binding| {
                CacheMediaSource::new(
                    binding.media_id.clone(),
                    binding.kind,
                    binding.logical_path.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let works = processing_app
            .state::<CacheEngine>()
            .plan_works(
                processing_app.state::<AppPaths>().inner(),
                &namespace,
                &roots,
                sources,
            )
            .map_err(|failure| failure.message)?
            .into_iter()
            .map(|work| (work.source.media_id().to_owned(), work))
            .collect();
        Ok(ProcessingAttempt {
            project_id: catalog.project_id,
            works,
        })
    })
    .await
    .map_err(|_| "Não foi possível inspecionar as imagens do Projeto.".to_string())?
}

fn processing_result(
    execution: Result<cache_engine::CacheExecution, CacheFailure>,
) -> Result<(), ProcessingFailure> {
    execution.map(|_| ()).map_err(|failure| {
        if failure.stage == CacheFailureStage::MemoryPressure {
            return ProcessingFailure::Operation(failure.message);
        }
        ProcessingFailure::File(format!(
            "A imagem foi vinculada, mas não foi possível preparar sua miniatura: {}",
            failure.message
        ))
    })
}
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_owned_cache(
    app: &AppHandle,
    logging: &LoggingState,
    app_paths: &myalbuns_paths::AppPaths,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    work: CacheWork,
    cancellation: CacheCancellation,
) -> Result<cache_engine::CacheExecution, CacheFailure> {
    let pending = prepare_owned_cache(
        app,
        logging,
        app_paths,
        engine,
        processor,
        work,
        cancellation.clone(),
    )
    .await?;
    if cancellation.reason() == Some(CacheCancellationReason::Paused) {
        cancellation.resume_after_pause();
    }
    let _permit = engine.begin_cancellable_work(cancellation).await;
    let publish_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        publish_app
            .state::<CacheEngine>()
            .publish_prepared_batch(vec![pending])
            .remove(0)
    })
    .await
    .map_err(|_| {
        CacheFailure::new(
            CacheFailureStage::PublishIndex,
            "Não foi possível publicar a prévia do Cache.",
        )
    })?
}

#[allow(clippy::too_many_arguments)]
async fn prepare_owned_cache(
    app: &AppHandle,
    logging: &LoggingState,
    app_paths: &AppPaths,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    work: CacheWork,
    cancellation: CacheCancellation,
) -> Result<PendingCachePublication, CacheFailure> {
    let estimated_work = work.clone();
    let estimate = tauri::async_runtime::spawn_blocking(move || {
        ImageMemoryEstimate::in_plan(
            &estimated_work.root_bindings,
            [estimated_work.source.source_path()],
        )
    })
    .await
    .map_err(|_| {
        CacheFailure::new(
            CacheFailureStage::Plan,
            "Não foi possível estimar os recursos da imagem.",
        )
    })?;
    loop {
        match cancellation.reason() {
            Some(CacheCancellationReason::Obsolete) => {
                return Err(CacheFailure::new(
                    CacheFailureStage::Cancelled,
                    "A demanda de Cache ficou obsoleta.",
                ));
            }
            Some(CacheCancellationReason::Paused) if !cancellation.resume_after_pause() => {
                return Err(CacheFailure::new(
                    CacheFailureStage::Cancelled,
                    "A demanda de Cache não pôde ser retomada.",
                ));
            }
            Some(CacheCancellationReason::Paused) | None => {}
        }
        let permit = engine.begin_cancellable_work(cancellation.clone()).await;
        if cancellation.reason() == Some(CacheCancellationReason::Paused) {
            drop(permit);
            continue;
        }
        if cancellation.reason() == Some(CacheCancellationReason::Obsolete) {
            drop(permit);
            return Err(CacheFailure::new(
                CacheFailureStage::Cancelled,
                "A demanda de Cache ficou obsoleta.",
            ));
        }
        let reservation = match processor
            .reserve_cache_for(estimate, cancellation.flag())
            .await
        {
            Ok(reservation) => reservation,
            Err(ProcessorAdmissionFailure::Cancelled) => {
                drop(permit);
                continue;
            }
            Err(error) => {
                return Err(CacheFailure::new(
                    if error == ProcessorAdmissionFailure::MemoryPressure {
                        CacheFailureStage::MemoryPressure
                    } else {
                        CacheFailureStage::Plan
                    },
                    error.to_string(),
                ));
            }
        };
        if cancellation
            .flag()
            .load(std::sync::atomic::Ordering::Acquire)
        {
            drop(reservation);
            drop(permit);
            continue;
        }
        let context = InvocationContext::new(
            work.request_id.clone(),
            Some(work.namespace.project_id().to_owned()),
        );
        let mut transport = TauriImagingTransport::new(app, logging, &reservation);
        let result = engine
            .prepare(
                &mut transport,
                app_paths,
                work.clone(),
                &context,
                &cancellation,
            )
            .await;
        drop(reservation);
        drop(permit);
        if result.as_ref().is_err_and(|failure| {
            matches!(
                failure.stage,
                CacheFailureStage::Cancelled
                    | CacheFailureStage::Processor(InvocationFailureStage::Cancelled)
            ) && cancellation.reason() == Some(CacheCancellationReason::Paused)
        }) {
            continue;
        }
        return result;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        future::Future,
        task::{Context, Poll, Waker},
    };

    #[test]
    fn placing_imported_media_does_not_start_foreground_processing() {
        use myalbuns_core::{
            CreateAuthorization, CreateProjectRequest, DecorativeDropRequest, DecorativeRole,
            DecorativeScope, ImportMedia, InitialProject, MediaKind, PhotoPlacementMode,
            PhotoSourceMetadata, ProjectCore, ProjectIntent, ProjectLocation,
        };

        for (kind, drop) in [
            (MediaKind::Photo, false),
            (MediaKind::Photo, true),
            (MediaKind::Decorative, false),
            (MediaKind::Decorative, true),
        ] {
            let root = tempfile::tempdir().unwrap();
            let location = root.path().join("Placement.myalbuns");
            let mut roots = OperationPathContext::new();
            roots.capture(&location).unwrap();
            let mut project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(location, roots.freeze()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            let path = root.path().join("Imported.png");
            image::RgbImage::from_pixel(48, 32, image::Rgb([20, 80, 160]))
                .save(&path)
                .unwrap();
            project
                .import_media(
                    kind,
                    vec![ImportMedia::new(
                        path,
                        PhotoSourceMetadata::new(48, 32, std::array::from_fn(|_| "#FFFFFF".into()))
                            .unwrap(),
                    )],
                )
                .unwrap();
            let host = ProjectHost::new(project);
            let previous = host.projection().unwrap();
            let previous_bindings = host.authorized_media_catalog().unwrap().bindings;
            assert_eq!(
                images_requiring_preparation(&[], &previous_bindings),
                previous_bindings,
                "new catalog bindings still require processing"
            );
            let sheet_id = previous.state.album.sheets[0].id.clone();
            let media_id = previous.state.album.media[0].id;
            let intent = match (kind, drop) {
                (MediaKind::Photo, false) => ProjectIntent::AddPhoto {
                    sheet_id,
                    media_id,
                    mode: PhotoPlacementMode::Normal,
                },
                (MediaKind::Photo, true) => ProjectIntent::DropPhoto {
                    sheet_id,
                    media_id,
                    x_um: 100_000,
                    y_um: 100_000,
                    mode: PhotoPlacementMode::Normal,
                },
                (MediaKind::Decorative, false) => ProjectIntent::ApplyDecorative {
                    sheet_id,
                    media_id,
                    role: DecorativeRole::Background,
                    scope: DecorativeScope::BothSides,
                },
                (MediaKind::Decorative, true) => ProjectIntent::DropDecorative {
                    request: DecorativeDropRequest {
                        sheet_id,
                        media_id,
                        role: DecorativeRole::Overlay,
                        x_um: 100_000,
                        y_um: 100_000,
                    },
                },
            };
            let outcome = host.apply_with_outcome(intent).unwrap();
            assert_eq!(
                outcome.projection.state.revision,
                previous.state.revision + 1
            );
            let catalog = host.authorized_media_catalog().unwrap();
            assert_eq!(catalog.bindings, previous_bindings);
            let pending = images_requiring_preparation(&previous_bindings, &catalog.bindings);
            assert!(
                pending.is_empty(),
                "placing {kind:?} (drop={drop}) must not open processing or wait for Cache"
            );
            host.undo().unwrap();
            assert!(
                images_requiring_preparation(
                    &catalog.bindings,
                    &host.authorized_media_catalog().unwrap().bindings,
                )
                .is_empty()
            );
            host.redo().unwrap();
            assert!(
                images_requiring_preparation(
                    &catalog.bindings,
                    &host.authorized_media_catalog().unwrap().bindings,
                )
                .is_empty(),
                "redoing placement of {kind:?} must not restart processing"
            );
        }
    }

    #[test]
    fn preparation_keeps_new_and_changed_sources_but_skips_unchanged_or_removed_bindings() {
        let binding = MediaBinding {
            media_id: "photo-1".into(),
            kind: myalbuns_core::MediaKind::Photo,
            logical_path: "Original.jpg".into(),
        };
        let changed = MediaBinding {
            logical_path: "Relinked.jpg".into(),
            ..binding.clone()
        };
        let added = MediaBinding {
            media_id: "decorative-1".into(),
            kind: myalbuns_core::MediaKind::Decorative,
            ..binding.clone()
        };
        assert!(images_requiring_preparation(std::slice::from_ref(&binding), &[]).is_empty());
        assert_eq!(
            images_requiring_preparation(
                std::slice::from_ref(&binding),
                &[binding.clone(), added.clone()],
            )
            .as_slice(),
            std::slice::from_ref(&added),
        );
        assert_eq!(
            images_requiring_preparation(&[binding], &[changed.clone(), added.clone()]),
            [changed, added],
        );
    }

    #[test]
    fn batch_fills_two_slots_and_reports_completion_order_without_stopping_on_error() {
        tauri::async_runtime::block_on(async {
            let published = RefCell::new(Vec::new());
            let started = RefCell::new(Vec::new());
            let bindings = (0..4)
                .map(|index| MediaBinding {
                    media_id: index.to_string(),
                    kind: myalbuns_core::MediaKind::Photo,
                    logical_path: format!("foto-{index}.jpg").into(),
                })
                .collect::<Vec<_>>();
            let (senders, receivers): (Vec<_>, Vec<_>) = (0..4)
                .map(|_| tokio::sync::oneshot::channel::<Result<(), String>>())
                .unzip();
            let receivers = RefCell::new(receivers.into_iter().map(Some).collect::<Vec<_>>());
            let mut batch =
                ImageProcessingBatch::new(4, |progress| published.borrow_mut().push(progress));
            let pending = batch.prepare_all_with(bindings, 2, |binding| {
                let index = binding.media_id.parse::<usize>().unwrap();
                started.borrow_mut().push(index);
                let receiver = receivers.borrow_mut()[index].take().unwrap();
                async move { receiver.await.unwrap() }
            });
            tokio::pin!(pending);
            let poll = |pending: std::pin::Pin<&mut _>| {
                Future::poll(pending, &mut Context::from_waker(Waker::noop()))
            };
            assert!(matches!(poll(pending.as_mut()), Poll::Pending));
            assert_eq!(*started.borrow(), [0, 1]);
            let mut senders = senders.into_iter().map(Some).collect::<Vec<_>>();
            senders[1]
                .take()
                .unwrap()
                .send(Err("Arquivo danificado".into()))
                .unwrap();
            assert!(matches!(poll(pending.as_mut()), Poll::Pending));
            assert_eq!(*started.borrow(), [0, 1, 2]);
            assert_eq!(
                published.borrow()[1].problem.as_ref().unwrap().file_name,
                "foto-1.jpg"
            );
            senders[2].take().unwrap().send(Ok(())).unwrap();
            assert!(matches!(poll(pending.as_mut()), Poll::Pending));
            assert_eq!(*started.borrow(), [0, 1, 2, 3]);
            senders[3].take().unwrap().send(Ok(())).unwrap();
            senders[0].take().unwrap().send(Ok(())).unwrap();
            pending.await;
            assert_eq!(
                published
                    .borrow()
                    .iter()
                    .map(|progress| progress.completed_files)
                    .collect::<Vec<_>>(),
                [0, 1, 2, 3, 4]
            );
        });
    }

    #[test]
    fn progress_waits_for_cache_and_continues_after_a_failed_image() {
        tauri::async_runtime::block_on(async {
            let published = RefCell::new(Vec::new());
            let binding = MediaBinding {
                media_id: "photo-a".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: "foto.jpg".into(),
            };
            let mut batch =
                ImageProcessingBatch::new(3, |progress| published.borrow_mut().push(progress));
            let (ready, cache) = tokio::sync::oneshot::channel::<()>();
            {
                let mut preparation = std::pin::pin!(batch.prepare_with(&binding, async {
                    cache.await.unwrap();
                    Ok(())
                }));
                assert!(matches!(
                    preparation
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
                assert_eq!(published.borrow().last().unwrap().completed_files, 0);
                ready.send(()).unwrap();
                preparation.await;
            }
            batch
                .prepare_with(&binding, async {
                    Err(ProcessingFailure::File("Cache indisponível".into()))
                })
                .await;
            batch.prepare_with(&binding, async { Ok(()) }).await;
            let published = published.borrow();
            assert_eq!(
                published
                    .iter()
                    .map(|progress| progress.completed_files)
                    .collect::<Vec<_>>(),
                [0, 1, 2, 3]
            );
            assert!(published.iter().all(|progress| progress.total_files == 3));
            assert_eq!(published[2].problem.as_ref().unwrap().file_name, "foto.jpg");
            assert!(published[3].problem.is_none());
        });
    }

    #[test]
    fn resource_failures_have_one_operation_reason_and_do_not_reject_files() {
        tauri::async_runtime::block_on(async {
            let mut events = Vec::new();
            let binding = MediaBinding {
                media_id: "photo-a".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: "foto.jpg".into(),
            };
            let mut batch = ImageProcessingBatch::new(3, |event| events.push(event));
            for _ in 0..2 {
                let failure = CacheFailure::new(
                    CacheFailureStage::MemoryPressure,
                    ProcessorAdmissionFailure::MemoryPressure.to_string(),
                );
                batch
                    .prepare_with(
                        &binding,
                        std::future::ready(processing_result(Err(failure))),
                    )
                    .await;
            }
            batch.prepare_with(&binding, async { Ok(()) }).await;
            assert!(events.iter().all(|event| event.problem.is_none()));
            assert_eq!(
                events
                    .iter()
                    .map(|event| event.completed_files)
                    .collect::<Vec<_>>(),
                [0, 0, 0, 1],
                "resource interruption must not complete unprocessed Originals"
            );
            assert_eq!(
                events.last().unwrap().operation_problem.as_deref(),
                Some(
                    ProcessorAdmissionFailure::MemoryPressure
                        .to_string()
                        .as_str()
                )
            );
        });
    }
}
