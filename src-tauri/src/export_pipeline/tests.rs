use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Barrier, Mutex},
    thread,
    time::Duration,
};

use image::{ImageFormat, Rgb, RgbImage};
use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{
    ImagingCommand, ImagingProgress, ImagingProgressStage, ImagingResponse, MediaSource,
    RenderCompletion, decode_command, encode_command,
};
use myalbuns_paths::{OperationPathContext, RootBindingPlan};
use sha2::{Digest, Sha256};

use super::{
    ExportCancellationResult, ExportExecutionControl, ExportFailureStage, ExportOptions,
    ExportPlan, ExportProgressStage, ExportProgressUnits, bind_execution_paths, execute,
    execute_group, plan,
};
use crate::{
    imaging_processor::{
        ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
        InvocationFailure, InvocationFuture,
    },
    sample_project::SampleProject,
};

struct ScriptedTransport {
    prepared_path: PathBuf,
    prepared_bytes: Vec<u8>,
    result: Option<Result<ImagingResponse, InvocationFailure>>,
    invocations: usize,
}

struct CancellationAwareTransport {
    prepared_path: PathBuf,
    invocation_started: Arc<Barrier>,
    invocations: usize,
}

struct GroupedTransport {
    prepared_outputs: VecDeque<Vec<u8>>,
    first_output: PathBuf,
    first_output_before_second_invocation: Option<Vec<u8>>,
    block_output_before_publication: Option<PathBuf>,
    invocations: usize,
}

struct PlanObservingTransport {
    expected_root_bindings: RootBindingPlan,
    prepared_path: PathBuf,
    prepared_bytes: Vec<u8>,
    observed_round_trip: bool,
}

impl ImagingTransport for CancellationAwareTransport {
    fn invoke<'a>(
        &'a mut self,
        _command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        assert_eq!(operation, ImagingOperation::Export);
        assert_eq!(attempt, 1);
        self.invocations += 1;
        std::fs::write(&self.prepared_path, b"incomplete export")
            .expect("the processor creates an incomplete preparation");
        self.invocation_started.wait();
        Box::pin(async move {
            loop {
                if control.is_cancelled() {
                    return Err(InvocationFailure::cancelled(4545));
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
    }
}

impl ImagingTransport for ScriptedTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let ImagingCommand::Render(request) = command else {
            panic!("the scripted Export receives a Render command");
        };
        assert_eq!(operation, ImagingOperation::Export);
        assert_eq!(attempt, 1);
        self.invocations += 1;
        std::fs::write(&self.prepared_path, &self.prepared_bytes)
            .expect("the processor writes its preparation");
        for (stage, completed_units) in [
            (ImagingProgressStage::LoadingSources, 1),
            (ImagingProgressStage::Composing, 1),
            (ImagingProgressStage::EncodingOutput, 1),
        ] {
            control.report(
                ImagingProgress::new(&request.request_id, stage, completed_units, 1)
                    .expect("scripted progress is valid"),
            );
        }
        let result = self.result.take().expect("one invocation");
        Box::pin(async move { result })
    }
}

impl ImagingTransport for GroupedTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        _control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let ImagingCommand::Render(request) = command else {
            panic!("the grouped Export receives a Render command");
        };
        assert_eq!(operation, ImagingOperation::Export);
        assert_eq!(attempt, 1);
        if self.invocations == 1 {
            self.first_output_before_second_invocation = Some(
                std::fs::read(&self.first_output)
                    .expect("the first previous output remains readable"),
            );
        }
        self.invocations += 1;
        let prepared = self
            .prepared_outputs
            .pop_front()
            .expect("one prepared payload exists per grouped Export unit");
        std::fs::write(&request.prepared_output_path, &prepared)
            .expect("the grouped Processor writes its preparation");
        if self.invocations == 2
            && let Some(blocked_output) = self.block_output_before_publication.take()
        {
            std::fs::create_dir(blocked_output)
                .expect("external interference blocks the second final output");
        }
        let completion = RenderCompletion {
            width_px: 10,
            height_px: 5,
            dpi: 25,
            source_count: 0,
            source_bytes: 0,
            output_bytes: prepared.len() as u64,
            output_sha256: format!("{:x}", Sha256::digest(&prepared)),
        };
        let response = ImagingResponse::completed(&request.request_id, completion);
        Box::pin(async move { Ok(response) })
    }
}

