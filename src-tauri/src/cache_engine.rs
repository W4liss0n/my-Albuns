use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use myalbuns_core::{MediaKind, ProjectIdentityAuthority};
use myalbuns_imaging_protocol::{
    CACHE_REPRESENTATION_VERSION, CacheArtifact, CacheArtifactFormat, CacheArtifactProperties,
    CacheBasicColorProfile, CacheCompletion, CacheFingerprint, CacheJob, CacheMediaSource,
    CacheRepresentationPolicy, CacheRequest, CacheReusableGeneration, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingResponse,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{
    AppPaths, CachePathPlan, PreparedCacheStorage, RootBindingPlan, project_data_namespace,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::{
    cache_activity_gate::{CacheActivityGate, CacheCancellation, CachePause, CacheWorkPermit},
    imaging_processor::{
        ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
        InvocationFailure, InvocationFailureStage, OperationFailure,
    },
};

const CACHE_METADATA_SCHEMA_VERSION: u32 = 5;
const SRGB_PROFILE: &[u8] = include_bytes!("../../crates/myalbuns-imaging/assets/sRGB2014.icc");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheMetadata {
    schema_version: u32,
    representation_version: u32,
    project_id: String,
    last_used_unix_ms: u64,
    policy: CacheRepresentationPolicy,
    entries: Vec<CacheMetadataEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheMetadataEntry {
    media_id: String,
    generation_id: String,
    artifact_name: String,
    width_px: u32,
    height_px: u32,
    preview_bytes: u64,
    format: CacheArtifactFormat,
    exif_orientation: Option<u8>,
    source_page_count: Option<u32>,
    basic_color_profile: CacheBasicColorProfile,
    fingerprint: CacheFingerprint,
}

impl CacheMetadataEntry {
    fn reusable(&self) -> Result<CacheReusableGeneration, String> {
        CacheReusableGeneration::new(
            self.generation_id.clone(),
            CacheArtifactProperties::new(
                self.format,
                self.width_px,
                self.height_px,
                self.preview_bytes,
                self.exif_orientation,
                self.source_page_count,
                self.basic_color_profile,
            ),
            self.fingerprint.clone(),
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthorizedCacheNamespace {
    project_id: String,
    cache_paths: CachePathPlan,
}

impl AuthorizedCacheNamespace {
    pub(crate) fn mount(
        app_paths: &AppPaths,
        authority: &ProjectIdentityAuthority,
    ) -> Result<Self, CacheFailure> {
        let project_id = authority.project_id().hyphenated().to_string();
        let cache_paths = app_paths
            .project_cache(&project_data_namespace(&project_id))
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::Plan,
                    format!("Não foi possível montar o namespace autorizado do Cache: {error}"),
                )
            })?;
        Ok(Self {
            project_id,
            cache_paths,
        })
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    pub(crate) fn paths(&self) -> &CachePathPlan {
        &self.cache_paths
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CacheWork {
    pub(crate) request_id: String,
    pub(crate) namespace: AuthorizedCacheNamespace,
    pub(crate) source: CacheMediaSource,
    pub(crate) root_bindings: RootBindingPlan,
}

impl CacheWork {
    pub(crate) fn new(
        request_id: impl Into<String>,
        namespace: AuthorizedCacheNamespace,
        source: CacheMediaSource,
        root_bindings: RootBindingPlan,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            namespace,
            source,
            root_bindings,
        }
    }

    fn flight_key(&self) -> CacheFlightKey {
        CacheFlightKey {
            project_id: self.namespace.project_id.clone(),
            media_id: self.source.media_id().to_owned(),
            source_path: self.source.source_path().to_path_buf(),
            decorative: self.source.kind() == MediaKind::Decorative,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CacheFlightKey {
    project_id: String,
    media_id: String,
    source_path: PathBuf,
    decorative: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheFailureStage {
    Plan,
    Processor(InvocationFailureStage),
    RecoveryCleanup,
    ValidateResponse,
    VerifyArtifacts,
    PublishIndex,
    Cancelled,
}

pub(crate) type CacheFailure = OperationFailure<CacheFailureStage>;

#[derive(Clone, Debug)]
pub(crate) struct CacheExecution {
    pub(crate) completion: CacheCompletion,
    pub(crate) recovery: Option<CacheRecovery>,
}

impl CacheExecution {
    pub(crate) fn artifact(&self) -> &CacheArtifact {
        &self.completion.artifacts[0]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheRecovery {
    pub(crate) failed_process_id: u32,
    pub(crate) removed_temporary_count: usize,
}

type FlightResult = Result<CacheExecution, CacheFailure>;

#[derive(Debug, Default)]
pub(crate) struct CacheEngine {
    flights: Arc<Mutex<HashMap<CacheFlightKey, Arc<CacheFlight>>>>,
    demands: Mutex<HashMap<String, CacheDemandState>>,
    active_owners: Arc<AtomicUsize>,
    activity: CacheActivityGate,
    metadata: Mutex<()>,
}

#[derive(Debug, Default)]
struct CacheDemandState {
    revision: u64,
    media_ids: HashSet<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CacheDemandRevision {
    project_id: String,
    revision: u64,
    accepted: bool,
    retired_media_ids: Vec<String>,
}

impl CacheDemandRevision {
    pub(crate) fn retired_media_ids(&self) -> &[String] {
        &self.retired_media_ids
    }
}

#[derive(Debug)]
struct CacheFlight {
    project_id: String,
    media_id: String,
    cancellation: CacheCancellation,
    result: Mutex<Option<FlightResult>>,
    completed: Notify,
}

pub(crate) enum CacheFlightClaim {
    Owner(CacheFlightOwner),
    Waiter(CacheFlightWaiter),
}

pub(crate) struct CacheFlightOwner {
    key: CacheFlightKey,
    flight: Arc<CacheFlight>,
    flights: Arc<Mutex<HashMap<CacheFlightKey, Arc<CacheFlight>>>>,
    active_owners: Arc<AtomicUsize>,
    completed: bool,
}

pub(crate) struct CacheFlightWaiter {
    flight: Arc<CacheFlight>,
}

impl CacheEngine {
    pub(crate) async fn begin_cancellable_work(
        &self,
        cancellation: CacheCancellation,
    ) -> CacheWorkPermit {
        self.activity.begin_cancellable_work(cancellation).await
    }

    pub(crate) async fn pause(&self) -> CachePause {
        self.activity.pause().await
    }

    pub(crate) async fn execute<T: ImagingTransport>(
        &self,
        transport: &mut T,
        app_paths: &AppPaths,
        work: CacheWork,
        context: &InvocationContext,
        cancellation: &CacheCancellation,
    ) -> Result<CacheExecution, CacheFailure> {
        execute_cache(self, transport, app_paths, work, context, cancellation).await
    }

    pub(crate) fn reconcile_demand<'a>(
        &self,
        project_id: &str,
        revision: u64,
        demanded_media_ids: impl IntoIterator<Item = &'a str>,
    ) -> CacheDemandRevision {
        let demanded = demanded_media_ids
            .into_iter()
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        let _metadata_guard = self
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let accepted = match demands.get(project_id) {
            None => true,
            Some(current) if revision > current.revision => true,
            Some(current) if revision == current.revision => current.media_ids == demanded,
            Some(_) => false,
        };
        let mut retired_media_ids = if accepted {
            demands
                .get(project_id)
                .into_iter()
                .flat_map(|current| current.media_ids.difference(&demanded))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        retired_media_ids.sort_unstable();
        if accepted {
            demands.insert(
                project_id.to_owned(),
                CacheDemandState {
                    revision,
                    media_ids: demanded.clone(),
                },
            );
            let mut flights = self
                .flights
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            flights.retain(|_, flight| {
                if flight.project_id == project_id && !demanded.contains(flight.media_id.as_str()) {
                    flight.cancellation.cancel_obsolete();
                    return false;
                }
                true
            });
        }
        CacheDemandRevision {
            project_id: project_id.to_owned(),
            revision,
            accepted,
            retired_media_ids,
        }
    }

    pub(crate) fn demand_is_current(&self, demand: &CacheDemandRevision) -> bool {
        if !demand.accepted {
            return false;
        }
        self.demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&demand.project_id)
            .is_some_and(|current| current.revision == demand.revision)
    }

    pub(crate) fn claim_demanded(
        &self,
        demand: &CacheDemandRevision,
        work: &CacheWork,
    ) -> Option<CacheFlightClaim> {
        if !demand.accepted || demand.project_id != work.namespace.project_id {
            return None;
        }
        let demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = demands.get(&demand.project_id)?;
        if current.revision != demand.revision
            || !current.media_ids.contains(work.source.media_id())
        {
            return None;
        }
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Some(self.claim_locked(work, &mut flights))
    }

    fn claim_locked(
        &self,
        work: &CacheWork,
        flights: &mut HashMap<CacheFlightKey, Arc<CacheFlight>>,
    ) -> CacheFlightClaim {
        let key = work.flight_key();
        if let Some(flight) = flights.get(&key) {
            return CacheFlightClaim::Waiter(CacheFlightWaiter {
                flight: Arc::clone(flight),
            });
        }
        let flight = Arc::new(CacheFlight {
            project_id: work.namespace.project_id().to_owned(),
            media_id: work.source.media_id().to_owned(),
            cancellation: CacheCancellation::default(),
            result: Mutex::new(None),
            completed: Notify::new(),
        });
        flights.insert(key.clone(), Arc::clone(&flight));
        self.active_owners.fetch_add(1, Ordering::AcqRel);
        CacheFlightClaim::Owner(CacheFlightOwner {
            key,
            flight,
            flights: Arc::clone(&self.flights),
            active_owners: Arc::clone(&self.active_owners),
            completed: false,
        })
    }

    pub(crate) fn invalidate_media<I, S>(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        media_ids: I,
    ) -> Result<usize, CacheFailure>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let media_ids = media_ids
            .into_iter()
            .map(|media_id| media_id.as_ref().to_owned())
            .collect::<HashSet<_>>();
        if media_ids.is_empty() {
            return Ok(0);
        }
        let _metadata_guard = self
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let mut flights = self
                .flights
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            flights.retain(|_, flight| {
                if flight.project_id == namespace.project_id && media_ids.contains(&flight.media_id)
                {
                    flight.cancellation.cancel_obsolete();
                    return false;
                }
                true
            });
        }
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::PublishIndex,
                    format!("Não foi possível preparar a invalidação do Cache: {error}"),
                )
            })?;
        let mut entries = current_entries(&storage, namespace.project_id(), namespace.paths());
        let invalidated_paths = entries
            .iter()
            .filter(|entry| media_ids.contains(&entry.media_id))
            .filter_map(|entry| entry_path(namespace.paths(), entry).ok())
            .collect::<Vec<_>>();
        entries.retain(|entry| !media_ids.contains(&entry.media_id));
        let metadata = current_metadata(namespace.project_id(), entries)?;
        publish_metadata(&storage, namespace.paths(), &metadata)?;
        let mut removed = 0;
        for path in invalidated_paths {
            removed += usize::from(storage.remove_existing_file(&path).map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::PublishIndex,
                    format!("Não foi possível remover a geração invalidada: {error}"),
                )
            })?);
        }
        if self.can_sweep_while_idle() {
            removed += sweep_unreferenced_generations(&storage, namespace.paths(), &metadata)?;
        }
        Ok(removed)
    }

    fn can_sweep_while_idle(&self) -> bool {
        self.active_owners.load(Ordering::Acquire) == 0
    }

    fn can_sweep_after_publication(&self) -> bool {
        self.active_owners.load(Ordering::Acquire) <= 1
    }
}

impl CacheFlightOwner {
    pub(crate) fn cancellation(&self) -> CacheCancellation {
        self.flight.cancellation.clone()
    }

    pub(crate) fn complete(mut self, result: FlightResult) -> FlightResult {
        *self
            .flight
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(result.clone());
        self.remove_flight();
        self.active_owners.fetch_sub(1, Ordering::AcqRel);
        self.flight.completed.notify_waiters();
        self.completed = true;
        result
    }

    fn remove_flight(&self) {
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if flights
            .get(&self.key)
            .is_some_and(|current| Arc::ptr_eq(current, &self.flight))
        {
            flights.remove(&self.key);
        }
    }
}

impl Drop for CacheFlightOwner {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let failure = CacheFailure::new(
            CacheFailureStage::Cancelled,
            "O proprietário do trabalho de Cache terminou antes de publicar um resultado.",
        );
        *self
            .flight
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Err(failure));
        self.remove_flight();
        self.active_owners.fetch_sub(1, Ordering::AcqRel);
        self.flight.completed.notify_waiters();
    }
}

impl CacheFlightWaiter {
    pub(crate) async fn wait(self) -> FlightResult {
        loop {
            let notified = self.flight.completed.notified();
            if let Some(result) = self
                .flight
                .result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
            {
                return result;
            }
            notified.await;
        }
    }
}

async fn execute_cache<T: ImagingTransport>(
    engine: &CacheEngine,
    transport: &mut T,
    app_paths: &AppPaths,
    work: CacheWork,
    context: &InvocationContext,
    cancellation: &CacheCancellation,
) -> Result<CacheExecution, CacheFailure> {
    let request = {
        let _metadata_guard = engine
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plan_request(app_paths, &work)?
    };
    let command = ImagingCommand::build_cache(request.clone());
    let (response, recovery) = invoke_with_recovery(
        transport,
        app_paths,
        work.namespace.paths(),
        &command,
        context,
        cancellation,
    )
    .await?;
    if let Some(failure) = response.failure_for(&work.request_id) {
        return Err(CacheFailure::new(
            CacheFailureStage::Processor(InvocationFailureStage::Processor(failure.code.stage())),
            "O Processador recusou o trabalho de Cache.",
        ));
    }
    let completion = response
        .cache_completed_for(&work.request_id)
        .cloned()
        .ok_or_else(|| {
            CacheFailure::new(
                CacheFailureStage::ValidateResponse,
                "O Processador devolveu uma resposta de Cache inesperada.",
            )
        })?;
    let storage = app_paths
        .prepare_cache_storage(work.namespace.paths())
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar o Cache: {error}"),
            )
        })?;
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request);
        return Err(cancelled_after_processor());
    }
    verify_completion(&storage, &request, &completion)?;
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request);
        return Err(cancelled_after_processor());
    }
    let _metadata_guard = engine
        .metadata
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request);
        return Err(cancelled_after_processor());
    }
    let metadata = publish_cache_metadata(&storage, &request, &completion.artifacts[0])?;
    if engine.can_sweep_after_publication() {
        sweep_unreferenced_generations(&storage, &request.cache_paths, &metadata)?;
    }
    Ok(CacheExecution {
        completion,
        recovery,
    })
}

