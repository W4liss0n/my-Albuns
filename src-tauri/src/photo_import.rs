//! A selected source remains an attempt-local value until its inspection is
//! accepted by Core. Native batches own no creative identifiers or UI state.
#[cfg(test)]
mod native_flow_tests;
mod progress;
use progress::NativeImportProgress;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use futures_util::{StreamExt, stream};
use myalbuns_core::{ImportPhoto, MediaKind, PhotoSourceMetadata};
use myalbuns_imaging_protocol::{
    CacheRepresentationPolicy, IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingProgress,
    ImagingResponse, ImportedPhotoPreview, PHOTO_IMPORT_PROCESS_BATCH, PhotoImportCandidate,
    PhotoImportCompletion, PhotoImportOutcome, PhotoImportRequest, PhotoImportSourceId,
};
use myalbuns_paths::{AppPaths, NativePathDto, OperationPathContext, RootBindingPlan};
use tauri::{AppHandle, Manager};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        AuthorizedCacheNamespace, CacheEngine, CacheFailureStage, CacheImportStage,
        CacheProcessorStatus,
    },
    cache_previews::CachePreviewRegistry,
    cache_service::ActiveCacheNamespace,
    image_processing::ImageProcessingBatch,
    imaging_processor::{
        ImageMemoryEstimate, ImagingProcessor, InvocationContext, InvocationControl,
        InvocationFailureStage, ProcessorAdmissionFailure, TauriImagingTransport,
    },
    ipc_contract::{ImageProcessingProblem, ImageProcessingProgress, ImportMediaResult},
    logging::LoggingState,
    media_runtime::{
        ImportedPhotoInspection, MediaBinding, MediaMonitor, MediaObservation, MediaResolver,
        MediaRuntime, PhotoImportsProposal,
    },
    project_host::{AuthorizedMediaCatalog, ProjectHost},
};

struct SelectedSource {
    candidate: PhotoImportCandidate,
    before: MediaObservation,
}

struct PhotoImportAttempt {
    kind: MediaKind,
    id: String,
    catalog: AuthorizedMediaCatalog,
    namespace: AuthorizedCacheNamespace,
    roots: RootBindingPlan,
    paths: Vec<PathBuf>,
    sources: Vec<SelectedSource>,
    selection_problems: Vec<ImageProcessingProblem>,
}

impl PhotoImportAttempt {
    #[cfg(test)]
    fn capture(
        catalog: AuthorizedMediaCatalog,
        namespace: AuthorizedCacheNamespace,
        paths: Vec<PathBuf>,
    ) -> Result<Self, String> {
        Self::capture_for_kind(MediaKind::Photo, catalog, namespace, paths)
    }

    fn capture_for_kind(
        kind: MediaKind,
        catalog: AuthorizedMediaCatalog,
        namespace: AuthorizedCacheNamespace,
        paths: Vec<PathBuf>,
    ) -> Result<Self, String> {
        if catalog.project_id != namespace.project_id() {
            return Err("O Projeto mudou durante a importação das imagens.".into());
        }
        let mut seen = HashSet::new();
        let paths = paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect::<Vec<_>>();
        let existing = catalog
            .bindings
            .iter()
            .filter(|binding| binding.kind == kind)
            .map(|binding| binding.logical_path.as_path())
            .collect::<HashSet<_>>();
        let mut context = OperationPathContext::new();
        // Failed roots remain uncovered, so the inspector can reject just their
        // candidates. Successful captures are never repeated during this attempt.
        for path in std::iter::once(namespace.paths().root())
            .chain(
                catalog
                    .bindings
                    .iter()
                    .map(|binding| binding.logical_path.as_path()),
            )
            .chain(paths.iter().map(PathBuf::as_path))
        {
            let _ = context.capture(path);
        }
        let roots = context.freeze();
        let (paths, selection_problems) = crate::media_import_selection::expand(paths, &roots);
        let sources = paths
            .iter()
            .filter(|path| !existing.contains(path.as_path()))
            .map(|path| {
                let candidate = PhotoImportCandidate {
                    source_id: PhotoImportSourceId::new(uuid::Uuid::new_v4().simple().to_string())
                        .expect("a UUID is a valid import source key"),
                    source_path: NativePathDto::from(path.clone()),
                    generation_id: uuid::Uuid::new_v4().simple().to_string(),
                };
                let before = MediaResolver.observe_in_plan(&roots, &source_binding(kind, path));
                SelectedSource { candidate, before }
            })
            .collect();
        Ok(Self {
            kind,
            id: uuid::Uuid::new_v4().simple().to_string(),
            catalog,
            namespace,
            roots,
            paths,
            sources,
            selection_problems,
        })
    }