impl ImagingTransport for PlanObservingTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        attempt: u8,
        _control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        assert_eq!(operation, ImagingOperation::Export);
        assert_eq!(attempt, 1);
        assert_eq!(command.root_bindings(), &self.expected_root_bindings);
        let payload = encode_command(command).expect("the real IPC command encodes");
        let decoded = decode_command(&payload).expect("the real IPC command decodes");
        assert_eq!(decoded.root_bindings(), &self.expected_root_bindings);
        self.observed_round_trip = true;
        std::fs::write(&self.prepared_path, &self.prepared_bytes)
            .expect("the observed Processor writes its preparation");
        let ImagingCommand::Render(request) = decoded else {
            panic!("the Export sends a Render command");
        };
        let response = ImagingResponse::completed(
            &request.request_id,
            RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: self.prepared_bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&self.prepared_bytes)),
            },
        );
        Box::pin(async move { Ok(response) })
    }
}

fn export_plan(output: PathBuf, request_id: &str) -> ExportPlan {
    let source = SampleProject::Horizon
        .persisted_source(2)
        .expect("the sample project serializes");
    let core = ProjectCore::new();
    let snapshot = core
        .open_editable_session(&source)
        .expect("the sample project opens")
        .render_snapshot();
    let sheet_id = snapshot.composition.sheets[0].sheet_id.clone();
    let mut media_ids = Vec::new();
    for media_id in snapshot.composition.sheets[0].referenced_media_ids() {
        if !media_ids.iter().any(|known| known == media_id) {
            media_ids.push(media_id.to_owned());
        }
    }
    let source_directory = output
        .parent()
        .expect("the test Export output has a parent directory");
    let source_directory = source_directory
        .ancestors()
        .find(|directory| directory.exists())
        .expect("the test Export output has an existing ancestor");
    let mut sources = Vec::with_capacity(media_ids.len());
    for (index, media_id) in media_ids.into_iter().enumerate() {
        let source_path = source_directory.join(format!(".test-original-{request_id}-{index}.jpg"));
        RgbImage::from_fn(16, 12, |x, y| {
            Rgb([
                (x * 11) as u8,
                (y * 17) as u8,
                (index as u8).wrapping_mul(61).wrapping_add(24),
            ])
        })
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the test original is written");
        let source_bytes = std::fs::read(&source_path).expect("the test original is readable");
        sources.push(
            MediaSource::new(
                media_id,
                source_path,
                source_bytes.len() as u64,
                format!("{:x}", Sha256::digest(&source_bytes)),
            )
            .expect("the test original source is valid"),
        );
    }
    plan(
        snapshot,
        ExportOptions::new(request_id, output, sheet_id, 25, sources),
    )
    .expect("the Export request is valid")
}

fn root_bindings(plan: &ExportPlan) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in plan.required_paths() {
        context
            .capture(path)
            .expect("the Export path root is captured");
    }
    context.freeze()
}

fn grouped_root_bindings(plans: &[&ExportPlan]) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in plans.iter().flat_map(|plan| plan.required_paths()) {
        context
            .capture(path)
            .expect("the grouped Export path root is captured");
    }
    context.freeze()
}

#[test]
fn export_plan_rejects_missing_originals_at_the_typed_plan_stage() {
    let source = SampleProject::Horizon
        .persisted_source(2)
        .expect("the sample project serializes");
    let snapshot = ProjectCore::new()
        .open_editable_session(&source)
        .expect("the sample project opens")
        .render_snapshot();
    let sheet_id = snapshot.composition.sheets[0].sheet_id.clone();
    let destination = tempfile::tempdir().expect("temporary Export destination");

    let failure = plan(
        snapshot,
        ExportOptions::new(
            "export-without-originals",
            destination.path().join("Album.png"),
            sheet_id,
            25,
            vec![],
        ),
    )
    .expect_err("an Export without linked originals is rejected");

    assert_eq!(failure.stage, ExportFailureStage::Plan);
}

