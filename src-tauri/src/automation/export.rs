use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use myalbuns_core::{ExportFormat, ExportMode};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::AppPaths;
use serde::Deserialize;
use tauri::Manager;

use super::{
    CANCEL, ConflictChoice, CurrentItem, Event, ItemReport, ItemStatus, Outcome, ProblemReport,
    Question, QuestionOption, REPLACE, RETRY, SKIP, Session, Stop, project_core,
};
use crate::{
    batch_runner::{BatchCancellation, BatchConfiguration, BatchRunner},
    cache_engine::CacheEngine,
    imaging_processor::{ImagingProcessor, TauriImagingTransport},
    ipc_contract::{
        BatchExportProgress, BatchExportView, BatchItemStatus, BatchItemView, BatchPhase,
        BatchProblemKind, ExportConflictPolicy,
    },
    logging::LoggingState,
    operation_gate::{OperationGate, OperationGateError},
    operation_lease::OperationLease,
};

const LOCATE: QuestionOption = QuestionOption {
    id: "relink",
    label: "Localizar imagens…",
    input: Some("folder"),
};
const RESUME: QuestionOption = QuestionOption {
    id: "resume",
    label: "Retomar",
    input: None,
};
const END: QuestionOption = QuestionOption {
    id: "end",
    label: "Encerrar",
    input: None,
};
const KEEP: QuestionOption = QuestionOption {
    id: "keep",
    label: "Decidir depois",
    input: None,
};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum JobFormat {
    Jpeg,
    Png,
    Pdf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExportJob {
    #[allow(dead_code)]
    version: u32,
    source_folder: PathBuf,
    /// Without one, each album is exported beside its project.
    destination_folder: Option<PathBuf>,
    format: JobFormat,
    mode: Option<ExportMode>,
    #[serde(default)]
    on_existing: ConflictChoice,
    /// Folders searched for missing images before any question is asked.
    #[serde(default)]
    relink: Vec<PathBuf>,
}

impl ExportJob {
    fn configuration(&self) -> BatchConfiguration {
        BatchConfiguration {
            source: self.source_folder.clone(),
            destination: self.destination_folder.clone(),
            format: match self.format {
                // The batch always exports JPEG at full quality.
                JobFormat::Jpeg => ExportFormat::Jpeg { quality: 100 },
                JobFormat::Png => ExportFormat::Png,
                JobFormat::Pdf => ExportFormat::Pdf,
            },
            mode: self.mode.unwrap_or(ExportMode::Sheet),
        }
    }
}

/// Decisions that outlive one verification, so a resumed batch does not ask again.
pub(super) struct Resolution {
    existing: ConflictChoice,
    skipped: HashSet<&'static str>,
}

impl Resolution {
    pub(super) fn new(existing: ConflictChoice) -> Self {
        Self {
            existing,
            skipped: HashSet::new(),
        }
    }
}

pub(super) fn run(job: ExportJob, paths: &AppPaths, session: &Session, dry_run: bool) -> i32 {
    let core = project_core(paths);
    let recovery = paths.recovery_dir().join("Batches");
    let mut batch = match prepare(&job, core.clone(), recovery.clone(), session) {
        Ok(batch) => batch,
        Err(message) => return session.fail("preparation-failed", message),
    };
    if dry_run {
        return session.finish(Outcome::Checked, reports(&batch.view()), None);
    }
    let mut resolution = Resolution::new(job.on_existing);
    let mut host = None;
    // Recovery exists from the first attempt on; until then nothing was written.
    let mut recoverable = false;
    loop {
        let policy = match resolve(&mut batch, &mut resolution, session) {
            Ok(policy) => policy,
            Err(Stop::Cancelled) if recoverable => return keep(session, &batch.view()),
            Err(stop) => return session.stop(stop, reports(&batch.view())),
        };
        if session.cancellation().is_requested() && !recoverable {
            return session.finish(Outcome::Cancelled, reports(&batch.view()), None);
        }
        if host.is_none() {
            host = match Host::start(paths) {
                Ok(host) => Some(host),
                Err(message) => return session.fail("export-unavailable", message),
            };
        }
        let host = host.as_ref().expect("the export host was started");
        let cancellation = Arc::new(BatchCancellation::default());
        session.cancellation().attach(cancellation.clone());
        recoverable = true;
        let attempt = host.execute(batch, &cancellation, policy, session);
        session.cancellation().detach();
        batch = match attempt {
            Ok(batch) => batch,
            Err(message) => return session.fail("export-failed", message),
        };
        let view = batch.view();
        let question = match view.phase {
            BatchPhase::Interrupted => Question::new(
                "interrupted",
                "A exportação foi interrompida. Os álbuns já exportados foram mantidos.",
                &[RESUME, END, KEEP],
            ),
            BatchPhase::StorageFull => Question::new(
                "storage-full",
                "Espaço insuficiente. Libere espaço para continuar. Os álbuns já exportados foram mantidos.",
                &[RESUME, END, KEEP],
            ),
            _ => break,
        };
        // A cancel request that stopped the attempt is not an answer to this question.
        session.discard_pending_input();
        let choice = if session.interactive() {
            session.ask(question).map(|answer| answer.option).ok()
        } else {
            None
        };
        match choice.as_deref() {
            Some("resume") => {
                if view.phase == BatchPhase::StorageFull {
                    // The full volume may have refused the last checkpoint; the
                    // live runner still holds the completed items and relinks.
                    batch.retry_preflight();
                } else {
                    batch = match BatchRunner::resume(&recovery, &view.id, core.clone()) {
                        Ok(batch) => batch,
                        Err(message) => return session.fail("resume-failed", message),
                    };
                    // Located images are temporary and do not survive a recovery.
                    locate_in_job_folders(&mut batch, &job, session);
                }
                session.emit(&checked(&batch.view()));
            }
            Some("end") => {
                let items = reports(&view);
                if let Err(message) = batch.abandon(host.can_clean_preparation()) {
                    return session.fail("end-failed", message);
                }
                return session.finish(Outcome::Cancelled, items, None);
            }
            _ => return keep(session, &view),
        }
    }
    let view = batch.view();
    let outcome = if view
        .items
        .iter()
        .all(|item| item.status == BatchItemStatus::Completed)
    {
        Outcome::Completed
    } else {
        Outcome::Partial
    };
    session.finish(outcome, reports(&view), None)
}

/// The recovery stays on disk; the batch export window of the app lists it.
fn keep(session: &Session, view: &BatchExportView) -> i32 {
    session.finish(Outcome::Interrupted, reports(view), Some(view.id.clone()))
}

pub(super) fn prepare(
    job: &ExportJob,
    core: myalbuns_core::ProjectCore,
    recovery: PathBuf,
    session: &Session,
) -> Result<BatchRunner, String> {
    let mut batch = BatchRunner::discover(job.configuration(), core, recovery)?;
    locate_in_job_folders(&mut batch, job, session);
    session.emit(&checked(&batch.view()));
    Ok(batch)
}

fn locate_in_job_folders(batch: &mut BatchRunner, job: &ExportJob, session: &Session) {
    for folder in &job.relink {
        if let Err(error) = batch.relink_all(folder) {
            refuse_folder(session, &error);
        }
    }
}

fn refuse_folder(session: &Session, error: &str) {
    tracing::warn!(target: "myalbuns.desktop", %error, event = "automation_relink_refused");
    session.refuse(
        "relink-failed",
        "Não foi possível procurar as imagens nessa pasta. Confira se ela existe e tente novamente.",
    );
}

fn question_code(item: &BatchItemView) -> &'static str {
    if item
        .problems
        .iter()
        .any(|problem| problem.kind == BatchProblemKind::MissingMedia)
    {
        return "missing-files";
    }
    item.problems
        .first()
        .map_or("failed", |problem| problem_code(item.status, problem.kind))
}