    fn requests(&self, capacity: usize) -> Vec<PhotoImportRequest> {
        if !self.roots.covers(self.namespace.paths().root()) {
            return Vec::new();
        }
        let candidates = self
            .sources
            .iter()
            .filter(|source| self.roots.covers(source.candidate.path()))
            .map(|source| source.candidate.clone())
            .collect::<Vec<_>>();
        let batch_size = candidates
            .len()
            .div_ceil(capacity)
            .clamp(1, PHOTO_IMPORT_PROCESS_BATCH);
        candidates
            .chunks(batch_size)
            .map(|candidates| PhotoImportRequest {
                protocol_version: IMAGING_PROTOCOL_VERSION,
                request_id: format!("import-{}", uuid::Uuid::new_v4().simple()),
                attempt_id: self.id.clone(),
                project_id: self.catalog.project_id.clone(),
                cache_paths: self.namespace.paths().clone(),
                candidates: candidates.to_vec(),
                policy: CacheRepresentationPolicy::measured_v1(),
                root_bindings: self.roots.clone(),
            })
            .collect()
    }
}

pub(crate) async fn import_selected_media(
    app: &AppHandle,
    kind: MediaKind,
    paths: Vec<PathBuf>,
    mut unsupported: Vec<ImageProcessingProblem>,
    mut publish: impl FnMut(ImageProcessingProgress) + Send + 'static,
) -> Result<ImportMediaResult, String> {
    let host = app.state::<ProjectHost>();
    let catalog = host.authorized_media_catalog()?;
    let namespace = app.state::<ActiveCacheNamespace>().namespace();
    publish(ImageProcessingProgress {
        completed_files: 0,
        total_files: 0,
        problem: None,
    });
    let capture_app = app.clone();
    let (mut attempt, mut stage, stage_error) = tauri::async_runtime::spawn_blocking(move || {
        let attempt = PhotoImportAttempt::capture_for_kind(kind, catalog, namespace, paths)?;
        let candidates = attempt
            .sources
            .iter()
            .map(|source| source.candidate.clone())
            .collect::<Vec<_>>();
        let stage = capture_app.state::<CacheEngine>().begin_import_stage(
            capture_app.state::<AppPaths>().inner(),
            attempt.namespace.clone(),
            attempt.id.clone(),
            &candidates,
        );
        let (stage, error) = match stage {
            Ok(stage) => (Some(stage), None),
            Err(error) => (None, Some(error)),
        };
        Ok::<_, String>((attempt, stage, error))
    })
    .await
    .map_err(|_| "Não foi possível preparar a importação das imagens.".to_string())??;
    unsupported.append(&mut attempt.selection_problems);
    let unsupported_count = unsupported.len();
    let total = attempt.paths.len() + unsupported_count;
    let progress = ImageProcessingBatch::new(total as u32, publish);

    let requests = if stage.is_some() {
        attempt.requests(app.state::<ImagingProcessor>().cache_capacity())
    } else {
        Vec::new()
    };
    let new_source_count = attempt.sources.len();
    let native_progress = NativeImportProgress::new(progress, &requests);
    let batches = stream::iter(requests)
        .map(|request| {
            let progress = &native_progress;
            async move {
                let result =
                    execute_import_batch(app, &request, &|event| progress.report(event)).await;
                (request, result)
            }
        })
        .buffer_unordered(app.state::<ImagingProcessor>().cache_capacity())
        .collect::<Vec<_>>()
        .await;
    if let Some(failure) = batches
        .iter()
        .find_map(|(_, result)| result.as_ref().err().filter(|failure| failure.quarantined))
    {
        if let Some(stage) = &mut stage {
            stage.defer_cleanup_until_restart();
        }
        return Err(failure.message.clone());
    }
    let mut outcomes = HashMap::new();
    let mut cache_problems = HashMap::new();
    for (request, result) in batches {
        match result {
            Ok(completion) => outcomes.extend(
                completion
                    .photos
                    .into_iter()
                    .map(|photo| (photo.source_id, photo.outcome)),
            ),
            Err(failure) => {
                for candidate in request.candidates {
                    cache_problems.insert(candidate.path().to_path_buf(), failure.message.clone());
                }
            }
        }
    }
    // Exceptions and changed sources use the existing inspector after native
    // batches drain. Each fallback acquires the same memory/Processor owner.
    let finish_app = app.clone();
    let inspection_roots = attempt.roots.clone();
    let inspection_bindings = attempt.catalog.bindings.clone();
    native_progress.begin_inspection(outcomes.iter().filter_map(|(source, outcome)| {
        matches!(outcome, PhotoImportOutcome::Validated { .. }).then_some(source)
    }));
    let (prepared, native_progress) = tauri::async_runtime::spawn_blocking(move || {
        let prepared = prepare_proposal_with_inspection(
            attempt,
            stage,
            outcomes,
            cache_problems,
            stage_error,
            unsupported,
            |candidate| {
                let proposal = inspect_with_capacity(
                    kind,
                    finish_app.state::<CacheEngine>().inner(),
                    finish_app.state::<ImagingProcessor>().inner(),
                    candidate.path(),
                    &inspection_bindings,
                    &inspection_roots,
                );
                native_progress.complete_inspection(&candidate.source_id);
                proposal
            },
        )?;
        Ok::<_, String>((prepared, native_progress))
    })
    .await
    .map_err(|_| "Não foi possível validar a importação das imagens.".to_string())??;

    let engine = app.state::<CacheEngine>();
    let _commit_permit = engine
        .begin_cancellable_work(CacheCancellation::default())
        .await;
    let commit_app = app.clone();
    let CommittedImport {
        mut result,
        existing,
        roots,
        cache_problems,
        new_paths,
    } = tauri::async_runtime::spawn_blocking(move || {
        commit_prepared_import(
            commit_app.state::<ProjectHost>().inner(),
            commit_app.state::<CacheEngine>().inner(),
            commit_app.state::<MediaMonitor>().inner(),
            commit_app.state::<MediaRuntime>().inner(),
            commit_app.state::<CachePreviewRegistry>().inner(),
            prepared,
        )
    })
    .await
    .map_err(|_| "Não foi possível concluir a importação das imagens.".to_string())??;
    drop(_commit_permit);
    let mut progress = native_progress.finish((new_source_count + unsupported_count) as u32);
    for path in new_paths {
        if let Some(reason) = cache_problems.get(&path) {
            progress.report_problem(cache_problem(&path, reason.clone()));
        }
    }
    progress.prepare_all_in_plan(app, existing, roots).await;
    if let ImportMediaResult::Completed { projection, .. } = &mut result {
        *projection = host.projection()?;
    }
    Ok(result)
}