fn cancelled_after_processor() -> CacheFailure {
    CacheFailure::new(
        CacheFailureStage::Cancelled,
        "O trabalho de Cache ficou obsoleto ou pausado antes da publicação do índice.",
    )
}

fn discard_candidate_generation(storage: &PreparedCacheStorage, request: &CacheRequest) {
    let job = &request.jobs[0];
    for format in [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png] {
        let Ok(path) = request.cache_paths.preview_file(
            job.source.media_id(),
            &job.candidate_generation_id,
            format,
        ) else {
            continue;
        };
        if let Err(error) = storage.remove_existing_file(&path) {
            tracing::warn!(
                target: "myalbuns.desktop",
                media_id = job.source.media_id(),
                generation_id = job.candidate_generation_id,
                error = %error,
                event = "cache_cancelled_generation_cleanup_failed",
            );
        }
    }
}

fn plan_request(app_paths: &AppPaths, work: &CacheWork) -> Result<CacheRequest, CacheFailure> {
    let storage = app_paths
        .prepare_cache_storage(work.namespace.paths())
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::Plan,
                format!("Não foi possível ler o índice do Cache: {error}"),
            )
        })?;
    let reusable = load_metadata(&storage, work.namespace.paths())
        .filter(|metadata| {
            metadata_is_current(
                metadata,
                work.namespace.project_id(),
                work.namespace.paths(),
            )
        })
        .and_then(|metadata| {
            metadata
                .entries
                .into_iter()
                .find(|entry| entry.media_id == work.source.media_id())
        })
        .and_then(|entry| entry.reusable().ok());
    let candidate_generation_id = format!("g-{}", uuid::Uuid::new_v4().simple());
    let job =
        CacheJob::new(work.source.clone(), candidate_generation_id, reusable).map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::Plan,
                format!("Não foi possível planejar o Cache: {error}"),
            )
        })?;
    CacheRequest::new(
        work.request_id.clone(),
        work.namespace.project_id().to_owned(),
        work.namespace.paths().clone(),
        vec![job],
        CacheRepresentationPolicy::measured_v1(),
        work.root_bindings.clone(),
    )
    .map_err(|error| {
        CacheFailure::new(
            CacheFailureStage::Plan,
            format!("Não foi possível planejar o Cache: {error}"),
        )
    })
}