fn problem_code(status: BatchItemStatus, kind: BatchProblemKind) -> &'static str {
    match kind {
        BatchProblemKind::Placeholder => "empty-frames",
        BatchProblemKind::MissingMedia => "missing-files",
        BatchProblemKind::Unavailable => "unavailable",
        BatchProblemKind::InvalidProject => "invalid-project",
        BatchProblemKind::Changed => "changed",
        // The run records only one failure on an ignored item: its existing
        // outputs were kept under the skip policy.
        BatchProblemKind::Failed if status == BatchItemStatus::Ignored => "output-kept",
        BatchProblemKind::Failed => "failed",
    }
}

pub(super) fn resolve(
    batch: &mut BatchRunner,
    resolution: &mut Resolution,
    session: &Session,
) -> Result<ExportConflictPolicy, Stop> {
    loop {
        let view = batch.view();
        let blocked = view
            .items
            .iter()
            .filter(|item| item.status == BatchItemStatus::Pending && !item.problems.is_empty())
            .collect::<Vec<_>>();
        let Some(item) = blocked.first() else {
            break;
        };
        let code = question_code(item);
        if !session.interactive() || resolution.skipped.contains(code) {
            batch.ignore(&item.id)?;
            continue;
        }
        let remaining = blocked
            .iter()
            .filter(|other| question_code(other) == code)
            .count()
            - 1;
        let question = if code == "missing-files" {
            let files = item
                .problems
                .iter()
                .filter(|problem| problem.kind == BatchProblemKind::MissingMedia)
                .filter_map(|problem| problem.file_name.clone())
                .collect::<Vec<_>>();
            let message = if files.len() == 1 {
                format!(
                    "Uma imagem de {} não foi encontrada. Escolha a pasta onde ela está.",
                    item.name
                )
            } else {
                format!(
                    "{} imagens de {} não foram encontradas. Escolha a pasta onde elas estão.",
                    files.len(),
                    item.name
                )
            };
            let mut question = Question::new(code, message, &[LOCATE, SKIP, RETRY, CANCEL]);
            question.files = files;
            question
        } else {
            let mut messages = Vec::<&str>::new();
            for problem in &item.problems {
                if !messages.contains(&problem.message.as_str()) {
                    messages.push(&problem.message);
                }
            }
            Question::new(code, messages.join(" "), &[RETRY, SKIP, CANCEL])
        };
        let answer = session.ask(question.about(&item.id, &item.name, remaining))?;
        match answer.option.as_str() {
            "relink" => {
                let folder = PathBuf::from(answer.value.unwrap_or_default());
                let found = if answer.apply_to_all {
                    batch.relink_all(&folder)
                } else {
                    batch.relink(&item.id, &folder)
                };
                match found {
                    Ok(()) => session.emit(&checked(&batch.view())),
                    Err(error) => refuse_folder(session, &error),
                }
            }
            "retry" => {
                batch.retry_preflight();
                session.emit(&checked(&batch.view()));
            }
            "skip" => {
                batch.ignore(&item.id)?;
                if answer.apply_to_all {
                    resolution.skipped.insert(code);
                }
            }
            _ => return Err(Stop::Cancelled),
        }
    }
    if !batch.view().has_conflicts {
        // Without a known conflict, only an explicit job choice may replace
        // outputs that appear after the verification.
        return Ok(match resolution.existing {
            ConflictChoice::Replace => ExportConflictPolicy::Replace,
            ConflictChoice::Skip => ExportConflictPolicy::Skip,
            ConflictChoice::Ask => ExportConflictPolicy::Ask,
        });
    }
    if resolution.existing == ConflictChoice::Ask {
        resolution.existing = if session.interactive() {
            let answer = session.ask(Question::new(
                "output-exists",
                "Já existe uma exportação no destino. Como deseja tratar os arquivos existentes?",
                &[REPLACE, SKIP, CANCEL],
            ))?;
            match answer.option.as_str() {
                "replace" => ConflictChoice::Replace,
                "skip" => ConflictChoice::Skip,
                _ => return Err(Stop::Cancelled),
            }
        } else {
            ConflictChoice::Skip
        };
    }
    Ok(if resolution.existing == ConflictChoice::Replace {
        ExportConflictPolicy::Replace
    } else {
        ExportConflictPolicy::Skip
    })
}

