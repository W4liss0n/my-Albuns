//! Projects that Windows opens together start together.
//!
//! Every activation launch that is in flight at the same time belongs to one
//! opening burst. A burst owns one opening window that lists its Projects, one
//! decision lane (Recovery and External-copy decisions appear one at a time in
//! that window) and one terminal: when the last launch of the burst ends, the
//! launch that ended it receives the burst terminal and runs the finalizer
//! exactly once. A launch that registers after that point starts a new burst.
//!
//! This module is window-free: the Global supplies the dialog type `L` and the
//! Tauri glue, so the bookkeeping below is testable on its own.

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use myalbuns_paths::NativePathDto;
use tokio::sync::{OwnedMutexGuard, watch};

use crate::{
    global_runtime::{ProjectLaunchFailure, ProjectLaunchOutcome},
    ipc_contract::{
        OpeningProgress, OpeningProjectProgress, OpeningProjectState, StartupImageProgress,
    },
    opening_focus::OpeningFocusClaim,
    project_bootstrap::StartupProgressReporter,
};

/// Folds launch outcomes in the order the launches were registered.
pub(crate) struct ActivationBatchSummary {
    success: ProjectLaunchOutcome,
    first_non_success: Option<ProjectLaunchOutcome>,
    pub(crate) opened_count: u32,
    pub(crate) focused_count: u32,
    pub(crate) failed_count: u32,
}

impl Default for ActivationBatchSummary {
    fn default() -> Self {
        Self {
            success: ProjectLaunchOutcome::Focused,
            first_non_success: None,
            opened_count: 0,
            focused_count: 0,
            failed_count: 0,
        }
    }
}

impl ActivationBatchSummary {
    pub(crate) fn observe(&mut self, outcome: ProjectLaunchOutcome) {
        match outcome {
            ProjectLaunchOutcome::Opened => {
                self.opened_count += 1;
                self.success = ProjectLaunchOutcome::Opened;
            }
            ProjectLaunchOutcome::Focused => self.focused_count += 1,
            other => {
                self.failed_count += 1;
                // A failure outranks a cancellation: answering `Agora não`
                // to one Project must not hide another Project's failure.
                let replaces = match &self.first_non_success {
                    None => true,
                    Some(ProjectLaunchOutcome::Cancelled) => {
                        matches!(other, ProjectLaunchOutcome::Failed { .. })
                    }
                    Some(_) => false,
                };
                if replaces {
                    self.first_non_success = Some(other);
                }
            }
        }
    }

    /// The first failure, else a cancellation, else `Opened` when any Project
    /// opened, else `Focused`.
    pub(crate) fn terminal(&self) -> ProjectLaunchOutcome {
        self.first_non_success
            .clone()
            .unwrap_or_else(|| self.success.clone())
    }
}

/// The name a Project has in the opening window: its file name without the
/// extension. The full path is never shown.
pub(crate) fn project_display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

type ProgressSink = Arc<dyn Fn(&OpeningProgress) + Send + Sync>;
/// Raises (`true`) or lowers (`false`) the window above other windows.
type FrontHook = Arc<dyn Fn(bool) + Send + Sync>;

/// The rows of one opening window. Updates never wait for the window: they
/// change the rows and, when a window is attached and no decision page is up,
/// deliver the whole snapshot to it.
#[derive(Clone, Default)]
pub(crate) struct OpeningProgressBoard(Arc<Mutex<BoardState>>);

#[derive(Default)]
struct BoardState {
    projects: Vec<OpeningProjectProgress>,
    decisions_on_screen: usize,
    sink: Option<ProgressSink>,
    /// Set while a decision is on screen: the window rises once it lists
    /// several Projects, so another Project's editor cannot cover the page.
    front: Option<FrontHook>,
}

impl BoardState {
    fn snapshot(&self) -> OpeningProgress {
        OpeningProgress {
            projects: self.projects.clone(),
        }
    }

    fn deliver(&self) {
        if self.decisions_on_screen == 0
            && let Some(sink) = &self.sink
        {
            sink(&self.snapshot());
        }
    }
}

impl OpeningProgressBoard {
    fn lock(&self) -> MutexGuard<'_, BoardState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn add_project(&self, name: String) -> OpeningRow {
        let mut state = self.lock();
        state.projects.push(OpeningProjectProgress {
            name,
            state: OpeningProjectState::Starting,
            completed_files: 0,
            total_files: 0,
        });
        let index = state.projects.len() - 1;
        state.deliver();
        let front = (state.projects.len() >= 2)
            .then(|| state.front.clone())
            .flatten();
        drop(state);
        if let Some(raise) = front {
            raise(true);
        }
        OpeningRow {
            board: self.clone(),
            index,
        }
    }

    /// Delivers the current rows now and every later change, unless a
    /// decision page is on screen.
    pub(crate) fn attach(&self, sink: impl Fn(&OpeningProgress) + Send + Sync + 'static) {
        let mut state = self.lock();
        state.sink = Some(Arc::new(sink));
        state.deliver();
    }

    pub(crate) fn detach(&self) {
        self.lock().sink = None;
    }

    /// What the window reads when it starts listening. Nothing while a
    /// decision page is up, so the page is not replaced by the list.
    pub(crate) fn window_snapshot(&self) -> Option<OpeningProgress> {
        let state = self.lock();
        (state.decisions_on_screen == 0).then(|| state.snapshot())
    }

    fn row_count(&self) -> usize {
        self.lock().projects.len()
    }

    fn update(&self, index: usize, change: impl FnOnce(&mut OpeningProjectProgress)) {
        let mut state = self.lock();
        let Some(project) = state.projects.get_mut(index) else {
            return;
        };
        change(project);
        state.deliver();
    }
}

fn is_final(state: OpeningProjectState) -> bool {
    matches!(
        state,
        OpeningProjectState::Ready | OpeningProjectState::Failed | OpeningProjectState::Cancelled
    )
}