async fn invoke_with_recovery<T: ImagingTransport>(
    transport: &mut T,
    app_paths: &AppPaths,
    cache_paths: &CachePathPlan,
    command: &ImagingCommand,
    context: &InvocationContext,
    cancellation: &CacheCancellation,
) -> Result<(ImagingResponse, Option<CacheRecovery>), CacheFailure> {
    let mut attempt = 1_u8;
    let mut recovery = None;
    let progress = |_| {};
    loop {
        match transport
            .invoke(
                command,
                context,
                ImagingOperation::Cache,
                attempt,
                InvocationControl::controlled(cancellation.flag(), &progress),
            )
            .await
        {
            Ok(response) => {
                if attempt > 1 {
                    tracing::info!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id.as_str(),
                        project_id = context.project_id.as_deref(),
                        attempts = attempt,
                        event = "imaging_processor_restart_completed",
                    );
                }
                return Ok((response, recovery));
            }
            Err(failure) if failure.is_cancelled() => {
                if let Some(process_id) = failure.process_id {
                    app_paths
                        .discard_project_cache_temporaries(cache_paths, process_id)
                        .map_err(|error| CacheFailure {
                            stage: CacheFailureStage::RecoveryCleanup,
                            exit_code: failure.exit_code,
                            message: format!(
                                "Não foi possível descartar o item cancelado do Cache: {error}"
                            ),
                        })?;
                }
                return Err(cache_processor_failure(failure));
            }
            Err(failure) if failure.is_unexpected_termination() => {
                let Some(failed_process_id) = failure.process_id else {
                    return Err(cache_processor_failure(failure));
                };
                let removed_temporary_count = app_paths
                    .discard_project_cache_temporaries(cache_paths, failed_process_id)
                    .map_err(|error| CacheFailure {
                        stage: CacheFailureStage::RecoveryCleanup,
                        exit_code: failure.exit_code,
                        message: format!(
                            "Não foi possível descartar o item incompleto do Cache: {error}"
                        ),
                    })?;
                if cancellation
                    .flag()
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(cancelled_after_processor());
                }
                if attempt == 1 {
                    recovery = Some(CacheRecovery {
                        failed_process_id,
                        removed_temporary_count,
                    });
                    attempt += 1;
                } else {
                    return Err(cache_processor_failure(failure));
                }
            }
            Err(failure) => return Err(cache_processor_failure(failure)),
        }
    }
}

fn cache_processor_failure(failure: InvocationFailure) -> CacheFailure {
    CacheFailure::from_invocation(failure, CacheFailureStage::Processor)
}

