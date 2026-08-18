use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use myalbuns_core::MediaKind;
use myalbuns_paths::{ExpectedObject, OperationPathContext, PhysicalFileIdentity, ResolveError};

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
                                resolved.physical_identity(),
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
                    logical_path: binding.logical_path.clone(),
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
        let revoked_preview_media_ids = proposal
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
    resolver: MediaResolver,
    transition: Arc<Mutex<MediaMonitorTransition>>,
}

#[derive(Debug, Default)]
struct MediaMonitorTransition {
    next_generation: u64,
    pending: Option<MediaResolutionProposal>,
}

impl MediaMonitor {
    pub(crate) fn poll(
        &self,
        runtime: &MediaRuntime,
        bindings: &[MediaBinding],
    ) -> MediaMonitorPoll {
        // A poll owns observation, stability classification and Runtime adoption as
        // one transition. The background loop and demand commands cannot reorder
        // samples or return a snapshot from another generation.
        let mut transition = self
            .transition
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        transition.next_generation = transition
            .next_generation
            .checked_add(1)
            .expect("a MediaMonitor generation cannot exhaust u64");
        let generation = transition.next_generation;
        let proposal = self.resolver.observe(generation, bindings);
        let current = runtime.snapshot();
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
    use std::{sync::mpsc, time::Duration};

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
