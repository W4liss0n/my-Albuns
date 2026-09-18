use std::{future::Future, path::PathBuf};

use myalbuns_core::EditorProjection;
use myalbuns_paths::{AppPaths, RootBindingPlan};
use tauri::{AppHandle, Manager};

use crate::{
    cache_activity_gate::{CacheCancellation, CachePause},
    cache_engine::CacheEngine,
    cache_previews::CachePreviewRegistry,
    cache_service::ActiveCacheNamespace,
    image_processing::ImageProcessingBatch,
    imaging_processor::{ImageMemoryEstimate, ImagingProcessor},
    ipc_contract::ImageProcessingProgress,
    media_runtime::{MediaBinding, MediaResolver},
    project_host::ProjectHost,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MediaChangeKind {
    Relink,
    Replace,
}

/// Completes one editable reference change, including preparation and the final projection.
/// The caller retains its UI operation across selection and the entire awaited change.
pub(crate) async fn change_in_app(
    app: &AppHandle,
    binding: MediaBinding,
    path: PathBuf,
    roots: RootBindingPlan,
    kind: MediaChangeKind,
    processing: &mut ImageProcessingBatch<impl FnMut(ImageProcessingProgress)>,
) -> Result<EditorProjection, String> {
    let pause = app.state::<CacheEngine>().pause().await;
    let changing_app = app.clone();
    let inspection_roots = roots.clone();
    let changing = tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(
            ReferenceChange {
                host: changing_app.state::<ProjectHost>().inner(),
                engine: changing_app.state::<CacheEngine>().inner(),
                registry: changing_app.state::<CachePreviewRegistry>().inner(),
                active: changing_app.state::<ActiveCacheNamespace>().inner(),
                processor: changing_app.state::<ImagingProcessor>().inner(),
                paths: changing_app.state::<AppPaths>().inner(),
            }
            .apply(binding, path, &inspection_roots, kind, pause),
        )
    });
    complete(
        app.state::<ProjectHost>().inner(),
        async {
            changing
                .await
                .map_err(|_| "Não foi possível atualizar a imagem.".to_string())?
        },
        roots,
        |binding, roots| async move {
            processing
                .prepare_all_in_plan(app, vec![binding], roots)
                .await;
        },
    )
    .await
}

async fn complete<F: Future<Output = ()>>(
    host: &ProjectHost,
    change: impl Future<Output = Result<MediaBinding, String>>,
    roots: RootBindingPlan,
    prepare: impl FnOnce(MediaBinding, RootBindingPlan) -> F,
) -> Result<EditorProjection, String> {
    let binding = change.await?;
    prepare(binding, roots).await;
    host.projection()
}

struct ReferenceChange<'a> {
    host: &'a ProjectHost,
    engine: &'a CacheEngine,
    registry: &'a CachePreviewRegistry,
    active: &'a ActiveCacheNamespace,
    processor: &'a ImagingProcessor,
    paths: &'a AppPaths,
}

