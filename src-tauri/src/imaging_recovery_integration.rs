use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    thread,
    time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage};
use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{
    CacheArtifactFormat, ImagingCommand, ImagingResponse, MediaSource, encode_command,
};
use myalbuns_paths::{AppPaths, ExportPathPlan, OperationPathContext, RootBindingPlan};
use sha2::{Digest, Sha256};

use crate::{
    cache_engine::{self, CacheWork},
    export_pipeline,
    imaging_processor::{
        ImagingOperation, ImagingTransport, InvocationContext, InvocationControl,
        InvocationFailure, InvocationFailureStage, InvocationFuture, complete_invocation,
    },
    sample_project::SampleProject,
};

const PROCESSOR_ENV: &str = "MYALBUNS_REAL_IMAGING_PROCESSOR";
const EVIDENCE_DIRECTORY_ENV: &str = "MYALBUNS_RECOVERY_EVIDENCE_DIR";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrashNext {
    Never,
    Cache,
    Export,
}

fn root_bindings(paths: &[&Path]) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in paths {
        context
            .capture(path)
            .expect("the recovery operation path is captured");
    }
    context.freeze()
}

struct RealProcessTransport {
    executable: PathBuf,
    log_directory: PathBuf,
    crash_next: CrashNext,
    process_ids: Vec<u32>,
    partial_preparation_observed: bool,
    cache_metadata_existed_after_failure: bool,
}

impl RealProcessTransport {
    fn new(executable: PathBuf, log_directory: PathBuf, crash_next: CrashNext) -> Self {
        Self {
            executable,
            log_directory,
            crash_next,
            process_ids: Vec::new(),
            partial_preparation_observed: false,
            cache_metadata_existed_after_failure: false,
        }
    }

    fn invoke_process(
        &mut self,
        command: &ImagingCommand,
        operation: ImagingOperation,
    ) -> Result<ImagingResponse, InvocationFailure> {
        let payload = encode_command(command).map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::EncodeRequest,
                None,
                format!("Não foi possível preparar a solicitação real: {error}"),
            )
        })?;
        let mut process = Command::new(&self.executable);
        process
            .env("MYALBUNS_LOG_DIR", &self.log_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = process.spawn().map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::SpawnSidecar,
                None,
                format!("Não foi possível iniciar o Processador real: {error}"),
            )
        })?;
        let process_id = child.id();
        self.process_ids.push(process_id);
        child
            .stdin
            .take()
            .ok_or_else(|| {
                InvocationFailure::at_stage(
                    InvocationFailureStage::WriteRequest,
                    Some(process_id),
                    "A entrada do Processador real está indisponível.",
                )
            })?
            .write_all(&payload)
            .map_err(|error| {
                InvocationFailure::at_stage(
                    InvocationFailureStage::WriteRequest,
                    Some(process_id),
                    format!("Não foi possível enviar a solicitação real: {error}"),
                )
            })?;

        let must_crash = matches!(
            (self.crash_next, operation),
            (CrashNext::Cache, ImagingOperation::Cache)
                | (CrashNext::Export, ImagingOperation::Export)
        );
        if must_crash {
            self.crash_next = CrashNext::Never;
            let partial_path = partial_path(command, process_id).expect("crashable command");
            wait_for_file_while_running(&mut child, &partial_path, Duration::from_secs(60));
            self.partial_preparation_observed = true;
            child
                .kill()
                .expect("the real imaging process can be terminated");
            let output = child
                .wait_with_output()
                .expect("the terminated real process is reaped");
            if let ImagingCommand::BuildCache(request) = command {
                self.cache_metadata_existed_after_failure =
                    request.cache_paths.metadata_file().exists();
            }
            return complete_invocation(process_id, output.status.code(), &output.stdout);
        }

        let output = child
            .wait_with_output()
            .expect("the real imaging process exits");
        complete_invocation(process_id, output.status.code(), &output.stdout)
    }
}

