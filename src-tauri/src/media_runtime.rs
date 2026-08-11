use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use myalbuns_core::MediaKind;
use myalbuns_paths::{ExpectedObject, OperationPathContext, ResolveError};

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
    pub(crate) availability: MediaAvailability,
    physical_identity: Option<String>,
    source_bytes: Option<u64>,
    source_created_unix_ms: Option<u64>,
    source_modified_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaResolutionProposal {
    generation: u64,
    observations: Vec<MediaObservation>,
}

impl MediaResolutionProposal {
    pub(crate) fn observations(&self) -> &[MediaObservation] {
        &self.observations
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MediaRuntimeUpdate {
    changed_media_ids: Vec<String>,
    invalidated_media_ids: Vec<String>,
}

impl MediaRuntimeUpdate {
    pub(crate) fn changed_media_ids(&self) -> &[String] {
        &self.changed_media_ids
    }

    pub(crate) fn invalidated_media_ids(&self) -> &[String] {
        &self.invalidated_media_ids
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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MediaResolver;

impl MediaResolver {
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
                let (
                    availability,
                    physical_identity,
                    source_bytes,
                    source_created_unix_ms,
                    source_modified_unix_ms,
                ) = if let Some(availability) =
                    capture_failures.get(binding.media_id.as_str()).copied()
                {
                    (availability, None, None, None, None)
                } else {
                    match plan.resolve_existing(&binding.logical_path, ExpectedObject::RegularFile)
                    {
                        Ok(resolved) => match resolved.file().metadata() {
                            Ok(metadata) => (
                                MediaAvailability::Candidate,
                                resolved
                                    .physical_identity()
                                    .map(|identity| identity.to_local_token()),
                                Some(metadata.len()),
                                file_time_millis(metadata.created()),
                                file_time_millis(metadata.modified()),
                            ),
                            Err(_) => (MediaAvailability::Unavailable, None, None, None, None),
                        },
                        Err(ResolveError::NotFound) => {
                            (MediaAvailability::Absent, None, None, None, None)
                        }
                        Err(
                            ResolveError::InvalidPath
                            | ResolveError::UnsupportedNamespace
                            | ResolveError::UnboundRoot
                            | ResolveError::AccessDenied
                            | ResolveError::Unavailable
                            | ResolveError::UnexpectedObjectType { .. }
                            | ResolveError::IoFailure,
                        ) => (MediaAvailability::Unavailable, None, None, None, None),
                    }
                };
                MediaObservation {
                    media_id: binding.media_id.clone(),
                    kind: binding.kind,
                    availability,
                    physical_identity,
                    source_bytes,
                    source_created_unix_ms,
                    source_modified_unix_ms,
                }
            })
            .collect();
        MediaResolutionProposal {
            generation,
            observations,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MediaRuntime {
    current: Arc<Mutex<Option<MediaResolutionProposal>>>,
}

impl MediaRuntime {
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
            .collect();
        *current = Some(proposal);
        MediaRuntimeUpdate {
            changed_media_ids,
            invalidated_media_ids,
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
    resolver: MediaResolver,
    next_generation: Arc<AtomicU64>,
    pending: Arc<Mutex<Option<MediaResolutionProposal>>>,
}

impl MediaMonitor {
    pub(crate) fn poll(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
    ) -> MediaMonitorPoll {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let proposal = self.resolver.observe(generation, bindings);
        let current = runtime.snapshot();
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let update = match current {
            Some(current) if current.observations == proposal.observations => {
                *pending = None;
                None
            }
            _ if pending
                .as_ref()
                .is_some_and(|candidate| candidate.observations == proposal.observations) =>
            {
                *pending = None;
                Some(runtime.apply(proposal.clone()))
            }
            _ => {
                *pending = Some(proposal.clone());
                None
            }
        };
        MediaMonitorPoll {
            confirmed_observation: runtime.snapshot(),
            update,
        }
    }
}

fn invalidates_cache(previous: &MediaObservation, current: &MediaObservation) -> bool {
    current.availability == MediaAvailability::Candidate
        && (previous.availability != MediaAvailability::Candidate
            || previous.kind != current.kind
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
    use myalbuns_core::MediaKind;

    use super::{
        MediaAvailability, MediaBinding, MediaMonitor, MediaResolutionProposal, MediaRuntime,
    };

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
}