/// One Project of an opening window.
#[derive(Clone)]
pub(crate) struct OpeningRow {
    board: OpeningProgressBoard,
    index: usize,
}

impl OpeningRow {
    /// Receives the Host's image progress for this Project.
    pub(crate) fn reporter(&self) -> StartupProgressReporter {
        let row = self.clone();
        StartupProgressReporter::new(move |progress| row.report_images(progress))
    }

    fn report_images(&self, progress: StartupImageProgress) {
        self.board.update(self.index, |project| {
            if is_final(project.state) {
                return;
            }
            project.completed_files = progress.completed_files.min(progress.total_files);
            project.total_files = progress.total_files;
            if project.state == OpeningProjectState::Starting {
                project.state = OpeningProjectState::Preparing;
            }
        });
    }

    pub(crate) fn finish(&self, outcome: &ProjectLaunchOutcome) {
        let state = match outcome {
            ProjectLaunchOutcome::Opened | ProjectLaunchOutcome::Focused => {
                OpeningProjectState::Ready
            }
            ProjectLaunchOutcome::Failed { .. } => OpeningProjectState::Failed,
            ProjectLaunchOutcome::Cancelled => OpeningProjectState::Cancelled,
        };
        self.board
            .update(self.index, |project| project.state = state);
    }

    /// The same path opened again after its launch ended: the row starts
    /// over instead of adding a second row for that Project.
    fn restart(&self) {
        self.board.update(self.index, |project| {
            project.state = OpeningProjectState::Starting;
            project.completed_files = 0;
            project.total_files = 0;
        });
    }

    /// The name a decision page shows: only when the window lists several
    /// Projects, because the list is hidden while the decision is up. One
    /// Project keeps today's wording.
    pub(crate) fn decision_name(&self) -> Option<String> {
        let state = self.board.lock();
        (state.projects.len() >= 2)
            .then(|| {
                state
                    .projects
                    .get(self.index)
                    .map(|project| project.name.clone())
            })
            .flatten()
    }

    /// Marks this Project as waiting for a decision and keeps the list away
    /// from the window until the returned guard is dropped; then the latest
    /// rows are delivered, which brings the window back to the list.
    pub(crate) fn present_decision(&self) -> DecisionOnScreen {
        let mut state = self.board.lock();
        state.decisions_on_screen += 1;
        if let Some(project) = state.projects.get_mut(self.index)
            && !is_final(project.state)
        {
            project.state = OpeningProjectState::Deciding;
        }
        DecisionOnScreen { row: self.clone() }
    }
}

pub(crate) struct DecisionOnScreen {
    row: OpeningRow,
}

impl DecisionOnScreen {
    /// Keeps the window above other windows while this decision is on screen
    /// and the window lists several Projects: now, or as soon as another
    /// Project joins. A single opening keeps the window's usual place. The
    /// window returns to it when the decision leaves the screen.
    pub(crate) fn keep_in_front_while_several(&self, front: impl Fn(bool) + Send + Sync + 'static) {
        let front: FrontHook = Arc::new(front);
        let several = {
            let mut state = self.row.board.lock();
            state.front = Some(Arc::clone(&front));
            state.projects.len() >= 2
        };
        if several {
            front(true);
        }
    }
}

impl Drop for DecisionOnScreen {
    fn drop(&mut self) {
        let front = self.row.board.lock().front.take();
        if let Some(front) = front {
            front(false);
        }
        let mut state = self.row.board.lock();
        if let Some(project) = state.projects.get_mut(self.row.index)
            && project.state == OpeningProjectState::Deciding
        {
            project.state = if project.total_files > 0 {
                OpeningProjectState::Preparing
            } else {
                OpeningProjectState::Starting
            };
        }
        state.decisions_on_screen = state.decisions_on_screen.saturating_sub(1);
        state.deliver();
    }
}

/// One burst: its window rows, its decision lane and the signal the next
/// burst waits for before presenting its own window under the same label.
pub(crate) struct OpeningBurst<L> {
    board: OpeningProgressBoard,
    lane: Arc<tokio::sync::Mutex<Option<L>>>,
    /// The window of the last burst that presented one. A burst without a
    /// window never takes that place, so a later window always waits for the
    /// last window that was actually on screen.
    previous_presentation: Option<watch::Receiver<bool>>,
    presentation_closed: watch::Sender<bool>,
    /// Launches whose Host is still starting, deciding or continuing. A
    /// launch that waits for the others before retrying leaves this count.
    running_leads: watch::Sender<usize>,
    /// The focus claim of the files that arrived together (see
    /// `opening_focus`): Windows sends the files of one Explorer selection as
    /// separate activations within a moment, while a file opened later is a
    /// new action whose editor may take the focus again.
    focus_group: Mutex<Option<(Instant, Arc<OpeningFocusClaim>)>>,
}

/// Activations that arrive within this time of a group's first one belong to
/// the same selection.
const FOCUS_GROUP_WINDOW: Duration = Duration::from_secs(5);

impl<L> OpeningBurst<L> {
    pub(crate) fn board(&self) -> &OpeningProgressBoard {
        &self.board
    }

    fn focus_claim_for_batch(&self) -> Option<Arc<OpeningFocusClaim>> {
        let mut group = self
            .focus_group
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((started, claim)) = group.as_ref()
            && started.elapsed() < FOCUS_GROUP_WINDOW
        {
            return Some(Arc::clone(claim));
        }
        let claim = Arc::new(OpeningFocusClaim::new()?);
        *group = Some((Instant::now(), Arc::clone(&claim)));
        Some(claim)
    }

    /// The opening window of the burst. Holding the lock is holding the
    /// decision lane: one decision at a time.
    pub(crate) fn lane(&self) -> &tokio::sync::Mutex<Option<L>> {
        &self.lane
    }

    /// Waits until the previous burst has closed its window.
    pub(crate) async fn previous_presentation_closed(&self) {
        if let Some(mut previous) = self.previous_presentation.clone() {
            let _ = previous.wait_for(|closed| *closed).await;
        }
    }
}