impl ReferenceChange<'_> {
    async fn apply(
        &self,
        binding: MediaBinding,
        path: PathBuf,
        roots: &RootBindingPlan,
        kind: MediaChangeKind,
        pause: CachePause,
    ) -> Result<MediaBinding, String> {
        let estimate = ImageMemoryEstimate::in_plan(roots, [path.as_path()]);
        let cancellation = CacheCancellation::default();
        let _reservation = self
            .processor
            .reserve_inspection(estimate, cancellation.flag())
            .await
            .map_err(|error| error.to_string())?;
        let proposal = match kind {
            MediaChangeKind::Relink => {
                MediaResolver.propose_relink_in_plan(&binding, path, roots)?
            }
            MediaChangeKind::Replace => {
                MediaResolver.propose_replacement_in_plan(&binding, path, roots)?
            }
        };
        // Reject duplicate references before invalidating the current Cache.
        if self
            .host
            .authorized_media_catalog()?
            .bindings
            .iter()
            .any(|other| {
                other.media_id != binding.media_id
                    && other.kind == binding.kind
                    && other.logical_path == proposal.replacement_path()
            })
        {
            return Err(
                "O arquivo escolhido já está vinculado a outra imagem deste projeto.".into(),
            );
        }
        self.engine
            .invalidate_relinked_media(
                &pause,
                self.paths,
                &self.active.namespace(),
                self.registry,
                &binding.media_id,
            )
            .map_err(|error| error.message)?;
        self.host.relink_media(proposal)?;
        // Both consumers prepare the authoritative binding. All inspection reserves
        // are dropped when this future returns, before preparation can acquire Cache work.
        self.host
            .authorized_media_catalog()?
            .bindings
            .into_iter()
            .find(|current| current.media_id == binding.media_id)
            .ok_or_else(|| "A imagem não pertence mais ao projeto.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache_service::CacheService, project_ui_operations::ProjectUiOperations};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::OperationPathContext;
    use std::{fs, time::Duration};

    struct Fixture {
        host: ProjectHost,
        engine: CacheEngine,
        registry: CachePreviewRegistry,
        active: ActiveCacheNamespace,
        processor: ImagingProcessor,
        paths: AppPaths,
        roots: RootBindingPlan,
        replacement: PathBuf,
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            fs::create_dir_all(root.path().join("roaming")).unwrap();
            fs::create_dir_all(root.path().join("local")).unwrap();
            let mut context = OperationPathContext::new();
            context.capture(root.path()).unwrap();
            let roots = context.freeze();
            let project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(root.path().join("Projeto.myalbuns"), roots.clone()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            let active = ActiveCacheNamespace::new(
                CacheService::new(paths.clone())
                    .reserve_namespace(project.identity_authority())
                    .unwrap(),
            );
            let host = ProjectHost::new(project);
            let photos = ["Original.png", "Outra.png"].map(|name| root.path().join(name));
            for photo in &photos {
                image::RgbImage::new(24, 16).save(photo).unwrap();
            }
            host.import_photos(photos.to_vec(), |_| {}).unwrap();
            host.save(host.projection().unwrap().state.revision)
                .unwrap();
            let replacement = root.path().join("Nova.png");
            image::RgbImage::new(36, 18).save(&replacement).unwrap();
            Self {
                host,
                engine: CacheEngine::default(),
                registry: CachePreviewRegistry::new("project"),
                active,
                processor: ImagingProcessor::default(),
                paths,
                roots,
                replacement,
                root,
            }
        }

        fn binding(&self) -> MediaBinding {
            self.host
                .authorized_media_catalog()
                .unwrap()
                .bindings
                .into_iter()
                .find(|binding| binding.logical_path.ends_with("Original.png"))
                .unwrap()
        }

        fn change(&self) -> ReferenceChange<'_> {
            ReferenceChange {
                host: &self.host,
                engine: &self.engine,
                registry: &self.registry,
                active: &self.active,
                processor: &self.processor,
                paths: &self.paths,
            }
        }
    }

    #[test]
    fn both_changes_wait_for_preparation_without_holding_the_cache_pause() {
        tauri::async_runtime::block_on(async {
            for kind in [MediaChangeKind::Relink, MediaChangeKind::Replace] {
                let fixture = Fixture::new();
                let binding = fixture.binding();
                if kind == MediaChangeKind::Relink {
                    fs::remove_file(&binding.logical_path).unwrap();
                }
                let before = fixture.host.authorized_media_catalog().unwrap().bindings;
                let persisted = fs::read(fixture.root.path().join("Projeto.myalbuns")).unwrap();
                let (started, mut beginning) = tokio::sync::oneshot::channel();
                let (release, ready) = tokio::sync::oneshot::channel();
                let change = fixture.change();
                let operations = ProjectUiOperations::default();
                let operation = operations.begin().unwrap();
                let expected_id = binding.media_id.clone();
                let expected_path = fixture.replacement.clone();
                let expected_roots = serde_json::to_value(&fixture.roots).unwrap();
                let mut running = Box::pin(async {
                    let result = complete(
                        &fixture.host,
                        change.apply(
                            binding.clone(),
                            fixture.replacement.clone(),
                            &fixture.roots,
                            kind,
                            fixture.engine.pause().await,
                        ),
                        fixture.roots.clone(),
                        |replacement, roots| async move {
                            assert_eq!(replacement.media_id, expected_id);
                            assert_eq!(replacement.logical_path, expected_path);
                            assert_eq!(serde_json::to_value(&roots).unwrap(), expected_roots);
                            started.send(()).unwrap();
                            ready.await.unwrap();
                        },
                    )
                    .await;
                    drop(operation);
                    result
                });
                tokio::select! {
                    result = &mut running => panic!("operation completed before preparation: {result:?}"),
                    result = &mut beginning => result.unwrap(),
                }
                let permit = tokio::time::timeout(
                    Duration::from_secs(2),
                    fixture
                        .engine
                        .begin_cancellable_work(CacheCancellation::default()),
                )
                .await
                .expect("preparation must not run under the inspection pause");
                drop(permit);
                let recovery = operations.recover().unwrap();
                assert!(!recovery.is_drained());
                release.send(()).unwrap();
                let result = running.await.unwrap();
                assert!(recovery.is_drained());
                assert_eq!(result, fixture.host.projection().unwrap());
                assert!(result.state.dirty);
                let after = fixture.host.authorized_media_catalog().unwrap().bindings;
                assert_eq!(
                    after
                        .iter()
                        .filter(|item| item.media_id != binding.media_id)
                        .collect::<Vec<_>>(),
                    before
                        .iter()
                        .filter(|item| item.media_id != binding.media_id)
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    fs::read(fixture.root.path().join("Projeto.myalbuns")).unwrap(),
                    persisted
                );
                fixture.host.undo().unwrap();
                assert_eq!(
                    fixture.host.authorized_media_catalog().unwrap().bindings,
                    before
                );
                fixture.host.redo().unwrap();
                assert_eq!(
                    fixture.host.authorized_media_catalog().unwrap().bindings,
                    after
                );
            }
        });
    }

    #[test]
    fn rejected_originals_and_duplicate_references_leave_the_project_and_cache_untouched() {
        tauri::async_runtime::block_on(async {
            for rejection in ["invalid", "duplicate"] {
                let fixture = Fixture::new();
                let binding = fixture.binding();
                let previous = fixture.host.projection().unwrap();
                let original = fs::read(&binding.logical_path).unwrap();
                let namespace = fixture.active.namespace();
                fixture
                    .paths
                    .prepare_cache_storage(namespace.paths())
                    .unwrap();
                let index = namespace.paths().metadata_file();
                // Rejection must not even reconcile a previously damaged index.
                // Invalidation would remove it, so the unchanged bytes prove ordering.
                let cached = b"unreconciled previous Cache index";
                fs::write(&index, cached).unwrap();
                let replacement = if rejection == "invalid" {
                    fs::write(&fixture.replacement, b"invalid image").unwrap();
                    fixture.replacement.clone()
                } else {
                    fixture.root.path().join("Outra.png")
                };
                let prepare_called = std::cell::Cell::new(false);
                let change = fixture.change();
                let result = complete(
                    &fixture.host,
                    change.apply(
                        binding.clone(),
                        replacement,
                        &fixture.roots,
                        MediaChangeKind::Replace,
                        fixture.engine.pause().await,
                    ),
                    fixture.roots.clone(),
                    |_, _| async {
                        prepare_called.set(true);
                    },
                )
                .await;
                let error = result.unwrap_err();
                if rejection == "duplicate" {
                    assert!(error.contains("já está vinculado"), "{error}");
                }
                assert!(!prepare_called.get());
                assert_eq!(fixture.host.projection().unwrap(), previous);
                assert_eq!(fs::read(index).unwrap(), cached);
                assert_eq!(fs::read(binding.logical_path).unwrap(), original);
                drop(
                    tokio::time::timeout(Duration::from_secs(2), fixture.engine.pause())
                        .await
                        .unwrap(),
                );
                drop(
                    tokio::time::timeout(Duration::from_secs(2), fixture.processor.reserve())
                        .await
                        .unwrap()
                        .unwrap(),
                );
            }
        });
    }

    #[test]
    fn preparation_failure_keeps_the_valid_change_and_its_processing_diagnostic() {
        tauri::async_runtime::block_on(async {
            let fixture = Fixture::new();
            let binding = fixture.binding();
            let original = fs::read(&binding.logical_path).unwrap();
            let mut progress = Vec::new();
            let mut processing = ImageProcessingBatch::new(1, |value| progress.push(value));
            let change = fixture.change();
            let completed = complete(
                &fixture.host,
                change.apply(
                    binding.clone(),
                    fixture.replacement.clone(),
                    &fixture.roots,
                    MediaChangeKind::Replace,
                    fixture.engine.pause().await,
                ),
                fixture.roots.clone(),
                |replacement, _| async move {
                    // The external preparation reports a failure without undoing the valid reference.
                    processing.complete(Some(crate::ipc_contract::ImageProcessingProblem {
                        file_name: replacement
                            .logical_path
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        reason: "Espaço insuficiente.".into(),
                    }));
                },
            )
            .await
            .unwrap();
            assert_eq!(progress.len(), 2);
            assert_eq!(progress[1].completed_files, 1);
            assert_eq!(
                progress[1].problem.as_ref().unwrap().reason,
                "Espaço insuficiente."
            );
            assert_eq!(
                completed
                    .state
                    .album
                    .media
                    .iter()
                    .find(|media| media.id.to_string() == binding.media_id)
                    .unwrap()
                    .source_width_px,
                Some(36)
            );
            assert!(completed.state.dirty);
            assert_eq!(fs::read(binding.logical_path).unwrap(), original);
            assert!(
                fixture
                    .host
                    .authorized_media_catalog()
                    .unwrap()
                    .bindings
                    .iter()
                    .any(|item| item.media_id == binding.media_id
                        && item.logical_path == fixture.replacement)
            );
            drop(
                tokio::time::timeout(Duration::from_secs(2), fixture.processor.reserve())
                    .await
                    .unwrap()
                    .unwrap(),
            );
        });
    }

    #[test]
    fn final_projection_includes_observations_confirmed_during_preparation() {
        tauri::async_runtime::block_on(async {
            let fixture = Fixture::new();
            let change = fixture.change();
            let before = fixture.host.projection().unwrap().state.revision;
            let host = &fixture.host;
            let completed = complete(
                host,
                change.apply(
                    fixture.binding(),
                    fixture.replacement.clone(),
                    &fixture.roots,
                    MediaChangeKind::Replace,
                    fixture.engine.pause().await,
                ),
                fixture.roots.clone(),
                |replacement, _| async move {
                    host.observe_photo_source(
                        &replacement,
                        myalbuns_core::PhotoSourceMetadata::new(
                            72,
                            36,
                            std::array::from_fn(|_| "#FFFFFF".into()),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                },
            )
            .await
            .unwrap();
            assert_eq!(completed.state.revision, before + 1);
            assert!(
                completed
                    .state
                    .album
                    .media
                    .iter()
                    .any(|media| media.source_width_px == Some(72))
            );
            assert_eq!(completed, host.projection().unwrap());
        });
    }
}