impl ImagingTransport for RealProcessTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        _attempt: u8,
        _control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let result = self.invoke_process(command, operation);
        Box::pin(async move { result })
    }
}

fn partial_path(command: &ImagingCommand, process_id: u32) -> Option<PathBuf> {
    match command {
        ImagingCommand::BuildCache(request) => {
            let job = request.jobs.first()?;
            request
                .cache_paths
                .preview_temporary_file(
                    job.source.media_id(),
                    &job.generation_id,
                    CacheArtifactFormat::Jpeg,
                    process_id,
                )
                .ok()
        }
        ImagingCommand::Render(request) => Some(request.prepared_output_path.clone()),
        ImagingCommand::ResetCache(_) => None,
    }
}

fn wait_for_file_while_running(child: &mut std::process::Child, path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if path.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().expect("processor state is observable") {
            let mut stderr = String::new();
            if let Some(mut stream) = child.stderr.take() {
                let _ = stream.read_to_string(&mut stderr);
            }
            panic!(
                "processor exited with {status} before materializing {}: {stderr}",
                path.display(),
            );
        }
        assert!(
            Instant::now() < deadline,
            "processor did not materialize {} within {timeout:?}",
            path.display()
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn real_source(directory: &Path) -> MediaSource {
    let source_path = directory.join("recovery-photo.jpg");
    let image = RgbImage::from_fn(3072, 2048, |x, y| {
        let mixed = x
            .wrapping_mul(73_856_093)
            .wrapping_add(y.wrapping_mul(19_349_663));
        Rgb([
            mixed as u8,
            mixed.rotate_left(11) as u8,
            mixed.rotate_left(23) as u8,
        ])
    });
    image
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let bytes = std::fs::read(&source_path).expect("the real source is readable");
    MediaSource::new(
        "media-serra",
        source_path,
        bytes.len() as u64,
        format!("{:x}", Sha256::digest(&bytes)),
    )
    .expect("the real source is valid")
}

fn export_snapshot() -> (
    myalbuns_core::ProjectSession,
    myalbuns_core::RenderSnapshot,
    String,
) {
    let source = SampleProject::Horizon
        .persisted_source(2)
        .expect("the sample project serializes");
    let session = ProjectCore::open_editable_session(&source).expect("the sample project opens");
    let mut snapshot = session.render_snapshot();
    let sheet = &mut snapshot.composition.sheets[0];
    sheet.frames.truncate(1);
    let sheet_id = sheet.sheet_id.clone();
    (session, snapshot, sheet_id)
}

fn write_evidence(name: &str, value: serde_json::Value) {
    let Some(directory) = std::env::var_os(EVIDENCE_DIRECTORY_ENV) else {
        return;
    };
    let path = PathBuf::from(directory).join(format!("{name}.json"));
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&value).expect("evidence serializes"),
    )
    .expect("evidence is writable");
}