/// Identifies one launch of a path: the path's entry and its generation. A
/// path opened again after its launch ended starts a new generation, which
/// replaces the earlier outcome of that path in the burst terminal.
type PathKey = (usize, u32);

enum Participation {
    Pending,
    Silent,
    Finished {
        outcome: ProjectLaunchOutcome,
        name: Option<String>,
        path: Option<PathKey>,
    },
}

/// One path of the burst and its row.
struct PathEntry {
    path: NativePathDto,
    row: OpeningRow,
    generation: u32,
    /// The launch of this path still in flight; repeats follow it.
    lead: Option<watch::Receiver<Option<ProjectLaunchOutcome>>>,
}

struct CurrentBurst<L> {
    burst: Arc<OpeningBurst<L>>,
    participants: Vec<Participation>,
    paths: Vec<PathEntry>,
    in_flight: usize,
    presenter_assigned: bool,
}

struct Registry<L> {
    current: Option<CurrentBurst<L>>,
    last_presentation: Option<watch::Receiver<bool>>,
}

/// All opening bursts of the Global, one at a time.
pub(crate) struct OpeningBursts<L> {
    registry: Arc<Mutex<Registry<L>>>,
}

impl<L> Clone for OpeningBursts<L> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
        }
    }
}

impl<L> Default for OpeningBursts<L> {
    fn default() -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry {
                current: None,
                last_presentation: None,
            })),
        }
    }
}

/// What a launch leaves behind when it ends.
struct LaunchEnd<'a> {
    outcome: Option<ProjectLaunchOutcome>,
    name: Option<String>,
    /// The path launch this outcome belongs to: its own, or the one a repeat
    /// followed. A later launch of the same path replaces it.
    path: Option<PathKey>,
    lead: Option<LeadEnd<'a>>,
}

struct LeadEnd<'a> {
    path: PathKey,
    outcome: &'a watch::Sender<Option<ProjectLaunchOutcome>>,
}

impl<L> OpeningBursts<L> {
    fn lock(&self) -> MutexGuard<'_, Registry<L>> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Joins the burst in flight, or starts one. The ticket keeps the burst
    /// open until it finishes, so a batch can fail before any Project starts
    /// and still deliver its outcome through the burst terminal.
    pub(crate) fn join_batch(&self) -> BatchTicket<L> {
        let mut registry = self.lock();
        if registry.current.is_none() {
            let previous_presentation = registry.last_presentation.clone();
            registry.current = Some(CurrentBurst {
                burst: Arc::new(OpeningBurst {
                    board: OpeningProgressBoard::default(),
                    lane: Arc::new(tokio::sync::Mutex::new(None)),
                    previous_presentation,
                    presentation_closed: watch::channel(false).0,
                    running_leads: watch::channel(0).0,
                    focus_group: Mutex::new(None),
                }),
                participants: Vec::new(),
                paths: Vec::new(),
                in_flight: 0,
                presenter_assigned: false,
            });
        }
        let current = registry
            .current
            .as_mut()
            .expect("a burst is in flight after joining");
        let participant = register(current);
        BatchTicket {
            bursts: self.clone(),
            focus_claim: current.burst.focus_claim_for_batch(),
            burst: Arc::clone(&current.burst),
            participant,
        }
    }

    fn finish(
        &self,
        burst: &Arc<OpeningBurst<L>>,
        participant: usize,
        end: LaunchEnd<'_>,
    ) -> Option<FinishedBurst<L>> {
        let mut registry = self.lock();
        let current = registry
            .current
            .as_mut()
            .filter(|current| Arc::ptr_eq(&current.burst, burst));
        if let (Some(lead), Some(outcome)) = (&end.lead, &end.outcome) {
            // Repeats follow only a launch in flight: from now on the same
            // path starts its own launch, which focuses the open editor or
            // retries a failed opening.
            if let Some(entry) = current.and_then(|current| {
                current
                    .paths
                    .get_mut(lead.path.0)
                    .filter(|entry| entry.generation == lead.path.1)
            }) {
                entry.lead = None;
            }
            lead.outcome.send_replace(Some(outcome.clone()));
        }
        let current = registry
            .current
            .as_mut()
            .filter(|current| Arc::ptr_eq(&current.burst, burst))?;
        let slot = current.participants.get_mut(participant)?;
        if !matches!(slot, Participation::Pending) {
            return None;
        }
        *slot = match end.outcome {
            Some(outcome) => Participation::Finished {
                outcome,
                name: end.name,
                path: end.path,
            },
            None => Participation::Silent,
        };
        current.in_flight -= 1;
        if current.in_flight > 0 {
            return None;
        }
        let ended = registry.current.take()?;
        let generations = ended
            .paths
            .iter()
            .map(|entry| entry.generation)
            .collect::<Vec<_>>();
        let rows = ended.burst.board.row_count();
        let terminal = burst_terminal(ended.participants, &generations, rows);
        Some(FinishedBurst {
            burst: ended.burst,
            terminal,
        })
    }
}

fn register<L>(current: &mut CurrentBurst<L>) -> usize {
    current.participants.push(Participation::Pending);
    current.in_flight += 1;
    current.participants.len() - 1
}