fn verify_completion(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    completion: &CacheCompletion,
) -> Result<(), CacheFailure> {
    if completion.artifacts.len() != 1
        || completion.generated_count + completion.reused_count != 1
        || completion.preview_bytes != completion.artifacts[0].preview_bytes
        || completion.source_bytes != completion.artifacts[0].fingerprint.source_bytes
    {
        return Err(CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            "A conclusão do Cache não corresponde ao trabalho solicitado.",
        ));
    }

    let job = &request.jobs[0];
    let artifact = &completion.artifacts[0];
    artifact.fingerprint.validate().map_err(|error| {
        CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            format!("O Processador devolveu um fingerprint inválido: {error}"),
        )
    })?;
    let generated = artifact.generation_id == job.candidate_generation_id;
    let reused = job.reusable.as_ref().is_some_and(|reusable| {
        artifact.generation_id == reusable.generation_id
            && artifact.fingerprint == reusable.fingerprint
            && artifact.format == reusable.format
    });
    if artifact.media_id != job.source.media_id()
        || (!generated && !reused)
        || completion.generated_count != usize::from(generated)
        || completion.reused_count != usize::from(reused)
        || artifact.width_px == 0
        || artifact.height_px == 0
        || artifact.width_px > request.policy.max_edge_px
        || artifact.height_px > request.policy.max_edge_px
        || artifact.preview_bytes == 0
    {
        return Err(CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            "A conclusão contém um artefato de Cache inesperado.",
        ));
    }
    verify_artifact_file(storage, request, artifact)
}

fn verify_artifact_file(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    artifact: &CacheArtifact,
) -> Result<(), CacheFailure> {
    let preview_path = request
        .cache_paths
        .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("O caminho do artefato de Cache é inválido: {error}"),
            )
        })?;
    let file = storage
        .open_existing_file(&preview_path)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar a prévia do Cache: {error}"),
            )
        })?
        .ok_or_else(|| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                "A prévia concluída não foi encontrada.",
            )
        })?;
    if file
        .metadata()
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar a prévia do Cache: {error}"),
            )
        })?
        .len()
        != artifact.preview_bytes
    {
        return Err(CacheFailure::new(
            CacheFailureStage::VerifyArtifacts,
            "A prévia concluída não corresponde à resposta recebida.",
        ));
    }
    let reader = ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| invalid_artifact(error.to_string()))?;
    if reader.format() != Some(image_format(artifact.format)) {
        return Err(invalid_artifact(
            "o formato detectado não corresponde ao índice".into(),
        ));
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| invalid_artifact(error.to_string()))?;
    if decoder.dimensions() != (artifact.width_px, artifact.height_px)
        || decoder
            .icc_profile()
            .map_err(|error| invalid_artifact(error.to_string()))?
            .as_deref()
            != Some(SRGB_PROFILE)
    {
        return Err(invalid_artifact(
            "dimensões ou perfil sRGB não correspondem à resposta".into(),
        ));
    }
    DynamicImage::from_decoder(decoder).map_err(|error| invalid_artifact(error.to_string()))?;
    Ok(())
}

fn invalid_artifact(message: String) -> CacheFailure {
    CacheFailure::new(
        CacheFailureStage::VerifyArtifacts,
        format!("A representação reduzida publicada é inválida: {message}"),
    )
}

fn publish_cache_metadata(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    artifact: &CacheArtifact,
) -> Result<CacheMetadata, CacheFailure> {
    let artifact_path = request
        .cache_paths
        .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("O caminho do artefato de Cache é inválido: {error}"),
            )
        })?;
    let artifact_name = artifact_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                "O nome do artefato de Cache é inválido.",
            )
        })?
        .to_owned();
    let mut entries = load_metadata(storage, &request.cache_paths)
        .filter(|metadata| metadata_is_current(metadata, &request.project_id, &request.cache_paths))
        .map(|metadata| metadata.entries)
        .unwrap_or_default();
    let superseded_artifact = entries
        .iter()
        .find(|entry| entry.media_id == artifact.media_id)
        .filter(|entry| {
            entry.generation_id != artifact.generation_id || entry.format != artifact.format
        })
        .and_then(|entry| {
            request
                .cache_paths
                .preview_file(&entry.media_id, &entry.generation_id, entry.format)
                .ok()
        });
    entries.retain(|entry| entry.media_id != artifact.media_id);
    entries.push(CacheMetadataEntry {
        media_id: artifact.media_id.clone(),
        generation_id: artifact.generation_id.clone(),
        artifact_name,
        width_px: artifact.width_px,
        height_px: artifact.height_px,
        preview_bytes: artifact.preview_bytes,
        format: artifact.format,
        exif_orientation: artifact.exif_orientation,
        source_page_count: artifact.source_page_count,
        basic_color_profile: artifact.basic_color_profile,
        fingerprint: artifact.fingerprint.clone(),
    });
    let metadata = current_metadata(&request.project_id, entries)?;
    publish_metadata(storage, &request.cache_paths, &metadata)?;
    if let Some(superseded_artifact) = superseded_artifact
        && let Err(error) = storage.remove_existing_file(&superseded_artifact)
    {
        tracing::warn!(
            target: "myalbuns.desktop",
            media_id = artifact.media_id.as_str(),
            generation_id = artifact.generation_id.as_str(),
            error = %error,
            event = "cache_superseded_generation_cleanup_failed",
        );
    }
    Ok(metadata)
}

fn current_entries(
    storage: &PreparedCacheStorage,
    project_id: &str,
    cache_paths: &CachePathPlan,
) -> Vec<CacheMetadataEntry> {
    load_metadata(storage, cache_paths)
        .filter(|metadata| metadata_is_current(metadata, project_id, cache_paths))
        .map(|metadata| metadata.entries)
        .unwrap_or_default()
}

fn current_metadata(
    project_id: &str,
    mut entries: Vec<CacheMetadataEntry>,
) -> Result<CacheMetadata, CacheFailure> {
    entries.sort_by(|left, right| left.media_id.cmp(&right.media_id));
    let last_used_unix_ms = unix_millis(SystemTime::now()).ok_or_else(|| {
        CacheFailure::new(
            CacheFailureStage::PublishIndex,
            "O relógio do sistema não representa o último uso do Cache.",
        )
    })?;
    Ok(CacheMetadata {
        schema_version: CACHE_METADATA_SCHEMA_VERSION,
        representation_version: CACHE_REPRESENTATION_VERSION,
        project_id: project_id.to_owned(),
        last_used_unix_ms,
        policy: CacheRepresentationPolicy::measured_v1(),
        entries,
    })
}

fn publish_metadata(
    storage: &PreparedCacheStorage,
    cache_paths: &CachePathPlan,
    metadata: &CacheMetadata,
) -> Result<(), CacheFailure> {
    let metadata_path = cache_paths.metadata_file();
    let temporary_path = cache_paths.metadata_temporary_file(std::process::id());
    let metadata_bytes = serde_json::to_vec_pretty(metadata).map_err(|error| {
        CacheFailure::new(
            CacheFailureStage::PublishIndex,
            format!("Não foi possível serializar o índice: {error}"),
        )
    })?;
    let mut publication = storage
        .begin_file_publication(&temporary_path, &metadata_path)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível criar o índice temporário: {error}"),
            )
        })?;
    publication.write_all(&metadata_bytes).map_err(|error| {
        CacheFailure::new(
            CacheFailureStage::PublishIndex,
            format!("Não foi possível gravar o índice temporário: {error}"),
        )
    })?;
    publication
        .sync()
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível sincronizar o índice: {error}"),
            )
        })?
        .publish()
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível publicar o índice: {error}"),
            )
        })
}

