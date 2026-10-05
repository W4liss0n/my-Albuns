//! Line-delimited JSON sessions that let another program drive batch work.
//!
//! The caller decides by `code` and by option `id`. `message` and `label` are
//! text to show a person and may be reworded without a protocol change.
mod export;
mod generation;
#[cfg(test)]
mod tests;

use std::{
    ffi::OsString,
    io::{BufRead, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
};

use myalbuns_core::ProjectCore;
use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};

use crate::batch_runner::BatchCancellation;

const PROTOCOL_VERSION: u32 = 1;
const JOB_VERSION: u32 = 1;

const EXIT_COMPLETED: i32 = 0;
const EXIT_FAILED: i32 = 1;
const EXIT_PARTIAL: i32 = 2;
const EXIT_INTERRUPTED: i32 = 3;
const EXIT_USAGE: i32 = 64;

const USAGE: &str = "\
Uso: myalbuns-cli <comando> --job <arquivo.json | -> [--non-interactive] [--dry-run]

Comandos:
  generate-batch   Gera um projeto para cada pasta com fotos, a partir de um projeto modelo.
  export-batch     Exporta todos os projetos de uma pasta.

Opções:
  --job <arquivo>      Arquivo JSON com o trabalho. Use - para ler o trabalho da primeira linha da entrada.
  --non-interactive    Não faz perguntas: ignora os projetos com problemas e mantém os arquivos existentes.
  --dry-run            Só verifica e informa o que seria feito.";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ConflictChoice {
    #[default]
    Ask,
    Skip,
    Replace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum ItemStatus {
    Pending,
    Completed,
    Skipped,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    /// `--dry-run` finished; nothing was written.
    Checked,
    Completed,
    Partial,
    Cancelled,
    /// A batch export stopped and kept its recovery for a later resume.
    Interrupted,
}