/// The outcome of a whole burst:
/// - a failure outranks a cancellation; with several Projects listed, the
///   failure names its Project, and several failures become one failure that
///   names each Project with its own reason;
/// - else a cancellation;
/// - else `Opened` when any Project opened, else `Focused`;
/// - `None` when no launch recorded an outcome.
///
/// An outcome replaced by a later launch of the same path does not count.
fn burst_terminal(
    participants: Vec<Participation>,
    generations: &[u32],
    rows: usize,
) -> Option<ProjectLaunchOutcome> {
    let mut summary = ActivationBatchSummary::default();
    let mut observed = false;
    // Keyed by path entry: a repeated path is listed once, while two Projects
    // that only share a file name stay two failures.
    let mut failures: Vec<(Option<usize>, Option<String>, ProjectLaunchFailure)> = Vec::new();
    for participation in participants {
        let Participation::Finished {
            outcome,
            name,
            path,
        } = participation
        else {
            continue;
        };
        if path.is_some_and(|(entry, generation)| generations.get(entry) != Some(&generation)) {
            continue;
        }
        observed = true;
        if let ProjectLaunchOutcome::Failed { error } = &outcome {
            let entry = path.map(|(entry, _)| entry);
            let listed = failures
                .iter()
                .any(|(listed, listed_name, listed_error)| match entry {
                    Some(_) => *listed == entry,
                    None => listed.is_none() && *listed_name == name && listed_error == error,
                });
            if !listed {
                failures.push((entry, name, error.clone()));
            }
        }
        summary.observe(outcome);
    }
    if !observed {
        return None;
    }
    let mut failures = failures
        .into_iter()
        .map(|(_, name, error)| (name, error))
        .collect::<Vec<_>>();
    let error = match failures.len() {
        0 => return Some(summary.terminal()),
        1 => match failures.pop() {
            Some((Some(name), error)) if rows >= 2 => error.naming_project(name),
            Some((_, error)) => error,
            None => return Some(summary.terminal()),
        },
        _ => ProjectLaunchFailure::of_projects(failures),
    };
    Some(ProjectLaunchOutcome::Failed { error })
}

/// One activation batch inside a burst.
pub(crate) struct BatchTicket<L> {
    bursts: OpeningBursts<L>,
    burst: Arc<OpeningBurst<L>>,
    participant: usize,
    focus_claim: Option<Arc<OpeningFocusClaim>>,
}

impl<L> BatchTicket<L> {
    /// Registers one Project of the batch. A path whose launch is still in
    /// flight does not start a second Host: it follows that launch and keeps
    /// a single row. A path whose launch already ended starts again in its
    /// own row.
    pub(crate) fn add_project(&self, path: NativePathDto, name: String) -> ProjectTicket<L> {
        let mut registry = self.bursts.lock();
        let Registry {
            current,
            last_presentation,
        } = &mut *registry;
        let current = current
            .as_mut()
            .filter(|current| Arc::ptr_eq(&current.burst, &self.burst))
            .expect("a batch keeps its burst open until every Project joined");
        let participant = register(current);
        let existing = current.paths.iter().position(|entry| entry.path == path);
        let role = match existing {
            Some(index) if current.paths[index].lead.is_some() => {
                let entry = &current.paths[index];
                ProjectRole::Duplicate {
                    lead: entry.lead.clone().expect("the lead is in flight"),
                    path: (index, entry.generation),
                }
            }
            existing => {
                let (outcome, receiver) = watch::channel(None);
                let (index, row) = match existing {
                    Some(index) => {
                        let entry = &mut current.paths[index];
                        entry.generation += 1;
                        entry.lead = Some(receiver);
                        entry.row.restart();
                        (index, entry.row.clone())
                    }
                    None => {
                        let row = self.burst.board.add_project(name.clone());
                        current.paths.push(PathEntry {
                            path,
                            row: row.clone(),
                            generation: 0,
                            lead: Some(receiver),
                        });
                        (current.paths.len() - 1, row)
                    }
                };
                self.burst.running_leads.send_modify(|count| *count += 1);
                // The first Project presents the window. It takes the lane
                // before anyone can wait on it, so a decision never precedes
                // the window. Only a burst that presents a window becomes the
                // window the next burst waits for.
                let presenter = if current.presenter_assigned {
                    None
                } else {
                    current.presenter_assigned = true;
                    *last_presentation = Some(self.burst.presentation_closed.subscribe());
                    Arc::clone(&self.burst.lane).try_lock_owned().ok()
                };
                ProjectRole::Lead {
                    path: (index, current.paths[index].generation),
                    row,
                    outcome,
                    presenter,
                    running: true,
                }
            }
        };
        ProjectTicket {
            bursts: self.bursts.clone(),
            burst: Arc::clone(&self.burst),
            participant,
            name,
            role,
            focus_claim: self.focus_claim.clone(),
        }
    }

    /// Ends the batch's own participation. `Some` only for a batch that
    /// failed before any Project started.
    pub(crate) fn finish(self, outcome: Option<ProjectLaunchOutcome>) -> Option<FinishedBurst<L>> {
        self.bursts.finish(
            &self.burst,
            self.participant,
            LaunchEnd {
                outcome,
                name: None,
                path: None,
                lead: None,
            },
        )
    }
}

enum ProjectRole<L> {
    Lead {
        path: PathKey,
        row: OpeningRow,
        outcome: watch::Sender<Option<ProjectLaunchOutcome>>,
        presenter: Option<OwnedMutexGuard<Option<L>>>,
        running: bool,
    },
    Duplicate {
        lead: watch::Receiver<Option<ProjectLaunchOutcome>>,
        path: PathKey,
    },
}

/// One Project launch inside a burst.
pub(crate) struct ProjectTicket<L> {
    bursts: OpeningBursts<L>,
    burst: Arc<OpeningBurst<L>>,
    participant: usize,
    name: String,
    role: ProjectRole<L>,
    /// Kept until the launch ends, so its Host can still read the claim.
    focus_claim: Option<Arc<OpeningFocusClaim>>,
}

impl<L> ProjectTicket<L> {
    /// The focus claim of the files opened together with this one.
    pub(crate) fn focus_claim_id(&self) -> Option<&str> {
        self.focus_claim.as_deref().map(OpeningFocusClaim::id)
    }

    pub(crate) fn burst(&self) -> &Arc<OpeningBurst<L>> {
        &self.burst
    }

    /// The row of a Project that starts its own Host; `None` for a repeated
    /// path.
    pub(crate) fn lead_row(&self) -> Option<&OpeningRow> {
        match &self.role {
            ProjectRole::Lead { row, .. } => Some(row),
            ProjectRole::Duplicate { .. } => None,
        }
    }