fn entry_path(
    cache_paths: &CachePathPlan,
    entry: &CacheMetadataEntry,
) -> Result<std::path::PathBuf, CacheFailure> {
    cache_paths
        .preview_file(&entry.media_id, &entry.generation_id, entry.format)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("O índice contém uma geração inválida: {error}"),
            )
        })
}

fn sweep_unreferenced_generations(
    storage: &PreparedCacheStorage,
    cache_paths: &CachePathPlan,
    metadata: &CacheMetadata,
) -> Result<usize, CacheFailure> {
    let referenced = metadata
        .entries
        .iter()
        .map(|entry| entry_path(cache_paths, entry))
        .collect::<Result<HashSet<_>, _>>()?;
    storage
        .remove_unreferenced_generations(&referenced)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível remover gerações órfãs do Cache: {error}"),
            )
        })
}

fn load_metadata(
    storage: &PreparedCacheStorage,
    cache_paths: &CachePathPlan,
) -> Option<CacheMetadata> {
    let file = storage
        .open_existing_file(&cache_paths.metadata_file())
        .ok()??;
    serde_json::from_reader(file).ok()
}

fn metadata_is_current(
    metadata: &CacheMetadata,
    project_id: &str,
    cache_paths: &CachePathPlan,
) -> bool {
    metadata.schema_version == CACHE_METADATA_SCHEMA_VERSION
        && metadata.representation_version == CACHE_REPRESENTATION_VERSION
        && metadata.project_id == project_id
        && metadata.policy == CacheRepresentationPolicy::measured_v1()
        && metadata.entries.iter().all(|entry| {
            entry.reusable().is_ok()
                && cache_paths
                    .preview_file(&entry.media_id, &entry.generation_id, entry.format)
                    .ok()
                    .and_then(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .map(str::to_owned)
                    })
                    .as_deref()
                    == Some(entry.artifact_name.as_str())
        })
}

const fn image_format(format: CacheArtifactFormat) -> ImageFormat {
    match format {
        CacheArtifactFormat::Jpeg => ImageFormat::Jpeg,
        CacheArtifactFormat::Png => ImageFormat::Png,
    }
}