struct PreparedImport {
    attempt: PhotoImportAttempt,
    stage: Option<CacheImportStage>,
    proposal: PhotoImportsProposal,
    cache_problems: HashMap<PathBuf, String>,
    accepted_paths: Vec<PathBuf>,
}

fn prepare_proposal_with_inspection(
    attempt: PhotoImportAttempt,
    mut stage: Option<CacheImportStage>,
    mut outcomes: HashMap<PhotoImportSourceId, PhotoImportOutcome>,
    mut cache_problems: HashMap<PathBuf, String>,
    stage_error: Option<String>,
    unsupported: Vec<ImageProcessingProblem>,
    inspect: impl Fn(&PhotoImportCandidate) -> PhotoImportsProposal,
) -> Result<PreparedImport, String> {
    let sources = attempt
        .sources
        .iter()
        .map(|source| (source.candidate.path(), source))
        .collect::<HashMap<_, _>>();
    let mut proposal = PhotoImportsProposal {
        kind: attempt.kind,
        commands: Vec::new(),
        problems: unsupported,
        inspections: Vec::new(),
    };
    let mut accepted_paths = Vec::new();
    for path in &attempt.paths {
        let Some(source) = sources.get(path.as_path()) else {
            proposal
                .commands
                .push(ImportPhoto::select_existing(path.clone()));
            continue;
        };
        if let Some(PhotoImportOutcome::Validated {
            dimensions,
            fingerprint,
            preview,
        }) = outcomes.remove(&source.candidate.source_id)
        {
            let current =
                MediaResolver.observe_in_plan(&attempt.roots, &source_binding(attempt.kind, path));
            if source.before.same_source(&current) && current.matches_fingerprint(&fingerprint) {
                let metadata = PhotoSourceMetadata::new(
                    dimensions.width_px,
                    dimensions.height_px,
                    ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
                )
                .map_err(|error| error.to_string())?;
                proposal
                    .commands
                    .push(ImportPhoto::new(path.clone(), metadata));
                proposal.inspections.push(ImportedPhotoInspection {
                    observation: current.clone(),
                });
                accepted_paths.push(path.clone());
                let prepared = match preview {
                    ImportedPhotoPreview::Prepared { generation } => stage
                        .as_mut()
                        .ok_or_else(|| "O Cache ficou indisponível.".to_string())
                        .and_then(|stage| {
                            stage.record(source.candidate.clone(), current, *generation)
                        }),
                    ImportedPhotoPreview::Unavailable { reason } => Err(reason),
                };
                if let Err(reason) = prepared {
                    cache_problems.insert(path.clone(), reason);
                }
                continue;
            }
            // The prior decode is no longer evidence for this Original. The
            // current frozen-plan inspector decides whether it can be imported.
            cache_problems.insert(
                path.clone(),
                "O Original mudou durante a preparação da miniatura.".into(),
            );
        }
        let fallback = inspect(&source.candidate);
        if !fallback.commands.is_empty() {
            accepted_paths.push(path.clone());
            cache_problems.entry(path.clone()).or_insert_with(|| {
                stage_error.clone().unwrap_or_else(|| {
                    "A imagem exige uma nova tentativa de preparação da miniatura.".into()
                })
            });
        }
        proposal.commands.extend(fallback.commands);
        proposal.problems.extend(fallback.problems);
        proposal.inspections.extend(fallback.inspections);
    }
    Ok(PreparedImport {
        attempt,
        stage,
        proposal,
        cache_problems,
        accepted_paths,
    })
}