#[test]
fn host_execution_uses_the_same_frozen_mapped_drive_binding_as_the_processor() {
    let logical_plan = myalbuns_paths::ExportPathPlan::new(
        PathBuf::from(r"Z:\Exports\Album.png"),
        "export-mapped",
    )
    .expect("the logical Export path is valid");
    let mut context = OperationPathContext::new();
    context
        .capture_with_binding(
            logical_plan.output_path(),
            std::path::Path::new(r"\\servidor\destino\"),
        )
        .expect("the platform binding is captured");

    let operational = bind_execution_paths(&logical_plan, &context.freeze(), "export-mapped")
        .expect("the host derives its paths from the frozen binding");

    assert_eq!(
        operational.output_path(),
        std::path::Path::new(r"\\servidor\destino\Exports\Album.png")
    );
    assert_eq!(
        operational.prepared_output_path(),
        std::path::Path::new(
            r"\\servidor\destino\Exports\.myalbuns-export-export-mapped.tmp\Album.png"
        )
    );
}

#[test]
fn export_pipeline_sends_the_exact_plan_used_by_the_host_through_the_ipc_boundary() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        let plan = export_plan(output.clone(), "export-plan-correlation");
        let bindings = root_bindings(&plan);
        let prepared_path = plan.path_plan().prepared_output_path().to_path_buf();
        let prepared_bytes = b"correlated plan".to_vec();
        let mut transport = PlanObservingTransport {
            expected_root_bindings: bindings.clone(),
            prepared_path,
            prepared_bytes,
            observed_round_trip: false,
        };

        execute(
            &mut transport,
            plan,
            &bindings,
            &ExportExecutionControl::default(),
            &|_| {},
            &context("export-plan-correlation"),
        )
        .await
        .expect("the correlated Export completes");

        assert!(transport.observed_round_trip);
        assert!(output.exists());
    });
}

#[test]
fn planning_an_export_does_not_create_its_destination() {
    let root = tempfile::tempdir().expect("temporary Export root");
    let destination = root.path().join("not-created").join("nested");
    let output = destination.join("Album.png");

    let _plan = export_plan(output, "export-pure-plan");

    assert!(
        !destination.exists(),
        "planning must remain a pure operation"
    );
}

fn context(request_id: &str) -> InvocationContext {
    InvocationContext::new(request_id, Some("project-test"))
}

#[test]
fn grouped_export_prepares_every_output_before_publishing_the_complete_set() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary grouped Export destination");
        let first_output = destination.path().join("Album_001.png");
        let second_output = destination.path().join("Album_002.png");
        std::fs::write(&first_output, b"previous first")
            .expect("the first previous Export is writable");
        std::fs::write(&second_output, b"previous second")
            .expect("the second previous Export is writable");
        let first_plan = export_plan(first_output.clone(), "export-group-success-1");
        let second_plan = export_plan(second_output.clone(), "export-group-success-2");
        let bindings = grouped_root_bindings(&[&first_plan, &second_plan]);
        let first_preparation = first_plan.path_plan().preparation_directory().to_path_buf();
        let second_preparation = second_plan
            .path_plan()
            .preparation_directory()
            .to_path_buf();
        let mut transport = GroupedTransport {
            prepared_outputs: VecDeque::from([b"new first".to_vec(), b"new second".to_vec()]),
            first_output: first_output.clone(),
            first_output_before_second_invocation: None,
            block_output_before_publication: None,
            invocations: 0,
        };
        let control = ExportExecutionControl::default();
        let progress_events = Mutex::new(Vec::new());
        let progress = |event: super::ExportProgress| {
            progress_events
                .lock()
                .expect("the grouped progress collector remains available")
                .push(event);
        };

        let published = execute_group(
            &mut transport,
            vec![
                (first_plan, context("export-group-success-1")),
                (second_plan, context("export-group-success-2")),
            ],
            &bindings,
            &control,
            &progress,
        )
        .await
        .expect("the complete prepared set is published");

        assert_eq!(transport.invocations, 2);
        assert_eq!(
            transport.first_output_before_second_invocation,
            Some(b"previous first".to_vec()),
            "the first output cannot be promoted while the second is still being prepared"
        );
        assert_eq!(
            published
                .iter()
                .map(|result| result.output_path.as_path())
                .collect::<Vec<_>>(),
            [first_output.as_path(), second_output.as_path()]
        );
        assert_eq!(
            std::fs::read(&first_output).expect("the first grouped output is readable"),
            b"new first"
        );
        assert_eq!(
            std::fs::read(&second_output).expect("the second grouped output is readable"),
            b"new second"
        );
        assert!(!first_preparation.exists());
        assert!(!second_preparation.exists());
        let events = progress_events
            .lock()
            .expect("the grouped progress collector remains available");
        let publishing_index = events
            .iter()
            .position(|event| event.stage == ExportProgressStage::Publishing)
            .expect("the grouped Export reaches Publication");
        assert_eq!(
            events[..publishing_index]
                .iter()
                .filter(|event| event.stage == ExportProgressStage::Verifying)
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.stage == ExportProgressStage::Completed)
                .map(|event| event.units)
                .collect::<Vec<_>>(),
            [ExportProgressUnits::Measured {
                completed_units: 2,
                total_units: 2,
            }]
        );
    });
}