    /// The lane, already locked, for the Project that presents the window.
    pub(crate) fn take_presenter(&mut self) -> Option<OwnedMutexGuard<Option<L>>> {
        match &mut self.role {
            ProjectRole::Lead { presenter, .. } => presenter.take(),
            ProjectRole::Duplicate { .. } => None,
        }
    }

    /// For a repeated path: the outcome of the launch it follows, with
    /// `Opened` reported as `Focused`. `None` when that launch vanished.
    pub(crate) async fn lead_outcome(&self) -> Option<ProjectLaunchOutcome> {
        let ProjectRole::Duplicate { lead, .. } = &self.role else {
            return None;
        };
        let mut lead = lead.clone();
        let outcome = lead
            .wait_for(Option::is_some)
            .await
            .ok()
            .and_then(|outcome| outcome.clone())?;
        Some(match outcome {
            ProjectLaunchOutcome::Opened => ProjectLaunchOutcome::Focused,
            other => other,
        })
    }

    /// Whether another launch of this burst is still running.
    pub(crate) fn others_running(&self) -> bool {
        let own = usize::from(matches!(self.role, ProjectRole::Lead { running: true, .. }));
        *self.burst.running_leads.borrow() > own
    }

    /// Leaves the running launches and waits until every other launch of the
    /// burst has ended or waits as well. Two launches that wait for each
    /// other both resume, so the wait always ends.
    pub(crate) async fn wait_for_other_launches(&mut self) {
        self.stop_running();
        let mut running = self.burst.running_leads.subscribe();
        let _ = running.wait_for(|count| *count == 0).await;
    }

    fn stop_running(&mut self) {
        if let ProjectRole::Lead { running, .. } = &mut self.role
            && *running
        {
            *running = false;
            self.burst
                .running_leads
                .send_modify(|count| *count = count.saturating_sub(1));
        }
    }

    /// Records the outcome. Returns the finished burst to the launch that
    /// ended it, which must run the finalizer.
    pub(crate) fn finish(mut self, outcome: ProjectLaunchOutcome) -> Option<FinishedBurst<L>> {
        self.stop_running();
        let (path, lead) = match &self.role {
            ProjectRole::Lead {
                path,
                row,
                outcome: sender,
                ..
            } => {
                row.finish(&outcome);
                (
                    *path,
                    Some(LeadEnd {
                        path: *path,
                        outcome: sender,
                    }),
                )
            }
            ProjectRole::Duplicate { path, .. } => (*path, None),
        };
        self.bursts.finish(
            &self.burst,
            self.participant,
            LaunchEnd {
                outcome: Some(outcome),
                name: Some(self.name.clone()),
                path: Some(path),
                lead,
            },
        )
    }
}

/// A burst whose last launch ended. Dropping it tells the next burst that
/// this burst's window is closed.
pub(crate) struct FinishedBurst<L> {
    pub(crate) burst: Arc<OpeningBurst<L>>,
    /// `None` when no launch recorded an outcome.
    pub(crate) terminal: Option<ProjectLaunchOutcome>,
}

impl<L> Drop for FinishedBurst<L> {
    fn drop(&mut self) {
        self.burst.presentation_closed.send_replace(true);
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use super::*;

    fn path(name: &str) -> NativePathDto {
        NativePathDto::from(PathBuf::from(format!(r"C:\Projetos\{name}.myalbuns")))
    }

    fn failure(code: &str) -> ProjectLaunchOutcome {
        serde_json::from_value::<crate::global_runtime::ProjectLaunchFailure>(serde_json::json!({
            "code": code,
            "message": "Não abriu.",
            "action": "Abra de novo."
        }))
        .map(|error| ProjectLaunchOutcome::Failed { error })
        .expect("the failure fixture is valid")
    }

    fn snapshot_rows(board: &OpeningProgressBoard) -> Vec<(String, OpeningProjectState, u32, u32)> {
        board
            .window_snapshot()
            .expect("no decision is on screen")
            .projects
            .into_iter()
            .map(|project| {
                (
                    project.name,
                    project.state,
                    project.completed_files,
                    project.total_files,
                )
            })
            .collect()
    }

    #[test]
    fn the_terminal_lists_failures_in_registration_order() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let first = batch.add_project(path("A"), "A".into());
        let second = batch.add_project(path("B"), "B".into());
        let third = batch.add_project(path("C"), "C".into());
        assert!(batch.finish(None).is_none());

        assert!(third.finish(failure("third")).is_none());
        assert!(second.finish(failure("second")).is_none());
        let finished = first
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the last launch ends the burst");

        let ProjectLaunchOutcome::Failed { error } = finished.terminal.clone().unwrap() else {
            panic!("the burst failed");
        };
        assert_eq!(
            error,
            ProjectLaunchFailure::of_projects(vec![
                (
                    Some("B".into()),
                    match failure("second") {
                        ProjectLaunchOutcome::Failed { error } => error,
                        _ => unreachable!(),
                    }
                ),
                (
                    Some("C".into()),
                    match failure("third") {
                        ProjectLaunchOutcome::Failed { error } => error,
                        _ => unreachable!(),
                    }
                ),
            ])
        );
    }

    #[test]
    fn a_burst_without_failures_ends_opened_when_any_project_opened() {
        for (outcomes, expected) in [
            (
                vec![ProjectLaunchOutcome::Focused, ProjectLaunchOutcome::Opened],
                ProjectLaunchOutcome::Opened,
            ),
            (
                vec![ProjectLaunchOutcome::Focused, ProjectLaunchOutcome::Focused],
                ProjectLaunchOutcome::Focused,
            ),
        ] {
            let bursts = OpeningBursts::<()>::default();
            let batch = bursts.join_batch();
            let tickets: Vec<_> = (0..outcomes.len())
                .map(|index| batch.add_project(path(&index.to_string()), index.to_string()))
                .collect();
            assert!(batch.finish(None).is_none());
            let mut finished = None;
            for (ticket, outcome) in tickets.into_iter().zip(outcomes) {
                finished = ticket.finish(outcome);
            }
            assert_eq!(finished.expect("the burst ended").terminal, Some(expected));
        }
    }