struct CommittedImport {
    result: ImportMediaResult,
    existing: Vec<MediaBinding>,
    roots: RootBindingPlan,
    cache_problems: HashMap<PathBuf, String>,
    new_paths: Vec<PathBuf>,
}

fn commit_prepared_import(
    host: &ProjectHost,
    engine: &CacheEngine,
    monitor: &MediaMonitor,
    runtime: &MediaRuntime,
    registry: &CachePreviewRegistry,
    mut prepared: PreparedImport,
) -> Result<CommittedImport, String> {
    let attempt = prepared.attempt;
    // The action may have waited for a causal pause after preparing its proposal.
    // Only unchanged inspected Originals may cross the Core commit boundary.
    let new_paths = prepared
        .accepted_paths
        .iter()
        .map(PathBuf::as_path)
        .collect::<HashSet<_>>();
    let evidence = prepared
        .proposal
        .inspections
        .iter()
        .map(|inspection| {
            (
                inspection.observation.logical_path(),
                &inspection.observation,
            )
        })
        .collect::<HashMap<_, _>>();
    let rejected = prepared
        .proposal
        .commands
        .iter()
        .filter(|command| {
            new_paths.contains(command.path())
                && !evidence.get(command.path()).is_some_and(|observed| {
                    observed.same_source(&MediaResolver.observe_in_plan(
                        &attempt.roots,
                        &source_binding(attempt.kind, command.path()),
                    ))
                })
        })
        .map(|command| command.path().to_path_buf())
        .collect::<HashSet<_>>();
    prepared
        .proposal
        .commands
        .retain(|command| !rejected.contains(command.path()));
    prepared
        .proposal
        .inspections
        .retain(|inspection| !rejected.contains(inspection.observation.logical_path()));
    for path in prepared
        .accepted_paths
        .iter()
        .filter(|path| rejected.contains(*path))
    {
        prepared.proposal.problems.push(ImageProcessingProblem {
            file_name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            reason:
                "O Original mudou antes da conclusão da importação. Selecione a imagem novamente."
                    .into(),
        });
    }
    prepared
        .accepted_paths
        .retain(|path| !rejected.contains(path));
    prepared
        .cache_problems
        .retain(|path, _| !rejected.contains(path));
    if let Some(stage) = &mut prepared.stage {
        stage.discard_sources(&rejected);
    }
    let inspections = prepared
        .proposal
        .inspections
        .iter()
        .map(|inspection| inspection.observation.clone())
        .collect::<Vec<_>>();
    let result =
        host.commit_photo_import_proposal(&attempt.catalog.project_id, prepared.proposal)?;
    let catalog = host.authorized_media_catalog()?;
    if catalog.project_id != attempt.catalog.project_id {
        return Err("O Projeto mudou durante a importação.".into());
    }
    let by_path = catalog
        .bindings
        .iter()
        .filter(|binding| binding.kind == attempt.kind)
        .map(|binding| (binding.logical_path.as_path(), binding))
        .collect::<HashMap<_, _>>();
    let inspections = inspections
        .into_iter()
        .filter_map(|mut observation| {
            observation.media_id = by_path.get(observation.logical_path())?.media_id.clone();
            Some(observation)
        })
        .collect::<Vec<_>>();
    let poll = monitor.adopt_prepared_inspections(
        runtime,
        &catalog.bindings,
        &attempt.roots,
        &inspections,
    );
    if let Some(update) = poll.update() {
        engine.apply_monitor_media_update(&attempt.namespace, registry, update);
    }
    // Core already owns these dimensions. Import adoption must not call the
    // Monitor's ordinary source refresh (which decodes changed Originals).
    if let Some(stage) = &mut prepared.stage {
        let source_paths = attempt
            .sources
            .iter()
            .map(|source| (&source.candidate.source_id, source.candidate.path()))
            .collect::<HashMap<_, _>>();
        let generation = poll
            .confirmed_observation()
            .map_or(0, |observation| observation.generation());
        for (source_id, reason) in
            engine.publish_import_stage(stage, &catalog.bindings, &attempt.roots, generation)
        {
            if let Some(path) = source_paths.get(&source_id) {
                prepared.cache_problems.insert(path.to_path_buf(), reason);
            }
        }
    }
    let selected = attempt
        .paths
        .iter()
        .map(PathBuf::as_path)
        .collect::<HashSet<_>>();
    let existing = attempt
        .catalog
        .bindings
        .into_iter()
        .filter(|binding| {
            binding.kind == attempt.kind && selected.contains(binding.logical_path.as_path())
        })
        .collect();
    Ok(CommittedImport {
        result,
        existing,
        roots: attempt.roots,
        cache_problems: prepared.cache_problems,
        new_paths: prepared.accepted_paths,
    })
}