#[test]
fn grouped_export_reports_a_typed_partial_publication_and_discards_the_remainder() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary grouped Export destination");
        let first_output = destination.path().join("Album_001.png");
        let second_output = destination.path().join("Album_002.png");
        std::fs::write(&first_output, b"previous first")
            .expect("the first previous Export is writable");
        let first_plan = export_plan(first_output.clone(), "export-group-partial-1");
        let second_plan = export_plan(second_output.clone(), "export-group-partial-2");
        let bindings = grouped_root_bindings(&[&first_plan, &second_plan]);
        let first_preparation = first_plan.path_plan().preparation_directory().to_path_buf();
        let second_preparation = second_plan
            .path_plan()
            .preparation_directory()
            .to_path_buf();
        let mut transport = GroupedTransport {
            prepared_outputs: VecDeque::from([b"new first".to_vec(), b"new second".to_vec()]),
            first_output: first_output.clone(),
            first_output_before_second_invocation: None,
            block_output_before_publication: Some(second_output.clone()),
            invocations: 0,
        };
        let control = ExportExecutionControl::default();
        let observed_stages = Mutex::new(Vec::new());
        let progress = |event: super::ExportProgress| {
            observed_stages
                .lock()
                .expect("the grouped progress collector remains available")
                .push(event.stage);
        };

        let failure = execute_group(
            &mut transport,
            vec![
                (first_plan, context("export-group-partial-1")),
                (second_plan, context("export-group-partial-2")),
            ],
            &bindings,
            &control,
            &progress,
        )
        .await
        .expect_err("the second real filesystem promotion is rejected");

        assert_eq!(
            failure.stage,
            ExportFailureStage::Publish {
                promoted_outputs: 1,
                total_outputs: 2,
            }
        );
        assert_eq!(transport.invocations, 2);
        assert_eq!(
            transport.first_output_before_second_invocation,
            Some(b"previous first".to_vec())
        );
        assert_eq!(
            std::fs::read(&first_output).expect("the first promoted output remains readable"),
            b"new first",
            "the limited transaction does not roll back an earlier atomic promotion"
        );
        assert!(second_output.is_dir());
        assert!(!first_preparation.exists());
        assert!(!second_preparation.exists());
        let stages = observed_stages
            .lock()
            .expect("the grouped progress collector remains available");
        assert!(stages.contains(&ExportProgressStage::Publishing));
        assert!(
            !stages.contains(&ExportProgressStage::Completed),
            "a partial publication is never announced as completed"
        );
    });
}

#[test]
fn a_processor_crash_is_not_retried_and_preserves_the_previous_output() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-failure");
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: b"incomplete".to_vec(),
            result: Some(Err(InvocationFailure::unexpected_termination(4242))),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let cancellation = ExportExecutionControl::default();
        let progress = |_| {};

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context("export-failure"),
        )
        .await
        .expect_err("the Export failure remains visible");

        assert_eq!(
            failure.stage,
            ExportFailureStage::Processor(
                crate::imaging_processor::InvocationFailureStage::ImagingProcess
            )
        );
        assert_eq!(transport.invocations, 1);
        assert_eq!(
            std::fs::read(output).expect("the previous Export remains readable"),
            b"previous export"
        );
        assert!(
            !destination
                .path()
                .join(".myalbuns-export-export-failure.tmp")
                .exists()
        );
    });
}

#[test]
fn unconfirmed_process_termination_preserves_the_preparation_for_safe_recovery() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-unconfirmed-termination");
        let prepared_path = plan.path_plan().prepared_output_path().to_path_buf();
        let preparation_directory = plan.path_plan().preparation_directory().to_path_buf();
        let mut transport = ScriptedTransport {
            prepared_path: prepared_path.clone(),
            prepared_bytes: b"possibly active export".to_vec(),
            result: Some(Err(InvocationFailure::termination_unconfirmed(
                4242,
                "the processor may still own the preparation",
            ))),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let cancellation = ExportExecutionControl::default();
        let progress = |_| {};

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context("export-unconfirmed-termination"),
        )
        .await
        .expect_err("the unconfirmed termination remains visible");

        assert_eq!(
            failure.stage,
            ExportFailureStage::Processor(
                crate::imaging_processor::InvocationFailureStage::TerminationUnconfirmed
            )
        );
        assert_eq!(transport.invocations, 1);
        assert_eq!(
            std::fs::read(output).expect("the previous Export remains readable"),
            b"previous export"
        );
        assert!(preparation_directory.exists());
        assert_eq!(
            std::fs::read(prepared_path).expect("the preparation remains for safe recovery"),
            b"possibly active export"
        );
    });
}

