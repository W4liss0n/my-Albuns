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
    cache_previews::CachePreviewRegistry,
    imaging_processor::{
        ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
        InvocationFailure, InvocationFailureStage, OperationFailure,
    },
    ipc_contract::{MediaPreview, MediaPreviewState},
    media_runtime::MediaRuntimeUpdate,
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

    fn artifact(&self) -> CacheArtifact {
        CacheArtifact {
            media_id: self.media_id.clone(),
            generation_id: self.generation_id.clone(),
            width_px: self.width_px,
            height_px: self.height_px,
            preview_bytes: self.preview_bytes,
            format: self.format,
            exif_orientation: self.exif_orientation,
            source_page_count: self.source_page_count,
            basic_color_profile: self.basic_color_profile,
            fingerprint: self.fingerprint.clone(),
        }
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
            kind: self.source.kind(),
            root_bindings: self.root_bindings.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CacheFlightKey {
    project_id: String,
    media_id: String,
    source_path: PathBuf,
    kind: MediaKind,
    root_bindings: RootBindingPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheFailureStage {
    Plan,
    Processor(InvocationFailureStage),
    ProcessorSuspended,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CacheNamespaceRecovery {
    pub(crate) removed_temporary_count: usize,
    pub(crate) removed_generation_count: usize,
    pub(crate) discarded_index: bool,
}

type FlightResult = Result<CacheExecution, CacheFailure>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CacheProcessorStatus {
    #[default]
    Ready,
    Suspended,
}

pub(crate) const CACHE_PROCESSOR_SUSPENDED_MESSAGE: &str =
    "O Cache foi suspenso após falhas repetidas do Processador de Imagens.";

#[derive(Debug, Default)]
pub(crate) struct CacheEngine {
    flights: Arc<Mutex<HashMap<CacheFlightKey, Arc<CacheFlight>>>>,
    demands: Mutex<HashMap<String, CacheDemandState>>,
    applied_observation_generations: Mutex<HashMap<(String, String), u64>>,
    active_owners: Arc<AtomicUsize>,
    activity: CacheActivityGate,
    processor_status: Mutex<CacheProcessorStatus>,
    /// Serializes Cache state transitions that must be atomic with preview or
    /// on-disk metadata publication.
    ///
    /// When both are needed, a `CacheActivityGate` permit is acquired first.
    /// This gate is then acquired before `demands`, observation generations,
    /// `flights`, the preview registry, or Cache metadata I/O. Code holding
    /// any of those inner resources must never attempt to enter this gate.
    transition_and_publication_gate: Mutex<()>,
}

#[derive(Debug, Default)]
struct CacheDemandState {
    revision: u64,
    media_ids: HashSet<String>,
    invalidation_epoch: uuid::Uuid,
    preview_publication_authorities: HashMap<String, CachePreviewPublicationAuthority>,
}

#[derive(Clone, Debug)]
pub(crate) struct CacheDemandRevision {
    project_id: String,
    revision: u64,
    invalidation_epoch: uuid::Uuid,
    accepted: bool,
    #[cfg(test)]
    retired_media_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheDemandMediaUpdate {
    demand_can_resume: bool,
    retry_required: bool,
}

impl CacheDemandMediaUpdate {
    pub(crate) fn demand_can_resume(self) -> bool {
        self.demand_can_resume
    }

    pub(crate) fn retry_required(self) -> bool {
        self.retry_required
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CacheMediaUpdateOutcome {
    removed_generation_count: usize,
    demand_can_resume: bool,
    update_applied: bool,
}

#[cfg(test)]
impl CacheDemandRevision {
    pub(crate) fn retired_media_ids(&self) -> &[String] {
        &self.retired_media_ids
    }
}

#[derive(Debug)]
struct CacheFlight {
    project_id: String,
    media_id: String,
    publication_id: uuid::Uuid,
    cancellation: CacheCancellation,
    result: Mutex<Option<FlightResult>>,
    completed: Notify,
}

pub(crate) enum CacheFlightClaim {
    Owner(CacheFlightOwner),
    Waiter(CacheFlightWaiter),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CachePreviewPublicationAuthority {
    key: CacheFlightKey,
    publication_id: uuid::Uuid,
}

pub(crate) struct CacheFlightOwner {
    key: CacheFlightKey,
    flight: Arc<CacheFlight>,
    flights: Arc<Mutex<HashMap<CacheFlightKey, Arc<CacheFlight>>>>,
    active_owners: Arc<AtomicUsize>,
    completed: bool,
}

pub(crate) struct CacheFlightWaiter {
    key: CacheFlightKey,
    flight: Arc<CacheFlight>,
}

impl CacheFlightClaim {
    pub(crate) fn preview_publication_authority(&self) -> CachePreviewPublicationAuthority {
        let (key, flight) = match self {
            Self::Owner(owner) => (owner.key.clone(), &owner.flight),
            Self::Waiter(waiter) => (waiter.key.clone(), &waiter.flight),
        };
        CachePreviewPublicationAuthority {
            key,
            publication_id: flight.publication_id,
        }
    }
}

impl CacheEngine {
    /// Recovers a namespace after its new Host acquired the exclusive
    /// reservation and before that Host can start a Processor.
    pub(crate) fn recover_reserved_namespace(
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
    ) -> Result<CacheNamespaceRecovery, CacheFailure> {
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::RecoveryCleanup,
                    format!("Não foi possível preparar a recuperação do Cache: {error}"),
                )
            })?;
        let removed_temporary_count = app_paths
            .discard_abandoned_project_cache_temporaries(namespace.paths())
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::RecoveryCleanup,
                    format!("Não foi possível remover temporários abandonados: {error}"),
                )
            })?;
        let metadata_path = namespace.paths().metadata_file();
        let metadata_present = storage
            .open_existing_file(&metadata_path)
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::RecoveryCleanup,
                    format!("Não foi possível inspecionar o índice descartável: {error}"),
                )
            })?
            .is_some();
        let metadata = load_metadata(&storage, namespace.paths()).filter(|metadata| {
            metadata_is_current(metadata, namespace.project_id(), namespace.paths())
        });
        let removed_generation_count = match metadata.as_ref() {
            Some(metadata) => {
                sweep_unreferenced_generations(&storage, namespace.paths(), metadata)?
            }
            None => storage
                .remove_unreferenced_generations(&HashSet::new())
                .map_err(|error| {
                    CacheFailure::new(
                        CacheFailureStage::RecoveryCleanup,
                        format!("Não foi possível descartar gerações sem índice: {error}"),
                    )
                })?,
        };
        let discarded_index = metadata.is_none() && metadata_present;
        if discarded_index {
            storage
                .remove_existing_file(&metadata_path)
                .map_err(|error| {
                    CacheFailure::new(
                        CacheFailureStage::RecoveryCleanup,
                        format!("Não foi possível descartar o índice incompatível: {error}"),
                    )
                })?;
        }
        Ok(CacheNamespaceRecovery {
            removed_temporary_count,
            removed_generation_count,
            discarded_index,
        })
    }

    pub(crate) fn processor_status(&self) -> CacheProcessorStatus {
        *self
            .processor_status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn suspend_processor(&self) {
        *self
            .processor_status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CacheProcessorStatus::Suspended;
    }

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

    pub(crate) fn reconcile_preview_demand<'a>(
        &self,
        registry: &CachePreviewRegistry,
        project_id: &str,
        revision: u64,
        demanded_media_ids: impl IntoIterator<Item = &'a str>,
    ) -> CacheDemandRevision {
        self.reconcile_demand_with(
            project_id,
            revision,
            demanded_media_ids,
            |retired_media_ids| {
                registry.invalidate_media(retired_media_ids.iter().map(String::as_str));
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn reconcile_demand<'a>(
        &self,
        project_id: &str,
        revision: u64,
        demanded_media_ids: impl IntoIterator<Item = &'a str>,
    ) -> CacheDemandRevision {
        self.reconcile_demand_with(project_id, revision, demanded_media_ids, |_| {})
    }

    fn reconcile_demand_with<'a>(
        &self,
        project_id: &str,
        revision: u64,
        demanded_media_ids: impl IntoIterator<Item = &'a str>,
        revoke_retired_previews: impl FnOnce(&[String]),
    ) -> CacheDemandRevision {
        let demanded = demanded_media_ids
            .into_iter()
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
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
        let invalidation_epoch = demands
            .get(project_id)
            .map(|current| current.invalidation_epoch)
            .unwrap_or_else(uuid::Uuid::nil);
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
                    invalidation_epoch,
                    preview_publication_authorities: HashMap::new(),
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
            revoke_retired_previews(&retired_media_ids);
        }
        CacheDemandRevision {
            project_id: project_id.to_owned(),
            revision,
            invalidation_epoch,
            accepted,
            #[cfg(test)]
            retired_media_ids,
        }
    }

    #[cfg(test)]
    pub(crate) fn commit_preview_if_demanded<T>(
        &self,
        demand: &CacheDemandRevision,
        media_id: &str,
        commit: impl FnOnce() -> T,
    ) -> Option<T> {
        if !demand.accepted {
            return None;
        }
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = demands.get(&demand.project_id)?;
        if current.revision != demand.revision
            || current.invalidation_epoch != demand.invalidation_epoch
            || !current.media_ids.contains(media_id)
        {
            return None;
        }
        Some(commit())
    }

    pub(crate) fn commit_claimed_preview_if_demanded<T>(
        &self,
        demand: &CacheDemandRevision,
        authority: &CachePreviewPublicationAuthority,
        commit: impl FnOnce() -> T,
    ) -> Option<T> {
        if !demand.accepted || demand.project_id != authority.key.project_id {
            return None;
        }
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = demands.get(&demand.project_id)?;
        if current.revision != demand.revision
            || current.invalidation_epoch != demand.invalidation_epoch
            || !current.media_ids.contains(&authority.key.media_id)
            || current
                .preview_publication_authorities
                .get(&authority.key.media_id)
                != Some(authority)
        {
            return None;
        }
        Some(commit())
    }

    pub(crate) fn demand_is_current(&self, demand: &CacheDemandRevision) -> bool {
        if !demand.accepted {
            return false;
        }
        self.demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&demand.project_id)
            .is_some_and(|current| {
                current.revision == demand.revision
                    && current.invalidation_epoch == demand.invalidation_epoch
            })
    }

    pub(crate) fn retain_last_known_preview(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        registry: &CachePreviewRegistry,
        demand: &CacheDemandRevision,
        media_id: &str,
        state: MediaPreviewState,
    ) -> Option<MediaPreview> {
        if state == MediaPreviewState::Ready {
            return None;
        }
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = demands.get(namespace.project_id())?;
        if !demand.accepted
            || demand.project_id != namespace.project_id()
            || current.revision != demand.revision
            || current.invalidation_epoch != demand.invalidation_epoch
            || !current.media_ids.contains(media_id)
        {
            return None;
        }
        if let Some(preview) = registry.retained_preview(media_id, state) {
            return Some(preview);
        }
        let storage = app_paths.prepare_cache_storage(namespace.paths()).ok()?;
        let metadata = load_metadata(&storage, namespace.paths())?;
        if !metadata_is_current(&metadata, namespace.project_id(), namespace.paths()) {
            return None;
        }
        let artifact = metadata
            .entries
            .iter()
            .find(|entry| entry.media_id == media_id)?
            .artifact();
        if verify_cached_artifact(&storage, namespace.paths(), &artifact).is_err() {
            return None;
        }
        registry.publish(app_paths, namespace, &artifact).ok()?;
        registry.retained_preview(media_id, state)
    }

    pub(crate) fn claim_demanded(
        &self,
        demand: &CacheDemandRevision,
        work: &CacheWork,
    ) -> Option<CacheFlightClaim> {
        if !demand.accepted || demand.project_id != work.namespace.project_id {
            return None;
        }
        // Lock order: transition/publication -> demand -> flights. A remapped attempt
        // must retire the old owner before that owner can enter its publication step.
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut demands = self
            .demands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = demands.get_mut(&demand.project_id)?;
        if current.revision != demand.revision
            || current.invalidation_epoch != demand.invalidation_epoch
            || !current.media_ids.contains(work.source.media_id())
        {
            return None;
        }
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let claim = self.claim_locked(work, &mut flights);
        let authority = claim.preview_publication_authority();
        current
            .preview_publication_authorities
            .insert(work.source.media_id().to_owned(), authority);
        Some(claim)
    }

    fn claim_locked(
        &self,
        work: &CacheWork,
        flights: &mut HashMap<CacheFlightKey, Arc<CacheFlight>>,
    ) -> CacheFlightClaim {
        let key = work.flight_key();
        if let Some(flight) = flights.get(&key) {
            return CacheFlightClaim::Waiter(CacheFlightWaiter {
                key,
                flight: Arc::clone(flight),
            });
        }
        flights.retain(|_, flight| {
            if flight.project_id == work.namespace.project_id()
                && flight.media_id == work.source.media_id()
            {
                flight.cancellation.cancel_obsolete();
                return false;
            }
            true
        });
        let flight = Arc::new(CacheFlight {
            project_id: work.namespace.project_id().to_owned(),
            media_id: work.source.media_id().to_owned(),
            publication_id: uuid::Uuid::new_v4(),
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

    pub(crate) fn apply_monitor_media_update(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        registry: &CachePreviewRegistry,
        update: &MediaRuntimeUpdate,
    ) -> Result<usize, CacheFailure> {
        let outcome = self.apply_media_update(app_paths, namespace, registry, update, None)?;
        Ok(outcome.removed_generation_count)
    }

    pub(crate) fn apply_demand_media_update(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        registry: &CachePreviewRegistry,
        demand: &mut CacheDemandRevision,
        update: &MediaRuntimeUpdate,
    ) -> Result<CacheDemandMediaUpdate, CacheFailure> {
        let outcome =
            self.apply_media_update(app_paths, namespace, registry, update, Some(demand))?;
        Ok(CacheDemandMediaUpdate {
            demand_can_resume: outcome.demand_can_resume,
            retry_required: outcome.update_applied && !outcome.demand_can_resume,
        })
    }

    fn apply_media_update(
        &self,
        _app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        registry: &CachePreviewRegistry,
        update: &MediaRuntimeUpdate,
        resume_demand: Option<&mut CacheDemandRevision>,
    ) -> Result<CacheMediaUpdateOutcome, CacheFailure> {
        let observation_generation = update.observation_generation();
        let mut changed = update
            .changed_media_ids()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let invalidated = update
            .invalidated_media_ids()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let revoked_previews = update
            .revoked_preview_media_ids()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        changed.extend(invalidated.iter().cloned());
        changed.extend(revoked_previews.iter().cloned());
        let _transition_and_publication_guard = self
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let applied_media_ids = {
            let generations = self
                .applied_observation_generations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            changed
                .iter()
                .filter(|media_id| {
                    observation_generation
                        > *generations
                            .get(&(namespace.project_id().to_owned(), (*media_id).clone()))
                            .unwrap_or(&0)
                })
                .cloned()
                .collect::<HashSet<_>>()
        };
        let update_applied = !applied_media_ids.is_empty();
        let demand_can_resume = {
            let mut demands = self
                .demands
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut demand_can_resume = resume_demand.is_none();
            match (demands.get_mut(namespace.project_id()), resume_demand) {
                (Some(current), Some(demand)) => {
                    demand_can_resume = demand.accepted
                        && demand.project_id == namespace.project_id()
                        && demand.revision == current.revision
                        && demand.invalidation_epoch == current.invalidation_epoch;
                    if !current.media_ids.is_disjoint(&applied_media_ids) {
                        current.invalidation_epoch = uuid::Uuid::new_v4();
                        if demand_can_resume {
                            demand.invalidation_epoch = current.invalidation_epoch;
                        }
                    }
                }
                (Some(current), None) if !current.media_ids.is_disjoint(&applied_media_ids) => {
                    current.invalidation_epoch = uuid::Uuid::new_v4();
                }
                (Some(_), None) | (None, None) | (None, Some(_)) => {}
            }
            demand_can_resume
        };
        if !update_applied {
            return Ok(CacheMediaUpdateOutcome {
                removed_generation_count: 0,
                demand_can_resume,
                update_applied,
            });
        }
        self.cancel_flights(namespace.project_id(), &applied_media_ids);
        let applied_revoked_previews = revoked_previews
            .intersection(&applied_media_ids)
            .cloned()
            .collect::<HashSet<_>>();
        registry.invalidate_media(applied_revoked_previews.iter().map(String::as_str));
        // A stable source change invalidates reuse and resident publication now,
        // but the indexed generation remains the last atomic Cache publication
        // until a verified successor replaces it. Planning revalidates its
        // fingerprint, so retaining it cannot make the stale bytes reusable.
        // `publish_cache_metadata` swaps the entry first and only then collects
        // the superseded file.
        let removed_generation_count = 0;
        {
            let mut generations = self
                .applied_observation_generations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for media_id in &applied_media_ids {
                generations.insert(
                    (namespace.project_id().to_owned(), media_id.clone()),
                    observation_generation,
                );
            }
        }
        Ok(CacheMediaUpdateOutcome {
            removed_generation_count,
            demand_can_resume,
            update_applied,
        })
    }

    fn cancel_flights(&self, project_id: &str, media_ids: &HashSet<String>) {
        let mut flights = self
            .flights
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        flights.retain(|_, flight| {
            if flight.project_id == project_id && media_ids.contains(&flight.media_id) {
                flight.cancellation.cancel_obsolete();
                return false;
            }
            true
        });
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
    if engine.processor_status() == CacheProcessorStatus::Suspended {
        return Err(CacheFailure::new(
            CacheFailureStage::ProcessorSuspended,
            CACHE_PROCESSOR_SUSPENDED_MESSAGE,
        ));
    }
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err(cancelled_before_publication());
    }
    let request = {
        let _transition_and_publication_guard = engine
            .transition_and_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        plan_request(app_paths, &work)?
    };
    let storage = app_paths
        .prepare_cache_storage(work.namespace.paths())
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar o Cache: {error}"),
            )
        })?;
    let (response, recovery) = invoke_with_recovery(
        engine,
        transport,
        app_paths,
        &storage,
        &request,
        context,
        cancellation,
    )
    .await?;
    if let Some(failure) = response.failure_for(&work.request_id) {
        discard_candidate_generation(&storage, &request)?;
        return Err(CacheFailure::new(
            CacheFailureStage::Processor(InvocationFailureStage::Processor(failure.code.stage())),
            "O Processador recusou o trabalho de Cache.",
        ));
    }
    let Some(completion) = response.cache_completed_for(&work.request_id).cloned() else {
        discard_candidate_generation(&storage, &request)?;
        return Err(CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            "O Processador devolveu uma resposta de Cache inesperada.",
        ));
    };
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request)?;
        return Err(cancelled_before_publication());
    }
    if let Err(failure) = verify_completion(&storage, &request, &completion) {
        discard_candidate_generation(&storage, &request)?;
        return Err(failure);
    }
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request)?;
        return Err(cancelled_before_publication());
    }
    let _transition_and_publication_guard = engine
        .transition_and_publication_gate
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if cancellation
        .flag()
        .load(std::sync::atomic::Ordering::Acquire)
    {
        discard_candidate_generation(&storage, &request)?;
        return Err(cancelled_before_publication());
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

fn cancelled_before_publication() -> CacheFailure {
    CacheFailure::new(
        CacheFailureStage::Cancelled,
        "O trabalho de Cache ficou obsoleto ou pausado antes da publicação do índice.",
    )
}

fn discard_candidate_generation(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
) -> Result<usize, CacheFailure> {
    let job = &request.jobs[0];
    let mut removed = 0;
    for format in [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png] {
        let Ok(path) = request.cache_paths.preview_file(
            job.source.media_id(),
            &job.candidate_generation_id,
            format,
        ) else {
            continue;
        };
        removed += usize::from(storage.remove_existing_file(&path).map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::RecoveryCleanup,
                format!("Não foi possível descartar a geração candidata do Cache: {error}"),
            )
        })?);
    }
    Ok(removed)
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
    engine: &CacheEngine,
    transport: &mut T,
    app_paths: &AppPaths,
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    context: &InvocationContext,
    cancellation: &CacheCancellation,
) -> Result<(ImagingResponse, Option<CacheRecovery>), CacheFailure> {
    let command = ImagingCommand::build_cache(request.clone());
    let mut attempt = 1_u8;
    let mut recovery = None;
    let progress = |_| {};
    loop {
        match transport
            .invoke(
                &command,
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
                        .discard_project_cache_temporaries(&request.cache_paths, process_id)
                        .map_err(|error| CacheFailure {
                            stage: CacheFailureStage::RecoveryCleanup,
                            exit_code: failure.exit_code,
                            message: format!(
                                "Não foi possível descartar o item cancelado do Cache: {error}"
                            ),
                        })?;
                }
                discard_candidate_generation(storage, request)?;
                return Err(cache_processor_failure(failure));
            }
            Err(failure) if failure.is_unexpected_termination() => {
                let repeated_failure = attempt > 1;
                if repeated_failure {
                    engine.suspend_processor();
                }
                let Some(failed_process_id) = failure.process_id else {
                    discard_candidate_generation(storage, request)?;
                    return Err(cache_processor_failure(failure));
                };
                let removed_temporary_count = app_paths
                    .discard_project_cache_temporaries(&request.cache_paths, failed_process_id)
                    .map_err(|error| CacheFailure {
                        stage: CacheFailureStage::RecoveryCleanup,
                        exit_code: failure.exit_code,
                        message: format!(
                            "Não foi possível descartar o item incompleto do Cache: {error}"
                        ),
                    })?;
                discard_candidate_generation(storage, request)?;
                if cancellation
                    .flag()
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err(cancelled_before_publication());
                }
                if !repeated_failure {
                    recovery = Some(CacheRecovery {
                        failed_process_id,
                        removed_temporary_count,
                    });
                    attempt += 1;
                } else {
                    return Err(cache_processor_failure(failure));
                }
            }
            Err(failure) => {
                discard_candidate_generation(storage, request)?;
                return Err(cache_processor_failure(failure));
            }
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
        || artifact
            .exif_orientation
            .is_some_and(|orientation| !(1..=8).contains(&orientation))
        || artifact
            .source_page_count
            .is_some_and(|page_count| page_count != 1)
    {
        return Err(CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            "A conclusão contém um artefato de Cache inesperado.",
        ));
    }
    verify_cached_artifact(storage, &request.cache_paths, artifact)
}