struct BatchFailure {
    message: String,
    quarantined: bool,
}

async fn execute_import_batch(
    app: &AppHandle,
    request: &PhotoImportRequest,
    progress: &(dyn Fn(ImagingProgress) + Send + Sync),
) -> Result<PhotoImportCompletion, BatchFailure> {
    let engine = app.state::<CacheEngine>();
    let processor = app.state::<ImagingProcessor>();
    let app_paths = app.state::<AppPaths>();
    let logging = app.state::<LoggingState>();
    let cancellation = CacheCancellation::default();
    let command = ImagingCommand::PreparePhotoImport(request.clone());
    let context = InvocationContext::new(&request.request_id, Some(&request.project_id));
    let estimated_request = request.clone();
    let estimate = tauri::async_runtime::spawn_blocking(move || {
        ImageMemoryEstimate::in_plan(
            &estimated_request.root_bindings,
            estimated_request
                .candidates
                .iter()
                .map(PhotoImportCandidate::path),
        )
    })
    .await
    .map_err(|_| BatchFailure {
        message: "Não foi possível estimar os recursos do lote.".into(),
        quarantined: false,
    })?;
    loop {
        if !app
            .state::<ProjectHost>()
            .is_current_project(&request.project_id)
        {
            return Err(BatchFailure {
                message: "O Projeto mudou durante a importação.".into(),
                quarantined: false,
            });
        }
        if cancellation.reason() == Some(CacheCancellationReason::Paused) {
            cancellation.resume_after_pause();
        }
        let permit = engine.begin_cancellable_work(cancellation.clone()).await;
        if cancellation.reason() == Some(CacheCancellationReason::Paused) {
            drop(permit);
            continue;
        }
        if engine.processor_status() == CacheProcessorStatus::Suspended {
            return Err(BatchFailure {
                message: "O Processador de Imagens está suspenso.".into(),
                quarantined: false,
            });
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
                return Err(BatchFailure {
                    message: error.to_string(),
                    quarantined: error == ProcessorAdmissionFailure::Unavailable,
                });
            }
        };
        if cancellation.reason().is_some() {
            drop(reservation);
            drop(permit);
            continue;
        }
        let mut transport = TauriImagingTransport::new(app, &logging, &reservation);
        let outcome = engine
            .invoke_cache_command(
                &mut transport,
                &app_paths,
                &command,
                &context,
                InvocationControl::controlled(cancellation.flag(), progress),
            )
            .await;
        drop(reservation);
        drop(permit);
        match outcome {
            Ok((
                ImagingResponse::PhotoImportCompleted {
                    request_id,
                    completion,
                },
                _,
            )) if request_id == request.request_id => {
                completion
                    .validate_for(request)
                    .map_err(|message| BatchFailure {
                        message,
                        quarantined: false,
                    })?;
                return Ok(completion);
            }
            Ok(_) => {
                return Err(BatchFailure {
                    message: "O Processador devolveu uma conclusão incompatível com a importação."
                        .into(),
                    quarantined: false,
                });
            }
            Err(failure)
                if failure.stage
                    == CacheFailureStage::Processor(
                        InvocationFailureStage::TerminationUnconfirmed,
                    ) =>
            {
                return Err(BatchFailure {
                    message: failure.message,
                    quarantined: true,
                });
            }
            Err(failure) => {
                if matches!(
                    failure.stage,
                    CacheFailureStage::Cancelled
                        | CacheFailureStage::Processor(InvocationFailureStage::Cancelled)
                ) && cancellation.reason() == Some(CacheCancellationReason::Paused)
                {
                    continue;
                }
                return Err(BatchFailure {
                    message: failure.message,
                    quarantined: false,
                });
            }
        }
    }
}

