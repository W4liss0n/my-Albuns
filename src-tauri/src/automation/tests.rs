use std::{
    ffi::OsString,
    io::{Cursor, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, EditableProject, InitialProject, ProjectCore,
    ProjectLocation,
};
use myalbuns_paths::OperationPathContext;

use super::{
    Arguments, Command, ConflictChoice, EXIT_COMPLETED, EXIT_FAILED, EXIT_INTERRUPTED,
    EXIT_PARTIAL, Invocation, Session, export, generation, parse_arguments, parse_job,
};
use crate::ipc_contract::{BatchItemStatus, ExportConflictPolicy};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn events(&self) -> Vec<serde_json::Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn last(&self, event: &str) -> serde_json::Value {
        self.events()
            .into_iter()
            .rfind(|value| value["event"] == event)
            .unwrap_or_else(|| panic!("the session emitted {event}"))
    }
}

fn session(answers: &[serde_json::Value], interactive: bool) -> (Session, Captured) {
    let output = Captured::default();
    let input = answers
        .iter()
        .map(|answer| format!("{answer}\n"))
        .collect::<String>();
    (
        Session::start(
            Box::new(output.clone()),
            Cursor::new(input.into_bytes()),
            interactive,
        ),
        output,
    )
}

struct GenerationFixture {
    root: tempfile::TempDir,
    core: ProjectCore,
    _model: EditableProject,
}

impl GenerationFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("origem/001")).unwrap();
        std::fs::create_dir(root.path().join("destino")).unwrap();
        image::RgbImage::new(8, 6)
            .save(root.path().join("origem/001/foto.png"))
            .unwrap();
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let model_path = root.path().join("modelo.myalbuns");
        let mut paths = OperationPathContext::new();
        paths.capture(&model_path).unwrap();
        let model = core
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(model_path, paths.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        Self {
            root,
            core,
            _model: model,
        }
    }

    fn generated(&self) -> std::path::PathBuf {
        self.root.path().join("destino/001.myalbuns")
    }

    fn run(
        &self,
        on_existing: &str,
        answers: &[serde_json::Value],
        interactive: bool,
        dry_run: bool,
    ) -> (i32, Captured) {
        let job = serde_json::json!({
            "version": 1,
            "model": self.root.path().join("modelo.myalbuns"),
            "sourceFolder": self.root.path().join("origem"),
            "destinationFolder": self.root.path().join("destino"),
            "onExisting": on_existing,
        });
        let (session, output) = session(answers, interactive);
        let code = generation::run_with_core(
            parse_job(&job.to_string()).unwrap(),
            self.core.clone(),
            &session,
            dry_run,
        );
        (code, output)
    }
}

#[test]
fn arguments_select_the_command_the_job_source_and_the_session_mode() {
    let parse = |arguments: &[&str]| {
        parse_arguments(
            std::iter::once("myalbuns-cli")
                .chain(arguments.iter().copied())
                .map(OsString::from),
        )
    };
    assert_eq!(
        parse(&["export-batch", "--job", "-", "--non-interactive"]),
        Arguments::Run(Invocation {
            command: Command::ExportBatch,
            job: None,
            interactive: false,
            dry_run: false,
        })
    );
    assert_eq!(
        parse(&["--dry-run", "generate-batch", "--job", "lote.json"]),
        Arguments::Run(Invocation {
            command: Command::GenerateBatch,
            job: Some("lote.json".into()),
            interactive: true,
            dry_run: true,
        })
    );
    assert!(matches!(parse(&["export-batch"]), Arguments::Invalid(_)));
    assert!(matches!(
        parse(&["--job", "lote.json"]),
        Arguments::Invalid(_)
    ));
    // A project path must never be mistaken for a command.
    assert!(matches!(
        parse(&["Album.myalbuns", "--job", "lote.json"]),
        Arguments::Invalid(_)
    ));
}