fn unix_millis(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io::Write};

    use image::{
        DynamicImage, ExtendedColorType, ImageEncoder, Rgba, RgbaImage,
        codecs::{jpeg::JpegEncoder, png::PngEncoder},
    };
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, EditableProject, InitialProject, MediaKind,
        ProjectCore, ProjectLocation,
    };
    use myalbuns_imaging_protocol::{
        CacheArtifact, CacheArtifactFormat, CacheBasicColorProfile, CacheCompletion,
        CacheFingerprint, CacheMediaSource, ImagingCommand, ImagingFailureStage, ImagingResponse,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext};
    use sha2::{Digest, Sha256};

    use super::{
        AuthorizedCacheNamespace, CacheEngine, CacheFailureStage, CacheFlightClaim, CacheWork,
    };
    use crate::{
        cache_activity_gate::CacheCancellation,
        imaging_processor::{
            ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
            InvocationFailure, InvocationFuture,
        },
    };

    const SRGB_PROFILE: &[u8] = include_bytes!("../../crates/myalbuns-imaging/assets/sRGB2014.icc");

    enum Script {
        Complete(CacheArtifactFormat),
        Crash(u32),
        CrashAndObsolete(u32, CacheCancellation),
        Cancel(u32),
        Deterministic(u32),
    }

    struct ScriptedTransport {
        app_paths: AppPaths,
        scripts: VecDeque<Script>,
        attempts: Vec<u8>,
    }

    impl ImagingTransport for ScriptedTransport {
        fn invoke<'a>(
            &'a mut self,
            command: &'a ImagingCommand,
            _context: &'a InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
            _control: InvocationControl<'a>,
        ) -> InvocationFuture<'a> {
            assert_eq!(operation, ImagingOperation::Cache);
            self.attempts.push(attempt);
            let script = self.scripts.pop_front().expect("one script per invocation");
            let result = match script {
                Script::Complete(format) => complete(command, &self.app_paths, format),
                Script::Crash(process_id) => {
                    write_partial(command, &self.app_paths, process_id);
                    Err(InvocationFailure::unexpected_termination(process_id))
                }
                Script::CrashAndObsolete(process_id, cancellation) => {
                    write_partial(command, &self.app_paths, process_id);
                    cancellation.cancel_obsolete();
                    Err(InvocationFailure::unexpected_termination(process_id))
                }
                Script::Cancel(process_id) => {
                    write_partial(command, &self.app_paths, process_id);
                    Err(InvocationFailure::cancelled(process_id))
                }
                Script::Deterministic(process_id) => Err(InvocationFailure::deterministic(
                    ImagingFailureStage::CacheProcessing,
                    process_id,
                )),
            };
            Box::pin(async move { result })
        }
    }

    struct Fixture {
        _root: tempfile::TempDir,
        _project: EditableProject,
        app_paths: AppPaths,
        work: CacheWork,
        context: InvocationContext,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().expect("temporary CacheEngine fixture");
        let roaming = root.path().join("roaming");
        let local = root.path().join("local");
        std::fs::create_dir_all(&roaming).expect("the roaming root is available");
        std::fs::create_dir_all(&local).expect("the local root is available");
        let app_paths = AppPaths::from_roots(&roaming, &local, root.path());
        let project_path = root.path().join("Projeto.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, project_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable Project is authorized");
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("the authorized namespace mounts");
        let source_path = root.path().join("photo.jpg");
        std::fs::write(&source_path, b"original-photo-v1")
            .expect("the Original fixture is writable");
        let source = myalbuns_imaging_protocol::CacheMediaSource::new(
            "photo-a",
            MediaKind::Photo,
            source_path,
        )
        .expect("the Cache source is valid");
        let mut operation_context = OperationPathContext::new();
        operation_context
            .capture(namespace.paths().root())
            .expect("the Cache root is captured");
        operation_context
            .capture(source.source_path())
            .expect("the Original root is captured");
        let project_id = namespace.project_id().to_owned();
        Fixture {
            _root: root,
            _project: project,
            app_paths,
            work: CacheWork::new("cache-test", namespace, source, operation_context.freeze()),
            context: InvocationContext::new("cache-test", Some(project_id)),
        }
    }

    fn complete(
        command: &ImagingCommand,
        app_paths: &AppPaths,
        requested_format: CacheArtifactFormat,
    ) -> Result<ImagingResponse, InvocationFailure> {
        let ImagingCommand::BuildCache(request) = command else {
            panic!("the scripted transport accepts Cache only");
        };
        let job = &request.jobs[0];
        let source_metadata =
            std::fs::metadata(job.source.source_path()).expect("the adapter inspects the Original");
        let source =
            std::fs::read(job.source.source_path()).expect("the adapter opens the Original");
        let fingerprint = CacheFingerprint::sha256_full_file_with_timestamps(
            source.len() as u64,
            source_metadata.created().ok().and_then(super::unix_millis),
            source_metadata.modified().ok().and_then(super::unix_millis),
            format!("{:x}", Sha256::digest(&source)),
        )
        .expect("the adapter fingerprint is valid");
        let artifact = if let Some(reusable) = job
            .reusable
            .as_ref()
            .filter(|reusable| reusable.fingerprint == fingerprint)
        {
            CacheArtifact {
                media_id: job.source.media_id().to_owned(),
                generation_id: reusable.generation_id.clone(),
                width_px: reusable.width_px,
                height_px: reusable.height_px,
                preview_bytes: reusable.preview_bytes,
                format: reusable.format,
                exif_orientation: reusable.exif_orientation,
                source_page_count: reusable.source_page_count,
                basic_color_profile: reusable.basic_color_profile,
                fingerprint,
            }
        } else {
            publish_generated_artifact(app_paths, request, requested_format, fingerprint)
        };
        let reused = job
            .reusable
            .as_ref()
            .is_some_and(|reusable| reusable.generation_id == artifact.generation_id);
        Ok(ImagingResponse::cache_completed(
            request.request_id.clone(),
            CacheCompletion {
                source_bytes: artifact.fingerprint.source_bytes,
                preview_bytes: artifact.preview_bytes,
                artifacts: vec![artifact],
                generated_count: usize::from(!reused),
                reused_count: usize::from(reused),
            },
        ))
    }

    fn publish_generated_artifact(
        app_paths: &AppPaths,
        request: &myalbuns_imaging_protocol::CacheRequest,
        format: CacheArtifactFormat,
        fingerprint: CacheFingerprint,
    ) -> CacheArtifact {
        let job = &request.jobs[0];
        let image = RgbaImage::from_pixel(10, 5, Rgba([20, 40, 60, 255]));
        let mut encoded = Vec::new();
        match format {
            CacheArtifactFormat::Jpeg => {
                let mut encoder = JpegEncoder::new_with_quality(&mut encoded, 84);
                encoder
                    .set_icc_profile(SRGB_PROFILE.to_vec())
                    .expect("the fixture ICC profile is accepted");
                encoder
                    .encode_image(&DynamicImage::ImageRgba8(image).to_rgb8())
                    .expect("the fixture JPEG encodes");
            }
            CacheArtifactFormat::Png => {
                let mut encoder = PngEncoder::new(&mut encoded);
                encoder
                    .set_icc_profile(SRGB_PROFILE.to_vec())
                    .expect("the fixture ICC profile is accepted");
                encoder
                    .write_image(image.as_raw(), 10, 5, ExtendedColorType::Rgba8)
                    .expect("the fixture PNG encodes");
            }
        }
        let final_path = request
            .cache_paths
            .preview_file(job.source.media_id(), &job.candidate_generation_id, format)
            .expect("the artifact path is valid");
        let temporary_path = request
            .cache_paths
            .preview_temporary_file(
                job.source.media_id(),
                &job.candidate_generation_id,
                format,
                9001,
            )
            .expect("the temporary artifact path is valid");
        let storage = app_paths
            .prepare_cache_storage(&request.cache_paths)
            .expect("the Cache storage is prepared");
        let mut publication = storage
            .begin_file_publication(&temporary_path, &final_path)
            .expect("the derived publication starts");
        publication
            .write_all(&encoded)
            .expect("the derived bytes are written");
        publication
            .sync()
            .expect("the derived bytes are synchronized")
            .publish()
            .expect("the immutable generation is published");
        CacheArtifact {
            media_id: job.source.media_id().to_owned(),
            generation_id: job.candidate_generation_id.clone(),
            width_px: 10,
            height_px: 5,
            preview_bytes: encoded.len() as u64,
            format,
            exif_orientation: (format == CacheArtifactFormat::Jpeg).then_some(1),
            source_page_count: None,
            basic_color_profile: CacheBasicColorProfile::Srgb,
            fingerprint,
        }
    }

    fn write_partial(command: &ImagingCommand, app_paths: &AppPaths, process_id: u32) {
        let ImagingCommand::BuildCache(request) = command else {
            panic!("the scripted transport accepts Cache only");
        };
        let job = &request.jobs[0];
        let temporary_path = request
            .cache_paths
            .preview_temporary_file(
                job.source.media_id(),
                &job.candidate_generation_id,
                CacheArtifactFormat::Jpeg,
                process_id,
            )
            .expect("the partial path is valid");
        let storage = app_paths
            .prepare_cache_storage(&request.cache_paths)
            .expect("the Cache storage is prepared");
        std::fs::write(temporary_path, b"incomplete").expect("the partial artifact is written");
        drop(storage);
    }

    #[test]
    fn cache_namespace_can_only_be_mounted_from_editable_identity_authority() {
        let root = tempfile::tempdir().expect("temporary authority fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable Project is authorized");
        let app_paths = myalbuns_paths::AppPaths::from_roots(
            &root.path().join("roaming"),
            &root.path().join("local"),
            root.path(),
        );

        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("the authority mounts one Cache namespace");

        let expected =
            myalbuns_paths::project_data_namespace(&project.project_id().hyphenated().to_string());
        assert_eq!(
            namespace.paths().root().file_name(),
            Some(std::ffi::OsStr::new(&expected)),
            "the only project directory key is project-<sha256>"
        );
    }

    #[test]
    fn equivalent_demands_share_one_flight_and_obsolete_media_is_cancelled() {
        let root = tempfile::tempdir().expect("temporary flight fixture");
        let paths = myalbuns_paths::AppPaths::from_roots(
            &root.path().join("roaming"),
            &root.path().join("local"),
            root.path(),
        );
        let project_path = root.path().join("Projeto.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, project_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable Project is authorized");
        let namespace = AuthorizedCacheNamespace::mount(&paths, project.identity_authority())
            .expect("the Cache namespace is authorized");
        let source_path = root.path().join("photo.jpg");
        let source = myalbuns_imaging_protocol::CacheMediaSource::new(
            "photo-a",
            myalbuns_core::MediaKind::Photo,
            source_path.clone(),
        )
        .expect("the source is valid");
        let mut context = OperationPathContext::new();
        context
            .capture(namespace.paths().root())
            .expect("the Cache root is captured");
        context
            .capture(&source_path)
            .expect("the source root is captured");
        let work = CacheWork::new("cache-a", namespace, source, context.freeze());
        let engine = CacheEngine::default();

        let demand = engine.reconcile_demand(work.namespace.project_id(), 1, ["photo-a"]);
        let CacheFlightClaim::Owner(owner) = engine
            .claim_demanded(&demand, &work)
            .expect("the current demand can claim its flight")
        else {
            panic!("the first equivalent demand owns the flight");
        };
        assert!(matches!(
            engine.claim_demanded(&demand, &work),
            Some(CacheFlightClaim::Waiter(_))
        ));
        let emptied = engine.reconcile_demand(work.namespace.project_id(), 2, std::iter::empty());
        assert_eq!(emptied.retired_media_ids(), ["photo-a"]);
        assert!(
            owner
                .cancellation()
                .flag()
                .load(std::sync::atomic::Ordering::Acquire)
        );
    }

    #[test]
    fn authoritative_demand_revision_rejects_queued_and_late_obsolete_work() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let project_id = fixture.work.namespace.project_id().to_owned();
        let older = engine.reconcile_demand(&project_id, 7, ["photo-a", "photo-b"]);
        let CacheFlightClaim::Owner(photo_a_owner) = engine
            .claim_demanded(&older, &fixture.work)
            .expect("the current demand can claim Photo A")
        else {
            panic!("the first current demand owns Photo A");
        };

        let newer = engine.reconcile_demand(&project_id, 8, ["photo-c"]);
        assert_eq!(
            photo_a_owner.cancellation().reason(),
            Some(crate::cache_activity_gate::CacheCancellationReason::Obsolete)
        );
        let photo_b = CacheWork::new(
            "cache-photo-b",
            fixture.work.namespace.clone(),
            CacheMediaSource::new(
                "photo-b",
                MediaKind::Photo,
                fixture.work.source.source_path().to_path_buf(),
            )
            .expect("Photo B is a valid Cache source"),
            fixture.work.root_bindings.clone(),
        );
        assert!(
            engine.claim_demanded(&older, &photo_b).is_none(),
            "an older sequential invocation cannot start its next queued item"
        );

        let photo_c = CacheWork::new(
            "cache-photo-c",
            fixture.work.namespace.clone(),
            CacheMediaSource::new(
                "photo-c",
                MediaKind::Photo,
                fixture.work.source.source_path().to_path_buf(),
            )
            .expect("Photo C is a valid Cache source"),
            fixture.work.root_bindings.clone(),
        );
        let CacheFlightClaim::Owner(photo_c_owner) = engine
            .claim_demanded(&newer, &photo_c)
            .expect("the newest demand can claim Photo C")
        else {
            panic!("the newest demand owns Photo C");
        };

        let delayed_older = engine.reconcile_demand(&project_id, 7, ["photo-a", "photo-b"]);
        assert!(engine.claim_demanded(&delayed_older, &photo_b).is_none());
        assert_eq!(
            photo_c_owner.cancellation().reason(),
            None,
            "a late older command cannot cancel a newer flight"
        );
    }

    #[test]
    fn invalidated_flight_is_detached_before_revalidation_claims_a_replacement() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let demand = engine.reconcile_demand(
            fixture.work.namespace.project_id(),
            1,
            [fixture.work.source.media_id()],
        );
        let CacheFlightClaim::Owner(invalidated_owner) = engine
            .claim_demanded(&demand, &fixture.work)
            .expect("the current demand can claim its flight")
        else {
            panic!("the initial demand owns its flight");
        };

        engine
            .invalidate_media(
                &fixture.app_paths,
                &fixture.work.namespace,
                [fixture.work.source.media_id()],
            )
            .expect("reactive invalidation succeeds");
        assert_eq!(
            invalidated_owner.cancellation().reason(),
            Some(crate::cache_activity_gate::CacheCancellationReason::Obsolete)
        );
        let CacheFlightClaim::Owner(revalidated_owner) = engine
            .claim_demanded(&demand, &fixture.work)
            .expect("the current demand can revalidate its flight")
        else {
            panic!("revalidation owns a fresh flight instead of waiting on the cancelled one");
        };

        drop(invalidated_owner);
        assert!(
            matches!(
                engine.claim_demanded(&demand, &fixture.work),
                Some(CacheFlightClaim::Waiter(_))
            ),
            "completion of the detached flight cannot remove its replacement"
        );
        drop(revalidated_owner);
    }

    #[test]
    fn invalidation_preserves_an_unpublished_generation_owned_by_an_unrelated_flight() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let demand = engine.reconcile_demand(
            fixture.work.namespace.project_id(),
            1,
            [fixture.work.source.media_id()],
        );
        let CacheFlightClaim::Owner(owner) = engine
            .claim_demanded(&demand, &fixture.work)
            .expect("the current demand can claim its flight")
        else {
            panic!("the first demand owns its flight");
        };
        let candidate = fixture
            .work
            .namespace
            .paths()
            .preview_file("photo-a", "g-active-flight", CacheArtifactFormat::Jpeg)
            .expect("the active candidate path is valid");
        let storage = fixture
            .app_paths
            .prepare_cache_storage(fixture.work.namespace.paths())
            .expect("the Cache storage is prepared");
        std::fs::write(&candidate, b"candidate owned by the active flight")
            .expect("the active candidate exists before publication");

        engine
            .invalidate_media(
                &fixture.app_paths,
                &fixture.work.namespace,
                ["unrelated-media"],
            )
            .expect("targeted invalidation succeeds");

        assert!(
            candidate.exists(),
            "invalidation cannot sweep a generation that an active unrelated flight may publish"
        );
        drop(owner);
        drop(storage);
    }

    #[test]
    fn cache_engine_publishes_index_last_reuses_and_invalidates_only_the_requested_media() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::Complete(CacheArtifactFormat::Jpeg),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };
            let cancellation = CacheCancellation::default();
            let engine = CacheEngine::default();

            let first = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work.clone(),
                    &fixture.context,
                    &cancellation,
                )
                .await
                .expect("the first generation is published");
            assert_eq!(first.completion.generated_count, 1);
            let first_artifact = first.artifact().clone();
            let first_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    &first_artifact.media_id,
                    &first_artifact.generation_id,
                    first_artifact.format,
                )
                .expect("the first artifact path is central");
            assert!(first_path.is_file());
            assert!(fixture.work.namespace.paths().metadata_file().is_file());
            let orphan = fixture
                .work
                .namespace
                .paths()
                .preview_file("orphan-media", "g-orphan", CacheArtifactFormat::Jpeg)
                .expect("the orphan generation path is valid");
            std::fs::write(&orphan, b"unreferenced-generation")
                .expect("the orphan fixture is writable");

            let mut second_work = fixture.work.clone();
            second_work.request_id = "cache-second".into();
            let second = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    second_work,
                    &InvocationContext::new(
                        "cache-second",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &cancellation,
                )
                .await
                .expect("the current immutable generation is reused");
            assert_eq!(second.completion.reused_count, 1);
            assert_eq!(
                second.artifact().generation_id,
                first_artifact.generation_id
            );
            assert!(
                !orphan.exists(),
                "a successful index publication sweeps orphans"
            );

            std::fs::write(fixture.work.source.source_path(), b"original-photo-v2")
                .expect("the Original is replaced in place");
            let mut third_work = fixture.work.clone();
            third_work.request_id = "cache-third".into();
            let third = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    third_work,
                    &InvocationContext::new(
                        "cache-third",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &cancellation,
                )
                .await
                .expect("the changed Original receives a new generation");
            assert_eq!(third.completion.generated_count, 1);
            assert_ne!(third.artifact().generation_id, first_artifact.generation_id);
            assert_ne!(third.artifact().fingerprint, first_artifact.fingerprint);
            assert!(
                !first_path.exists(),
                "invalidation removes only the superseded generation after publishing the new index"
            );

            let third_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    &third.artifact().media_id,
                    &third.artifact().generation_id,
                    third.artifact().format,
                )
                .expect("the current Photo A path is central");
            let media_b = CacheMediaSource::new(
                "photo-b",
                MediaKind::Photo,
                fixture.work.source.source_path().to_path_buf(),
            )
            .expect("the second media source is valid");
            let fourth_work = CacheWork::new(
                "cache-fourth",
                fixture.work.namespace.clone(),
                media_b,
                fixture.work.root_bindings.clone(),
            );
            let fourth = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fourth_work,
                    &InvocationContext::new(
                        "cache-fourth",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &cancellation,
                )
                .await
                .expect("a second media receives its own generation");
            let fourth_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    &fourth.artifact().media_id,
                    &fourth.artifact().generation_id,
                    fourth.artifact().format,
                )
                .expect("the current Photo B path is central");
            engine
                .invalidate_media(&fixture.app_paths, &fixture.work.namespace, ["photo-a"])
                .expect("targeted invalidation publishes a rebuilt index");
            assert!(!third_path.exists());
            assert!(fourth_path.is_file());

            let metadata_bytes = std::fs::read(fixture.work.namespace.paths().metadata_file())
                .expect("the disposable index is readable");
            let metadata: serde_json::Value = serde_json::from_slice(&metadata_bytes)
                .expect("the disposable index is valid JSON");
            assert_eq!(metadata["schemaVersion"], 5);
            assert_eq!(metadata["representationVersion"], 1);
            assert_eq!(metadata["policy"]["maxEdgePx"], 1_600);
            assert_eq!(metadata["entries"].as_array().map(Vec::len), Some(1));
            assert_eq!(metadata["entries"][0]["mediaId"], "photo-b");
            assert!(metadata["entries"][0].get("kind").is_none());
            assert!(metadata["entries"][0]["fingerprint"]["sourceCreatedUnixMs"].is_u64());
            assert!(metadata["entries"][0]["fingerprint"]["sourceModifiedUnixMs"].is_u64());
            assert!(metadata["entries"][0]["sourcePageCount"].is_null());
            assert_eq!(metadata["entries"][0]["basicColorProfile"], "srgb");
            let serialized = String::from_utf8(metadata_bytes).expect("the index uses UTF-8");
            for forbidden in [
                "sourcePath",
                "rootBindings",
                "physicalIdentity",
                "available",
            ] {
                assert!(
                    !serialized.contains(forbidden),
                    "the disposable index cannot persist path observation {forbidden}"
                );
            }
            assert_eq!(transport.attempts, [1, 1, 1, 1]);
        });
    }

    #[test]
    fn cache_engine_recovers_one_crash_and_discards_only_that_process_temporary() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::Crash(4_242),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };

            let engine = CacheEngine::default();
            let execution = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect("one unexpected crash is restarted");

            assert_eq!(transport.attempts, [1, 2]);
            assert_eq!(
                execution.recovery,
                Some(super::CacheRecovery {
                    failed_process_id: 4_242,
                    removed_temporary_count: 1,
                })
            );
        });
    }

    #[test]
    fn cache_engine_does_not_restart_obsolete_work_after_a_processor_crash() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let cancellation = CacheCancellation::default();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::CrashAndObsolete(4_243, cancellation.clone()),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };

            let engine = CacheEngine::default();
            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &cancellation,
                )
                .await
                .expect_err("obsolete Cache work cannot be restarted");

            assert_eq!(failure.stage, CacheFailureStage::Cancelled);
            assert_eq!(transport.attempts, [1]);
        });
    }

    #[test]
    fn cache_engine_cancellation_cleans_its_temporary_and_does_not_publish_index() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let media_directory = fixture.work.namespace.paths().media_directory();
            let metadata_path = fixture.work.namespace.paths().metadata_file();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::Cancel(4_343)]),
                attempts: Vec::new(),
            };

            let engine = CacheEngine::default();
            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("cancelled Cache work remains a typed terminal");

            assert_eq!(
                failure.stage,
                CacheFailureStage::Processor(
                    crate::imaging_processor::InvocationFailureStage::Cancelled,
                )
            );
            assert_eq!(transport.attempts, [1]);
            assert!(!metadata_path.exists());
            let remaining_temporaries = std::fs::read_dir(media_directory)
                .expect("the Cache Media directory remains available")
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-4343"))
                .count();
            assert_eq!(remaining_temporaries, 0);
        });
    }

    #[test]
    fn obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let media_directory = fixture.work.namespace.paths().media_directory();
            let metadata_path = fixture.work.namespace.paths().metadata_file();
            let cancellation = CacheCancellation::default();
            cancellation.cancel_obsolete();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::Complete(CacheArtifactFormat::Jpeg)]),
                attempts: Vec::new(),
            };

            let engine = CacheEngine::default();
            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &cancellation,
                )
                .await
                .expect_err("an obsolete completed job is discarded before index publication");

            assert_eq!(failure.stage, CacheFailureStage::Cancelled);
            assert!(!metadata_path.exists());
            assert_eq!(
                std::fs::read_dir(media_directory)
                    .expect("the Cache Media directory remains available")
                    .filter_map(Result::ok)
                    .count(),
                0,
            );
        });
    }

    #[test]
    fn deterministic_processor_failure_is_not_retried() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::Deterministic(4_444)]),
                attempts: Vec::new(),
            };

            let engine = CacheEngine::default();
            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("deterministic failure remains visible");

            assert_eq!(
                failure.stage,
                CacheFailureStage::Processor(
                    crate::imaging_processor::InvocationFailureStage::Processor(
                        ImagingFailureStage::CacheProcessing,
                    ),
                )
            );
            assert_eq!(transport.attempts, [1]);
        });
    }
}