    #[test]
    fn only_the_launch_that_empties_the_burst_runs_the_finalizer() {
        let bursts = OpeningBursts::<()>::default();
        let initial = bursts.join_batch();
        let forwarded = bursts.join_batch();
        assert!(Arc::ptr_eq(&initial.burst, &forwarded.burst));
        let a = initial.add_project(path("A"), "A".into());
        let b = forwarded.add_project(path("B"), "B".into());
        assert!(initial.finish(None).is_none());
        assert!(a.finish(ProjectLaunchOutcome::Opened).is_none());
        assert!(forwarded.finish(None).is_none(), "B is still opening");
        let finished = b.finish(ProjectLaunchOutcome::Opened);
        assert!(finished.is_some());
    }

    #[test]
    fn a_batch_that_fails_before_any_project_still_reaches_the_terminal() {
        let bursts = OpeningBursts::<()>::default();
        let failed = bursts.join_batch();
        let other = bursts.join_batch();
        let project = other.add_project(path("A"), "A".into());
        assert!(other.finish(None).is_none());
        assert!(failed.finish(Some(failure("binding"))).is_none());
        let finished = project
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the last launch ends the burst");
        assert_eq!(finished.terminal, Some(failure("binding")));
    }

    #[test]
    fn a_burst_that_records_no_outcome_has_no_terminal() {
        let bursts = OpeningBursts::<()>::default();
        let withdrawn = bursts.join_batch();
        let finished = withdrawn
            .finish(None)
            .expect("the only ticket ends the burst");
        assert_eq!(finished.terminal, None);
    }

    #[tokio::test]
    async fn a_repeated_path_follows_the_first_launch_in_one_row() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let lead = batch.add_project(path("A"), "A".into());
        let duplicate = batch.add_project(path("A"), "A".into());
        assert!(batch.finish(None).is_none());
        assert!(lead.lead_row().is_some());
        assert!(duplicate.lead_row().is_none());
        assert_eq!(
            lead.burst()
                .board()
                .window_snapshot()
                .unwrap()
                .projects
                .len(),
            1
        );

        let waiting = tokio::spawn(async move {
            let outcome = duplicate.lead_outcome().await;
            (duplicate, outcome)
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !waiting.is_finished(),
            "the duplicate waits for the first launch"
        );
        assert!(lead.finish(ProjectLaunchOutcome::Opened).is_none());
        let (duplicate, outcome) = waiting.await.unwrap();
        assert_eq!(outcome, Some(ProjectLaunchOutcome::Focused));
        let finished = duplicate
            .finish(ProjectLaunchOutcome::Focused)
            .expect("the duplicate ends the burst");
        assert_eq!(finished.terminal, Some(ProjectLaunchOutcome::Opened));

        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let lead = batch.add_project(path("B"), "B".into());
        let duplicate = batch.add_project(path("B"), "B".into());
        let _ = batch.finish(None);
        let _ = lead.finish(failure("in_use"));
        assert_eq!(duplicate.lead_outcome().await, Some(failure("in_use")));
    }

    #[tokio::test]
    async fn a_launch_after_the_end_starts_a_new_burst_that_waits_for_the_old_window() {
        let bursts = OpeningBursts::<()>::default();
        let first = bursts.join_batch();
        let project = first.add_project(path("A"), "A".into());
        let _ = first.finish(None);
        let finished = project
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the first burst ends");

        let second = bursts.join_batch();
        assert!(!Arc::ptr_eq(&finished.burst, &second.burst));
        let next = second.add_project(path("A"), "A".into());
        assert!(next.lead_row().is_some(), "a new burst starts its own Host");
        assert_eq!(
            snapshot_rows(next.burst().board()),
            [("A".to_owned(), OpeningProjectState::Starting, 0, 0)]
        );

        let waiting = tokio::spawn({
            let burst = Arc::clone(next.burst());
            async move { burst.previous_presentation_closed().await }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !waiting.is_finished(),
            "the new window waits for the finalizer of the old one"
        );
        drop(finished);
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("the old window closed")
            .unwrap();
    }

    #[tokio::test]
    async fn the_first_project_presents_the_window_before_any_decision() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let mut first = batch.add_project(path("A"), "A".into());
        let mut second = batch.add_project(path("B"), "B".into());
        let presenter = first.take_presenter().expect("the first Project presents");
        assert!(second.take_presenter().is_none());

        let lane = Arc::clone(&second.burst().lane);
        let decision = tokio::spawn(async move {
            let lane = lane.lock().await;
            *lane
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!decision.is_finished(), "a decision waits for the window");
        let mut presenter = presenter;
        *presenter = Some(());
        drop(presenter);
        assert_eq!(decision.await.unwrap(), Some(()));
    }

    #[test]
    fn rows_follow_progress_and_hide_while_a_decision_is_on_screen() {
        let board = OpeningProgressBoard::default();
        let delivered = Arc::new(Mutex::new(Vec::<OpeningProgress>::new()));
        let a = board.add_project("YUELSON RODRIGO".into());
        let b = board.add_project("SARAH XAVIER".into());
        board.attach({
            let delivered = Arc::clone(&delivered);
            move |snapshot| delivered.lock().unwrap().push(snapshot.clone())
        });
        assert_eq!(
            delivered.lock().unwrap().len(),
            1,
            "attaching delivers the rows"
        );

        b.reporter().publish(StartupImageProgress {
            completed_files: 32,
            total_files: 70,
        });
        a.finish(&ProjectLaunchOutcome::Opened);
        assert_eq!(
            snapshot_rows(&board),
            [
                (
                    "YUELSON RODRIGO".to_owned(),
                    OpeningProjectState::Ready,
                    0,
                    0
                ),
                (
                    "SARAH XAVIER".to_owned(),
                    OpeningProjectState::Preparing,
                    32,
                    70
                ),
            ]
        );
        let delivered_before = delivered.lock().unwrap().len();

        let decision = b.present_decision();
        assert_eq!(board.window_snapshot(), None);
        a.reporter().publish(StartupImageProgress {
            completed_files: 1,
            total_files: 2,
        });
        assert_eq!(
            delivered.lock().unwrap().len(),
            delivered_before,
            "nothing replaces the decision page"
        );
        drop(decision);
        let last = delivered.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.projects[1].state, OpeningProjectState::Preparing);
        assert_eq!(
            last.projects[0].state,
            OpeningProjectState::Ready,
            "a finished Project ignores late progress"
        );

        board.detach();
        b.finish(&failure("late"));
        assert_eq!(
            delivered.lock().unwrap().len(),
            delivered_before + 1,
            "a closed window receives nothing"
        );
        assert_eq!(snapshot_rows(&board)[1].1, OpeningProjectState::Failed);
    }