fn checked(view: &BatchExportView) -> Event {
    Event::Checked {
        items: reports(view),
        can_continue: view.can_continue,
        has_conflicts: view.has_conflicts,
    }
}

pub(super) fn reports(view: &BatchExportView) -> Vec<ItemReport> {
    view.items
        .iter()
        .map(|item| ItemReport {
            id: item.id.clone(),
            name: item.name.clone(),
            project_path: Some(item.project_path.clone()),
            destination: item.destination.clone(),
            status: match item.status {
                BatchItemStatus::Pending => ItemStatus::Pending,
                BatchItemStatus::Completed => ItemStatus::Completed,
                BatchItemStatus::Ignored => ItemStatus::Skipped,
                BatchItemStatus::Failed => ItemStatus::Failed,
            },
            problems: item
                .problems
                .iter()
                .map(|problem| ProblemReport {
                    code: problem_code(item.status, problem.kind),
                    message: problem.message.clone(),
                    file_name: problem.file_name.clone(),
                })
                .collect(),
        })
        .collect()
}

/// The batch reports only counts. Albums are exported one at a time in list
/// order, so the count of finished ones names the album in progress.
pub(super) struct ExportOrder {
    finished_before: u32,
    pending: Vec<(String, String)>,
}

impl ExportOrder {
    pub(super) fn of(view: &BatchExportView) -> Self {
        let pending = view
            .items
            .iter()
            .filter(|item| item.status == BatchItemStatus::Pending)
            .map(|item| (item.id.clone(), item.name.clone()))
            .collect::<Vec<_>>();
        Self {
            finished_before: (view.items.len() - pending.len()) as u32,
            pending,
        }
    }