fn source_binding(kind: MediaKind, path: &Path) -> MediaBinding {
    MediaBinding {
        media_id: String::new(),
        kind,
        logical_path: path.to_path_buf(),
    }
}

fn inspect_with_capacity(
    kind: MediaKind,
    engine: &CacheEngine,
    processor: &ImagingProcessor,
    path: &Path,
    bindings: &[MediaBinding],
    roots: &RootBindingPlan,
) -> PhotoImportsProposal {
    let estimate = ImageMemoryEstimate::in_plan(roots, [path]);
    let cancellation = CacheCancellation::default();
    let result = tauri::async_runtime::block_on(async {
        loop {
            if cancellation.reason() == Some(CacheCancellationReason::Paused) {
                cancellation.resume_after_pause();
            }
            let permit = engine.begin_cancellable_work(cancellation.clone()).await;
            let reservation = match processor
                .reserve_inspection(estimate, cancellation.flag())
                .await
            {
                Ok(reservation) => reservation,
                Err(ProcessorAdmissionFailure::Cancelled) => {
                    drop(permit);
                    continue;
                }
                Err(error) => return Err(error),
            };
            if cancellation.reason().is_some() {
                drop(reservation);
                drop(permit);
                continue;
            }
            // This function runs on the blocking pool. A started decoder is
            // drained before returning its memory and exclusive CPU reservation.
            let proposal = MediaResolver.propose_media_imports_in_plan(
                kind,
                vec![path.to_path_buf()],
                bindings,
                roots,
                |_| {},
            );
            drop(reservation);
            drop(permit);
            return Ok(proposal);
        }
    });
    result.unwrap_or_else(|error| PhotoImportsProposal {
        kind,
        commands: Vec::new(),
        inspections: Vec::new(),
        problems: vec![ImageProcessingProblem {
            file_name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            reason: error.to_string(),
        }],
    })
}

