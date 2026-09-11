use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, Read},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation};
use myalbuns_core::{ImportPhoto, MediaKind, PhotoSourceMetadata};
use myalbuns_paths::{
    ExpectedObject, OperationPathContext, PhysicalFileIdentity, ResolveError, RootBindingPlan,
};

use crate::ipc_contract::ImageProcessingProblem;

#[cfg(test)]
std::thread_local! {
    static PHOTO_SOURCE_DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn photo_source_decode_count() -> usize {
    PHOTO_SOURCE_DECODES.get()
}

pub(crate) struct PhotoImportsProposal {
    pub(crate) kind: MediaKind,
    pub(crate) commands: Vec<ImportPhoto>,
    pub(crate) problems: Vec<ImageProcessingProblem>,
    pub(crate) operation_problem: Option<String>,
    pub(crate) inspections: Vec<ImportedPhotoInspection>,
}

/// A completed decode can be adopted once if the same source is still observed.
pub(crate) struct ImportedPhotoInspection {
    pub(crate) observation: MediaObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaBinding {
    pub(crate) media_id: String,
    pub(crate) kind: MediaKind,
    pub(crate) logical_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaAvailability {
    Candidate,
    Absent,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaObservation {
    pub(crate) media_id: String,
    pub(crate) kind: MediaKind,
    logical_path: PathBuf,
    pub(crate) availability: MediaAvailability,
    physical_identity: Option<PhysicalFileIdentity>,
    source_bytes: Option<u64>,
    source_created_unix_ms: Option<u64>,
    source_modified_unix_ms: Option<u64>,
}

impl MediaObservation {
    pub(crate) fn same_source(&self, current: &Self) -> bool {
        self.availability == MediaAvailability::Candidate
            && current.availability == MediaAvailability::Candidate
            && self.physical_identity.is_some()
            && self.source_modified_unix_ms.is_some()
            && self.kind == current.kind
            && self.logical_path == current.logical_path
            && self.physical_identity == current.physical_identity
            && self.source_bytes == current.source_bytes
            && self.source_created_unix_ms == current.source_created_unix_ms
            && self.source_modified_unix_ms == current.source_modified_unix_ms
    }

    pub(crate) fn matches_fingerprint(
        &self,
        fingerprint: &myalbuns_imaging_protocol::CacheFingerprint,
    ) -> bool {
        self.availability == MediaAvailability::Candidate
            && self.source_bytes == Some(fingerprint.source_bytes)
            && self.source_created_unix_ms == fingerprint.source_created_unix_ms
            && self.source_modified_unix_ms == fingerprint.source_modified_unix_ms
    }

    pub(crate) fn logical_path(&self) -> &std::path::Path {
        &self.logical_path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaResolutionProposal {
    generation: u64,
    observations: Vec<MediaObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaRelinkProposal {
    media_id: String,
    kind: MediaKind,
    expected_logical_path: PathBuf,
    replacement_path: PathBuf,
    source_metadata: Option<PhotoSourceMetadata>,
}

impl MediaRelinkProposal {
    pub(crate) fn media_id(&self) -> &str {
        &self.media_id
    }

    pub(crate) fn kind(&self) -> MediaKind {
        self.kind
    }

    pub(crate) fn expected_logical_path(&self) -> &std::path::Path {
        &self.expected_logical_path
    }

    pub(crate) fn replacement_path(&self) -> &std::path::Path {
        &self.replacement_path
    }

    pub(crate) fn source_metadata(&self) -> Option<&PhotoSourceMetadata> {
        self.source_metadata.as_ref()
    }
}

impl MediaResolutionProposal {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn observations(&self) -> &[MediaObservation] {
        &self.observations
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MediaRuntimeUpdate {
    observation_generation: u64,
    changed_media_ids: Vec<String>,
    invalidated_media_ids: Vec<String>,
    revoked_preview_media_ids: Vec<String>,
}

impl MediaRuntimeUpdate {
    pub(crate) fn observation_generation(&self) -> u64 {
        self.observation_generation
    }

    pub(crate) fn changed_media_ids(&self) -> &[String] {
        &self.changed_media_ids
    }

    pub(crate) fn invalidated_media_ids(&self) -> &[String] {
        &self.invalidated_media_ids
    }

    pub(crate) fn revoked_preview_media_ids(&self) -> &[String] {
        &self.revoked_preview_media_ids
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        observation_generation: u64,
        changed_media_ids: Vec<String>,
        invalidated_media_ids: Vec<String>,
    ) -> Self {
        let mut revoked_preview_media_ids = changed_media_ids.clone();
        for media_id in &invalidated_media_ids {
            if !revoked_preview_media_ids.contains(media_id) {
                revoked_preview_media_ids.push(media_id.clone());
            }
        }
        Self {
            observation_generation,
            changed_media_ids,
            invalidated_media_ids,
            revoked_preview_media_ids,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_preserving_previews(
        observation_generation: u64,
        changed_media_ids: Vec<String>,
    ) -> Self {
        Self {
            observation_generation,
            changed_media_ids,
            invalidated_media_ids: Vec::new(),
            revoked_preview_media_ids: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaMonitorPoll {
    confirmed_observation: Option<MediaResolutionProposal>,
    update: Option<MediaRuntimeUpdate>,
}

impl MediaMonitorPoll {
    pub(crate) fn confirmed_observation(&self) -> Option<&MediaResolutionProposal> {
        self.confirmed_observation.as_ref()
    }

    pub(crate) fn update(&self) -> Option<&MediaRuntimeUpdate> {
        self.update.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaRetryInspection {
    availability: MediaAvailability,
    update: MediaRuntimeUpdate,
}

impl MediaRetryInspection {
    pub(crate) fn availability(&self) -> MediaAvailability {
        self.availability
    }

    pub(crate) fn update(&self) -> &MediaRuntimeUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MediaRetryError {
    NotUnavailable,
}

impl std::fmt::Display for MediaRetryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUnavailable => formatter.write_str(
                "A ocorrência de mídia não está confirmada como temporariamente indisponível.",
            ),
        }
    }
}

impl std::error::Error for MediaRetryError {}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MediaResolver;

impl MediaResolver {
    #[cfg(test)]
    pub(crate) fn propose_photo_imports(
        &self,
        paths: Vec<PathBuf>,
        bindings: &[MediaBinding],
        on_progress: impl FnMut(crate::ipc_contract::ImageProcessingProgress),
    ) -> PhotoImportsProposal {
        let existing = bindings
            .iter()
            .filter(|binding| binding.kind == MediaKind::Photo)
            .map(|binding| binding.logical_path.as_path())
            .collect::<HashSet<_>>();
        let mut context = OperationPathContext::new();
        for path in &paths {
            if !existing.contains(path.as_path()) {
                // An unbound or invalid candidate is reported by the shared inspector;
                // one failed root must not discard other valid selections.
                let _ = context.capture(path);
            }
        }
        self.propose_photo_imports_in_plan(paths, bindings, &context.freeze(), on_progress)
    }

    #[cfg(test)]
    pub(crate) fn propose_photo_imports_in_plan(
        &self,
        paths: Vec<PathBuf>,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
        on_progress: impl FnMut(crate::ipc_contract::ImageProcessingProgress),
    ) -> PhotoImportsProposal {
        self.propose_media_imports_in_plan(MediaKind::Photo, paths, bindings, plan, on_progress)
    }

    pub(crate) fn propose_media_imports_in_plan(
        &self,
        kind: MediaKind,
        paths: Vec<PathBuf>,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
        mut on_progress: impl FnMut(crate::ipc_contract::ImageProcessingProgress),
    ) -> PhotoImportsProposal {
        let existing = bindings
            .iter()
            .filter(|binding| binding.kind == kind)
            .map(|binding| binding.logical_path.as_path())
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let paths = paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect::<Vec<_>>();
        let total_files = paths.len() as u32;
        on_progress(crate::ipc_contract::ImageProcessingProgress {
            completed_files: 0,
            total_files,
            problem: None,
            operation_problem: None,
        });
        let candidates = paths
            .into_iter()
            .map(|path| {
                let capture = if existing.contains(path.as_path()) || plan.covers(&path) {
                    Ok(())
                } else {
                    Err("O caminho escolhido não está disponível no plano da tentativa.".into())
                };
                (path, capture)
            })
            .collect::<Vec<_>>();
        let inspected = inspect_photo_candidates(
            candidates,
            |(path, capture)| {
                if existing.contains(path.as_path()) {
                    return Ok((ImportPhoto::select_existing(path), None));
                }
                capture
                    .and_then(|()| {
                        let binding = MediaBinding {
                            media_id: String::new(),
                            kind,
                            logical_path: path.clone(),
                        };
                        let before = self.observe_in_plan(plan, &binding);
                        let metadata = inspect_media_source_in_plan(plan, &path, false)?;
                        let after = self.observe_in_plan(plan, &binding);
                        if !before.same_source(&after) {
                            return Err("O Original mudou durante a inspeção.".into());
                        }
                        Ok((
                            ImportPhoto::new(path.clone(), metadata),
                            Some(ImportedPhotoInspection { observation: after }),
                        ))
                    })
                    .map_err(|reason| ImageProcessingProblem {
                        file_name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        reason,
                    })
            },
            |completed_files| {
                on_progress(crate::ipc_contract::ImageProcessingProgress {
                    completed_files,
                    total_files,
                    problem: None,
                    operation_problem: None,
                })
            },
        );
        let mut commands = Vec::new();
        let mut problems = Vec::new();
        let mut inspections = Vec::new();
        for result in inspected {
            match result {
                Ok((command, inspection)) => {
                    commands.push(command);
                    inspections.extend(inspection);
                }
                Err(problem) => problems.push(problem),
            }
        }
        PhotoImportsProposal {
            kind,
            commands,
            problems,
            operation_problem: None,
            inspections,
        }
    }

    #[cfg(test)]
    pub(crate) fn inspect_photo_binding(
        &self,
        binding: &MediaBinding,
    ) -> Result<PhotoSourceMetadata, String> {
        if binding.kind != MediaKind::Photo {
            return Err("A ocorrência escolhida não é uma Foto.".into());
        }
        inspect_media_source(&binding.logical_path, false)
    }

    pub(crate) fn inspect_photo_binding_in_plan(
        &self,
        binding: &MediaBinding,
        plan: &RootBindingPlan,
    ) -> Result<PhotoSourceMetadata, String> {
        if binding.kind != MediaKind::Photo {
            return Err("A ocorrência escolhida não é uma Foto.".into());
        }
        inspect_media_source_in_plan(plan, &binding.logical_path, false)
    }

    pub(crate) fn propose_relink(
        &self,
        binding: &MediaBinding,
        replacement_path: PathBuf,
    ) -> Result<MediaRelinkProposal, String> {
        let inspected = inspect_media_source(&replacement_path, false)?;

        Ok(MediaRelinkProposal {
            media_id: binding.media_id.clone(),
            kind: binding.kind,
            expected_logical_path: binding.logical_path.clone(),
            replacement_path,
            source_metadata: (binding.kind == MediaKind::Photo).then_some(inspected),
        })
    }

    pub(crate) fn observe(
        &self,
        generation: u64,
        bindings: &[MediaBinding],
    ) -> MediaResolutionProposal {
        let mut context = OperationPathContext::new();
        let mut capture_failures = HashMap::new();
        for binding in bindings {
            if context.capture(&binding.logical_path).is_err() {
                capture_failures.insert(binding.media_id.as_str(), MediaAvailability::Unavailable);
            }
        }
        let plan = context.freeze();
        let observations = bindings
            .iter()
            .map(|binding| {
                let resolved = if capture_failures.contains_key(binding.media_id.as_str()) {
                    Err(ResolveError::Unavailable)
                } else {
                    plan.resolve_existing(&binding.logical_path, ExpectedObject::RegularFile)
                };
                observe_resolved_source(binding, resolved)
            })
            .collect();
        MediaResolutionProposal {
            generation,
            observations,
        }
    }

    pub(crate) fn observe_in_plan(
        &self,
        plan: &RootBindingPlan,
        binding: &MediaBinding,
    ) -> MediaObservation {
        observe_resolved_source(
            binding,
            plan.resolve_existing(&binding.logical_path, ExpectedObject::RegularFile),
        )
    }
}

fn observe_resolved_source(
    binding: &MediaBinding,
    resolved: Result<myalbuns_paths::ResolvedObject, ResolveError>,
) -> MediaObservation {
    let (
        availability,
        physical_identity,
        source_bytes,
        source_created_unix_ms,
        source_modified_unix_ms,
    ) = match resolved {
        Ok(resolved) => match readable_source_metadata(&resolved) {
            Ok(metadata) => (
                MediaAvailability::Candidate,
                resolved.physical_identity(),
                Some(metadata.len()),
                file_time_millis(metadata.created()),
                file_time_millis(metadata.modified()),
            ),
            Err(_) => (MediaAvailability::Unavailable, None, None, None, None),
        },
        Err(ResolveError::NotFound) => (MediaAvailability::Absent, None, None, None, None),
        Err(
            ResolveError::InvalidPath
            | ResolveError::UnsupportedNamespace
            | ResolveError::UnboundRoot
            | ResolveError::AccessDenied
            | ResolveError::Unavailable
            | ResolveError::UnexpectedObjectType { .. }
            | ResolveError::IoFailure,
        ) => (MediaAvailability::Unavailable, None, None, None, None),
    };
    MediaObservation {
        media_id: binding.media_id.clone(),
        kind: binding.kind,
        logical_path: binding.logical_path.clone(),
        availability,
        physical_identity,
        source_bytes,
        source_created_unix_ms,
        source_modified_unix_ms,
    }
}

fn readable_source_metadata(
    resolved: &myalbuns_paths::ResolvedObject,
) -> std::io::Result<std::fs::Metadata> {
    // A metadata-only handle may succeed while Photoshop holds the original
    // against readers. Such a sample cannot revoke the last usable preview.
    let mut file = resolved.reopen_for_read()?;
    file.read_exact(&mut [0u8; 1])?;
    file.metadata()
}

/// Inspection can finish out of order; the proposal preserves the user's
/// selection order so the eventual single creative command stays deterministic.
fn inspect_photo_candidates<T: Send, R: Send>(
    candidates: Vec<T>,
    inspect: impl Fn(T) -> R + Sync,
    mut completed: impl FnMut(u32),
) -> Vec<R> {
    let total = candidates.len();
    if total <= 1 {
        return candidates
            .into_iter()
            .map(|candidate| {
                let result = inspect(candidate);
                completed(1);
                result
            })
            .collect();
    }
    let candidates = Mutex::new(candidates.into_iter().enumerate());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..total.min(crate::imaging_processor::IMAGE_PROCESSING_CONCURRENCY) {
            let candidates = &candidates;
            let inspect = &inspect;
            let sender = sender.clone();
            scope.spawn(move || {
                loop {
                    let Some((index, candidate)) = candidates
                        .lock()
                        .expect("the photo inspection queue is healthy")
                        .next()
                    else {
                        break;
                    };
                    if sender.send((index, inspect(candidate))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut results = Vec::with_capacity(total);
        for result in receiver {
            results.push(result);
            completed(results.len() as u32);
        }
        results.sort_unstable_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, result)| result).collect()
    })
}

fn inspect_media_source(
    path: &std::path::Path,
    require_jpeg: bool,
) -> Result<PhotoSourceMetadata, String> {
    let mut context = OperationPathContext::new();
    context
        .capture(path)
        .map_err(|error| format!("O caminho escolhido é inválido: {error}"))?;
    inspect_media_source_in_plan(&context.freeze(), path, require_jpeg)
}

fn inspect_media_source_in_plan(
    plan: &RootBindingPlan,
    path: &std::path::Path,
    require_jpeg: bool,
) -> Result<PhotoSourceMetadata, String> {
    let resolved = plan
        .resolve_existing(path, ExpectedObject::RegularFile)
        .map_err(|error| format!("O Arquivo escolhido não está disponível: {error}"))?;
    let file = resolved
        .reopen_for_read()
        .map_err(|error| format!("Não foi possível inspecionar o Arquivo escolhido: {error}"))?;
    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| format!("Não foi possível validar a mídia escolhida: {error}"))?;
    let compatible = if require_jpeg {
        reader.format() == Some(ImageFormat::Jpeg)
    } else {
        matches!(
            reader.format(),
            Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Tiff)
        )
    };
    if !compatible {
        return Err(if require_jpeg {
            "Escolha um Arquivo JPEG válido (.jpg ou .jpeg).".into()
        } else {
            "O Arquivo escolhido não usa um formato de mídia compatível.".into()
        });
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("Não foi possível validar a mídia escolhida: {error}"))?;
    let (mut width, mut height) = decoder.dimensions();
    let orientation = decoder.orientation().map_err(|error| {
        format!("Não foi possível ler a orientação da mídia escolhida: {error}")
    })?;
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        std::mem::swap(&mut width, &mut height);
    }
    #[cfg(test)]
    PHOTO_SOURCE_DECODES.set(PHOTO_SOURCE_DECODES.get() + 1);
    DynamicImage::from_decoder(decoder).map_err(|_| {
        if require_jpeg {
            "O JPEG está corrompido ou não pôde ser decodificado.".to_string()
        } else {
            "A imagem está corrompida ou não pôde ser decodificada.".to_string()
        }
    })?;
    PhotoSourceMetadata::new(
        width,
        height,
        ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
    )
    .map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaRuntime {
    current: Arc<Mutex<Option<MediaResolutionProposal>>>,
}

impl MediaRuntime {
    /// UI metadata comes only from stabilized observations of current bindings.
    /// Reading the catalog neither inspects Originals nor consults Cache demand.
    pub(crate) fn files_for(
        &self,
        bindings: &[MediaBinding],
    ) -> Vec<crate::ipc_contract::MediaFileInfo> {
        use crate::ipc_contract::{MediaFileInfo, MediaFileState};
        let Some(snapshot) = self.snapshot() else {
            return Vec::new();
        };
        let by_id: HashMap<_, _> = snapshot
            .observations
            .iter()
            .map(|file| (file.media_id.as_str(), file))
            .collect();
        bindings
            .iter()
            .filter_map(|binding| {
                let file = by_id.get(binding.media_id.as_str())?;
                if file.logical_path != binding.logical_path || file.kind != binding.kind {
                    return None;
                }
                Some(MediaFileInfo {
                    media_id: binding.media_id.clone(),
                    state: match file.availability {
                        MediaAvailability::Candidate => MediaFileState::Available,
                        MediaAvailability::Absent => MediaFileState::Absent,
                        MediaAvailability::Unavailable => MediaFileState::Unavailable,
                    },
                    created_at_ms: file.source_created_unix_ms,
                    modified_at_ms: file.source_modified_unix_ms,
                })
            })
            .collect()
    }

    pub(crate) fn apply(&self, proposal: MediaResolutionProposal) -> MediaRuntimeUpdate {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if current
            .as_ref()
            .is_some_and(|current| current.generation >= proposal.generation)
        {
            return MediaRuntimeUpdate::default();
        }
        let observation_generation = proposal.generation;
        let previous = current
            .as_ref()
            .map(|current| {
                current
                    .observations
                    .iter()
                    .map(|observation| (observation.media_id.clone(), observation.clone()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let changed_media_ids = proposal
            .observations
            .iter()
            .filter(|observation| {
                previous
                    .get(observation.media_id.as_str())
                    .is_none_or(|previous| *previous != **observation)
            })
            .map(|observation| observation.media_id.clone())
            .collect::<Vec<_>>();
        let invalidated_media_ids = proposal
            .observations
            .iter()
            .filter(|observation| {
                previous
                    .get(observation.media_id.as_str())
                    .is_some_and(|previous| invalidates_cache(previous, observation))
            })
            .map(|observation| observation.media_id.clone())
            .collect::<Vec<_>>();
        let revoked_preview_media_ids = invalidated_media_ids.clone();
        *current = Some(proposal);
        MediaRuntimeUpdate {
            observation_generation,
            changed_media_ids,
            invalidated_media_ids,
            revoked_preview_media_ids,
        }
    }

    pub(crate) fn snapshot(&self) -> Option<MediaResolutionProposal> {
        self.current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn apply_occurrence(
        &self,
        generation: u64,
        observation: MediaObservation,
    ) -> MediaRuntimeUpdate {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if current
            .as_ref()
            .is_some_and(|current| current.generation >= generation)
        {
            return MediaRuntimeUpdate::default();
        }
        apply_occurrence(&mut current, generation, observation)
    }
}

fn apply_occurrence(
    current: &mut Option<MediaResolutionProposal>,
    generation: u64,
    observation: MediaObservation,
) -> MediaRuntimeUpdate {
    let current = current.get_or_insert_with(|| MediaResolutionProposal {
        generation,
        observations: Vec::new(),
    });
    let previous = current
        .observations
        .iter_mut()
        .find(|current| current.media_id == observation.media_id);
    let (changed, invalidated) = if let Some(previous) = previous {
        let changed = *previous != observation;
        let invalidated = invalidates_cache(previous, &observation);
        *previous = observation.clone();
        (changed, invalidated)
    } else {
        current.observations.push(observation.clone());
        (true, false)
    };
    let media_id = observation.media_id.clone();
    current.generation = generation;
    MediaRuntimeUpdate {
        observation_generation: generation,
        changed_media_ids: changed.then(|| media_id.clone()).into_iter().collect(),
        invalidated_media_ids: invalidated.then(|| media_id.clone()).into_iter().collect(),
        revoked_preview_media_ids: invalidated.then_some(media_id).into_iter().collect(),
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaMonitor {
    resolver: MediaResolver,
    transition: Arc<Mutex<MediaMonitorTransition>>,
}

#[derive(Debug, Default)]
struct MediaMonitorTransition {
    next_generation: u64,
    pending: Option<MediaResolutionProposal>,
}

impl MediaMonitor {
    /// Image preparation or startup cache recovery already owns source evidence.
    /// Adopt only evidence whose
    /// exact binding and current source still match, without stabilizing the
    /// entire catalog again or decoding the Original in the Monitor.
    pub(crate) fn adopt_prepared_inspections(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
        inspections: &[MediaObservation],
    ) -> MediaMonitorPoll {
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = runtime.snapshot();
        let evidence = inspections
            .iter()
            .map(|observation| (observation.media_id.as_str(), observation))
            .collect::<HashMap<_, _>>();
        let mut merged = current
            .as_ref()
            .map(|current| {
                current
                    .observations
                    .iter()
                    .map(|observation| (observation.media_id.clone(), observation.clone()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let mut adopted = false;
        for binding in bindings {
            let Some(expected) = evidence.get(binding.media_id.as_str()) else {
                continue;
            };
            if binding.kind != expected.kind || binding.logical_path != expected.logical_path {
                continue;
            }
            let observed = self.resolver.observe_in_plan(plan, binding);
            if expected.same_source(&observed) {
                adopted = true;
                merged.insert(binding.media_id.clone(), observed);
            }
        }
        let update = adopted.then(|| {
            let generation = next_observation_generation(
                &mut transition,
                current.as_ref().map(|current| current.generation),
            );
            let observations = bindings
                .iter()
                .filter_map(|binding| merged.remove(&binding.media_id))
                .collect();
            transition.pending = None;
            runtime.apply(MediaResolutionProposal {
                generation,
                observations,
            })
        });
        MediaMonitorPoll {
            confirmed_observation: runtime.snapshot(),
            update,
        }
    }

    /// Stabilization is owned here; foreground callers only supply the frozen
    /// path plan and consume updates. Import evidence uses the explicit path above.
    pub(crate) fn synchronize_processing(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
    ) -> Vec<MediaMonitorPoll> {
        (0..2)
            .map(|_| self.poll_in_plan(runtime, bindings, plan))
            .collect()
    }

    pub(crate) fn poll_in_plan(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
    ) -> MediaMonitorPoll {
        self.poll_with_observation(runtime, |generation| MediaResolutionProposal {
            generation,
            observations: bindings
                .iter()
                .map(|binding| self.resolver.observe_in_plan(plan, binding))
                .collect(),
        })
    }

    pub(crate) fn retry_unavailable(
        &self,
        runtime: &MediaRuntime,
        binding: &MediaBinding,
        apply_update: impl FnOnce(&MediaRuntimeUpdate),
    ) -> Result<MediaRetryInspection, MediaRetryError> {
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = runtime.snapshot();
        if !current
            .as_ref()
            .and_then(|current| {
                current
                    .observations
                    .iter()
                    .find(|observation| observation.media_id == binding.media_id)
            })
            .is_some_and(|observation| observation.availability == MediaAvailability::Unavailable)
        {
            return Err(MediaRetryError::NotUnavailable);
        }
        let generation = next_observation_generation(
            &mut transition,
            current.as_ref().map(|current| current.generation),
        );
        let proposal = self
            .resolver
            .observe(generation, std::slice::from_ref(binding));
        let observation = proposal
            .observations
            .into_iter()
            .next()
            .expect("an occurrence retry produces exactly one observation");
        let availability = observation.availability;
        let mut staged = current;
        let update = apply_occurrence(&mut staged, generation, observation.clone());
        apply_update(&update);
        let committed = runtime.apply_occurrence(generation, observation);
        debug_assert_eq!(committed, update);
        transition.pending = None;
        Ok(MediaRetryInspection {
            availability,
            update: committed,
        })
    }

    #[cfg(test)]
    pub(crate) fn poll(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
    ) -> MediaMonitorPoll {
        self.poll_with_observation(runtime, |generation| {
            self.resolver.observe(generation, bindings)
        })
    }

    fn poll_with_observation(
        &self,
        runtime: &MediaRuntime,
        observe: impl FnOnce(u64) -> MediaResolutionProposal,
    ) -> MediaMonitorPoll {
        // A poll owns observation, stability classification and Runtime adoption as
        // one transition. The background loop and demand commands cannot reorder
        // samples or return a snapshot from another generation.
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = runtime.snapshot();
        let generation = next_observation_generation(
            &mut transition,
            current.as_ref().map(|current| current.generation),
        );
        let proposal = observe(generation);
        let update = match current {
            Some(current) if current.observations == proposal.observations => {
                transition.pending = None;
                None
            }
            _ if transition
                .pending
                .as_ref()
                .is_some_and(|candidate| candidate.observations == proposal.observations) =>
            {
                transition.pending = None;
                Some(runtime.apply(proposal.clone()))
            }
            _ => {
                transition.pending = Some(proposal.clone());
                None
            }
        };
        MediaMonitorPoll {
            confirmed_observation: runtime.snapshot(),
            update,
        }
    }
}

fn next_observation_generation(
    transition: &mut MediaMonitorTransition,
    current_generation: Option<u64>,
) -> u64 {
    transition.next_generation = transition
        .next_generation
        .max(current_generation.unwrap_or_default())
        .checked_add(1)
        .expect("a MediaMonitor generation cannot exhaust u64");
    transition.next_generation
}

fn invalidates_cache(previous: &MediaObservation, current: &MediaObservation) -> bool {
    current.availability == MediaAvailability::Candidate
        && (previous.availability != MediaAvailability::Candidate
            || previous.kind != current.kind
            || previous.logical_path != current.logical_path
            || previous.physical_identity != current.physical_identity
            || previous.source_bytes != current.source_bytes
            || previous.source_created_unix_ms != current.source_created_unix_ms
            || previous.source_modified_unix_ms != current.source_modified_unix_ms)
}

fn file_time_millis(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    #[test]
    fn import_adoption_requires_current_evidence_and_leaves_other_bindings_untouched() {
        use myalbuns_paths::OperationPathContext;
        let root = tempfile::tempdir().unwrap();
        let paths = [
            root.path().join("existing.jpg"),
            root.path().join("imported.jpg"),
            root.path().join("changed.jpg"),
        ];
        let bindings = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                std::fs::write(path, b"original").unwrap();
                MediaBinding {
                    media_id: format!("photo-{index}"),
                    kind: MediaKind::Photo,
                    logical_path: path.clone(),
                }
            })
            .collect::<Vec<_>>();
        let mut context = OperationPathContext::new();
        for path in &paths {
            context.capture(path).unwrap();
        }
        let roots = context.freeze();
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();
        runtime.apply(MediaResolver.observe(1, &bindings[..1]));
        let observations = bindings[1..]
            .iter()
            .map(|binding| MediaResolver.observe_in_plan(&roots, binding))
            .collect::<Vec<_>>();
        std::fs::write(&paths[2], b"changed after decode").unwrap();
        let before = super::photo_source_decode_count();
        let poll = monitor.adopt_prepared_inspections(&runtime, &bindings, &roots, &observations);
        assert_eq!(super::photo_source_decode_count(), before);
        assert_eq!(poll.update().unwrap().changed_media_ids(), &["photo-1"]);
        let current = poll.confirmed_observation().unwrap();
        assert_eq!(
            current
                .observations()
                .iter()
                .map(|value| value.media_id.as_str())
                .collect::<Vec<_>>(),
            ["photo-0", "photo-1"]
        );
        assert!(
            monitor.poll(&runtime, &bindings).update().is_none(),
            "unproven changes still need ordinary stabilization"
        );
    }

    #[test]
    fn photo_inspections_overlap_with_two_workers_and_preserve_selection_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let first_pair = std::sync::Barrier::new(2);
        let mut progress = Vec::new();
        let results = super::inspect_photo_candidates(
            vec![0, 1, 2, 3, 4],
            |index| {
                let count = active.fetch_add(1, Ordering::AcqRel) + 1;
                peak.fetch_max(count, Ordering::AcqRel);
                if index < 2 {
                    first_pair.wait();
                }
                active.fetch_sub(1, Ordering::AcqRel);
                if index == 1 { Err(index) } else { Ok(index) }
            },
            |completed| progress.push(completed),
        );
        assert_eq!(peak.load(Ordering::Acquire), 2);
        assert_eq!(results, [Ok(0), Err(1), Ok(2), Ok(3), Ok(4)]);
        assert_eq!(progress, [1, 2, 3, 4, 5]);
    }
    use std::{sync::mpsc, time::Duration};

    use image::{ImageFormat, Rgb, RgbImage};
    use myalbuns_core::MediaKind;

    use super::{
        MediaAvailability, MediaBinding, MediaMonitor, MediaObservation, MediaResolutionProposal,
        MediaResolver, MediaRetryError, MediaRuntime,
    };

    #[test]
    fn image_import_accepts_png_and_tiff_alongside_photos() {
        let root = tempfile::tempdir().unwrap();
        let paths = [
            ("fundo.png", ImageFormat::Png),
            ("overlay.tiff", ImageFormat::Tiff),
        ]
        .map(|(name, format)| {
            let path = root.path().join(name);
            RgbImage::from_pixel(37, 23, Rgb([20, 80, 160]))
                .save_with_format(&path, format)
                .unwrap();
            path
        });
        let result = MediaResolver.propose_photo_imports(paths.to_vec(), &[], |_| {});
        assert!(result.problems.is_empty(), "{:?}", result.problems);
        assert_eq!(result.commands.len(), 2);
        assert_eq!(result.inspections.len(), 2);
    }

    #[test]
    fn multiple_photo_import_keeps_valid_files_and_reports_each_rejection() {
        let root = tempfile::tempdir().unwrap();
        let good = root.path().join("boa.JPG");
        let second = root.path().join("segunda.jpeg");
        let invalid = root.path().join("invalida.jpg");
        let corrupt = root.path().join("corrompida.jpg");
        let missing = root.path().join("ausente.jpg");
        let existing = root.path().join("existente indisponivel.jpg");
        let original = RgbImage::from_pixel(37, 23, Rgb([20, 80, 160]));
        original.save_with_format(&good, ImageFormat::Jpeg).unwrap();
        original
            .save_with_format(&second, ImageFormat::Jpeg)
            .unwrap();
        std::fs::write(&invalid, b"GIF89a unsupported image").unwrap();
        let before = std::fs::read(&good).unwrap();
        let scan = before
            .windows(2)
            .position(|bytes| bytes == [0xff, 0xda])
            .unwrap();
        std::fs::write(&corrupt, &before[..scan + 2]).unwrap();
        let mut progress = Vec::new();
        let result = MediaResolver.propose_photo_imports(
            vec![
                good.clone(),
                invalid,
                existing.clone(),
                good.clone(),
                missing,
                corrupt,
                second,
            ],
            &[MediaBinding {
                media_id: "existing".into(),
                kind: MediaKind::Photo,
                logical_path: existing,
            }],
            |event| progress.push(event),
        );
        assert_eq!(
            progress
                .iter()
                .map(|event| (event.completed_files, event.total_files))
                .collect::<Vec<_>>(),
            (0..=6).map(|completed| (completed, 6)).collect::<Vec<_>>(),
            "progress counts unique files, including existing and rejected items"
        );
        assert_eq!(
            result.commands.len(),
            3,
            "two new Photos plus one existing selection"
        );
        assert_eq!(
            result
                .problems
                .iter()
                .map(|problem| problem.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["invalida.jpg", "ausente.jpg", "corrompida.jpg"]
        );
        assert!(
            result
                .problems
                .iter()
                .all(|problem| !problem.reason.is_empty())
        );
        assert_eq!(std::fs::read(good).unwrap(), before);
    }

    #[test]
    fn photo_import_accepts_decodable_jpeg_bytes_and_never_rewrites_the_original() {
        let root = tempfile::tempdir().expect("temporary JPEG import fixture");
        let source = root.path().join("Foto externa.jpeg");
        RgbImage::from_pixel(37, 23, Rgb([20, 80, 160]))
            .save_with_format(&source, ImageFormat::Jpeg)
            .expect("the external JPEG is writable");
        let before = std::fs::read(&source).expect("the Original is readable before import");

        let proposal = MediaResolver.propose_photo_imports(vec![source.clone()], &[], |_| {});
        assert_eq!(proposal.commands.len(), 1);
        assert!(proposal.problems.is_empty());
        assert_eq!(
            std::fs::read(&source).expect("the Original remains readable"),
            before,
            "import inspection never modifies the linked Original"
        );
    }

    #[test]
    fn photo_import_accepts_supported_content_even_with_a_different_extension() {
        let root = tempfile::tempdir().expect("temporary invalid JPEG fixture");
        let source = root.path().join("Nao e JPEG.jpg");
        RgbImage::from_pixel(12, 8, Rgb([90, 30, 10]))
            .save_with_format(&source, ImageFormat::Png)
            .expect("the renamed PNG is writable");
        let before = std::fs::read(&source).expect("the renamed Original is readable");

        let proposal = MediaResolver.propose_photo_imports(vec![source.clone()], &[], |_| {});
        assert_eq!(proposal.commands.len(), 1);
        assert!(proposal.problems.is_empty());
        assert_eq!(
            std::fs::read(source).expect("the rejected file remains"),
            before
        );
    }

    #[test]
    fn explicit_retry_reinspects_only_the_unavailable_occurrence_with_a_fresh_path_context() {
        let root = tempfile::tempdir().expect("temporary explicit retry fixture");
        let selected_path = root.path().join("photo-a.jpg");
        let untouched_path = root.path().join("photo-b.jpg");
        std::fs::write(&selected_path, b"photo-a").expect("the selected Original is writable");
        std::fs::write(&untouched_path, b"photo-b").expect("the other Original is writable");
        let selected = MediaBinding {
            media_id: "photo-a".into(),
            kind: MediaKind::Photo,
            logical_path: selected_path.clone(),
        };
        let selected_before = selected.clone();
        let untouched = MediaBinding {
            media_id: "photo-b".into(),
            kind: MediaKind::Photo,
            logical_path: untouched_path.clone(),
        };
        let runtime = MediaRuntime::default();
        let resolver = MediaResolver;
        let mut prior = resolver.observe(7, &[selected.clone(), untouched.clone()]);
        let selected_prior = prior
            .observations
            .iter_mut()
            .find(|observation| observation.media_id == selected.media_id)
            .expect("the selected observation is present in the fixture");
        selected_prior.availability = MediaAvailability::Unavailable;
        selected_prior.physical_identity = None;
        selected_prior.source_bytes = None;
        selected_prior.source_created_unix_ms = None;
        selected_prior.source_modified_unix_ms = None;
        runtime.apply(prior);
        let untouched_before = runtime
            .snapshot()
            .expect("the unavailable state is registered")
            .observations()
            .iter()
            .find(|observation| observation.media_id == untouched.media_id)
            .expect("the other occurrence is registered")
            .clone();

        let retried = MediaMonitor::default()
            .retry_unavailable(&runtime, &selected, |_| {})
            .expect("an unavailable occurrence can be inspected explicitly");

        assert_eq!(retried.availability(), MediaAvailability::Candidate);
        assert_eq!(retried.update().changed_media_ids(), ["photo-a"]);
        assert_eq!(retried.update().invalidated_media_ids(), ["photo-a"]);
        assert_eq!(
            selected, selected_before,
            "retry never rewrites the binding"
        );
        let snapshot = runtime.snapshot().expect("retry updates observed state");
        assert_eq!(snapshot.observations().len(), 2);
        assert_eq!(
            snapshot
                .observations()
                .iter()
                .find(|observation| observation.media_id == "photo-a")
                .expect("the selected observation remains present")
                .logical_path,
            selected_path
        );
        assert_eq!(
            snapshot
                .observations()
                .iter()
                .find(|observation| observation.media_id == "photo-b")
                .expect("the other observation remains present"),
            &untouched_before,
            "retry is scoped to one occurrence"
        );
    }

    #[test]
    fn explicit_retry_rejects_absent_occurrences_without_changing_runtime() {
        let root = tempfile::tempdir().expect("temporary absent retry fixture");
        let binding = MediaBinding {
            media_id: "photo-absent".into(),
            kind: MediaKind::Photo,
            logical_path: root.path().join("missing.jpg"),
        };
        let resolver = MediaResolver;
        let runtime = MediaRuntime::default();
        runtime.apply(resolver.observe(3, std::slice::from_ref(&binding)));
        let before = runtime.snapshot().expect("absence is registered");
        assert_eq!(
            before.observations()[0].availability,
            MediaAvailability::Absent
        );

        let error = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect_err("absence is not eligible for the unavailable retry action");

        assert_eq!(error, MediaRetryError::NotUnavailable);
        assert_eq!(runtime.snapshot(), Some(before));
    }

    #[test]
    fn explicit_retry_changes_unavailable_to_absent_after_authoritative_inspection() {
        let root = tempfile::tempdir().expect("temporary unavailable-to-absent fixture");
        let binding = MediaBinding {
            media_id: "photo-returned-root".into(),
            kind: MediaKind::Photo,
            logical_path: root.path().join("still-missing.jpg"),
        };
        let runtime = MediaRuntime::default();
        runtime.apply(MediaResolutionProposal {
            generation: 4,
            observations: vec![MediaObservation {
                media_id: binding.media_id.clone(),
                kind: binding.kind,
                logical_path: binding.logical_path.clone(),
                availability: MediaAvailability::Unavailable,
                physical_identity: None,
                source_bytes: None,
                source_created_unix_ms: None,
                source_modified_unix_ms: None,
            }],
        });

        let retried = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect("the reachable root can authoritatively establish absence");

        assert_eq!(retried.availability(), MediaAvailability::Absent);
        assert_eq!(retried.update().changed_media_ids(), [binding.media_id]);
        assert!(retried.update().invalidated_media_ids().is_empty());
        assert_eq!(
            runtime
                .snapshot()
                .expect("the new authoritative absence is stored")
                .observations()[0]
                .availability,
            MediaAvailability::Absent
        );
    }

    #[test]
    fn explicit_retry_rejects_a_missing_authoritative_observation() {
        let root = tempfile::tempdir().expect("temporary missing observation fixture");
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo").expect("the Original fixture is writable");
        let binding = MediaBinding {
            media_id: "photo-first".into(),
            kind: MediaKind::Photo,
            logical_path: source,
        };
        let runtime = MediaRuntime::default();

        let error = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect_err("Retry requires an authoritative Unavailable observation");

        assert_eq!(error, MediaRetryError::NotUnavailable);
        assert!(runtime.snapshot().is_none());
    }

    #[test]
    fn explicit_retry_rejects_an_occurrence_missing_from_the_authoritative_snapshot() {
        let root = tempfile::tempdir().expect("temporary missing occurrence fixture");
        let selected_path = root.path().join("selected.jpg");
        let observed_path = root.path().join("observed.jpg");
        std::fs::write(&selected_path, b"selected").expect("the selected Original is writable");
        std::fs::write(&observed_path, b"observed").expect("the observed Original is writable");
        let selected = MediaBinding {
            media_id: "photo-selected".into(),
            kind: MediaKind::Photo,
            logical_path: selected_path,
        };
        let observed = MediaBinding {
            media_id: "photo-observed".into(),
            kind: MediaKind::Photo,
            logical_path: observed_path,
        };
        let runtime = MediaRuntime::default();
        runtime.apply(MediaResolver.observe(3, std::slice::from_ref(&observed)));
        let before = runtime
            .snapshot()
            .expect("the other occurrence is observed");

        let error = MediaMonitor::default()
            .retry_unavailable(&runtime, &selected, |_| {})
            .expect_err("an occurrence absent from the snapshot is not Unavailable");

        assert_eq!(error, MediaRetryError::NotUnavailable);
        assert_eq!(runtime.snapshot(), Some(before));
    }

    #[test]
    fn explicit_retry_rejects_available_occurrences_without_changing_runtime() {
        let root = tempfile::tempdir().expect("temporary available retry fixture");
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo").expect("the Original fixture is writable");
        let binding = MediaBinding {
            media_id: "photo-available".into(),
            kind: MediaKind::Photo,
            logical_path: source,
        };
        let runtime = MediaRuntime::default();
        runtime.apply(MediaResolver.observe(4, std::slice::from_ref(&binding)));
        let before = runtime.snapshot().expect("availability is registered");
        assert_eq!(
            before.observations()[0].availability,
            MediaAvailability::Candidate
        );

        let error = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect_err("an available occurrence is not eligible for Retry");

        assert_eq!(error, MediaRetryError::NotUnavailable);
        assert_eq!(runtime.snapshot(), Some(before));
    }

    #[test]
    fn explicit_retry_preserves_unavailable_when_the_new_context_still_cannot_access_the_root() {
        let binding = MediaBinding {
            media_id: "photo-offline".into(),
            kind: MediaKind::Photo,
            logical_path: "relative-path-remains-unavailable.jpg".into(),
        };
        let resolver = MediaResolver;
        let runtime = MediaRuntime::default();
        runtime.apply(resolver.observe(2, std::slice::from_ref(&binding)));
        let binding_before = binding.clone();

        let retried = MediaMonitor::default()
            .retry_unavailable(&runtime, &binding, |_| {})
            .expect("an inaccessible root is still a completed retry inspection");

        assert_eq!(retried.availability(), MediaAvailability::Unavailable);
        assert!(retried.update().changed_media_ids().is_empty());
        assert!(retried.update().invalidated_media_ids().is_empty());
        assert_eq!(binding, binding_before);
        assert_eq!(
            runtime
                .snapshot()
                .expect("the retry keeps an observed unavailable state")
                .observations()[0]
                .availability,
            MediaAvailability::Unavailable
        );
    }

    #[test]
    fn explicit_retry_reacts_to_cache_before_committing_runtime() {
        let root = tempfile::tempdir().expect("temporary transactional retry fixture");
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo").expect("the Original fixture is writable");
        let binding = MediaBinding {
            media_id: "photo-transactional".into(),
            kind: MediaKind::Photo,
            logical_path: source,
        };
        let runtime = MediaRuntime::default();
        runtime.apply(MediaResolutionProposal {
            generation: 2,
            observations: vec![MediaObservation {
                media_id: binding.media_id.clone(),
                kind: binding.kind,
                logical_path: binding.logical_path.clone(),
                availability: MediaAvailability::Unavailable,
                physical_identity: None,
                source_bytes: None,
                source_created_unix_ms: None,
                source_modified_unix_ms: None,
            }],
        });
        let monitor = MediaMonitor::default();
        let mut cache_reacted = false;

        let retried = monitor
            .retry_unavailable(&runtime, &binding, |update| {
                assert_eq!(
                    update.changed_media_ids(),
                    std::slice::from_ref(&binding.media_id)
                );
                assert_eq!(
                    update.invalidated_media_ids(),
                    std::slice::from_ref(&binding.media_id)
                );
                assert_eq!(
                    runtime
                        .snapshot()
                        .expect("Runtime remains at the authoritative prior state during reaction")
                        .observations()[0]
                        .availability,
                    MediaAvailability::Unavailable
                );
                cache_reacted = true;
            })
            .expect("the infallible Cache reaction precedes the Runtime commit");

        assert!(cache_reacted);
        assert_eq!(retried.availability(), MediaAvailability::Candidate);
        assert_eq!(
            runtime
                .snapshot()
                .expect("the successful retry commits the new observation")
                .observations()[0]
                .availability,
            MediaAvailability::Candidate
        );
    }

    #[test]
    fn resolver_monitor_and_runtime_keep_observed_state_outside_media_refs() {
        let root = tempfile::tempdir().expect("temporary media-runtime fixture");
        let available = root.path().join("photo.jpg");
        std::fs::write(&available, b"photo").expect("the available fixture is written");
        let bindings = vec![
            MediaBinding {
                media_id: "photo-a".into(),
                kind: MediaKind::Photo,
                logical_path: available,
            },
            MediaBinding {
                media_id: "overlay-a".into(),
                kind: MediaKind::Decorative,
                logical_path: root.path().join("missing.png"),
            },
        ];
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();

        let first = monitor.poll(&runtime, &bindings);
        assert!(runtime.files_for(&bindings).is_empty());
        assert!(
            first.confirmed_observation().is_none(),
            "one raw filesystem sample cannot reach Runtime"
        );
        assert!(runtime.snapshot().is_none());

        let confirmed = monitor.poll(&runtime, &bindings);
        assert_eq!(
            confirmed.confirmed_observation().unwrap().observations()[0].availability,
            MediaAvailability::Candidate
        );
        assert_eq!(
            confirmed.confirmed_observation().unwrap().observations()[1].availability,
            MediaAvailability::Absent
        );
        assert_eq!(
            runtime.snapshot(),
            confirmed.confirmed_observation().cloned()
        );
        assert_eq!(bindings[0].logical_path, root.path().join("photo.jpg"));
        let files = runtime.files_for(&bindings);
        assert_eq!(files.len(), 2);
        assert_eq!(
            files[0].state,
            crate::ipc_contract::MediaFileState::Available
        );
        let metadata = std::fs::metadata(&bindings[0].logical_path).unwrap();
        assert_eq!(
            files[0].created_at_ms,
            super::file_time_millis(metadata.created())
        );
        assert_eq!(
            files[0].modified_at_ms,
            super::file_time_millis(metadata.modified())
        );
        assert_eq!(files[1].state, crate::ipc_contract::MediaFileState::Absent);
        assert_eq!(files[1].created_at_ms, None);
        let mut relinked = bindings.clone();
        relinked[0].logical_path = root.path().join("new-original.jpg");
        assert_eq!(
            runtime.files_for(&relinked).len(),
            1,
            "old-path metadata cannot survive relinking"
        );
    }

    #[test]
    fn runtime_rejects_stale_immutable_proposals() {
        let runtime = MediaRuntime::default();
        let newer = MediaResolutionProposal {
            generation: 2,
            observations: Vec::new(),
        };
        let stale = MediaResolutionProposal {
            generation: 1,
            observations: Vec::new(),
        };
        runtime.apply(newer.clone());

        let update = runtime.apply(stale);
        assert!(update.changed_media_ids().is_empty());
        assert!(update.invalidated_media_ids().is_empty());
        assert_eq!(runtime.snapshot(), Some(newer));
    }

    #[test]
    fn concurrent_pollers_cannot_sample_outside_the_monitor_transition() {
        let root = tempfile::tempdir().expect("temporary serialized-monitor fixture");
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo").expect("the Original fixture is writable");
        let bindings = vec![MediaBinding {
            media_id: "photo-a".into(),
            kind: MediaKind::Photo,
            logical_path: source,
        }];
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();
        let transition_guard = monitor
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (started_sender, started_receiver) = mpsc::sync_channel(0);
        let (finished_sender, finished_receiver) = mpsc::sync_channel(0);
        let queued_monitor = monitor.clone();
        let queued_runtime = runtime.clone();
        let queued = std::thread::spawn(move || {
            started_sender
                .send(())
                .expect("the concurrent poller reaches the transition");
            let poll = queued_monitor.poll(&queued_runtime, &bindings);
            finished_sender
                .send(poll)
                .expect("the serialized poll result is observed");
        });
        started_receiver
            .recv()
            .expect("the concurrent poller starts");
        assert!(
            finished_receiver
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "a poll cannot inspect or mutate stability while another transition owns the gate"
        );
        drop(transition_guard);
        let poll = finished_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the queued poll completes after the transition");
        assert!(poll.update().is_none());
        assert!(poll.confirmed_observation().is_none());
        queued.join().expect("the concurrent poller does not panic");
    }

    #[test]
    fn monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes() {
        let root = tempfile::tempdir().expect("temporary reactive-monitor fixture");
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo-v1").expect("the first Original is written");
        let bindings = vec![MediaBinding {
            media_id: "photo-a".into(),
            kind: MediaKind::Photo,
            logical_path: source.clone(),
        }];
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();

        let first_hint = monitor.poll(&runtime, &bindings);
        assert!(
            first_hint.update().is_none(),
            "one filesystem hint cannot seed Runtime"
        );
        let initial = monitor.poll(&runtime, &bindings);
        assert!(
            initial.update().is_some(),
            "two stable samples seed Runtime"
        );
        assert!(initial.update().unwrap().invalidated_media_ids().is_empty());
        assert_eq!(
            initial.update().unwrap().observation_generation(),
            2,
            "the confirmed update carries its monotonic observation generation"
        );

        std::fs::write(&source, b"photo-version-two-with-a-new-size")
            .expect("the Original changes in place");
        let unstable = monitor.poll(&runtime, &bindings);
        assert!(
            unstable.update().is_none(),
            "one hint cannot invalidate Cache"
        );
        let stable = monitor.poll(&runtime, &bindings);
        assert_eq!(
            stable.update().unwrap().invalidated_media_ids(),
            ["photo-a"]
        );

        std::fs::remove_file(&source).expect("the Original becomes absent");
        let transient_absence = monitor.poll(&runtime, &bindings);
        assert!(transient_absence.update().is_none());
        assert_eq!(
            transient_absence
                .confirmed_observation()
                .unwrap()
                .observations()[0]
                .availability,
            MediaAvailability::Candidate,
            "a raw NotFound sample cannot reach consumers"
        );
        let absent = monitor.poll(&runtime, &bindings);
        assert!(absent.update().unwrap().invalidated_media_ids().is_empty());
        assert!(
            absent
                .update()
                .unwrap()
                .revoked_preview_media_ids()
                .is_empty(),
            "a confirmed absence preserves the last representation as visual context"
        );
        assert_eq!(
            absent.confirmed_observation().unwrap().observations()[0].availability,
            MediaAvailability::Absent
        );

        std::fs::write(&source, b"photo-v3").expect("the Original reappears");
        assert!(monitor.poll(&runtime, &bindings).update().is_none());
        let reappeared = monitor.poll(&runtime, &bindings);
        assert_eq!(
            reappeared.update().unwrap().invalidated_media_ids(),
            ["photo-a"]
        );
    }

    #[test]
    fn external_save_waits_for_read_access_and_refreshes_every_project_occurrence() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("shared-original.jpg");
        std::fs::write(&source, b"previous original").unwrap();
        let binding = |id: &str| MediaBinding {
            media_id: id.into(),
            kind: MediaKind::Photo,
            logical_path: source.clone(),
        };
        let projects = [
            (
                MediaRuntime::default(),
                MediaMonitor::default(),
                vec![binding("a1"), binding("a2")],
            ),
            (
                MediaRuntime::default(),
                MediaMonitor::default(),
                vec![binding("b1")],
            ),
        ];
        for (runtime, monitor, bindings) in &projects {
            monitor.poll(runtime, bindings);
            monitor.poll(runtime, bindings);
        }
        // Photoshop can temporarily deny readers or truncate before writing.
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&source)
            .unwrap();
        for (runtime, monitor, bindings) in &projects {
            assert!(monitor.poll(runtime, bindings).update().is_none());
            let blocked = monitor.poll(runtime, bindings);
            let update = blocked.update().unwrap();
            assert!(update.invalidated_media_ids().is_empty());
            assert!(update.revoked_preview_media_ids().is_empty());
            assert!(
                blocked
                    .confirmed_observation()
                    .unwrap()
                    .observations()
                    .iter()
                    .all(|observation| observation.availability == MediaAvailability::Unavailable)
            );
        }
        drop(writer);
        std::fs::write(&source, b"").unwrap();
        for (runtime, monitor, bindings) in &projects {
            for _ in 0..2 {
                assert!(monitor.poll(runtime, bindings).update().is_none());
            }
        }
        std::fs::write(&source, b"new stable original after the external save").unwrap();
        for (runtime, monitor, bindings) in &projects {
            assert!(monitor.poll(runtime, bindings).update().is_none());
            let restored = monitor.poll(runtime, bindings);
            assert_eq!(
                restored.update().unwrap().invalidated_media_ids(),
                bindings
                    .iter()
                    .map(|binding| binding.media_id.clone())
                    .collect::<Vec<_>>()
            );
            assert!(monitor.poll(runtime, bindings).update().is_none());
        }
    }

    #[test]
    fn relink_invalidates_only_the_changed_occurrence_even_for_the_same_physical_file() {
        let root = tempfile::tempdir().expect("temporary relink fixture");
        let original = root.path().join("photo.jpg");
        let relinked = root.path().join("photo-relinked.jpg");
        std::fs::write(&original, b"same Original bytes").expect("the Original is writable");
        std::fs::hard_link(&original, &relinked)
            .expect("the relink fixture aliases the same physical file");
        let bindings = vec![
            MediaBinding {
                media_id: "photo-a".into(),
                kind: MediaKind::Photo,
                logical_path: original.clone(),
            },
            MediaBinding {
                media_id: "photo-b".into(),
                kind: MediaKind::Photo,
                logical_path: original.clone(),
            },
        ];
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();

        assert!(monitor.poll(&runtime, &bindings).update().is_none());
        assert!(monitor.poll(&runtime, &bindings).update().is_some());

        let relinked_bindings = vec![
            MediaBinding {
                media_id: "photo-a".into(),
                kind: MediaKind::Photo,
                logical_path: relinked,
            },
            bindings[1].clone(),
        ];
        assert!(
            monitor
                .poll(&runtime, &relinked_bindings)
                .update()
                .is_none(),
            "one relink observation cannot invalidate Cache"
        );
        let stable = monitor.poll(&runtime, &relinked_bindings);
        assert_eq!(stable.update().unwrap().changed_media_ids(), ["photo-a"]);
        assert_eq!(
            stable.update().unwrap().invalidated_media_ids(),
            ["photo-a"],
            "relink is scoped by media occurrence, not physical identity"
        );
    }
}