#[test]
fn a_verified_preparation_is_published_only_after_the_response_is_validated() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-success");
        let bytes = b"verified export".to_vec();
        let completion = RenderCompletion {
            width_px: 10,
            height_px: 5,
            dpi: 25,
            source_count: 0,
            source_bytes: 0,
            output_bytes: bytes.len() as u64,
            output_sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        let response = ImagingResponse::completed("export-success", completion.clone());
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: bytes,
            result: Some(Ok(response)),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let cancellation = ExportExecutionControl::default();
        let stages = Mutex::new(Vec::new());
        let progress = |progress: super::ExportProgress| {
            stages
                .lock()
                .expect("the progress collector is available")
                .push((progress.stage, progress.units, progress.cancellable));
        };

        let published = execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context("export-success"),
        )
        .await
        .expect("the verified Export is published");

        assert_eq!(published.completion, completion);
        assert_eq!(published.output_path, output);
        assert_eq!(
            std::fs::read(&published.output_path).expect("the published Export is readable"),
            b"verified export"
        );
        assert_eq!(
            *stages.lock().expect("the progress collector is available"),
            [
                (
                    ExportProgressStage::Preparing,
                    ExportProgressUnits::Unmeasured,
                    true,
                ),
                (
                    ExportProgressStage::LoadingSources,
                    ExportProgressUnits::Measured {
                        completed_units: 1,
                        total_units: 1,
                    },
                    true,
                ),
                (
                    ExportProgressStage::Composing,
                    ExportProgressUnits::Measured {
                        completed_units: 1,
                        total_units: 1,
                    },
                    true,
                ),
                (
                    ExportProgressStage::EncodingOutput,
                    ExportProgressUnits::Measured {
                        completed_units: 1,
                        total_units: 1,
                    },
                    true,
                ),
                (
                    ExportProgressStage::Verifying,
                    ExportProgressUnits::Unmeasured,
                    true,
                ),
                (
                    ExportProgressStage::Publishing,
                    ExportProgressUnits::Unmeasured,
                    false,
                ),
                (
                    ExportProgressStage::Completed,
                    ExportProgressUnits::Measured {
                        completed_units: 1,
                        total_units: 1,
                    },
                    false,
                ),
            ]
        );
    });
}

#[test]
fn pipeline_claims_publication_before_observers_receive_the_event() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-publication-boundary");
        let bytes = b"verified replacement".to_vec();
        let response = ImagingResponse::completed(
            "export-publication-boundary",
            RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&bytes)),
            },
        );
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: bytes,
            result: Some(Ok(response)),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let control = ExportExecutionControl::default();
        let cancellation_result = Mutex::new(None);
        let progress = |event: super::ExportProgress| {
            if event.stage == ExportProgressStage::Publishing {
                assert!(!event.cancellable);
                cancellation_result
                    .lock()
                    .expect("the cancellation result remains available")
                    .replace(control.request_cancel());
            }
        };

        let published = execute(
            &mut transport,
            plan,
            &bindings,
            &control,
            &progress,
            &context("export-publication-boundary"),
        )
        .await
        .expect("publication is already non-cancellable when observers are notified");

        assert_eq!(
            *cancellation_result
                .lock()
                .expect("the cancellation result remains available"),
            Some(ExportCancellationResult::TooLate)
        );
        assert_eq!(
            std::fs::read(&published.output_path).expect("the new Export is readable"),
            b"verified replacement"
        );
    });
}