fn cache_problem(path: &Path, reason: String) -> ImageProcessingProblem {
    ImageProcessingProblem {
        file_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        reason: format!(
            "A imagem foi vinculada, mas não foi possível preparar sua miniatura: {reason}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
    };
    use myalbuns_imaging_protocol::{CacheFingerprint, ImportedPhotoDimensions};
    use sha2::{Digest, Sha256};

    fn prepare_proposal(
        attempt: PhotoImportAttempt,
        stage: Option<CacheImportStage>,
        outcomes: HashMap<PhotoImportSourceId, PhotoImportOutcome>,
        problems: HashMap<PathBuf, String>,
        stage_error: Option<String>,
        unsupported: Vec<ImageProcessingProblem>,
    ) -> Result<PreparedImport, String> {
        let bindings = attempt.catalog.bindings.clone();
        let roots = attempt.roots.clone();
        prepare_proposal_with_inspection(
            attempt,
            stage,
            outcomes,
            problems,
            stage_error,
            unsupported,
            |candidate| {
                MediaResolver.propose_photo_imports_in_plan(
                    vec![candidate.path().to_path_buf()],
                    &bindings,
                    &roots,
                    |_| {},
                )
            },
        )
    }

    struct Fixture {
        root: tempfile::TempDir,
        host: ProjectHost,
        namespace: AuthorizedCacheNamespace,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("Projeto.myalbuns");
            let mut context = OperationPathContext::new();
            context.capture(&path).unwrap();
            let project = ProjectCore::new()
                .with_identity_storage_roots(
                    root.path().join("leases"),
                    root.path().join("identities"),
                )
                .create_editable(CreateProjectRequest::new(
                    ProjectLocation::new(path, context.freeze()),
                    InitialProject::neutral(),
                    CreateAuthorization::CreateOnly,
                ))
                .unwrap();
            let app_paths =
                AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
            let namespace =
                AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority()).unwrap();
            Self {
                root,
                host: ProjectHost::new(project),
                namespace,
            }
        }

        fn photo(&self, name: &str) -> PathBuf {
            let path = self.root.path().join(name);
            RgbImage::from_pixel(37, 23, Rgb([20, 80, 160]))
                .save_with_format(&path, ImageFormat::Jpeg)
                .unwrap();
            path
        }

        fn attempt(&self, paths: Vec<PathBuf>) -> PhotoImportAttempt {
            PhotoImportAttempt::capture(
                self.host.authorized_media_catalog().unwrap(),
                self.namespace.clone(),
                paths,
            )
            .unwrap()
        }
    }

    fn validated(source: &SelectedSource) -> PhotoImportOutcome {
        let metadata = std::fs::metadata(source.candidate.path()).unwrap();
        let millis = |time: std::io::Result<std::time::SystemTime>| {
            time.ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64)
        };
        let fingerprint = CacheFingerprint::sha256_full_file_with_timestamps(
            metadata.len(),
            millis(metadata.created()),
            millis(metadata.modified()),
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(source.candidate.path()).unwrap())
            ),
        )
        .unwrap();
        PhotoImportOutcome::Validated {
            dimensions: ImportedPhotoDimensions {
                width_px: 37,
                height_px: 23,
            },
            fingerprint,
            preview: ImportedPhotoPreview::Unavailable {
                reason: "Cache indisponível".into(),
            },
        }
    }

    #[test]
    fn prepared_sources_keep_selection_order_and_one_history_action_without_host_decodes() {
        let fixture = Fixture::new();
        let first = fixture.photo("primeira.jpg");
        let second = fixture.photo("segunda.jpg");
        let bad = fixture.root.path().join("invalida.jpg");
        std::fs::write(&bad, b"not JPEG").unwrap();
        let attempt = fixture.attempt(vec![second.clone(), bad, first.clone(), second.clone()]);
        let outcomes = attempt
            .sources
            .iter()
            .filter(|source| source.candidate.path() == first || source.candidate.path() == second)
            .rev()
            .map(|source| (source.candidate.source_id.clone(), validated(source)))
            .collect();
        let before = crate::media_runtime::photo_source_decode_count();
        let prepared =
            prepare_proposal(attempt, None, outcomes, HashMap::new(), None, Vec::new()).unwrap();
        assert_eq!(crate::media_runtime::photo_source_decode_count(), before);
        assert_eq!(prepared.accepted_paths, [second, first]);
        assert_eq!(prepared.cache_problems.len(), 2);
        assert_eq!(prepared.proposal.problems.len(), 1);
        let result = fixture
            .host
            .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
            .unwrap();
        let ImportMediaResult::Completed {
            imported_count,
            media_ids,
            projection,
            ..
        } = result
        else {
            panic!("completed import")
        };
        assert_eq!(imported_count, 2);
        assert_eq!(media_ids.len(), 2);
        assert_eq!(projection.state.revision, 1);
        assert!(fixture.host.undo().unwrap().state.album.media.is_empty());
    }

    #[test]
    fn a_source_changed_after_proposal_is_rejected_at_commit_without_losing_its_neighbor() {
        let fixture = Fixture::new();
        let first = fixture.photo("alterada.jpg");
        let second = fixture.photo("valida.jpg");
        let attempt = fixture.attempt(vec![first.clone(), second.clone()]);
        let outcomes = attempt
            .sources
            .iter()
            .map(|source| (source.candidate.source_id.clone(), validated(source)))
            .collect();
        let prepared =
            prepare_proposal(attempt, None, outcomes, HashMap::new(), None, Vec::new()).unwrap();
        assert_eq!(prepared.proposal.commands.len(), 2);
        std::fs::write(first, b"changed after proposal, before Core commit").unwrap();
        let committed = commit_prepared_import(
            &fixture.host,
            &CacheEngine::default(),
            &MediaMonitor::default(),
            &MediaRuntime::default(),
            &CachePreviewRegistry::new("project"),
            prepared,
        )
        .unwrap();
        let ImportMediaResult::Completed {
            imported_count,
            problems,
            projection,
            ..
        } = committed.result
        else {
            panic!("healthy neighbor completes the action")
        };
        assert_eq!(imported_count, 1);
        assert_eq!(problems.len(), 1);
        assert_eq!(committed.new_paths, [second]);
        assert_eq!(projection.state.revision, 1);
        assert_eq!(projection.state.album.media[0].source_width_px, Some(37));
        assert!(fixture.host.undo().unwrap().state.album.media.is_empty());
    }

    #[test]
    fn selecting_an_existing_missing_original_keeps_its_identity_without_reinspection() {
        let fixture = Fixture::new();
        let first = fixture.photo("existente.jpg");
        let attempt = fixture.attempt(vec![first.clone()]);
        let outcomes = HashMap::from([(
            attempt.sources[0].candidate.source_id.clone(),
            validated(&attempt.sources[0]),
        )]);
        let prepared =
            prepare_proposal(attempt, None, outcomes, HashMap::new(), None, Vec::new()).unwrap();
        fixture
            .host
            .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
            .unwrap();
        std::fs::remove_file(&first).unwrap();
        let before = fixture.host.authorized_media_catalog().unwrap().bindings;
        let attempt = fixture.attempt(vec![first.clone(), first]);
        assert!(attempt.sources.is_empty());
        let prepared = prepare_proposal(
            attempt,
            None,
            HashMap::new(),
            HashMap::new(),
            None,
            Vec::new(),
        )
        .unwrap();
        assert!(prepared.proposal.problems.is_empty());
        let result = fixture
            .host
            .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
            .unwrap();
        assert!(matches!(
            result,
            ImportMediaResult::Completed {
                imported_count: 0,
                ..
            }
        ));
        assert_eq!(
            fixture.host.authorized_media_catalog().unwrap().bindings,
            before
        );
    }

    #[test]
    fn changed_source_requires_a_new_inspection_and_obsolete_project_cannot_commit() {
        let fixture = Fixture::new();
        let path = fixture.photo("alterada.jpg");
        let attempt = fixture.attempt(vec![path.clone()]);
        let outcomes = HashMap::from([(
            attempt.sources[0].candidate.source_id.clone(),
            validated(&attempt.sources[0]),
        )]);
        std::fs::write(&path, b"damaged after native completion").unwrap();
        let prepared =
            prepare_proposal(attempt, None, outcomes, HashMap::new(), None, Vec::new()).unwrap();
        assert!(prepared.proposal.commands.is_empty());
        assert_eq!(prepared.proposal.problems.len(), 1);
        let other = Fixture::new();
        assert!(
            other
                .host
                .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
                .is_err()
        );
        assert_eq!(other.host.projection().unwrap().state.revision, 0);
    }

    #[test]
    fn memory_pressure_keeps_partial_imports_and_allows_retry_without_duplicates() {
        let fixture = Fixture::new();
        let first = fixture.photo("preparada.jpg");
        let second = fixture.photo("pendente.jpg");
        let engine = CacheEngine::default();
        let blocked = ImagingProcessor::with_available_memory_for_test(256, 3393);
        let attempt = fixture.attempt(vec![first.clone(), second.clone()]);
        let roots = attempt.roots.clone();
        let outcomes = attempt
            .sources
            .iter()
            .filter(|source| source.candidate.path() == first)
            .map(|source| (source.candidate.source_id.clone(), validated(source)))
            .collect();
        let prepared = prepare_proposal_with_inspection(
            attempt,
            None,
            outcomes,
            HashMap::new(),
            None,
            Vec::new(),
            |candidate| {
                inspect_with_capacity(
                    MediaKind::Photo,
                    &engine,
                    &blocked,
                    candidate.path(),
                    &[],
                    &roots,
                )
            },
        )
        .unwrap();
        let ImportMediaResult::Completed {
            imported_count,
            problems,
            ..
        } = fixture
            .host
            .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
            .unwrap()
        else {
            panic!("partial import completes");
        };
        assert_eq!(imported_count, 1);
        assert_eq!(problems.len(), 1);
        assert_eq!(
            problems[0].reason,
            ProcessorAdmissionFailure::MemoryPressure.to_string()
        );
        let first_binding = fixture.host.authorized_media_catalog().unwrap().bindings[0].clone();

        let available = ImagingProcessor::with_available_memory_for_test(1699, 3393);
        let attempt = fixture.attempt(vec![first, second]);
        let roots = attempt.roots.clone();
        let bindings = attempt.catalog.bindings.clone();
        let prepared = prepare_proposal_with_inspection(
            attempt,
            None,
            HashMap::new(),
            HashMap::new(),
            None,
            Vec::new(),
            |candidate| {
                inspect_with_capacity(
                    MediaKind::Photo,
                    &engine,
                    &available,
                    candidate.path(),
                    &bindings,
                    &roots,
                )
            },
        )
        .unwrap();
        let ImportMediaResult::Completed {
            imported_count,
            problems,
            projection,
            ..
        } = fixture
            .host
            .commit_photo_import_proposal(fixture.namespace.project_id(), prepared.proposal)
            .unwrap()
        else {
            panic!("retry completes");
        };
        assert_eq!(imported_count, 1, "retry adds only the remaining image");
        assert!(problems.is_empty());
        assert_eq!(projection.state.album.media.len(), 2);
        assert_eq!(fixture.host.undo().unwrap().state.album.media.len(), 1);
        assert_eq!(
            fixture.host.authorized_media_catalog().unwrap().bindings[0].media_id,
            first_binding.media_id
        );
    }

    #[test]
    fn missing_native_results_use_the_frozen_inspector_and_keep_partial_success() {
        let fixture = Fixture::new();
        let first = fixture.photo("valida.jpg");
        let missing = fixture.root.path().join("ausente.jpg");
        let attempt = fixture.attempt(vec![first, missing]);
        let prepared = prepare_proposal(
            attempt,
            None,
            HashMap::new(),
            HashMap::new(),
            Some("Cache bloqueado".into()),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(prepared.proposal.commands.len(), 1);
        assert_eq!(prepared.proposal.problems.len(), 1);
        assert_eq!(prepared.cache_problems.len(), 1);
        assert_eq!(prepared.proposal.inspections.len(), 1);
    }
}
