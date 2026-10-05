mod relink;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::SystemTime,
};

use myalbuns_core::{ImportPhoto, MediaKind, PhotoSourceMetadata};
use myalbuns_paths::{OperationPathContext, RootBindingPlan};

use crate::ipc_contract::ImageProcessingProblem;
use crate::linked_files::{LinkedFiles, map_concurrently};
/// The Monitor's vocabulary; `linked_files` produces these observations.
pub(crate) use crate::linked_files::{MediaAvailability, MediaObservation};

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

/// Size and dates of every binding as its folder lists them. One listing per
/// folder replaces opening each Original, so the Monitor can look often and
/// run a full observation only when this hint changes. It is never evidence:
/// only the full observation confirms a source.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MediaListingHint(Vec<Option<ListedFile>>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ListedFile {
    bytes: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
}

impl MediaListingHint {
    pub(crate) fn read(
        files: &LinkedFiles,
        plan: &RootBindingPlan,
        bindings: &[MediaBinding],
    ) -> Self {
        let mut folders = Vec::<&Path>::new();
        for folder in bindings
            .iter()
            .filter_map(|binding| binding.logical_path.parent())
        {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        let listings = folders
            .iter()
            .copied()
            .zip(files.list_folders(plan, folders.iter().copied()))
            .map(|(folder, listed)| {
                let entries = listed.ok().map(|listed| {
                    listed
                        .entries
                        .into_iter()
                        .map(|entry| {
                            (
                                listing_key(&entry.name),
                                ListedFile {
                                    bytes: entry.bytes,
                                    created: entry.created,
                                    modified: entry.modified,
                                },
                            )
                        })
                        .collect::<HashMap<_, _>>()
                });
                (folder, entries)
            })
            .collect::<HashMap<_, _>>();
        Self(
            bindings
                .iter()
                .map(|binding| {
                    let path = binding.logical_path.as_path();
                    let (folder, name) = (path.parent()?, path.file_name()?);
                    listings
                        .get(folder)?
                        .as_ref()?
                        .get(&listing_key(name))
                        .copied()
                })
                .collect(),
        )
    }
}

/// Windows names compare without case.
fn listing_key(name: &std::ffi::OsStr) -> String {
    name.to_string_lossy().to_lowercase()
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

    pub(crate) fn inspection_update(&self, runtime: &MediaRuntime) -> MediaRuntimeUpdate {
        let current = runtime.snapshot();
        MediaRuntimeUpdate {
            observation_generation: self.generation,
            changed_media_ids: self
                .observations
                .iter()
                .filter(|observation| {
                    observation.availability == MediaAvailability::Candidate
                        && current.as_ref().is_none_or(|current| {
                            !current
                                .observations
                                .iter()
                                .any(|previous| previous == *observation)
                        })
                })
                .map(|observation| observation.media_id.clone())
                .collect(),
            ..MediaRuntimeUpdate::default()
        }
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
    pub(crate) fn unchanged(runtime: &MediaRuntime) -> Self {
        Self {
            confirmed_observation: runtime.snapshot(),
            update: None,
        }
    }
    pub(crate) fn confirmed_observation(&self) -> Option<&MediaResolutionProposal> {
        self.confirmed_observation.as_ref()
    }

    pub(crate) fn update(&self) -> Option<&MediaRuntimeUpdate> {
        self.update.as_ref()
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaRetryInspection {
    availability: MediaAvailability,
    update: MediaRuntimeUpdate,
}

#[cfg(test)]
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
        let files = LinkedFiles::new();
        let inspected = map_concurrently(
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
                        let before = files.observe_one(plan, &binding);
                        let metadata = files
                            .inspect_decoded(plan, &path)
                            .and_then(|header| header.photo_metadata())?;
                        let after = files.observe_one(plan, &binding);
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
            return Err("A ocorrência escolhida não é uma foto.".into());
        }
        let mut context = OperationPathContext::new();
        context
            .capture(&binding.logical_path)
            .map_err(|error| format!("O caminho escolhido é inválido: {error}"))?;
        LinkedFiles::new()
            .inspect_decoded(&context.freeze(), &binding.logical_path)
            .and_then(|header| header.photo_metadata())
    }

    #[cfg(test)]
    pub(crate) fn propose_relink(
        &self,
        binding: &MediaBinding,
        replacement_path: PathBuf,
    ) -> Result<MediaRelinkProposal, String> {
        let mut context = OperationPathContext::new();
        context
            .capture(&binding.logical_path)
            .map_err(|error| error.to_string())?;
        context
            .capture(&replacement_path)
            .map_err(|error| error.to_string())?;
        self.propose_relink_in_plan(binding, replacement_path, &context.freeze())
    }

    pub(crate) fn propose_relink_in_plan(
        &self,
        binding: &MediaBinding,
        replacement_path: PathBuf,
        roots: &RootBindingPlan,
    ) -> Result<MediaRelinkProposal, String> {
        if LinkedFiles::new().observe_one(roots, binding).availability != MediaAvailability::Absent
        {
            return Err("Somente um Arquivo comprovadamente ausente pode ser religado.".into());
        }
        self.propose_replacement_in_plan(binding, replacement_path, roots)
    }

    pub(crate) fn propose_replacement_in_plan(
        &self,
        binding: &MediaBinding,
        replacement_path: PathBuf,
        roots: &RootBindingPlan,
    ) -> Result<MediaRelinkProposal, String> {
        let candidate = MediaBinding {
            logical_path: replacement_path.clone(),
            ..binding.clone()
        };
        let files = LinkedFiles::new();
        let before = files.observe_one(roots, &candidate);
        let inspected = files
            .inspect_decoded(roots, &replacement_path)
            .and_then(|header| header.photo_metadata())?;
        if !before.same_source(&files.observe_one(roots, &candidate)) {
            return Err("O Original mudou durante a inspeção. Tente novamente.".into());
        }

        Ok(MediaRelinkProposal {
            media_id: binding.media_id.clone(),
            kind: binding.kind,
            expected_logical_path: binding.logical_path.clone(),
            replacement_path,
            source_metadata: (binding.kind == MediaKind::Photo).then_some(inspected),
        })
    }

    /// Observes bindings through a plan captured for them alone. A binding
    /// whose root cannot be captured is observed as unavailable.
    pub(crate) fn observe(
        &self,
        generation: u64,
        bindings: &[MediaBinding],
    ) -> MediaResolutionProposal {
        let mut context = OperationPathContext::new();
        for binding in bindings {
            let _ = context.capture(&binding.logical_path);
        }
        MediaResolutionProposal {
            generation,
            observations: LinkedFiles::new().observe(&context.freeze(), bindings),
        }
    }
}

/// The Monitor abandoned an observation of the catalogue because Cache work
/// was asked to pause.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MediaObservationInterrupted;

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
                if file.logical_path() != binding.logical_path || file.kind != binding.kind {
                    return None;
                }
                Some(MediaFileInfo {
                    media_id: binding.media_id.clone(),
                    state: match file.availability {
                        MediaAvailability::Candidate => MediaFileState::Available,
                        MediaAvailability::Absent => MediaFileState::Absent,
                        MediaAvailability::Unavailable => MediaFileState::Unavailable,
                    },
                    created_at_ms: file.created_unix_ms(),
                    modified_at_ms: file.modified_unix_ms(),
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
                    .is_some_and(|previous| observation.changes_content_of(previous))
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
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaMonitor {
    files: LinkedFiles,
    transition: Arc<Mutex<MediaMonitorTransition>>,
}

#[derive(Debug, Default)]
struct MediaMonitorTransition {
    next_generation: u64,
    pending: Option<MediaResolutionProposal>,
}

impl MediaMonitor {
    /// A Monitor observing through a test adapter, such as a slow share.
    #[cfg(test)]
    pub(crate) fn with_linked_files(files: LinkedFiles) -> Self {
        Self {
            files,
            ..Self::default()
        }
    }

    /// A changed source needs two equal observations before it is confirmed.
    pub(crate) fn has_pending_observation(&self) -> bool {
        self.transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending
            .is_some()
    }

    pub(crate) fn prepare_retry_in_plan(
        &self,
        runtime: &MediaRuntime,
        binding: &MediaBinding,
        plan: &RootBindingPlan,
    ) -> Result<MediaResolutionProposal, MediaRetryError> {
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut proposal = runtime.snapshot().ok_or(MediaRetryError::NotUnavailable)?;
        let observation = proposal
            .observations
            .iter_mut()
            .find(|observation| {
                observation.media_id == binding.media_id
                    && observation.availability == MediaAvailability::Unavailable
            })
            .ok_or(MediaRetryError::NotUnavailable)?;
        // Explicit retry observes only its requested occurrence. Every other
        // observation remains unchanged while image inspection is admitted.
        *observation = self.files.observe_one(plan, binding);
        proposal.generation =
            next_observation_generation(&mut transition, Some(proposal.generation));
        Ok(proposal)
    }

    /// Stable filesystem hints are prepared first. Image inspection/admission
    /// runs outside the transition lock; only confirmed sources can be committed.
    pub(crate) fn prepare_in_plan(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
    ) -> Option<MediaResolutionProposal> {
        self.prepare_in_plan_unless(runtime, bindings, plan, &AtomicBool::new(false))
            .expect("an observation nobody interrupts completes")
    }

    /// `prepare_in_plan` for the periodic Monitor, which holds Cache work while
    /// it observes every Original. Once `interrupted` is set it stops waiting,
    /// even for observations already running, and leaves the pending
    /// stabilization untouched, so a Cache pause does not wait for the
    /// catalogue nor for a server that is switched off.
    pub(crate) fn prepare_in_plan_unless(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
        interrupted: &AtomicBool,
    ) -> Result<Option<MediaResolutionProposal>, MediaObservationInterrupted> {
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = runtime.snapshot();
        let observations = self
            .files
            .observe_unless(plan, bindings, interrupted)
            .ok_or(MediaObservationInterrupted)?;
        let generation = next_observation_generation(
            &mut transition,
            current.as_ref().map(|current| current.generation),
        );
        let proposal = MediaResolutionProposal {
            generation,
            observations,
        };
        if current
            .as_ref()
            .is_some_and(|current| current.observations == proposal.observations)
        {
            transition.pending = None;
            return Ok(None);
        }
        let stable = transition
            .pending
            .as_ref()
            .is_some_and(|pending| pending.observations == proposal.observations);
        transition.pending = Some(proposal.clone());
        Ok(stable.then_some(proposal))
    }

    pub(crate) fn commit_prepared(
        &self,
        runtime: &MediaRuntime,
        mut proposal: MediaResolutionProposal,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
        readable_media: &[String],
    ) -> MediaMonitorPoll {
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = runtime.snapshot();
        if current
            .as_ref()
            .is_some_and(|current| current.generation >= proposal.generation)
        {
            return MediaMonitorPoll {
                confirmed_observation: current,
                update: None,
            };
        }
        let previous = current
            .as_ref()
            .map(|current| current.observations.as_slice())
            .unwrap_or_default();
        let previous_of = |observation: &MediaObservation| {
            previous
                .iter()
                .find(|old| old.media_id == observation.media_id)
        };
        let rechecked = self.files.observe(
            plan,
            proposal
                .observations
                .iter()
                .filter(|observation| {
                    observation.availability == MediaAvailability::Candidate
                        && previous_of(observation) != Some(*observation)
                })
                .filter_map(|observation| {
                    bindings
                        .iter()
                        .find(|binding| binding.media_id == observation.media_id)
                }),
        );
        let rechecked = rechecked
            .iter()
            .map(|current| (current.media_id.as_str(), current))
            .collect::<HashMap<_, _>>();
        for observation in &mut proposal.observations {
            let old = previous_of(observation);
            if old == Some(observation) {
                continue;
            }
            if observation.availability == MediaAvailability::Candidate {
                let source_unchanged = rechecked
                    .get(observation.media_id.as_str())
                    .is_some_and(|current| observation.same_source(current));
                let readable = readable_media.contains(&observation.media_id);
                if !source_unchanged || !readable {
                    if let Some(old) = old {
                        *observation = old.clone();
                    } else {
                        observation.availability = MediaAvailability::Unavailable;
                    }
                }
            }
        }
        transition.pending = None;
        let update = runtime.apply(proposal);
        MediaMonitorPoll {
            confirmed_observation: runtime.snapshot(),
            update: Some(update),
        }
    }

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
        let evidenced = bindings
            .iter()
            .filter_map(|binding| {
                let expected = evidence.get(binding.media_id.as_str())?;
                (binding.kind == expected.kind && binding.logical_path == expected.logical_path())
                    .then_some((binding, *expected))
            })
            .collect::<Vec<_>>();
        let observations = self
            .files
            .observe(plan, evidenced.iter().map(|(binding, _)| *binding));
        for ((binding, expected), observed) in evidenced.into_iter().zip(observations) {
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

    // These filesystem-state fixtures inject successful image inspection;
    // decoder/admission behavior is covered by the real-image integration tests.
    #[cfg(test)]
    pub(crate) fn poll_readable_fixture_in_plan(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
        plan: &RootBindingPlan,
    ) -> MediaMonitorPoll {
        let Some(prepared) = self.prepare_in_plan(runtime, bindings, plan) else {
            return MediaMonitorPoll::unchanged(runtime);
        };
        let readable = bindings
            .iter()
            .map(|binding| binding.media_id.clone())
            .collect::<Vec<_>>();
        self.commit_prepared(runtime, prepared, bindings, plan, &readable)
    }

    #[cfg(test)]
    pub(crate) fn poll_readable_fixture(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
    ) -> MediaMonitorPoll {
        let mut paths = OperationPathContext::new();
        for binding in bindings {
            let _ = paths.capture(&binding.logical_path);
        }
        self.poll_readable_fixture_in_plan(runtime, bindings, &paths.freeze())
    }

    #[cfg(test)]
    pub(crate) fn retry_readable_fixture(
        &self,
        runtime: &MediaRuntime,
        binding: &MediaBinding,
    ) -> Result<MediaRetryInspection, MediaRetryError> {
        let mut paths = OperationPathContext::new();
        let _ = paths.capture(&binding.logical_path);
        let plan = paths.freeze();
        let prepared = self.prepare_retry_in_plan(runtime, binding, &plan)?;
        let poll = self.commit_prepared(
            runtime,
            prepared,
            std::slice::from_ref(binding),
            &plan,
            std::slice::from_ref(&binding.media_id),
        );
        let availability = poll
            .confirmed_observation()
            .unwrap()
            .observations()
            .iter()
            .find(|observation| observation.media_id == binding.media_id)
            .unwrap()
            .availability;
        Ok(MediaRetryInspection {
            availability,
            update: poll.update().cloned().unwrap_or_default(),
        })
    }

    #[cfg(test)]
    pub(crate) fn hold_transition_for_test(&self, while_held: impl FnOnce()) {
        let _transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while_held();
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

#[cfg(test)]
mod tests {
    #[test]
    fn listing_hint_changes_only_when_a_listed_original_changes() {
        let root = tempfile::tempdir().unwrap();
        let photo = root.path().join("Foto.JPG");
        let other = root.path().join("Outra.jpg");
        std::fs::write(&photo, b"photo").unwrap();
        std::fs::write(&other, b"other").unwrap();
        let bindings = [
            // Windows names compare without case.
            super::MediaBinding {
                media_id: "foto".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: root.path().join("foto.jpg"),
            },
            super::MediaBinding {
                media_id: "outra".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: other.clone(),
            },
            super::MediaBinding {
                media_id: "ausente".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: root.path().join("ausente.jpg"),
            },
        ];

        let mut paths = myalbuns_paths::OperationPathContext::new();
        paths.capture(root.path()).unwrap();
        let plan = paths.freeze();
        let read = || {
            super::MediaListingHint::read(
                &crate::linked_files::LinkedFiles::new(),
                &plan,
                &bindings,
            )
        };
        let first = read();
        assert_eq!(first, read());
        assert!(first.0[0].is_some() && first.0[1].is_some() && first.0[2].is_none());

        std::fs::write(&photo, b"edited photo").unwrap();
        assert_ne!(first, read());
    }

    #[test]
    fn prepared_retry_confirms_only_its_occurrence_after_image_inspection() {
        use myalbuns_paths::OperationPathContext;
        let root = tempfile::tempdir().unwrap();
        let bindings = ["selected", "other"].map(|id| MediaBinding {
            media_id: id.into(),
            kind: MediaKind::Photo,
            logical_path: root.path().join(format!("{id}.jpg")),
        });
        for binding in &bindings {
            RgbImage::from_pixel(20, 10, Rgb([30, 80, 120]))
                .save_with_format(&binding.logical_path, ImageFormat::Jpeg)
                .unwrap();
        }
        let runtime = MediaRuntime::default();
        let mut prior = MediaResolver.observe(1, &bindings);
        prior.observations[0].availability = MediaAvailability::Unavailable;
        runtime.apply(prior.clone());
        std::fs::write(&bindings[1].logical_path, b"another changed Original").unwrap();
        let mut context = OperationPathContext::new();
        context.capture(&bindings[0].logical_path).unwrap();
        let roots = context.freeze();
        let monitor = MediaMonitor::default();
        let prepared = monitor
            .prepare_retry_in_plan(&runtime, &bindings[0], &roots)
            .unwrap();
        assert_eq!(
            runtime.snapshot(),
            Some(prior.clone()),
            "preparation cannot adopt uninspected bytes"
        );
        crate::linked_files::LinkedFiles::new()
            .inspect_decoded(&roots, &bindings[0].logical_path)
            .and_then(|header| header.photo_metadata())
            .unwrap();
        let poll = monitor.commit_prepared(
            &runtime,
            prepared,
            &bindings[..1],
            &roots,
            &["selected".into()],
        );
        assert_eq!(poll.update().unwrap().changed_media_ids(), ["selected"]);
        assert_eq!(
            runtime.snapshot().unwrap().observations[1],
            prior.observations[1]
        );
        assert_eq!(
            monitor.prepare_retry_in_plan(&runtime, &bindings[0], &roots),
            Err(MediaRetryError::NotUnavailable)
        );
    }

    #[test]
    fn prepared_confirmation_rejects_replaced_source_and_older_generation() {
        use myalbuns_paths::OperationPathContext;
        let root = tempfile::tempdir().unwrap();
        let binding = MediaBinding {
            media_id: "photo".into(),
            kind: MediaKind::Photo,
            logical_path: root.path().join("photo.jpg"),
        };
        let bindings = std::slice::from_ref(&binding);
        let write = |width| {
            RgbImage::from_pixel(width, 10, Rgb([30, 80, 120]))
                .save_with_format(&binding.logical_path, ImageFormat::Jpeg)
                .unwrap()
        };
        write(20);
        let runtime = MediaRuntime::default();
        runtime.apply(MediaResolver.observe(1, bindings));
        let before = runtime.snapshot().unwrap().observations;
        let monitor = MediaMonitor::default();
        let mut context = OperationPathContext::new();
        context.capture(&binding.logical_path).unwrap();
        let roots = context.freeze();
        write(60);
        monitor.prepare_in_plan(&runtime, bindings, &roots);
        let older = monitor.prepare_in_plan(&runtime, bindings, &roots).unwrap();
        crate::linked_files::LinkedFiles::new()
            .inspect_decoded(&roots, &binding.logical_path)
            .and_then(|header| header.photo_metadata())
            .unwrap();
        write(120);
        monitor.commit_prepared(&runtime, older.clone(), bindings, &roots, &["photo".into()]);
        assert_eq!(runtime.snapshot().unwrap().observations, before);
        monitor.prepare_in_plan(&runtime, bindings, &roots);
        let newer = monitor.prepare_in_plan(&runtime, bindings, &roots).unwrap();
        crate::linked_files::LinkedFiles::new()
            .inspect_decoded(&roots, &binding.logical_path)
            .and_then(|header| header.photo_metadata())
            .unwrap();
        monitor.commit_prepared(&runtime, newer, bindings, &roots, &["photo".into()]);
        let committed = runtime.snapshot();
        let stale = monitor.commit_prepared(&runtime, older, bindings, &roots, &["photo".into()]);
        assert!(stale.update().is_none());
        assert_eq!(runtime.snapshot(), committed);
    }

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
            .map(|binding| crate::linked_files::LinkedFiles::new().observe_one(&roots, binding))
            .collect::<Vec<_>>();
        std::fs::write(&paths[2], b"changed after decode").unwrap();
        let before = crate::linked_files::photo_source_decode_count();
        let poll = monitor.adopt_prepared_inspections(&runtime, &bindings, &roots, &observations);
        assert_eq!(crate::linked_files::photo_source_decode_count(), before);
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
            monitor
                .poll_readable_fixture(&runtime, &bindings)
                .update()
                .is_none(),
            "unproven changes still need ordinary stabilization"
        );
    }

    #[test]
    fn an_interrupted_monitor_observation_keeps_the_pending_stabilization() {
        use std::sync::atomic::AtomicBool;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("photo.jpg");
        std::fs::write(&source, b"photo").unwrap();
        let bindings = vec![MediaBinding {
            media_id: "photo".into(),
            kind: MediaKind::Photo,
            logical_path: source.clone(),
        }];
        let mut paths = myalbuns_paths::OperationPathContext::new();
        paths.capture(&source).unwrap();
        let roots = paths.freeze();
        let runtime = MediaRuntime::default();
        let monitor = MediaMonitor::default();

        assert!(
            monitor
                .prepare_in_plan(&runtime, &bindings, &roots)
                .is_none(),
            "a first sample is not stable yet"
        );
        assert_eq!(
            monitor.prepare_in_plan_unless(&runtime, &bindings, &roots, &AtomicBool::new(true)),
            Err(super::MediaObservationInterrupted)
        );
        assert!(monitor.has_pending_observation());
        assert!(
            monitor
                .prepare_in_plan(&runtime, &bindings, &roots)
                .is_some(),
            "the next sample still stabilizes against the one before the interruption"
        );
    }

    #[test]
    fn a_cache_pause_waits_neither_for_a_monitor_sweep_nor_for_one_slow_access() {
        use crate::{cache_activity_gate::CacheCancellation, cache_engine::CacheEngine};
        let root = tempfile::tempdir().unwrap();
        let bindings = (0..80)
            .map(|index| {
                let logical_path = root.path().join(format!("{index}.jpg"));
                std::fs::write(&logical_path, b"photo").unwrap();
                MediaBinding {
                    media_id: format!("photo-{index}"),
                    kind: MediaKind::Photo,
                    logical_path,
                }
            })
            .collect::<Vec<_>>();
        let mut paths = myalbuns_paths::OperationPathContext::new();
        paths.capture(root.path()).unwrap();
        let roots = paths.freeze();
        // Like a server that is switched off: each access waits a long time.
        let latency = std::time::Duration::from_secs(1);
        let monitor = MediaMonitor::with_linked_files(
            crate::linked_files::LinkedFiles::with_latency(latency),
        );
        let runtime = MediaRuntime::default();
        let engine = CacheEngine::default();

        tauri::async_runtime::block_on(async {
            let cancellation = CacheCancellation::default();
            let permit = engine.begin_cancellable_work(cancellation.clone()).await;
            let sweeping = std::thread::spawn({
                let (monitor, runtime, bindings, roots) = (
                    monitor.clone(),
                    runtime.clone(),
                    bindings.clone(),
                    roots.clone(),
                );
                let cancellation = cancellation.clone();
                move || {
                    let result = monitor.prepare_in_plan_unless(
                        &runtime,
                        &bindings,
                        &roots,
                        cancellation.flag(),
                    );
                    drop(permit);
                    result
                }
            });
            tokio::time::sleep(latency / 10).await;
            let started = std::time::Instant::now();
            let _pause = engine.pause().await;
            let waited = started.elapsed();
            assert_eq!(
                sweeping.join().unwrap(),
                Err(super::MediaObservationInterrupted)
            );
            assert!(
                waited < latency / 2,
                "the pause waited {waited:?}; one access takes {latency:?}"
            );
        });
        assert!(
            !monitor.has_pending_observation(),
            "an interrupted sweep proposes nothing"
        );
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
        *selected_prior = MediaObservation::unavailable_for_test(&selected);
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
            .retry_readable_fixture(&runtime, &selected)
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
                .logical_path(),
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
            .retry_readable_fixture(&runtime, &binding)
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
            observations: vec![MediaObservation::unavailable_for_test(&binding)],
        });

        let retried = MediaMonitor::default()
            .retry_readable_fixture(&runtime, &binding)
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
            .retry_readable_fixture(&runtime, &binding)
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
            .retry_readable_fixture(&runtime, &selected)
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
            .retry_readable_fixture(&runtime, &binding)
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
            .retry_readable_fixture(&runtime, &binding)
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

        let first = monitor.poll_readable_fixture(&runtime, &bindings);
        assert!(runtime.files_for(&bindings).is_empty());
        assert!(
            first.confirmed_observation().is_none(),
            "one raw filesystem sample cannot reach Runtime"
        );
        assert!(runtime.snapshot().is_none());

        let confirmed = monitor.poll_readable_fixture(&runtime, &bindings);
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
        let millis = |time: std::io::Result<std::time::SystemTime>| {
            time.ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        };
        assert_eq!(files[0].created_at_ms, millis(metadata.created()));
        assert_eq!(files[0].modified_at_ms, millis(metadata.modified()));
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
            let poll = queued_monitor.poll_readable_fixture(&queued_runtime, &bindings);
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

        let first_hint = monitor.poll_readable_fixture(&runtime, &bindings);
        assert!(
            first_hint.update().is_none(),
            "one filesystem hint cannot seed Runtime"
        );
        let initial = monitor.poll_readable_fixture(&runtime, &bindings);
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
        let unstable = monitor.poll_readable_fixture(&runtime, &bindings);
        assert!(
            unstable.update().is_none(),
            "one hint cannot invalidate prévias temporárias"
        );
        let stable = monitor.poll_readable_fixture(&runtime, &bindings);
        assert_eq!(
            stable.update().unwrap().invalidated_media_ids(),
            ["photo-a"]
        );

        std::fs::remove_file(&source).expect("the Original becomes absent");
        let transient_absence = monitor.poll_readable_fixture(&runtime, &bindings);
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
        let absent = monitor.poll_readable_fixture(&runtime, &bindings);
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
        assert!(
            monitor
                .poll_readable_fixture(&runtime, &bindings)
                .update()
                .is_none()
        );
        let reappeared = monitor.poll_readable_fixture(&runtime, &bindings);
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
            monitor.poll_readable_fixture(runtime, bindings);
            monitor.poll_readable_fixture(runtime, bindings);
        }
        // Photoshop can temporarily deny readers or truncate before writing.
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&source)
            .unwrap();
        for (runtime, monitor, bindings) in &projects {
            assert!(
                monitor
                    .poll_readable_fixture(runtime, bindings)
                    .update()
                    .is_none()
            );
            let blocked = monitor.poll_readable_fixture(runtime, bindings);
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
                assert!(
                    monitor
                        .poll_readable_fixture(runtime, bindings)
                        .update()
                        .is_none()
                );
            }
        }
        std::fs::write(&source, b"new stable original after the external save").unwrap();
        for (runtime, monitor, bindings) in &projects {
            assert!(
                monitor
                    .poll_readable_fixture(runtime, bindings)
                    .update()
                    .is_none()
            );
            let restored = monitor.poll_readable_fixture(runtime, bindings);
            assert_eq!(
                restored.update().unwrap().invalidated_media_ids(),
                bindings
                    .iter()
                    .map(|binding| binding.media_id.clone())
                    .collect::<Vec<_>>()
            );
            assert!(
                monitor
                    .poll_readable_fixture(runtime, bindings)
                    .update()
                    .is_none()
            );
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

        assert!(
            monitor
                .poll_readable_fixture(&runtime, &bindings)
                .update()
                .is_none()
        );
        assert!(
            monitor
                .poll_readable_fixture(&runtime, &bindings)
                .update()
                .is_some()
        );

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
                .poll_readable_fixture(&runtime, &relinked_bindings)
                .update()
                .is_none(),
            "one relink observation cannot invalidate prévias temporárias"
        );
        let stable = monitor.poll_readable_fixture(&runtime, &relinked_bindings);
        assert_eq!(stable.update().unwrap().changed_media_ids(), ["photo-a"]);
        assert_eq!(
            stable.update().unwrap().invalidated_media_ids(),
            ["photo-a"],
            "relink is scoped by media occurrence, not physical identity"
        );
    }
}