#[test]
fn cancellation_that_wins_before_the_publication_claim_preserves_the_previous_output() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-cancel-before-publication");
        let bytes = b"verified replacement".to_vec();
        let response = ImagingResponse::completed(
            "export-cancel-before-publication",
            RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: format!("{:x}", Sha256::digest(&bytes)),
            },
        );
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: bytes,
            result: Some(Ok(response)),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let control = ExportExecutionControl::default();
        let observed_stages = Mutex::new(Vec::new());
        let progress = |event: super::ExportProgress| {
            observed_stages
                .lock()
                .expect("the progress collector remains available")
                .push(event.stage);
            if event.stage == ExportProgressStage::Verifying {
                assert_eq!(
                    control.request_cancel(),
                    ExportCancellationResult::Requested
                );
            }
        };

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            &control,
            &progress,
            &context("export-cancel-before-publication"),
        )
        .await
        .expect_err("cancellation wins before publication is claimed");

        assert_eq!(failure.stage, ExportFailureStage::Cancelled);
        assert!(
            !observed_stages
                .lock()
                .expect("the progress collector remains available")
                .contains(&ExportProgressStage::Publishing)
        );
        assert_eq!(
            std::fs::read(output).expect("the previous Export remains readable"),
            b"previous export"
        );
        assert!(
            !destination
                .path()
                .join(".myalbuns-export-export-cancel-before-publication.tmp")
                .exists()
        );
    });
}

#[test]
fn an_unverified_preparation_never_replaces_the_previous_output() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-invalid");
        let bytes = b"unverified export".to_vec();
        let response = ImagingResponse::completed(
            "export-invalid",
            RenderCompletion {
                width_px: 10,
                height_px: 5,
                dpi: 25,
                source_count: 0,
                source_bytes: 0,
                output_bytes: bytes.len() as u64,
                output_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
            },
        );
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: bytes,
            result: Some(Ok(response)),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let cancellation = ExportExecutionControl::default();
        let progress = |_| {};

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context("export-invalid"),
        )
        .await
        .expect_err("the mismatched preparation is rejected");

        assert_eq!(failure.stage, ExportFailureStage::VerifyPreparation);
        assert_eq!(
            std::fs::read(output).expect("the previous Export remains readable"),
            b"previous export"
        );
    });
}

#[test]
fn cancellation_before_execution_creates_no_preparation_or_processor_work() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        let plan = export_plan(output, "export-cancelled");
        let preparation_path = plan.path_plan().preparation_directory().to_path_buf();
        let bindings = root_bindings(&plan);
        let cancellation = ExportExecutionControl::default();
        assert_eq!(
            cancellation.request_cancel(),
            ExportCancellationResult::Requested
        );
        let mut transport = ScriptedTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            prepared_bytes: Vec::new(),
            result: None,
            invocations: 0,
        };
        let progress_count = Mutex::new(0_u32);
        let progress = |_| {
            *progress_count
                .lock()
                .expect("the progress counter is available") += 1;
        };

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context("export-cancelled"),
        )
        .await
        .expect_err("the cancelled Export does not start");

        assert_eq!(failure.stage, ExportFailureStage::Cancelled);
        assert_eq!(transport.invocations, 0);
        assert_eq!(
            *progress_count
                .lock()
                .expect("the progress counter is available"),
            0
        );
        assert!(!preparation_path.exists());
        assert!(cancellation.is_cancelled());
    });
}

#[test]
fn cancellation_during_processing_discards_preparation_and_preserves_previous_output() {
    tauri::async_runtime::block_on(async {
        let destination = tempfile::tempdir().expect("temporary Export destination");
        let output = destination.path().join("Album.png");
        std::fs::write(&output, b"previous export").expect("the previous Export is writable");
        let plan = export_plan(output.clone(), "export-cancelled-during-processing");
        let preparation_directory = plan.path_plan().preparation_directory().to_path_buf();
        let invocation_started = Arc::new(Barrier::new(2));
        let mut transport = CancellationAwareTransport {
            prepared_path: plan.path_plan().prepared_output_path().to_path_buf(),
            invocation_started: Arc::clone(&invocation_started),
            invocations: 0,
        };
        let bindings = root_bindings(&plan);
        let cancellation = Arc::new(ExportExecutionControl::default());
        let cancellation_request = Arc::clone(&cancellation);
        let canceller = thread::spawn(move || {
            invocation_started.wait();
            cancellation_request.request_cancel();
        });
        let progress = |_| {};

        let failure = execute(
            &mut transport,
            plan,
            &bindings,
            cancellation.as_ref(),
            &progress,
            &context("export-cancelled-during-processing"),
        )
        .await
        .expect_err("the in-flight Export is cancelled");
        canceller
            .join()
            .expect("the cancellation request completes");

        assert_eq!(failure.stage, ExportFailureStage::Cancelled);
        assert_eq!(transport.invocations, 1);
        assert_eq!(
            std::fs::read(output).expect("the previous Export remains readable"),
            b"previous export"
        );
        assert!(!preparation_directory.exists());
    });
}