#[test]
#[ignore = "executed by scripts/Test-ImagingRecovery.ps1 with the real sidecar"]
fn real_processor_recovery_flows_through_production_modules() {
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os(PROCESSOR_ENV)
                .expect("the real imaging executable path is configured"),
        );
        assert!(executable.is_file(), "the real sidecar exists");
        let fixture = tempfile::tempdir().expect("temporary recovery fixture");
        let log_directory = fixture.path().join("logs");
        std::fs::create_dir(&log_directory).expect("sidecar log directory");
        let source = real_source(fixture.path());

        let app_paths = AppPaths::discover().expect("the application paths are discoverable");
        let project_id = format!("recovery-gate-{}", std::process::id());
        let cache_paths = app_paths
            .project_cache(&project_id)
            .expect("the Cache plan is valid");
        let cache_request_id = "cache-real-recovery";
        let cache_work = CacheWork::new(
            cache_request_id,
            project_id.clone(),
            cache_paths.clone(),
            vec![source.clone()],
            4096,
            root_bindings(&[cache_paths.root(), source.source_path()]),
        );
        let generation_id = format!(
            "{}-v1-4096",
            source.source_sha256()[..16].to_ascii_lowercase()
        );
        let foreign_process_id = u32::MAX - 7;
        let foreign_temporary = cache_paths
            .preview_temporary_file(
                source.media_id(),
                &generation_id,
                CacheArtifactFormat::Jpeg,
                foreign_process_id,
            )
            .expect("the foreign temporary path is valid");
        let cache_storage = app_paths
            .prepare_cache_storage(&cache_paths)
            .expect("the recovery Cache storage is prepared");
        std::fs::write(&foreign_temporary, b"foreign process")
            .expect("the foreign temporary is writable");
        drop(cache_storage);
        let cache_context = InvocationContext::new(cache_request_id, Some(project_id.clone()));
        let mut cache_transport =
            RealProcessTransport::new(executable.clone(), log_directory.clone(), CrashNext::Cache);

        let cache_execution =
            cache_engine::execute(&mut cache_transport, &app_paths, cache_work, &cache_context)
                .await
                .expect("CacheEngine restarts the real sidecar once");
        let cache_recovery = cache_execution
            .recovery
            .expect("the successful Cache records its recovery");
        let cache_completion = cache_execution.completion;

        assert_eq!(cache_transport.process_ids.len(), 2);
        assert_ne!(
            cache_transport.process_ids[0],
            cache_transport.process_ids[1]
        );
        assert_eq!(
            cache_recovery.failed_process_id,
            cache_transport.process_ids[0]
        );
        assert_eq!(cache_recovery.removed_temporary_count, 1);
        assert!(cache_transport.partial_preparation_observed);
        assert!(!cache_transport.cache_metadata_existed_after_failure);
        let artifact = &cache_completion.artifacts[0];
        let failed_temporary = cache_paths
            .preview_temporary_file(
                &artifact.media_id,
                &artifact.generation_id,
                artifact.format,
                cache_transport.process_ids[0],
            )
            .expect("the failed temporary path is valid");
        assert!(!failed_temporary.exists());
        assert!(
            foreign_temporary.is_file(),
            "cleanup preserves another process temporary"
        );
        assert!(cache_paths.metadata_file().is_file());

        write_evidence(
            "cache",
            serde_json::json!({
                "failedProcessId": cache_transport.process_ids[0],
                "restartedProcessId": cache_transport.process_ids[1],
                "temporaryObservedAfterFailure": cache_transport.partial_preparation_observed,
                "removedTemporaryCount": cache_recovery.removed_temporary_count,
                "temporaryExistedAfterCleanup": failed_temporary.exists(),
                "foreignTemporarySurvivedCleanup": foreign_temporary.is_file(),
                "metadataExistedAfterFailure":
                    cache_transport.cache_metadata_existed_after_failure,
                "metadataExistedAfterRestart": cache_paths.metadata_file().is_file(),
                "generatedCountAfterRestart": cache_completion.generated_count,
            }),
        );

        let (session, snapshot, sheet_id) = export_snapshot();
        let export_media_id = snapshot.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the recovery frame contains a Photo")
            .media_id
            .clone();
        let export_source = MediaSource::new(
            export_media_id,
            source.source_path().to_path_buf(),
            source.source_bytes(),
            source.source_sha256(),
        )
        .expect("the Export source matches the recovery frame");
        let project_before = session
            .persisted_revision()
            .expect("the Project revision serializes before failure");
        let project_sha256_before = format!("{:x}", Sha256::digest(project_before.as_bytes()));
        let output_path = fixture.path().join("recoverable-output.png");
        let previous_output = b"previous completed export";
        std::fs::write(&output_path, previous_output).expect("the previous Export is writable");
        let previous_output_sha256 = format!("{:x}", Sha256::digest(previous_output));
        let failed_request_id = "export-real-failure";
        let failed_path_plan = ExportPathPlan::new(output_path.clone(), failed_request_id)
            .expect("the failed Export path is valid");
        let failed_plan = export_pipeline::plan(
            snapshot.clone(),
            export_pipeline::ExportOptions::new(
                failed_request_id,
                output_path.clone(),
                sheet_id.clone(),
                300,
                Some(vec![export_source.clone()]),
            ),
        )
        .expect("the failed Export is planned");
        let failed_bindings = root_bindings(&failed_plan.required_paths());
        let failed_context = InvocationContext::new(failed_request_id, Some(project_id.clone()));
        let mut failed_transport =
            RealProcessTransport::new(executable.clone(), log_directory.clone(), CrashNext::Export);
        let failed_cancellation = AtomicBool::new(false);
        let failed_progress = |_| {};

        export_pipeline::execute(
            &mut failed_transport,
            failed_plan,
            &failed_bindings,
            &failed_cancellation,
            &failed_progress,
            &failed_context,
        )
        .await
        .expect_err("ExportPipeline surfaces the real process failure");

        assert_eq!(failed_transport.process_ids.len(), 1);
        assert!(failed_transport.partial_preparation_observed);
        assert!(!failed_path_plan.preparation_directory().exists());
        let output_after_failure =
            std::fs::read(&output_path).expect("the previous Export remains readable");
        let output_sha256_after_failure = format!("{:x}", Sha256::digest(&output_after_failure));
        assert_eq!(output_sha256_after_failure, previous_output_sha256);
        let project_after = session
            .persisted_revision()
            .expect("the Project revision serializes after failure");
        let project_sha256_after = format!("{:x}", Sha256::digest(project_after.as_bytes()));
        assert_eq!(project_sha256_after, project_sha256_before);

        let retry_request_id = "export-real-retry";
        let retry_plan = export_pipeline::plan(
            snapshot,
            export_pipeline::ExportOptions::new(
                retry_request_id,
                output_path.clone(),
                sheet_id,
                300,
                Some(vec![export_source]),
            ),
        )
        .expect("the explicit retry is planned");
        let retry_bindings = root_bindings(&retry_plan.required_paths());
        let retry_context = InvocationContext::new(retry_request_id, Some(project_id.clone()));
        let mut retry_transport =
            RealProcessTransport::new(executable, log_directory, CrashNext::Never);
        let retry_cancellation = AtomicBool::new(false);
        let retry_progress = |_| {};
        let published = export_pipeline::execute(
            &mut retry_transport,
            retry_plan,
            &retry_bindings,
            &retry_cancellation,
            &retry_progress,
            &retry_context,
        )
        .await
        .expect("the explicit retry is published");
        assert_eq!(retry_transport.process_ids.len(), 1);
        assert_ne!(
            failed_transport.process_ids[0],
            retry_transport.process_ids[0]
        );
        let final_output_sha256 = published.completion.output_sha256;
        assert_ne!(final_output_sha256, previous_output_sha256);

        write_evidence(
            "export",
            serde_json::json!({
                "failedProcessId": failed_transport.process_ids[0],
                "retryProcessId": retry_transport.process_ids[0],
                "sourcePolicy": "linkedOriginals",
                "processCountBeforeExplicitRetry": failed_transport.process_ids.len(),
                "successResponseBeforeExplicitRetry": false,
                "partialPreparationObserved":
                    failed_transport.partial_preparation_observed,
                "previousOutputSha256BeforeFailure": previous_output_sha256,
                "previousOutputSha256AfterFailure": output_sha256_after_failure,
                "projectSha256BeforeFailure": project_sha256_before,
                "projectSha256AfterFailure": project_sha256_after,
                "finalOutputSha256AfterExplicitRetry": final_output_sha256,
            }),
        );

        app_paths
            .clear_project_cache(&cache_paths)
            .expect("the isolated Cache is removed");
    });
}
