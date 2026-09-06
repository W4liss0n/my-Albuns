use futures_util::{StreamExt, stream};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        self, CacheEngine, CacheFailure, CacheFailureStage, CacheFlightClaim, CacheWork,
    },
    cache_service::ActiveCacheNamespace,
    imaging_processor::{
        IMAGE_PROCESSING_CONCURRENCY, ImagingProcessor, InvocationContext, InvocationFailureStage,
        TauriImagingTransport,
    },
    logging::LoggingState,
    media_runtime::MediaBinding,
    path_io,
    project_host::ProjectHost,
};
use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::AppPaths;

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
    previous: &myalbuns_core::EditorProjection,
    current: &myalbuns_core::EditorProjection,
    publish: impl FnMut(crate::ipc_contract::ImageProcessingProgress),
) -> Result<(), String> {
    let previous_references = previous
        .composition
        .sheets
        .iter()
        .flat_map(|sheet| sheet.referenced_media_ids())
        .collect::<std::collections::HashSet<_>>();
    let added_references = current
        .composition
        .sheets
        .iter()
        .flat_map(|sheet| sheet.referenced_media_ids())
        .filter(|id| !previous_references.contains(id))
        .map(|id| id.to_string())
        .collect::<std::collections::HashSet<_>>();
    let catalog = app.state::<ProjectHost>().authorized_media_catalog()?;
    let bindings = catalog
        .bindings
        .iter()
        .filter(|binding| {
            !previous_bindings.contains(binding) || added_references.contains(&binding.media_id)
        })
        .collect::<Vec<_>>();
    if !bindings.is_empty() {
        let mut batch = ImageProcessingBatch::new(bindings.len() as u32, publish);
        batch
            .prepare_all(app, bindings.into_iter().cloned().collect())
            .await;
    }
    Ok(())
}

pub(crate) struct ImageProcessingBatch<F: FnMut(crate::ipc_contract::ImageProcessingProgress)> {
    completed: u32,
    total: u32,
    publish: F,
    sources_synchronized: bool,
}

impl<F: FnMut(crate::ipc_contract::ImageProcessingProgress)> ImageProcessingBatch<F> {
    pub(crate) fn new(total: u32, mut publish: F) -> Self {
        publish(crate::ipc_contract::ImageProcessingProgress {
            completed_files: 0,
            total_files: total,
            problem: None,
        });
        Self {
            completed: 0,
            total,
            publish,
            sources_synchronized: false,
        }
    }

    pub(crate) async fn prepare(&mut self, app: &AppHandle, binding: &MediaBinding) {
        self.prepare_all(app, vec![binding.clone()]).await;
    }

    pub(crate) async fn prepare_all(&mut self, app: &AppHandle, bindings: Vec<MediaBinding>) {
        let synchronization = if !self.sources_synchronized {
            self.sources_synchronized = true;
            synchronize_processing_sources(app).await
        } else {
            Ok(())
        };
        self.prepare_all_with(bindings, |binding| {
            let synchronization = synchronization.clone();
            async move {
                synchronization?;
                prepare_project_image(app, &binding).await
            }
        })
        .await;
    }

    async fn prepare_all_with<Fut: std::future::Future<Output = Result<(), String>>>(
        &mut self,
        bindings: Vec<MediaBinding>,
        preparation: impl Fn(MediaBinding) -> Fut,
    ) {
        let pending = stream::iter(bindings)
            .map(|binding| {
                let future = preparation(binding.clone());
                async move { (binding, future.await) }
            })
            .buffer_unordered(IMAGE_PROCESSING_CONCURRENCY);
        tokio::pin!(pending);
        while let Some((binding, result)) = pending.next().await {
            self.prepare_with(&binding, std::future::ready(result))
                .await;
        }
    }

    async fn prepare_with(
        &mut self,
        binding: &MediaBinding,
        preparation: impl std::future::Future<Output = Result<(), String>>,
    ) {
        let problem =
            preparation
                .await
                .err()
                .map(|reason| crate::ipc_contract::ImageProcessingProblem {
                    file_name: binding
                        .logical_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    reason,
                });
        self.complete(problem);
    }

    pub(crate) fn complete(
        &mut self,
        problem: Option<crate::ipc_contract::ImageProcessingProblem>,
    ) {
        self.completed += 1;
        (self.publish)(crate::ipc_contract::ImageProcessingProgress {
            completed_files: self.completed,
            total_files: self.total,
            problem,
        });
    }
}