    #[tokio::test]
    async fn a_path_opened_again_after_its_launch_ended_starts_its_own_launch() {
        let bursts = OpeningBursts::<()>::default();
        let first = bursts.join_batch();
        let a = first.add_project(path("A"), "A".into());
        let b = first.add_project(path("B"), "B".into());
        assert!(first.finish(None).is_none());
        assert!(a.finish(ProjectLaunchOutcome::Opened).is_none());

        // B still prepares, so the burst is open when Windows opens A again.
        let again = bursts.join_batch();
        let repeated = again.add_project(path("A"), "A".into());
        assert!(again.finish(None).is_none());
        assert!(
            repeated.lead_row().is_some(),
            "A goes through its own launch, which focuses the editor already open"
        );
        assert_eq!(
            snapshot_rows(repeated.burst().board()),
            [
                ("A".to_owned(), OpeningProjectState::Starting, 0, 0),
                ("B".to_owned(), OpeningProjectState::Starting, 0, 0),
            ],
            "the Project keeps one row"
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(20), repeated.lead_outcome()).await,
            Ok(None),
            "no outcome of the ended launch is replayed"
        );

        assert!(repeated.finish(ProjectLaunchOutcome::Focused).is_none());
        let finished = b
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the last launch ends the burst");
        assert_eq!(finished.terminal, Some(ProjectLaunchOutcome::Opened));
    }

    #[test]
    fn a_retry_that_opens_replaces_the_earlier_failure_of_its_path() {
        let bursts = OpeningBursts::<()>::default();
        let first = bursts.join_batch();
        let a = first.add_project(path("A"), "A".into());
        let b = first.add_project(path("B"), "B".into());
        assert!(first.finish(None).is_none());
        assert!(a.finish(failure("unavailable")).is_none());

        let again = bursts.join_batch();
        let retry = again.add_project(path("A"), "A".into());
        assert!(again.finish(None).is_none());
        assert!(
            retry.lead_row().is_some(),
            "a failed opening is tried again"
        );
        assert!(retry.finish(ProjectLaunchOutcome::Opened).is_none());
        let finished = b
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the last launch ends the burst");
        assert_eq!(finished.terminal, Some(ProjectLaunchOutcome::Opened));
    }

    #[test]
    fn a_retry_that_opens_also_replaces_the_failure_of_a_repeat_that_followed() {
        let bursts = OpeningBursts::<()>::default();
        let first = bursts.join_batch();
        let a = first.add_project(path("A"), "A".into());
        let repeat = first.add_project(path("A"), "A".into());
        let b = first.add_project(path("B"), "B".into());
        assert!(first.finish(None).is_none());
        assert!(a.finish(failure("unavailable")).is_none());
        assert!(repeat.finish(failure("unavailable")).is_none());

        let again = bursts.join_batch();
        let retry = again.add_project(path("A"), "A".into());
        assert!(again.finish(None).is_none());
        assert!(retry.finish(ProjectLaunchOutcome::Opened).is_none());
        let finished = b
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the last launch ends the burst");
        assert_eq!(finished.terminal, Some(ProjectLaunchOutcome::Opened));
    }

    #[test]
    fn a_decision_rises_above_other_windows_once_several_projects_are_listed() {
        let board = OpeningProgressBoard::default();
        let first = board.add_project("A".into());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let decision = first.present_decision();
        decision.keep_in_front_while_several({
            let calls = Arc::clone(&calls);
            move |front| calls.lock().unwrap().push(front)
        });
        assert!(
            calls.lock().unwrap().is_empty(),
            "a single opening keeps its place"
        );

        let _second = board.add_project("B".into());
        assert_eq!(
            *calls.lock().unwrap(),
            [true],
            "a Project joining raises the window"
        );

        drop(decision);
        assert_eq!(
            *calls.lock().unwrap(),
            [true, false],
            "the window lowers after the decision"
        );
        let _third = board.add_project("C".into());
        assert_eq!(
            *calls.lock().unwrap(),
            [true, false],
            "without a decision on screen nothing rises"
        );
    }

    #[test]
    fn two_projects_that_share_a_file_name_stay_two_failures() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let first = batch.add_project(path("X\\Album"), "Album".into());
        let second = batch.add_project(path("Y\\Album"), "Album".into());
        let _ = batch.finish(None);
        assert!(first.finish(failure("not_found")).is_none());
        let terminal = second
            .finish(failure("not_found"))
            .and_then(|finished| finished.terminal.clone())
            .expect("the burst ends with a terminal");

        let terminal = serde_json::to_value(terminal).unwrap();
        assert_eq!(terminal["error"]["code"], "projects_not_opened");
        assert_eq!(terminal["error"]["projects"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_burst_without_a_window_does_not_end_the_wait_for_the_last_window() {
        let bursts = OpeningBursts::<()>::default();
        let shown = bursts.join_batch();
        let project = shown.add_project(path("A"), "A".into());
        let _ = shown.finish(None);
        let shown = project
            .finish(ProjectLaunchOutcome::Opened)
            .expect("the burst with a window ends");

        // Its window is still closing when a batch fails before any Project.
        let windowless = bursts.join_batch();
        let windowless = windowless
            .finish(Some(failure("binding")))
            .expect("the burst without a window ends at once");
        drop(windowless);

        let next = bursts.join_batch();
        let project = next.add_project(path("B"), "B".into());
        let waiting = tokio::spawn({
            let burst = Arc::clone(project.burst());
            async move { burst.previous_presentation_closed().await }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !waiting.is_finished(),
            "the next window waits for the last window that was on screen"
        );
        drop(shown);
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("the window closed")
            .unwrap();
    }

    #[tokio::test]
    async fn a_launch_retries_only_after_the_other_running_launches_end() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let mapped = batch.add_project(path("A"), "A".into());
        let mut network = batch.add_project(
            NativePathDto::from(PathBuf::from(r"\\servidor\Projetos\A.myalbuns")),
            "A".into(),
        );
        assert!(batch.finish(None).is_none());
        assert!(network.others_running());

        let waiting = tokio::spawn(async move {
            network.wait_for_other_launches().await;
            network
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !waiting.is_finished(),
            "the retry waits while the other spelling still starts"
        );
        assert!(mapped.finish(ProjectLaunchOutcome::Opened).is_none());
        let network = tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("the retry resumes after the other launch")
            .unwrap();
        assert!(!network.others_running());
        assert!(network.finish(ProjectLaunchOutcome::Focused).is_some());

        // Two launches that wait for each other both resume.
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let mut first = batch.add_project(path("C"), "C".into());
        let mut second = batch.add_project(path("D"), "D".into());
        let _ = batch.finish(None);
        let first_wait = tokio::spawn(async move {
            first.wait_for_other_launches().await;
            first
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        tokio::time::timeout(Duration::from_secs(1), second.wait_for_other_launches())
            .await
            .expect("the second wait ends");
        tokio::time::timeout(Duration::from_secs(1), first_wait)
            .await
            .expect("the first wait ends")
            .unwrap();
    }

    #[test]
    fn a_failure_outranks_a_cancellation_and_names_its_project() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let deferred = batch.add_project(path("A"), "YUELSON RODRIGO".into());
        let failed = batch.add_project(path("B"), "SARAH XAVIER".into());
        let opened = batch.add_project(path("C"), "SARAH DA SILVA".into());
        let _ = batch.finish(None);
        assert!(deferred.finish(ProjectLaunchOutcome::Cancelled).is_none());
        assert!(failed.finish(failure("project_in_use")).is_none());
        let terminal = opened
            .finish(ProjectLaunchOutcome::Opened)
            .and_then(|finished| finished.terminal.clone())
            .expect("the burst ends with a terminal");

        let terminal = serde_json::to_value(terminal).unwrap();
        assert_eq!(terminal["status"], "failed");
        assert_eq!(terminal["error"]["code"], "project_in_use");
        assert_eq!(terminal["error"]["message"], "Não abriu.");
        assert_eq!(
            terminal["error"]["projects"],
            serde_json::json!([{
                "name": "SARAH XAVIER",
                "message": "Não abriu.",
                "action": "Abra de novo."
            }])
        );
    }

    #[test]
    fn several_failures_become_one_failure_that_names_each_project() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let first = batch.add_project(path("A"), "A".into());
        let repeated = batch.add_project(path("A"), "A".into());
        let second = batch.add_project(path("B"), "B".into());
        let _ = batch.finish(None);
        assert!(second.finish(failure("not_found")).is_none());
        assert!(first.finish(failure("project_in_use")).is_none());
        let terminal = repeated
            .finish(failure("project_in_use"))
            .and_then(|finished| finished.terminal.clone())
            .expect("the burst ends with a terminal");

        let terminal = serde_json::to_value(terminal).unwrap();
        assert_eq!(terminal["error"]["code"], "projects_not_opened");
        let names = terminal["error"]["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|project| project["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["A", "B"], "a repeated path is listed once");
    }

    #[test]
    fn a_single_project_keeps_its_failure_and_cancelled_ends_cancelled() {
        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let only = batch.add_project(path("A"), "A".into());
        let _ = batch.finish(None);
        let finished = only.finish(failure("not_found")).unwrap();
        assert_eq!(finished.terminal, Some(failure("not_found")));

        let bursts = OpeningBursts::<()>::default();
        let batch = bursts.join_batch();
        let deferred = batch.add_project(path("A"), "A".into());
        let opened = batch.add_project(path("B"), "B".into());
        let _ = batch.finish(None);
        assert!(opened.finish(ProjectLaunchOutcome::Opened).is_none());
        let finished = deferred.finish(ProjectLaunchOutcome::Cancelled).unwrap();
        assert_eq!(finished.terminal, Some(ProjectLaunchOutcome::Cancelled));
    }

    #[test]
    fn a_decision_names_its_project_only_when_the_window_lists_several() {
        let board = OpeningProgressBoard::default();
        let first = board.add_project("SARAH XAVIER".into());
        assert_eq!(first.decision_name(), None);
        let second = board.add_project("YUELSON RODRIGO".into());
        assert_eq!(first.decision_name().as_deref(), Some("SARAH XAVIER"));
        assert_eq!(second.decision_name().as_deref(), Some("YUELSON RODRIGO"));
    }

    #[test]
    fn the_display_name_is_the_file_stem() {
        assert_eq!(
            project_display_name(Path::new(r"\\servidor\clientes\SARAH XAVIER.myalbuns")),
            "SARAH XAVIER"
        );
        assert_eq!(project_display_name(Path::new(r"C:\a\b")), "b");
    }
}