impl Outcome {
    fn exit_code(self) -> i32 {
        match self {
            Self::Checked | Self::Completed => EXIT_COMPLETED,
            Self::Partial => EXIT_PARTIAL,
            Self::Cancelled | Self::Interrupted => EXIT_INTERRUPTED,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProblemReport {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemReport {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<String>,
    destination: String,
    status: ItemStatus,
    problems: Vec<ProblemReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct CurrentItem {
    id: String,
    name: String,
    percent: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct QuestionOption {
    id: &'static str,
    label: &'static str,
    /// Names the value the answer must carry, such as a folder path.
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<&'static str>,
}

const RETRY: QuestionOption = QuestionOption {
    id: "retry",
    label: "Verificar novamente",
    input: None,
};
const SKIP: QuestionOption = QuestionOption {
    id: "skip",
    label: "Ignorar",
    input: None,
};
const REPLACE: QuestionOption = QuestionOption {
    id: "replace",
    label: "Substituir",
    input: None,
};
const CANCEL: QuestionOption = QuestionOption {
    id: "cancel",
    label: "Cancelar",
    input: None,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Question {
    /// Assigned by the session; the answer repeats it.
    id: String,
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<String>,
    /// Other items waiting for the same decision; `applyToAll` covers them.
    remaining: u32,
    options: Vec<QuestionOption>,
}

impl Question {
    fn new(code: &'static str, message: impl Into<String>, options: &[QuestionOption]) -> Self {
        Self {
            id: String::new(),
            code,
            message: message.into(),
            item: None,
            name: None,
            files: Vec::new(),
            remaining: 0,
            options: options.to_vec(),
        }
    }

    fn about(mut self, id: &str, name: &str, remaining: usize) -> Self {
        self.item = Some(id.into());
        self.name = Some(name.into());
        self.remaining = remaining as u32;
        self
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
enum Event {
    Started {
        protocol: u32,
        command: &'static str,
    },
    #[serde(rename_all = "camelCase")]
    Checked {
        items: Vec<ItemReport>,
        can_continue: bool,
        /// Something already exists at a destination and needs a decision.
        has_conflicts: bool,
    },
    Question(Question),
    Progress {
        completed: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        percent: Option<f64>,
        /// The album being exported; generation writes several projects at once.
        #[serde(skip_serializing_if = "Option::is_none")]
        current: Option<CurrentItem>,
    },
    #[serde(rename_all = "camelCase")]
    Result {
        status: Outcome,
        items: Vec<ItemReport>,
        /// Identifies a kept batch export recovery.
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
    },
    /// A fatal error ends the session; any other one only reports a refused step.
    Error {
        code: &'static str,
        message: String,
        fatal: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IncomingLine {
    answer: Option<String>,
    option: Option<String>,
    value: Option<String>,
    #[serde(default)]
    apply_to_all: bool,
    command: Option<String>,
}

#[derive(Debug)]
struct Answer {
    option: String,
    value: Option<String>,
    apply_to_all: bool,
}

#[derive(Debug)]
enum Incoming {
    Answer { question: String, answer: Answer },
    Cancel,
    Invalid,
    Closed,
}

#[derive(Debug)]
enum Stop {
    Cancelled,
    InputClosed,
    Failed(String),
}

impl From<String> for Stop {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// One cancellation request reaches the blocking generation and the current
/// batch export attempt alike.
#[derive(Default)]
struct Cancellation {
    requested: AtomicBool,
    batch: Mutex<Option<Arc<BatchCancellation>>>,
}

impl Cancellation {
    fn request(&self) {
        self.requested.store(true, Ordering::Release);
        if let Some(batch) = &*self.batch.lock().expect("cancellation is available") {
            batch.request();
        }
    }

    fn flag(&self) -> &AtomicBool {
        &self.requested
    }

    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    fn attach(&self, batch: Arc<BatchCancellation>) {
        let mut current = self.batch.lock().expect("cancellation is available");
        if self.is_requested() {
            batch.request();
        }
        *current = Some(batch);
    }

    /// Ends an attempt; a resumed one must be cancelled again to stop.
    fn detach(&self) {
        self.batch.lock().expect("cancellation is available").take();
        self.requested.store(false, Ordering::Release);
    }
}

struct Session {
    output: Mutex<Box<dyn Write + Send>>,
    input: Mutex<mpsc::Receiver<Incoming>>,
    cancellation: Arc<Cancellation>,
    interactive: bool,
    questions: AtomicU32,
}

impl Session {
    fn start(
        output: Box<dyn Write + Send>,
        input: impl BufRead + Send + 'static,
        interactive: bool,
    ) -> Self {
        let cancellation = Arc::new(Cancellation::default());
        let (sender, receiver) = mpsc::channel();
        let requests = cancellation.clone();
        let reader = std::thread::Builder::new()
            .name("automation-input".into())
            .spawn(move || {
                for line in input.lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let incoming = match serde_json::from_str::<IncomingLine>(&line) {
                        Ok(IncomingLine {
                            command: Some(command),
                            ..
                        }) if command == "cancel" => {
                            requests.request();
                            Incoming::Cancel
                        }
                        Ok(IncomingLine {
                            answer: Some(question),
                            option: Some(option),
                            value,
                            apply_to_all,
                            ..
                        }) => Incoming::Answer {
                            question,
                            answer: Answer {
                                option,
                                value,
                                apply_to_all,
                            },
                        },
                        _ => Incoming::Invalid,
                    };
                    if sender.send(incoming).is_err() {
                        return;
                    }
                }
                let _ = sender.send(Incoming::Closed);
            });
        if let Err(error) = reader {
            tracing::warn!(target: "myalbuns.desktop", %error, event = "automation_input_unavailable");
        }
        Self {
            output: Mutex::new(output),
            input: Mutex::new(receiver),
            cancellation,
            interactive,
            questions: AtomicU32::new(0),
        }
    }

    fn interactive(&self) -> bool {
        self.interactive
    }

    fn cancellation(&self) -> &Cancellation {
        &self.cancellation
    }

    fn emit(&self, event: &Event) {
        let Ok(mut line) = serde_json::to_vec(event) else {
            return;
        };
        line.push(b'\n');
        let mut output = self
            .output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A caller that stopped reading cannot be told so; the work decides its own end.
        let _ = output.write_all(&line).and_then(|()| output.flush());
    }

    fn refuse(&self, code: &'static str, message: impl Into<String>) {
        self.emit(&Event::Error {
            code,
            message: message.into(),
            fatal: false,
        });
    }

    fn fail(&self, code: &'static str, message: impl Into<String>) -> i32 {
        self.emit(&Event::Error {
            code,
            message: message.into(),
            fatal: true,
        });
        EXIT_FAILED
    }

    fn finish(&self, status: Outcome, items: Vec<ItemReport>, batch_id: Option<String>) -> i32 {
        self.emit(&Event::Result {
            status,
            items,
            batch_id,
        });
        status.exit_code()
    }

    /// Ends a session stopped while a decision was pending.
    fn stop(&self, stop: Stop, items: Vec<ItemReport>) -> i32 {
        match stop {
            Stop::Cancelled => self.finish(Outcome::Cancelled, items, None),
            Stop::InputClosed => self.fail(
                "input-closed",
                "A entrada foi fechada antes da resposta. Nada foi alterado.",
            ),
            Stop::Failed(message) => self.fail("failed", message),
        }
    }

    /// Drops requests that arrived for an attempt that already ended.
    fn discard_pending_input(&self) {
        let input = self.input.lock().expect("automation input is available");
        while let Ok(incoming) = input.try_recv() {
            if matches!(incoming, Incoming::Closed) {
                break;
            }
        }
    }

    fn ask(&self, mut question: Question) -> Result<Answer, Stop> {
        question.id = format!("q{}", self.questions.fetch_add(1, Ordering::Relaxed) + 1);
        let options = question.options.clone();
        let id = question.id.clone();
        self.emit(&Event::Question(question));
        let input = self.input.lock().expect("automation input is available");
        loop {
            match input.recv() {
                Ok(Incoming::Answer { question, answer }) if question == id => {
                    let Some(option) = options.iter().find(|option| option.id == answer.option)
                    else {
                        self.refuse("invalid-answer", "A resposta não é uma das opções.");
                        continue;
                    };
                    if option.input.is_some() && answer.value.as_deref().is_none_or(str::is_empty) {
                        self.refuse("invalid-answer", "A opção escolhida precisa de um valor.");
                        continue;
                    }
                    return Ok(answer);
                }
                Ok(Incoming::Answer { .. }) => {
                    self.refuse(
                        "invalid-answer",
                        "A resposta não corresponde à pergunta atual.",
                    );
                }
                Ok(Incoming::Invalid) => {
                    self.refuse(
                        "invalid-input",
                        "A linha recebida não é uma resposta válida.",
                    );
                }
                Ok(Incoming::Cancel) => return Err(Stop::Cancelled),
                Ok(Incoming::Closed) | Err(_) => return Err(Stop::InputClosed),
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Command {
    GenerateBatch,
    ExportBatch,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::GenerateBatch => "generate-batch",
            Self::ExportBatch => "export-batch",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Invocation {
    command: Command,
    /// `None` reads the job from the first input line.
    job: Option<PathBuf>,
    interactive: bool,
    dry_run: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum Arguments {
    Run(Invocation),
    Help,
    Version,
    Invalid(String),
}

fn parse_arguments(arguments: impl IntoIterator<Item = OsString>) -> Arguments {
    let mut command = None;
    let mut job = None;
    let mut interactive = true;
    let mut dry_run = false;
    let mut arguments = arguments.into_iter().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help" | "-h") => return Arguments::Help,
            Some("--version") => return Arguments::Version,
            Some("--non-interactive") => interactive = false,
            Some("--dry-run") => dry_run = true,
            Some("--job") => match arguments.next() {
                Some(path) if path == "-" => job = Some(None),
                Some(path) => job = Some(Some(PathBuf::from(path))),
                None => return Arguments::Invalid("Informe o arquivo depois de --job.".into()),
            },
            Some("generate-batch") if command.is_none() => command = Some(Command::GenerateBatch),
            Some("export-batch") if command.is_none() => command = Some(Command::ExportBatch),
            _ => {
                return Arguments::Invalid(format!(
                    "Argumento desconhecido: {}",
                    argument.to_string_lossy()
                ));
            }
        }
    }
    match (command, job) {
        (Some(command), Some(job)) => Arguments::Run(Invocation {
            command,
            job,
            interactive,
            dry_run,
        }),
        (None, _) => Arguments::Invalid("Informe o comando.".into()),
        (_, None) => Arguments::Invalid("Informe o trabalho com --job.".into()),
    }
}

#[derive(Deserialize)]
struct JobVersion {
    version: u32,
}

fn parse_job<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, (&'static str, String)> {
    let version = serde_json::from_str::<JobVersion>(text)
        .map_err(|error| ("invalid-job", format!("O trabalho não é válido: {error}")))?
        .version;
    if version != JOB_VERSION {
        return Err((
            "unsupported-job-version",
            format!("Esta versão do myAlbuns aceita trabalhos na versão {JOB_VERSION}."),
        ));
    }
    serde_json::from_str(text)
        .map_err(|error| ("invalid-job", format!("O trabalho não é válido: {error}")))
}

fn project_core(paths: &AppPaths) -> ProjectCore {
    ProjectCore::new().with_identity_storage_roots(
        paths.project_identity_leases_dir(),
        paths.project_identities_dir(),
    )
}

pub(crate) fn run(arguments: impl IntoIterator<Item = OsString>) -> i32 {
    let invocation = match parse_arguments(arguments) {
        Arguments::Run(invocation) => invocation,
        Arguments::Help => {
            println!("{USAGE}");
            return EXIT_COMPLETED;
        }
        Arguments::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return EXIT_COMPLETED;
        }
        Arguments::Invalid(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let mut input = std::io::BufReader::new(std::io::stdin());
    let job = match &invocation.job {
        Some(path) => std::fs::read_to_string(path).map_err(|error| error.to_string()),
        None => {
            let mut line = String::new();
            input
                .read_line(&mut line)
                .map(|_| line)
                .map_err(|error| error.to_string())
        }
    };
    let session = Session::start(Box::new(std::io::stdout()), input, invocation.interactive);
    session.emit(&Event::Started {
        protocol: PROTOCOL_VERSION,
        command: invocation.command.name(),
    });
    let job = match job {
        Ok(job) => job,
        Err(error) => {
            return session.fail(
                "job-unavailable",
                format!("Não foi possível ler o trabalho: {error}"),
            );
        }
    };
    let paths = match AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            return session.fail(
                "application-unavailable",
                format!("Não foi possível localizar as pastas do myAlbuns: {error}"),
            );
        }
    };
    match invocation.command {
        Command::GenerateBatch => match parse_job(&job) {
            Ok(job) => generation::run(job, &paths, &session, invocation.dry_run),
            Err((code, message)) => session.fail(code, message),
        },
        Command::ExportBatch => match parse_job(&job) {
            Ok(job) => export::run(job, &paths, &session, invocation.dry_run),
            Err((code, message)) => session.fail(code, message),
        },
    }
}