fn verify_cached_artifact(
    storage: &PreparedCacheStorage,
    cache_paths: &CachePathPlan,
    artifact: &CacheArtifact,
) -> Result<(), CacheFailure> {
    let preview_path = cache_paths
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
    if metadata.schema_version != CACHE_METADATA_SCHEMA_VERSION
        || metadata.representation_version != CACHE_REPRESENTATION_VERSION
        || metadata.project_id != project_id
        || metadata.policy != CacheRepresentationPolicy::measured_v1()
    {
        return false;
    }
    let mut media_ids = HashSet::new();
    metadata.entries.iter().all(|entry| {
        media_ids.insert(entry.media_id.as_str())
            && entry.reusable().is_ok()
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
    use std::{
        collections::VecDeque,
        io::Write,
        sync::{Arc, mpsc},
        thread,
        time::Duration,
    };

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
    use tauri::http::{Method, Request, StatusCode};

    use super::{
        AuthorizedCacheNamespace, CacheEngine, CacheFailureStage, CacheFlightClaim, CacheWork,
    };
    use crate::{
        cache_activity_gate::CacheCancellation,
        cache_previews::CachePreviewRegistry,
        imaging_processor::{
            ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
            InvocationFailure, InvocationFuture,
        },
        media_runtime::MediaRuntimeUpdate,
    };

    const SRGB_PROFILE: &[u8] = include_bytes!("../../crates/myalbuns-imaging/assets/sRGB2014.icc");

    enum Script {
        Complete(CacheArtifactFormat),
        MalformedCounts,
        MalformedOrientation,
        MalformedPageCount,
        WrongRequestId,
        Crash(u32),
        CrashWithInvalidCandidate(u32),
        PublishThenCrash(u32),
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
                Script::MalformedCounts => complete_with(
                    command,
                    &self.app_paths,
                    CacheArtifactFormat::Jpeg,
                    |completion| {
                        completion.generated_count = usize::MAX;
                        completion.reused_count = 1;
                    },
                ),
                Script::MalformedOrientation => complete_with(
                    command,
                    &self.app_paths,
                    CacheArtifactFormat::Jpeg,
                    |completion| completion.artifacts[0].exif_orientation = Some(9),
                ),
                Script::MalformedPageCount => complete_with(
                    command,
                    &self.app_paths,
                    CacheArtifactFormat::Jpeg,
                    |completion| completion.artifacts[0].source_page_count = Some(2),
                ),
                Script::WrongRequestId => {
                    let response = complete(command, &self.app_paths, CacheArtifactFormat::Jpeg);
                    response.map(|response| {
                        let ImagingResponse::CacheCompleted { completion, .. } = response else {
                            unreachable!("the scripted Cache completion has the expected kind")
                        };
                        ImagingResponse::cache_completed("another-request", completion)
                    })
                }
                Script::Crash(process_id) => {
                    write_partial(command, &self.app_paths, process_id);
                    Err(InvocationFailure::unexpected_termination(process_id))
                }
                Script::CrashWithInvalidCandidate(process_id) => {
                    let ImagingCommand::BuildCache(request) = command else {
                        panic!("the scripted transport accepts Cache only");
                    };
                    let job = &request.jobs[0];
                    let candidate = request
                        .cache_paths
                        .preview_file(
                            job.source.media_id(),
                            &job.candidate_generation_id,
                            CacheArtifactFormat::Jpeg,
                        )
                        .expect("the invalid candidate path is valid");
                    std::fs::create_dir(&candidate)
                        .expect("the invalid candidate fixture is created");
                    Err(InvocationFailure::unexpected_termination(process_id))
                }
                Script::PublishThenCrash(process_id) => {
                    let ImagingCommand::BuildCache(request) = command else {
                        panic!("the scripted transport accepts Cache only");
                    };
                    let job = &request.jobs[0];
                    let candidate = request
                        .cache_paths
                        .preview_file(
                            job.source.media_id(),
                            &job.candidate_generation_id,
                            CacheArtifactFormat::Jpeg,
                        )
                        .expect("the candidate path is valid");
                    if !candidate.exists() {
                        let source = std::fs::read(job.source.source_path())
                            .expect("the scripted adapter reads the Original");
                        let metadata = std::fs::metadata(job.source.source_path())
                            .expect("the scripted adapter inspects the Original");
                        let fingerprint = CacheFingerprint::sha256_full_file_with_timestamps(
                            source.len() as u64,
                            metadata.created().ok().and_then(super::unix_millis),
                            metadata.modified().ok().and_then(super::unix_millis),
                            format!("{:x}", Sha256::digest(source)),
                        )
                        .expect("the scripted fingerprint is valid");
                        publish_generated_artifact(
                            &self.app_paths,
                            request,
                            CacheArtifactFormat::Jpeg,
                            fingerprint,
                        );
                    }
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
        let app_paths = AppPaths::from_roots(&roaming, &local);
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
        complete_with(command, app_paths, requested_format, |_| {})
    }

    fn complete_with(
        command: &ImagingCommand,
        app_paths: &AppPaths,
        requested_format: CacheArtifactFormat,
        mutate: impl FnOnce(&mut CacheCompletion),
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
        let mut completion = CacheCompletion {
            source_bytes: artifact.fingerprint.source_bytes,
            preview_bytes: artifact.preview_bytes,
            artifacts: vec![artifact],
            generated_count: usize::from(!reused),
            reused_count: usize::from(reused),
        };
        mutate(&mut completion);
        Ok(ImagingResponse::cache_completed(
            request.request_id.clone(),
            completion,
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

    async fn verified_preview_artifact(fixture: &Fixture, engine: &CacheEngine) -> CacheArtifact {
        verified_preview_artifact_for_work(fixture, engine, &fixture.work, &fixture.context).await
    }

    async fn verified_preview_artifact_for_work(
        fixture: &Fixture,
        engine: &CacheEngine,
        work: &CacheWork,
        context: &InvocationContext,
    ) -> CacheArtifact {
        let mut transport = ScriptedTransport {
            app_paths: fixture.app_paths.clone(),
            scripts: VecDeque::from([Script::Complete(CacheArtifactFormat::Jpeg)]),
            attempts: Vec::new(),
        };
        engine
            .execute(
                &mut transport,
                &fixture.app_paths,
                work.clone(),
                context,
                &CacheCancellation::default(),
            )
            .await
            .expect("the in-flight demand produces one verified artifact")
            .artifact()
            .clone()
    }

    fn preview_status(registry: &CachePreviewRegistry, token: &str) -> StatusCode {
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{token}"))
            .body(Vec::new())
            .expect("the opaque request is valid");
        registry.serve("project", request).status()
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
    fn a_new_root_binding_plan_retires_the_older_flight() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let demand = engine.reconcile_demand(
                fixture.work.namespace.project_id(),
                1,
                [fixture.work.source.media_id()],
            );
            let work_for = |request_id: &str, operational_root: &std::path::Path| {
                let mut context = OperationPathContext::new();
                context
                    .capture_with_binding(fixture.work.source.source_path(), operational_root)
                    .expect("the attempt-specific operational root is captured");
                CacheWork::new(
                    request_id,
                    fixture.work.namespace.clone(),
                    fixture.work.source.clone(),
                    context.freeze(),
                )
            };
            let work_a = fixture.work.clone();
            let work_b = work_for("cache-plan-b", &fixture._root.path().join("binding-b"));

            let claim_a = engine
                .claim_demanded(&demand, &work_a)
                .expect("the first binding plan can claim a flight");
            let authority_a = claim_a.preview_publication_authority();
            let CacheFlightClaim::Owner(owner_a) = claim_a else {
                panic!("the first binding plan owns its flight");
            };
            let claim_b = engine
                .claim_demanded(&demand, &work_b)
                .expect("the remapped binding plan can claim a distinct flight");
            let authority_b = claim_b.preview_publication_authority();
            let CacheFlightClaim::Owner(owner_b) = claim_b else {
                panic!(
                    "a new attempt with a different binding plan must not wait on the old attempt"
                );
            };
            assert_eq!(
                owner_a.cancellation().reason(),
                Some(crate::cache_activity_gate::CacheCancellationReason::Obsolete),
                "the old attempt cannot publish after the remapped attempt starts"
            );

            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::new(),
                attempts: Vec::new(),
            };
            let old_cancellation = owner_a.cancellation();
            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    work_a,
                    &fixture.context,
                    &old_cancellation,
                )
                .await
                .expect_err("the retired binding plan cannot reopen the Original or publish");
            assert_eq!(failure.stage, CacheFailureStage::Cancelled);
            assert!(transport.attempts.is_empty());
            assert!(!fixture.work.namespace.paths().metadata_file().exists());
            assert!(
                engine
                    .commit_claimed_preview_if_demanded(&demand, &authority_a, || {
                        panic!("the retired plan cannot publish an opaque preview token")
                    })
                    .is_none()
            );
            assert_eq!(
                engine.commit_claimed_preview_if_demanded(&demand, &authority_b, || "current"),
                Some("current")
            );

            drop(owner_a);
            drop(owner_b);
        });
    }

    #[test]
    fn a_reclaimed_equivalent_flight_cannot_restore_the_previous_publication_authority() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let demand = engine.reconcile_demand(
            fixture.work.namespace.project_id(),
            1,
            [fixture.work.source.media_id()],
        );
        let first_claim = engine
            .claim_demanded(&demand, &fixture.work)
            .expect("the first equivalent attempt claims its flight");
        let first_authority = first_claim.preview_publication_authority();
        let CacheFlightClaim::Owner(first_owner) = first_claim else {
            panic!("the first equivalent attempt owns its flight");
        };
        drop(first_owner);

        let second_claim = engine
            .claim_demanded(&demand, &fixture.work)
            .expect("the equivalent attempt can be reclaimed after its owner terminates");
        let second_authority = second_claim.preview_publication_authority();
        assert_ne!(first_authority, second_authority);
        assert!(
            engine
                .commit_claimed_preview_if_demanded(&demand, &first_authority, || {
                    panic!("a completed incarnation cannot regain token publication authority")
                })
                .is_none()
        );
        assert_eq!(
            engine.commit_claimed_preview_if_demanded(&demand, &second_authority, || "latest"),
            Some("latest")
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
    fn a_newer_demand_revokes_a_preview_committed_by_an_inflight_old_revision() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = Arc::new(CacheEngine::default());
            let registry = Arc::new(CachePreviewRegistry::new("project"));
            let project_id = fixture.work.namespace.project_id().to_owned();
            let demand = engine.reconcile_preview_demand(
                &registry,
                &project_id,
                1,
                [fixture.work.source.media_id()],
            );
            let artifact = verified_preview_artifact(&fixture, &engine).await;

            let (commit_entered_tx, commit_entered_rx) = mpsc::channel();
            let (release_commit_tx, release_commit_rx) = mpsc::channel();
            let old_engine = Arc::clone(&engine);
            let old_registry = Arc::clone(&registry);
            let old_app_paths = fixture.app_paths.clone();
            let old_namespace = fixture.work.namespace.clone();
            let media_id = fixture.work.source.media_id().to_owned();
            let old_commit = thread::spawn(move || {
                old_engine.commit_preview_if_demanded(&demand, &media_id, || {
                    commit_entered_tx
                        .send(())
                        .expect("the test observes the serialized preview commit");
                    release_commit_rx
                        .recv()
                        .expect("the test releases the serialized preview commit");
                    old_registry.publish(&old_app_paths, &old_namespace, &artifact)
                })
            });
            commit_entered_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("the old revision reaches its preview commit");

            let newer_engine = Arc::clone(&engine);
            let newer_registry = Arc::clone(&registry);
            let (reconciled_tx, reconciled_rx) = mpsc::channel();
            let newer_reconciliation = thread::spawn(move || {
                newer_engine.reconcile_preview_demand(
                    &newer_registry,
                    &project_id,
                    2,
                    std::iter::empty(),
                );
                reconciled_tx
                    .send(())
                    .expect("the test observes the newer reconciliation");
            });
            assert!(
                reconciled_rx
                    .recv_timeout(Duration::from_millis(20))
                    .is_err(),
                "reconciliation waits until the old commit reaches one serial endpoint"
            );

            release_commit_tx
                .send(())
                .expect("the old serialized commit is released");
            let preview = old_commit
                .join()
                .expect("the old commit thread joins")
                .expect("the old revision was current when its commit began")
                .expect("the verified preview is published");
            newer_reconciliation
                .join()
                .expect("the newer reconciliation thread joins");
            reconciled_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("the newer demand completes after revocation");

            let token = preview
                .url
                .expect("the committed preview has an opaque URL")
                .rsplit('/')
                .next()
                .expect("the opaque URL has a token")
                .to_owned();
            let request = Request::builder()
                .method(Method::GET)
                .uri(format!("/{token}"))
                .body(Vec::new())
                .expect("the revoked opaque request is valid");
            assert_eq!(
                registry.serve("project", request).status(),
                StatusCode::NOT_FOUND,
                "the newer demand leaves no bytes or token resident"
            );
        });
    }

    #[test]
    fn a_monitor_invalidation_serializes_with_commit_and_expires_the_old_epoch() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = Arc::new(CacheEngine::default());
            let registry = Arc::new(CachePreviewRegistry::new("project"));
            let project_id = fixture.work.namespace.project_id().to_owned();
            let demand = engine.reconcile_preview_demand(
                &registry,
                &project_id,
                1,
                [fixture.work.source.media_id()],
            );
            let artifact = verified_preview_artifact(&fixture, &engine).await;

            let (commit_entered_tx, commit_entered_rx) = mpsc::channel();
            let (release_commit_tx, release_commit_rx) = mpsc::channel();
            let old_engine = Arc::clone(&engine);
            let old_registry = Arc::clone(&registry);
            let old_app_paths = fixture.app_paths.clone();
            let old_namespace = fixture.work.namespace.clone();
            let media_id = fixture.work.source.media_id().to_owned();
            let demand_for_commit = demand.clone();
            let old_commit = thread::spawn(move || {
                old_engine.commit_preview_if_demanded(&demand_for_commit, &media_id, || {
                    commit_entered_tx
                        .send(())
                        .expect("the test observes the serialized preview commit");
                    release_commit_rx
                        .recv()
                        .expect("the test releases the serialized preview commit");
                    old_registry.publish(&old_app_paths, &old_namespace, &artifact)
                })
            });
            commit_entered_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("the old revision reaches its preview commit");

            let monitor_engine = Arc::clone(&engine);
            let monitor_registry = Arc::clone(&registry);
            let monitor_app_paths = fixture.app_paths.clone();
            let monitor_namespace = fixture.work.namespace.clone();
            let (invalidated_tx, invalidated_rx) = mpsc::channel();
            let monitor_update = thread::spawn(move || {
                monitor_engine
                    .apply_monitor_media_update(
                        &monitor_app_paths,
                        &monitor_namespace,
                        &monitor_registry,
                        &MediaRuntimeUpdate::for_test(1, vec!["photo-a".to_owned()], vec![]),
                    )
                    .expect("the stable Monitor update revokes the resident preview");
                invalidated_tx
                    .send(())
                    .expect("the test observes the Monitor update");
            });
            assert!(
                invalidated_rx
                    .recv_timeout(Duration::from_millis(20))
                    .is_err(),
                "the Monitor update waits for the old commit's serial endpoint"
            );

            release_commit_tx
                .send(())
                .expect("the old serialized commit is released");
            let preview = old_commit
                .join()
                .expect("the old commit thread joins")
                .expect("the old revision was current when its commit began")
                .expect("the verified preview is published before invalidation");
            monitor_update
                .join()
                .expect("the Monitor update thread joins");
            invalidated_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("the Monitor update completes after revocation");

            let token = preview
                .url
                .expect("the committed preview has an opaque URL")
                .rsplit('/')
                .next()
                .expect("the opaque URL has a token")
                .to_owned();
            let request = Request::builder()
                .method(Method::GET)
                .uri(format!("/{token}"))
                .body(Vec::new())
                .expect("the revoked opaque request is valid");
            assert_eq!(
                registry.serve("project", request).status(),
                StatusCode::NOT_FOUND,
                "the Monitor update wins after the serialized old commit"
            );
            assert!(
                engine
                    .commit_preview_if_demanded(&demand, "photo-a", || {
                        panic!("an expired observation epoch cannot commit")
                    })
                    .is_none(),
                "the old demand cannot republish after Monitor invalidation"
            );
        });
    }

    #[test]
    fn a_monitor_update_cannot_be_adopted_by_an_older_demand_command() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let registry = CachePreviewRegistry::new("project");
        let project_id = fixture.work.namespace.project_id().to_owned();
        let mut old_demand = engine.reconcile_preview_demand(
            &registry,
            &project_id,
            1,
            [fixture.work.source.media_id()],
        );

        engine
            .apply_monitor_media_update(
                &fixture.app_paths,
                &fixture.work.namespace,
                &registry,
                &MediaRuntimeUpdate::for_test(
                    2,
                    vec![fixture.work.source.media_id().to_owned()],
                    vec![],
                ),
            )
            .expect("the Monitor wins and advances the invalidation epoch");
        let stale_update = engine
            .apply_demand_media_update(
                &fixture.app_paths,
                &fixture.work.namespace,
                &registry,
                &mut old_demand,
                &MediaRuntimeUpdate::for_test(
                    1,
                    vec![fixture.work.source.media_id().to_owned()],
                    vec![],
                ),
            )
            .expect("the old command observes that it lost authority");

        assert!(
            !stale_update.demand_can_resume() && !stale_update.retry_required(),
            "the command must report that the Monitor already won"
        );

        assert!(
            !engine.demand_is_current(&old_demand),
            "an old command cannot adopt the epoch established by the Monitor"
        );
        assert!(
            engine
                .commit_preview_if_demanded(&old_demand, fixture.work.source.media_id(), || {
                    panic!("the stale demand cannot regain publication authority")
                })
                .is_none()
        );
    }

    #[test]
    fn an_observation_owned_by_a_retired_command_is_applied_once_and_requests_retry() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let project_id = fixture.work.namespace.project_id().to_owned();
            let mut old_demand = engine.reconcile_preview_demand(
                &registry,
                &project_id,
                1,
                [fixture.work.source.media_id()],
            );
            let artifact = verified_preview_artifact(&fixture, &engine).await;
            let preview = engine
                .commit_preview_if_demanded(&old_demand, fixture.work.source.media_id(), || {
                    registry.publish(&fixture.app_paths, &fixture.work.namespace, &artifact)
                })
                .expect("the original demand is current")
                .expect("the original preview is resident");
            let token = preview
                .url
                .expect("the resident preview has an opaque URL")
                .rsplit('/')
                .next()
                .expect("the opaque URL has a token")
                .to_owned();

            engine.reconcile_preview_demand(
                &registry,
                &project_id,
                2,
                [fixture.work.source.media_id()],
            );
            let outcome = engine
                .apply_demand_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &mut old_demand,
                    &MediaRuntimeUpdate::for_test(
                        1,
                        vec![fixture.work.source.media_id().to_owned()],
                        vec![],
                    ),
                )
                .expect("the confirmed observation is applied despite its retired caller");

            assert!(!outcome.demand_can_resume());
            assert!(
                outcome.retry_required(),
                "the winning demand needs a new attempt after its epoch is invalidated"
            );
            let request = Request::builder()
                .method(Method::GET)
                .uri(format!("/{token}"))
                .body(Vec::new())
                .expect("the opaque request is valid");
            assert_eq!(
                registry.serve("project", request).status(),
                StatusCode::NOT_FOUND,
                "Candidate to Absent cannot leave an addressable preview resident"
            );
        });
    }

    #[test]
    fn absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let project_id = fixture.work.namespace.project_id().to_owned();
            let mut demand = engine.reconcile_preview_demand(
                &registry,
                &project_id,
                1,
                [fixture.work.source.media_id()],
            );
            let artifact = verified_preview_artifact(&fixture, &engine).await;
            let ready = registry
                .publish(&fixture.app_paths, &fixture.work.namespace, &artifact)
                .expect("the last known preview is resident");

            engine
                .apply_demand_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &mut demand,
                    &MediaRuntimeUpdate::for_test_preserving_previews(
                        1,
                        vec![fixture.work.source.media_id().to_owned()],
                    ),
                )
                .expect("the unavailable observation updates Runtime without revoking pixels");

            let unavailable = registry
                .retained_preview(
                    fixture.work.source.media_id(),
                    crate::ipc_contract::MediaPreviewState::Unavailable,
                )
                .expect("the unavailable state serves the last known representation");
            assert_eq!(unavailable.url, ready.url);
            let token = unavailable
                .url
                .expect("the retained preview remains opaque")
                .rsplit('/')
                .next()
                .expect("the opaque URL has a token")
                .to_owned();
            let request = Request::builder()
                .method(Method::GET)
                .uri(format!("/{token}"))
                .body(Vec::new())
                .expect("the retained opaque request is valid");
            assert_eq!(registry.serve("project", request).status(), StatusCode::OK);

            let reopened_registry = CachePreviewRegistry::new("project");
            let reopened = engine
                .retain_last_known_preview(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &reopened_registry,
                    &demand,
                    fixture.work.source.media_id(),
                    crate::ipc_contract::MediaPreviewState::Unavailable,
                )
                .expect("a new Host process can hydrate the last published generation");
            assert_eq!(
                reopened.state,
                crate::ipc_contract::MediaPreviewState::Unavailable
            );
            let reopened_token = reopened
                .url
                .expect("the hydrated preview remains opaque")
                .rsplit('/')
                .next()
                .expect("the hydrated URL has a token")
                .to_owned();
            assert_eq!(
                preview_status(&reopened_registry, &reopened_token),
                StatusCode::OK
            );

            let cache_failure = engine
                .retain_last_known_preview(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &CachePreviewRegistry::new("project"),
                    &demand,
                    fixture.work.source.media_id(),
                    crate::ipc_contract::MediaPreviewState::CacheUnavailable,
                )
                .expect("a Cache failure can retain G1 without classifying the Original");
            assert_eq!(
                cache_failure.state,
                crate::ipc_contract::MediaPreviewState::CacheUnavailable
            );
            assert!(cache_failure.url.is_some());

            let artifact_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
                .expect("the published artifact pathname is valid");
            let mut corrupt = std::fs::read(&artifact_path)
                .expect("the published artifact can be corrupted deterministically");
            corrupt.fill(0);
            std::fs::write(&artifact_path, corrupt)
                .expect("the malformed bytes preserve the indexed length");
            assert!(
                engine
                    .retain_last_known_preview(
                        &fixture.app_paths,
                        &fixture.work.namespace,
                        &CachePreviewRegistry::new("project"),
                        &demand,
                        fixture.work.source.media_id(),
                        crate::ipc_contract::MediaPreviewState::Absent,
                    )
                    .is_none(),
                "a malformed on-disk generation cannot be rehydrated after restart"
            );
        });
    }

    #[test]
    fn a_retired_demand_cannot_rehydrate_an_unavailable_preview_from_disk() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let project_id = fixture.work.namespace.project_id().to_owned();
            let old_demand = engine.reconcile_preview_demand(
                &registry,
                &project_id,
                1,
                [fixture.work.source.media_id()],
            );
            verified_preview_artifact(&fixture, &engine).await;
            engine.reconcile_preview_demand(&registry, &project_id, 2, std::iter::empty());

            assert!(
                engine
                    .retain_last_known_preview(
                        &fixture.app_paths,
                        &fixture.work.namespace,
                        &registry,
                        &old_demand,
                        fixture.work.source.media_id(),
                        crate::ipc_contract::MediaPreviewState::Unavailable,
                    )
                    .is_none(),
                "a removed medium cannot regain resident bytes through unavailable fallback"
            );
        });
    }

    #[test]
    fn out_of_order_observation_deltas_are_ordered_independently_per_media() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let project_id = fixture.work.namespace.project_id().to_owned();
            let photo_b_path = fixture._root.path().join("photo-b.jpg");
            std::fs::write(&photo_b_path, b"original-photo-b")
                .expect("the second Original fixture is writable");
            let source_b = CacheMediaSource::new("photo-b", MediaKind::Photo, photo_b_path)
                .expect("the second Cache source is valid");
            let mut operation_context_b = OperationPathContext::new();
            operation_context_b
                .capture(fixture.work.namespace.paths().root())
                .expect("the Cache root is captured for the second media");
            operation_context_b
                .capture(source_b.source_path())
                .expect("the second Original root is captured");
            let work_b = CacheWork::new(
                "cache-test-b",
                fixture.work.namespace.clone(),
                source_b,
                operation_context_b.freeze(),
            );
            let context_b = InvocationContext::new("cache-test-b", Some(project_id.clone()));
            let artifact_a = verified_preview_artifact(&fixture, &engine).await;
            let artifact_b =
                verified_preview_artifact_for_work(&fixture, &engine, &work_b, &context_b).await;
            let demand =
                engine.reconcile_preview_demand(&registry, &project_id, 1, ["photo-a", "photo-b"]);
            let preview_a = engine
                .commit_preview_if_demanded(&demand, "photo-a", || {
                    registry.publish(&fixture.app_paths, &fixture.work.namespace, &artifact_a)
                })
                .expect("the demand authorizes photo-a")
                .expect("photo-a is resident");
            let preview_b = engine
                .commit_preview_if_demanded(&demand, "photo-b", || {
                    registry.publish(&fixture.app_paths, &fixture.work.namespace, &artifact_b)
                })
                .expect("the demand authorizes photo-b")
                .expect("photo-b is resident");
            let token_a = preview_a
                .url
                .expect("photo-a has an opaque URL")
                .rsplit('/')
                .next()
                .expect("photo-a has a token")
                .to_owned();
            let token_b = preview_b
                .url
                .expect("photo-b has an opaque URL")
                .rsplit('/')
                .next()
                .expect("photo-b has a token")
                .to_owned();

            engine
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(11, vec!["photo-b".to_owned()], vec![]),
                )
                .expect("generation 11 applies to photo-b first");
            engine
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(10, vec!["photo-a".to_owned()], vec![]),
                )
                .expect("generation 10 still applies independently to photo-a");

            assert_eq!(preview_status(&registry, &token_a), StatusCode::NOT_FOUND);
            assert_eq!(preview_status(&registry, &token_b), StatusCode::NOT_FOUND);
        });
    }

    #[test]
    fn an_older_observation_for_the_same_media_cannot_revoke_a_newer_preview() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let project_id = fixture.work.namespace.project_id().to_owned();
            let artifact = verified_preview_artifact(&fixture, &engine).await;
            let demand = engine.reconcile_preview_demand(&registry, &project_id, 1, ["photo-a"]);
            engine
                .commit_preview_if_demanded(&demand, "photo-a", || {
                    registry.publish(&fixture.app_paths, &fixture.work.namespace, &artifact)
                })
                .expect("the first demand is current")
                .expect("the first preview is resident");
            engine
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(11, vec!["photo-a".to_owned()], vec![]),
                )
                .expect("generation 11 revokes the older preview");
            let newer_demand =
                engine.reconcile_preview_demand(&registry, &project_id, 2, ["photo-a"]);
            let newer_preview = engine
                .commit_preview_if_demanded(&newer_demand, "photo-a", || {
                    registry.publish(&fixture.app_paths, &fixture.work.namespace, &artifact)
                })
                .expect("the newer demand is current")
                .expect("the newer preview is resident");
            let newer_token = newer_preview
                .url
                .expect("the newer preview has an opaque URL")
                .rsplit('/')
                .next()
                .expect("the newer preview has a token")
                .to_owned();

            engine
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(10, vec!["photo-a".to_owned()], vec![]),
                )
                .expect("the stale generation is ignored");

            assert_eq!(preview_status(&registry, &newer_token), StatusCode::OK);
        });
    }

    #[test]
    fn invalidated_flight_is_detached_before_revalidation_claims_a_replacement() {
        let fixture = fixture();
        let engine = CacheEngine::default();
        let registry = CachePreviewRegistry::new("project");
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
            .apply_monitor_media_update(
                &fixture.app_paths,
                &fixture.work.namespace,
                &registry,
                &MediaRuntimeUpdate::for_test(
                    1,
                    vec![fixture.work.source.media_id().to_owned()],
                    vec![fixture.work.source.media_id().to_owned()],
                ),
            )
            .expect("the stable Monitor update cancels the obsolete flight");
        assert_eq!(
            invalidated_owner.cancellation().reason(),
            Some(crate::cache_activity_gate::CacheCancellationReason::Obsolete)
        );
        let revalidated_demand = engine.reconcile_demand(
            fixture.work.namespace.project_id(),
            2,
            [fixture.work.source.media_id()],
        );
        let CacheFlightClaim::Owner(revalidated_owner) = engine
            .claim_demanded(&revalidated_demand, &fixture.work)
            .expect("the current demand can revalidate its flight")
        else {
            panic!("revalidation owns a fresh flight instead of waiting on the cancelled one");
        };

        drop(invalidated_owner);
        assert!(
            matches!(
                engine.claim_demanded(&revalidated_demand, &fixture.work),
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
        let registry = CachePreviewRegistry::new("project");
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
            .apply_monitor_media_update(
                &fixture.app_paths,
                &fixture.work.namespace,
                &registry,
                &MediaRuntimeUpdate::for_test(
                    1,
                    vec!["unrelated-media".into()],
                    vec!["unrelated-media".into()],
                ),
            )
            .expect("the unrelated stable update is accepted");

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
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };
            let cancellation = CacheCancellation::default();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");

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
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(
                        10,
                        vec!["photo-a".into()],
                        vec!["photo-a".into()],
                    ),
                )
                .expect("targeted invalidation revokes reuse without deleting G1");
            assert!(third_path.is_file());
            assert!(fourth_path.is_file());

            std::fs::write(fixture.work.source.source_path(), b"original-photo-v3")
                .expect("the Original changes before its verified replacement");
            let mut fifth_work = fixture.work.clone();
            fifth_work.request_id = "cache-fifth".into();
            let fifth = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fifth_work,
                    &InvocationContext::new(
                        "cache-fifth",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &cancellation,
                )
                .await
                .expect("the verified replacement atomically supersedes Photo A");
            assert_ne!(
                fifth.artifact().generation_id,
                third.artifact().generation_id
            );
            assert!(!third_path.exists());
            assert!(fourth_path.is_file());

            let metadata_bytes = std::fs::read(fixture.work.namespace.paths().metadata_file())
                .expect("the disposable index is readable");
            let metadata: serde_json::Value = serde_json::from_slice(&metadata_bytes)
                .expect("the disposable index is valid JSON");
            assert_eq!(metadata["schemaVersion"], 5);
            assert_eq!(metadata["representationVersion"], 1);
            assert_eq!(metadata["policy"]["maxEdgePx"], 1_600);
            assert_eq!(metadata["entries"].as_array().map(Vec::len), Some(2));
            assert_eq!(metadata["entries"][0]["mediaId"], "photo-a");
            assert_eq!(metadata["entries"][1]["mediaId"], "photo-b");
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
            assert_eq!(transport.attempts, [1, 1, 1, 1, 1]);
        });
    }

    #[test]
    fn duplicate_media_entries_make_the_discardable_cache_index_non_current() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            verified_preview_artifact(&fixture, &engine).await;
            let storage = fixture
                .app_paths
                .prepare_cache_storage(fixture.work.namespace.paths())
                .expect("the Cache storage remains available");
            let mut metadata = super::load_metadata(&storage, fixture.work.namespace.paths())
                .expect("the verified publication has metadata");
            metadata.entries.push(metadata.entries[0].clone());

            assert!(!super::metadata_is_current(
                &metadata,
                fixture.work.namespace.project_id(),
                fixture.work.namespace.paths(),
            ));
        });
    }

    #[test]
    fn reserved_namespace_recovery_discards_abandoned_files_and_preserves_indexed_generation() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let published = verified_preview_artifact(&fixture, &engine).await;
            let published_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    &published.media_id,
                    &published.generation_id,
                    published.format,
                )
                .expect("the indexed generation path is valid");
            let orphan = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    "photo-orphan",
                    "orphan-generation",
                    CacheArtifactFormat::Png,
                )
                .expect("the orphan generation path is valid");
            let preview_temporary = fixture
                .work
                .namespace
                .paths()
                .preview_temporary_file(
                    "photo-partial",
                    "partial-generation",
                    CacheArtifactFormat::Jpeg,
                    4_242,
                )
                .expect("the abandoned preview path is valid");
            let metadata_temporary = fixture
                .work
                .namespace
                .paths()
                .metadata_temporary_file(4_343);
            std::fs::write(&orphan, b"orphan generation")
                .expect("the orphan generation is materialized");
            std::fs::write(&preview_temporary, b"abandoned preview")
                .expect("the preview temporary is materialized");
            std::fs::write(&metadata_temporary, b"abandoned metadata")
                .expect("the metadata temporary is materialized");
            let metadata_before = std::fs::read(fixture.work.namespace.paths().metadata_file())
                .expect("the published index is readable");

            let recovered = CacheEngine::recover_reserved_namespace(
                &fixture.app_paths,
                &fixture.work.namespace,
            )
            .expect("exclusive namespace recovery succeeds");

            assert_eq!(recovered.removed_temporary_count, 2);
            assert_eq!(recovered.removed_generation_count, 1);
            assert!(!recovered.discarded_index);
            assert!(published_path.is_file());
            assert!(!orphan.exists());
            assert!(!preview_temporary.exists());
            assert!(!metadata_temporary.exists());
            assert_eq!(
                std::fs::read(fixture.work.namespace.paths().metadata_file())
                    .expect("the published index remains readable"),
                metadata_before
            );
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
    fn corrupted_or_incompatible_index_is_discarded_and_rebuilt() {
        tauri::async_runtime::block_on(async {
            for incompatible_schema in [false, true] {
                let fixture = fixture();
                let engine = CacheEngine::default();
                let previous = verified_preview_artifact(&fixture, &engine).await;
                let previous_path = fixture
                    .work
                    .namespace
                    .paths()
                    .preview_file(&previous.media_id, &previous.generation_id, previous.format)
                    .unwrap();
                let metadata_path = fixture.work.namespace.paths().metadata_file();
                if incompatible_schema {
                    let mut metadata: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
                    metadata["schemaVersion"] = serde_json::json!(u32::MAX);
                    std::fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
                } else {
                    std::fs::write(&metadata_path, b"{corrupted index").unwrap();
                }

                let mut transport = ScriptedTransport {
                    app_paths: fixture.app_paths.clone(),
                    scripts: VecDeque::from([Script::Complete(CacheArtifactFormat::Jpeg)]),
                    attempts: Vec::new(),
                };
                let mut work = fixture.work.clone();
                work.request_id = if incompatible_schema {
                    "cache-incompatible-index"
                } else {
                    "cache-corrupted-index"
                }
                .into();
                let rebuilt = engine
                    .execute(
                        &mut transport,
                        &fixture.app_paths,
                        work,
                        &InvocationContext::new(
                            "cache-index-rebuild",
                            Some(fixture.work.namespace.project_id()),
                        ),
                        &CacheCancellation::default(),
                    )
                    .await
                    .expect("a disposable invalid index is rebuilt");

                assert_eq!(rebuilt.completion.generated_count, 1);
                assert_ne!(rebuilt.artifact().generation_id, previous.generation_id);
                assert!(
                    !previous_path.exists(),
                    "the unreferenced previous generation is collected only after rebuild"
                );
                let storage = fixture
                    .app_paths
                    .prepare_cache_storage(fixture.work.namespace.paths())
                    .unwrap();
                let metadata = super::load_metadata(&storage, fixture.work.namespace.paths())
                    .expect("the rebuilt index is readable");
                assert!(super::metadata_is_current(
                    &metadata,
                    fixture.work.namespace.project_id(),
                    fixture.work.namespace.paths(),
                ));
            }
        });
    }

    #[test]
    fn repeated_processor_crashes_suspend_new_cache_work_after_one_restart() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::Crash(5_001),
                    Script::Crash(5_002),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };
            let engine = CacheEngine::default();

            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work.clone(),
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("a second crash is a repeated processor failure");
            assert!(matches!(failure.stage, CacheFailureStage::Processor(_)));
            assert_eq!(
                engine.processor_status(),
                super::CacheProcessorStatus::Suspended
            );

            let suspended = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("a suspended Cache cannot start a third processor attempt");
            assert_eq!(suspended.stage, CacheFailureStage::ProcessorSuspended);
            assert_eq!(
                transport.attempts,
                [1, 2],
                "suspension does not consume another transport invocation"
            );
        });
    }

    #[test]
    fn repeated_processor_failure_suspends_before_fallible_recovery_cleanup() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::Crash(5_101),
                    Script::CrashWithInvalidCandidate(5_102),
                    Script::Complete(CacheArtifactFormat::Jpeg),
                ]),
                attempts: Vec::new(),
            };
            let engine = CacheEngine::default();

            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work.clone(),
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("the invalid repeated-crash candidate cannot be cleaned");
            assert_eq!(failure.stage, CacheFailureStage::RecoveryCleanup);

            let suspended = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("cleanup failure cannot reopen the repeated-crash loop");
            assert_eq!(suspended.stage, CacheFailureStage::ProcessorSuspended);
            assert_eq!(transport.attempts, [1, 2]);
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
            let remaining = std::fs::read_dir(media_directory)
                .map(|entries| entries.filter_map(Result::ok).count())
                .unwrap_or(0);
            assert_eq!(remaining, 0);
        });
    }

    #[test]
    fn semantically_malformed_processor_completions_are_typed_protocol_failures() {
        tauri::async_runtime::block_on(async {
            for script in [
                Script::MalformedCounts,
                Script::MalformedOrientation,
                Script::MalformedPageCount,
            ] {
                let fixture = fixture();
                let mut transport = ScriptedTransport {
                    app_paths: fixture.app_paths.clone(),
                    scripts: VecDeque::from([script]),
                    attempts: Vec::new(),
                };

                let failure = CacheEngine::default()
                    .execute(
                        &mut transport,
                        &fixture.app_paths,
                        fixture.work,
                        &fixture.context,
                        &CacheCancellation::default(),
                    )
                    .await
                    .expect_err("malformed sidecar data cannot become a Cache publication");

                assert_eq!(failure.stage, CacheFailureStage::ValidateResponse);
                assert_eq!(transport.attempts, [1]);
            }
        });
    }

    #[test]
    fn failed_validation_discards_the_candidate_and_preserves_the_last_published_generation() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let engine = CacheEngine::default();
            let registry = CachePreviewRegistry::new("project");
            let published = verified_preview_artifact(&fixture, &engine).await;
            let published_path = fixture
                .work
                .namespace
                .paths()
                .preview_file(
                    &published.media_id,
                    &published.generation_id,
                    published.format,
                )
                .expect("the published generation path is valid");
            std::fs::write(
                fixture.work.source.source_path(),
                b"changed Original requiring a candidate generation",
            )
            .expect("the Original changes before the invalid completion");
            engine
                .apply_monitor_media_update(
                    &fixture.app_paths,
                    &fixture.work.namespace,
                    &registry,
                    &MediaRuntimeUpdate::for_test(
                        10,
                        vec![published.media_id.clone()],
                        vec![published.media_id.clone()],
                    ),
                )
                .expect("the stable change invalidates logical reuse");
            assert!(
                published_path.is_file(),
                "logical invalidation retains G1 until a verified replacement is published"
            );
            let mut work = fixture.work.clone();
            work.request_id = "cache-invalid-completion".into();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::MalformedCounts]),
                attempts: Vec::new(),
            };

            let failure = engine
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    work,
                    &InvocationContext::new(
                        "cache-invalid-completion",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("a malformed completion cannot publish its candidate");

            assert_eq!(failure.stage, CacheFailureStage::ValidateResponse);
            assert!(published_path.is_file());
            let storage = fixture
                .app_paths
                .prepare_cache_storage(fixture.work.namespace.paths())
                .expect("the last published Cache remains readable");
            let metadata = super::load_metadata(&storage, fixture.work.namespace.paths())
                .expect("the previous index remains published");
            assert_eq!(metadata.entries.len(), 1);
            assert_eq!(metadata.entries[0].generation_id, published.generation_id);
            let generation_count =
                std::fs::read_dir(fixture.work.namespace.paths().media_directory())
                    .expect("the Cache media directory remains readable")
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                    .count();
            assert_eq!(
                generation_count, 1,
                "the rejected candidate cannot remain as an orphan generation"
            );

            let mut replacement_work = fixture.work.clone();
            replacement_work.request_id = "cache-valid-replacement".into();
            let mut replacement_transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::Complete(CacheArtifactFormat::Jpeg)]),
                attempts: Vec::new(),
            };
            let replacement = engine
                .execute(
                    &mut replacement_transport,
                    &fixture.app_paths,
                    replacement_work,
                    &InvocationContext::new(
                        "cache-valid-replacement",
                        Some(fixture.work.namespace.project_id()),
                    ),
                    &CacheCancellation::default(),
                )
                .await
                .expect("a verified replacement can supersede G1");

            assert_ne!(
                replacement.artifact().generation_id,
                published.generation_id
            );
            assert!(
                !published_path.exists(),
                "G1 is collected only after G2 is atomically published"
            );
            let metadata = super::load_metadata(&storage, fixture.work.namespace.paths())
                .expect("the replacement index is published");
            assert_eq!(metadata.entries.len(), 1);
            assert_eq!(
                metadata.entries[0].generation_id,
                replacement.artifact().generation_id
            );
        });
    }

    #[test]
    fn a_wrong_response_correlation_discards_the_unpublished_candidate_generation() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let media_directory = fixture.work.namespace.paths().media_directory();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([Script::WrongRequestId]),
                attempts: Vec::new(),
            };

            let failure = CacheEngine::default()
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("an uncorrelated completion cannot publish Cache metadata");

            assert_eq!(failure.stage, CacheFailureStage::ValidateResponse);
            assert_eq!(transport.attempts, [1]);
            assert_eq!(
                std::fs::read_dir(media_directory)
                    .expect("the Media directory remains readable")
                    .filter_map(Result::ok)
                    .count(),
                0,
                "the sidecar candidate must not survive a terminal protocol failure"
            );
        });
    }

    #[test]
    fn repeated_crashes_after_candidate_publication_leave_no_orphan_generation() {
        tauri::async_runtime::block_on(async {
            let fixture = fixture();
            let media_directory = fixture.work.namespace.paths().media_directory();
            let mut transport = ScriptedTransport {
                app_paths: fixture.app_paths.clone(),
                scripts: VecDeque::from([
                    Script::PublishThenCrash(7_001),
                    Script::PublishThenCrash(7_002),
                ]),
                attempts: Vec::new(),
            };

            let failure = CacheEngine::default()
                .execute(
                    &mut transport,
                    &fixture.app_paths,
                    fixture.work,
                    &fixture.context,
                    &CacheCancellation::default(),
                )
                .await
                .expect_err("the second processor crash suspends Cache work");

            assert_eq!(transport.attempts, [1, 2]);
            assert!(matches!(failure.stage, CacheFailureStage::Processor(_)));
            assert_eq!(
                std::fs::read_dir(media_directory)
                    .expect("the Media directory remains readable")
                    .filter_map(Result::ok)
                    .count(),
                0,
                "no final candidate may survive before metadata publication"
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