    pub(super) fn current(&self, progress: &BatchExportProgress) -> Option<CurrentItem> {
        let position = progress.completed.checked_sub(self.finished_before)?;
        let (id, name) = self.pending.get(position as usize)?;
        let percent =
            progress.percent * f64::from(progress.total) - f64::from(progress.completed) * 100.0;
        Some(CurrentItem {
            id: id.clone(),
            name: name.clone(),
            percent: percent.clamp(0.0, 100.0),
        })
    }
}

/// A windowless application: the Image Processor is a sidecar that only the
/// application shell can resolve and supervise.
struct Host {
    app: tauri::App,
    paths: AppPaths,
    gate: OperationGate,
    cache: CacheEngine,
    processor: ImagingProcessor,
}

impl Host {
    fn start(paths: &AppPaths) -> Result<Self, String> {
        let mut app = tauri::Builder::default()
            .plugin(tauri_plugin_shell::init())
            .build(tauri::generate_context!())
            .map_err(|error| {
                eprintln!("could not start the export host: {error}");
                "Não foi possível iniciar a exportação. Confira a instalação do myAlbuns."
            })?;
        crate::logging::initialize(&mut app, paths, ProcessRole::Global);
        Ok(Self {
            app,
            paths: paths.clone(),
            gate: OperationGate::new(paths),
            cache: CacheEngine::default(),
            processor: ImagingProcessor::default(),
        })
    }

    /// A quarantined Processor cannot prove that its child stopped writing.
    fn can_clean_preparation(&self) -> bool {
        tauri::async_runtime::block_on(self.processor.reserve()).is_ok()
    }

    fn execute(
        &self,
        batch: BatchRunner,
        cancellation: &BatchCancellation,
        policy: ExportConflictPolicy,
        session: &Session,
    ) -> Result<BatchRunner, String> {
        let order = ExportOrder::of(&batch.view());
        let reported = Mutex::new((u32::MAX, -1_i64));
        let progress = |progress: BatchExportProgress| {
            let current = order.current(&progress);
            // The Processor reports far more often than a caller can show.
            let step = (
                progress.completed,
                current
                    .as_ref()
                    .map_or(-1, |current| current.percent.floor() as i64),
            );
            let mut reported = reported.lock().expect("export progress is available");
            if *reported == step {
                return;
            }
            *reported = step;
            session.emit(&Event::Progress {
                completed: progress.completed,
                total: Some(progress.total),
                percent: Some(progress.percent),
                current,
            });
        };
        tauri::async_runtime::block_on(async {
            let acquisition = OperationLease::begin(&self.gate).map_err(|error| match error {
                OperationGateError::Conflict => {
                    "Outra exportação está em andamento no myAlbuns. Aguarde o término e tente novamente."
                        .to_owned()
                }
                OperationGateError::Unavailable { reason } => {
                    tracing::warn!(target: "myalbuns.desktop", %reason, event = "automation_operation_gate_unavailable");
                    "Não foi possível iniciar a exportação. Tente novamente.".to_owned()
                }
            })?;
            // Open albums pause their own image work while this owner holds the batch mode.
            let mode = crate::batch_exclusivity::acquire(&self.paths, cancellation).await?;
            let lease = acquisition
                .complete(&self.cache, &self.processor)
                .await
                .map_err(|error| error.to_string())?;
            let handle = self.app.handle();
            let logging = handle.state::<LoggingState>();
            let mut transport =
                TauriImagingTransport::new(handle, &logging, lease.processor_reservation());
            let outcome = batch
                .run(&mut transport, cancellation, policy, &progress)
                .await;
            drop(lease);
            drop(mode);
            outcome
        })
    }
}