async fn synchronize_processing_sources(app: &AppHandle) -> Result<(), String> {
    let engine = app.state::<CacheEngine>();
    let _permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let processing_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let host = processing_app.state::<ProjectHost>();
        let catalog = host.authorized_media_catalog()?;
        let namespace = processing_app.state::<ActiveCacheNamespace>().namespace();
        if namespace.project_id() != catalog.project_id {
            return Err("O Projeto mudou durante o processamento das imagens.".into());
        }
        processing_app
            .state::<CacheEngine>()
            .retain_prepared_catalog(&catalog.project_id, &catalog.bindings);
        let monitor = processing_app.state::<crate::media_runtime::MediaMonitor>();
        let runtime = processing_app.state::<crate::media_runtime::MediaRuntime>();
        // Two matching inspections adopt the selected bindings before the first
        // foreground job can race their initial Monitor notification.
        for _ in 0..2 {
            let poll = monitor.poll(&runtime, &catalog.bindings);
            if let Some(update) = poll.update() {
                processing_app
                    .state::<CacheEngine>()
                    .apply_monitor_media_update(
                        &namespace,
                        processing_app
                            .state::<crate::cache_previews::CachePreviewRegistry>()
                            .inner(),
                        update,
                    );
                crate::product_runtime::refresh_project_photos_for_media_update(
                    &host,
                    &catalog.bindings,
                    update,
                );
                if !update.changed_media_ids().is_empty()
                    && let Some(window) = processing_app
                        .get_webview_window(crate::product_runtime::PROJECT_WINDOW_LABEL)
                {
                    window
                        .emit(
                            crate::product_runtime::LINKED_MEDIA_CHANGED_EVENT,
                            crate::ipc_contract::LinkedMediaChanged {
                                media_ids: update.changed_media_ids().to_vec(),
                            },
                        )
                        .map_err(|_| {
                            "Não foi possível atualizar as imagens do Projeto.".to_string()
                        })?;
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| "Não foi possível inspecionar as imagens do Projeto.".to_string())?
}

/// Prepares the same canonical Cache used by the Panel and Canvas. This work
/// belongs to a user action, so viewport changes do not cancel it and offscreen
/// results stay on disk instead of filling the resident preview registry.
pub(crate) async fn prepare_project_image(
    app: &AppHandle,
    binding: &MediaBinding,
) -> Result<(), String> {
    let engine = app.state::<CacheEngine>();
    let permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let namespace = app.state::<ActiveCacheNamespace>().namespace();
    let catalog = app.state::<ProjectHost>().authorized_media_catalog()?;
    if catalog.project_id != namespace.project_id()
        || !catalog.bindings.iter().any(|current| {
            current.media_id == binding.media_id
                && current.logical_path == binding.logical_path
                && current.kind == binding.kind
        })
    {
        return Err("A referência da imagem mudou durante o processamento.".into());
    }
    let source = CacheMediaSource::new(
        binding.media_id.clone(),
        binding.kind,
        binding.logical_path.clone(),
    )
    .map_err(|_| "Não foi possível preparar a imagem selecionada.".to_string())?;
    let root_bindings = path_io::capture_root_bindings(vec![
        namespace.paths().root().to_path_buf(),
        source.source_path().to_path_buf(),
    ])
    .await
    .map_err(|_| "Não foi possível acessar a origem da imagem ou o Cache.".to_string())?;
    let work = CacheWork::new(
        format!("cache-{}", uuid::Uuid::new_v4().simple()),
        namespace,
        source,
        root_bindings,
    );
    let claim = engine.claim_for_processing(&work);
    drop(permit);
    let execution = match claim {
        CacheFlightClaim::Waiter(waiter) => waiter.wait().await,
        CacheFlightClaim::Owner(owner) => {
            let result = execute_owned_cache(
                app,
                app.state::<LoggingState>().inner(),
                app.state::<AppPaths>().inner(),
                &engine,
                app.state::<ImagingProcessor>().inner(),
                work,
                owner.cancellation(),
            )
            .await;
            owner.complete(result)
        }
    };
    execution.map(|_| ()).map_err(|failure| {
        format!(
            "A imagem foi vinculada, mas não foi possível preparar sua miniatura: {}",
            failure.message
        )
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
        let reservation = processor.reserve_cache().await.map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::Processor(InvocationFailureStage::ResolveSidecar),
                error.to_string(),
            )
        })?;
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
            .execute(
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
            let pending = batch.prepare_all_with(bindings, |binding| {
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
                .prepare_with(&binding, async { Err("Cache indisponível".into()) })
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
}