#[test]
fn a_job_from_another_version_is_refused_before_its_fields_are_read() {
    let refused = parse_job::<serde_json::Value>(r#"{"version":2,"unknown":true}"#).unwrap_err();
    assert_eq!(refused.0, "unsupported-job-version");
}

#[test]
fn a_dry_run_reports_the_projects_without_writing_them() {
    let fixture = GenerationFixture::new();
    let (code, output) = fixture.run("ask", &[], true, true);
    assert_eq!(code, EXIT_COMPLETED);
    let result = output.last("result");
    assert_eq!(result["status"], "checked");
    assert_eq!(result["items"][0]["name"], "001");
    assert!(!fixture.generated().exists());
}

#[test]
fn generation_without_questions_keeps_an_existing_project() {
    let fixture = GenerationFixture::new();
    let (code, output) = fixture.run("ask", &[], false, false);
    assert_eq!(code, EXIT_COMPLETED);
    assert_eq!(output.last("result")["status"], "completed");
    let first = std::fs::read(fixture.generated()).unwrap();

    let (code, output) = fixture.run("ask", &[], false, false);
    assert_eq!(code, EXIT_PARTIAL);
    let result = output.last("result");
    assert_eq!(result["items"][0]["status"], "skipped");
    assert_eq!(
        result["items"][0]["problems"][0]["code"],
        "destination-exists"
    );
    assert_eq!(std::fs::read(fixture.generated()).unwrap(), first);
}

#[test]
fn an_existing_project_is_replaced_only_after_the_caller_answers() {
    let fixture = GenerationFixture::new();
    fixture.run("ask", &[], false, false);

    let (code, output) = fixture.run(
        "ask",
        &[
            serde_json::json!({ "answer": "q1", "option": "unknown" }),
            serde_json::json!({ "answer": "q1", "option": "replace" }),
        ],
        true,
        false,
    );
    assert_eq!(code, EXIT_COMPLETED);
    let question = output.last("question");
    assert_eq!(question["code"], "destination-exists");
    assert_eq!(question["name"], "001");
    assert_eq!(
        question["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["replace", "skip", "cancel"]
    );
    let refused = output.last("error");
    assert_eq!(refused["code"], "invalid-answer");
    assert_eq!(refused["fatal"], false);
    assert_eq!(output.last("result")["status"], "completed");
}

#[test]
fn a_pending_question_ends_with_cancellation_or_a_closed_input() {
    let fixture = GenerationFixture::new();
    fixture.run("ask", &[], false, false);
    let first = std::fs::read(fixture.generated()).unwrap();

    let (code, output) = fixture.run(
        "ask",
        &[serde_json::json!({ "command": "cancel" })],
        true,
        false,
    );
    assert_eq!(code, EXIT_INTERRUPTED);
    assert_eq!(output.last("result")["status"], "cancelled");

    let (code, output) = fixture.run("ask", &[], true, false);
    assert_eq!(code, EXIT_FAILED);
    assert_eq!(output.last("error")["code"], "input-closed");
    assert_eq!(std::fs::read(fixture.generated()).unwrap(), first);
}

fn export_job(root: &Path) -> export::ExportJob {
    parse_job(
        &serde_json::json!({
            "version": 1,
            "sourceFolder": root.join("source"),
            "format": "png",
        })
        .to_string(),
    )
    .unwrap()
}

#[test]
fn a_missing_image_is_located_in_the_folder_the_caller_answers() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor, original) =
        crate::batch_runner::tests::background_fixture(root.path(), "A");
    let relocated = root.path().join("relocated");
    std::fs::create_dir_all(&relocated).unwrap();
    std::fs::rename(&original, relocated.join("001.jpg")).unwrap();
    let (session, output) = session(
        &[serde_json::json!({ "answer": "q1", "option": "relink", "value": relocated })],
        true,
    );
    let mut batch = export::prepare(
        &export_job(root.path()),
        core,
        root.path().join("checkpoints"),
        &session,
    )
    .unwrap();
    assert_eq!(
        output.last("checked")["items"][0]["problems"][0]["code"],
        "missing-files"
    );

    let policy = export::resolve(
        &mut batch,
        &mut export::Resolution::new(ConflictChoice::Ask),
        &session,
    )
    .unwrap();

    assert_eq!(policy, ExportConflictPolicy::Ask);
    let question = output.last("question");
    assert_eq!(question["code"], "missing-files");
    assert_eq!(question["files"], serde_json::json!(["001.jpg"]));
    assert_eq!(question["options"][0]["input"], "folder");
    assert!(batch.view().can_continue);
    assert_eq!(output.last("checked")["canContinue"], true);
}

#[test]
fn export_without_questions_ignores_the_project_with_a_missing_image() {
    let root = tempfile::tempdir().unwrap();
    let (core, _editor, original) =
        crate::batch_runner::tests::background_fixture(root.path(), "A");
    std::fs::remove_file(original).unwrap();
    let (session, output) = session(&[], false);
    let mut batch = export::prepare(
        &export_job(root.path()),
        core,
        root.path().join("checkpoints"),
        &session,
    )
    .unwrap();

    export::resolve(
        &mut batch,
        &mut export::Resolution::new(ConflictChoice::Ask),
        &session,
    )
    .unwrap();

    assert_eq!(batch.view().items[0].status, BatchItemStatus::Ignored);
    assert!(
        output
            .events()
            .iter()
            .all(|event| event["event"] != "question")
    );
    assert_eq!(
        export::reports(&batch.view())[0].problems[0].code,
        "missing-files"
    );
}

#[test]
fn export_progress_names_the_album_in_progress_and_its_own_percentage() {
    let root = tempfile::tempdir().unwrap();
    let (core, _first, _) = crate::batch_runner::tests::background_fixture(root.path(), "A");
    let (_, _second, _) = crate::batch_runner::tests::background_fixture(root.path(), "B");
    let (_, _third, _) = crate::batch_runner::tests::background_fixture(root.path(), "C");
    let (session, _) = session(&[], false);
    let mut batch = export::prepare(
        &export_job(root.path()),
        core,
        root.path().join("checkpoints"),
        &session,
    )
    .unwrap();
    let ignored = batch.view().items[0].id.clone();
    batch.ignore(&ignored).unwrap();
    let order = export::ExportOrder::of(&batch.view());
    let progress = |completed, percent| crate::ipc_contract::BatchExportProgress {
        completed,
        total: 3,
        percent,
    };

    let current = order.current(&progress(1, 50.0)).unwrap();
    assert_eq!(current.name, "B");
    assert_eq!(current.percent.round(), 50.0);
    let current = order.current(&progress(2, 200.0 / 3.0)).unwrap();
    assert_eq!(current.name, "C");
    assert_eq!(current.percent.round(), 0.0);
    assert_eq!(order.current(&progress(3, 100.0)), None);
}
