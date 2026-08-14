use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use myalbuns_core::{
    ComposedBackground, ComposedDecorative, ComposedFrame, ComposedPhoto, CreateAuthorization,
    CreateProjectRequest, EditableProject, InitialBackground, InitialBackgroundContent,
    InitialFrameBorder, InitialOverlay, InitialOverlayContent, InitialProject,
    InitialProjectPersonalization, Matrix2, MediaKind, NormalizedPan, NumberRange, PhotoPlacement,
    PhotoPlacementPlan, ProjectCore, ProjectLocation, RenderSnapshot, SizeUm, VectorUm,
};
use myalbuns_imaging_protocol::{
    CacheArtifactFormat, CacheMediaSource, IMAGING_PROTOCOL_VERSION, ImagingCommand,
    ImagingResponse, RenderSource, encode_command,
};
use myalbuns_paths::{
    AppPaths, ExportPathPlan, ExportWriteAuthorization, OperationPathContext, RootBindingPlan,
};
use sha2::{Digest, Sha256};

use crate::{
    cache_activity_gate::{CacheCancellation, CacheCancellationReason},
    cache_engine::{
        AuthorizedCacheNamespace, CacheEngine, CacheFailureStage, CacheFlightClaim, CacheWork,
    },
    cache_previews::CachePreviewRegistry,
    export_pipeline,
    imaging_processor::{
        ImagingOperation, ImagingProcessor, ImagingTransport, InvocationContext, InvocationControl,
        InvocationFailure, InvocationFailureStage, InvocationFuture, complete_invocation,
    },
    operation_gate::OperationGate,
    operation_lease::OperationLease,
};

const PROCESSOR_ENV: &str = "MYALBUNS_REAL_IMAGING_PROCESSOR";
const EVIDENCE_DIRECTORY_ENV: &str = "MYALBUNS_RECOVERY_EVIDENCE_DIR";
const PATH_GATE_LOCAL_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_LOCAL_ROOT";
const PATH_GATE_UNC_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_UNC_ROOT";
const PATH_GATE_DRIVE_ENV: &str = "MYALBUNS_PATH_GATE_DRIVE";
const PATH_GATE_SIDECAR_EVIDENCE_ENV: &str = "MYALBUNS_PATH_GATE_SIDECAR_EVIDENCE";

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

fn export_plan(
    snapshot: myalbuns_core::RenderSnapshot,
    request_id: &str,
    output_path: PathBuf,
    authorization: ExportWriteAuthorization,
    sheet_id: String,
    sources: Vec<RenderSource>,
) -> export_pipeline::ExportPlan {
    export_pipeline::plan(
        snapshot,
        export_pipeline::ExportOptions::new(
            request_id,
            output_path,
            authorization,
            sheet_id,
            sources,
        ),
    )
    .expect("the Exportação is planned")
}

pub(crate) struct RealProcessTransport {
    executable: PathBuf,
    log_directory: PathBuf,
    crash_next: CrashNext,
    process_ids: Vec<u32>,
    partial_preparation_observed: bool,
    cache_metadata_existed_after_failure: bool,
    progressive_decode_barrier: Option<PathBuf>,
    started_process_id: Option<Arc<AtomicU32>>,
    cancelled_process_reaped: bool,
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
            progressive_decode_barrier: None,
            started_process_id: None,
            cancelled_process_reaped: false,
        }
    }

    pub(crate) fn stable(executable: PathBuf, log_directory: PathBuf) -> Self {
        Self::new(executable, log_directory, CrashNext::Never)
    }

    fn stable_with_barrier_and_probe(
        executable: PathBuf,
        log_directory: PathBuf,
        progressive_decode_barrier: PathBuf,
        started_process_id: Arc<AtomicU32>,
    ) -> Self {
        let mut transport = Self::stable(executable, log_directory);
        transport.progressive_decode_barrier = Some(progressive_decode_barrier);
        transport.started_process_id = Some(started_process_id);
        transport
    }

    fn invoke_process(
        &mut self,
        command: &ImagingCommand,
        operation: ImagingOperation,
        control: InvocationControl<'_>,
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
        if let Some(barrier) = &self.progressive_decode_barrier {
            process.env("MYALBUNS_TEST_PROGRESSIVE_DECODE_BARRIER", barrier);
        }
        let mut child = process.spawn().map_err(|error| {
            InvocationFailure::at_stage(
                InvocationFailureStage::SpawnSidecar,
                None,
                format!("Não foi possível iniciar o Processador real: {error}"),
            )
        })?;
        let process_id = child.id();
        self.process_ids.push(process_id);
        if let Some(started_process_id) = &self.started_process_id {
            started_process_id.store(process_id, Ordering::Release);
        }
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
            wait_for_file_while_running(
                &mut child,
                &partial_path,
                &self.log_directory,
                Duration::from_secs(60),
            );
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

        loop {
            if control.is_cancelled() {
                let _ = child.kill();
                let _output = child
                    .wait_with_output()
                    .expect("the cancelled real imaging process is reaped");
                self.cancelled_process_reaped = true;
                return Err(InvocationFailure::cancelled(process_id));
            }
            if child
                .try_wait()
                .expect("the real imaging process remains observable")
                .is_some()
            {
                let output = child
                    .wait_with_output()
                    .expect("the real imaging process exits");
                return complete_invocation(process_id, output.status.code(), &output.stdout);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl ImagingTransport for RealProcessTransport {
    fn invoke<'a>(
        &'a mut self,
        command: &'a ImagingCommand,
        _context: &'a InvocationContext,
        operation: ImagingOperation,
        _attempt: u8,
        control: InvocationControl<'a>,
    ) -> InvocationFuture<'a> {
        let result = self.invoke_process(command, operation, control);
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
                    &job.candidate_generation_id,
                    CacheArtifactFormat::Jpeg,
                    process_id,
                )
                .ok()
        }
        ImagingCommand::Render(request) => Some(request.prepared_output_path().to_path_buf()),
    }
}

fn wait_for_file_while_running(
    child: &mut std::process::Child,
    path: &Path,
    log_directory: &Path,
    timeout: Duration,
) {
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
                "processor exited with {status} before materializing {}: {stderr}\n{}",
                path.display(),
                read_test_logs(log_directory),
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

fn real_source(directory: &Path) -> CacheMediaSource {
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
    CacheMediaSource::new("media-serra", MediaKind::Photo, source_path)
        .expect("the real source is valid")
}

fn progressive_source(directory: &Path) -> CacheMediaSource {
    let source_path = directory.join("causal-pause-progressive.jpg");
    std::fs::write(
        &source_path,
        include_bytes!("../../crates/myalbuns-imaging/tests/fixtures/progressive-420-dri.jpg"),
    )
    .expect("the progressive JPEG fixture is written");
    CacheMediaSource::new("media-causal-pause", MediaKind::Photo, source_path)
        .expect("the progressive source is valid")
}

struct ProductiveExportFixture {
    _root: tempfile::TempDir,
    project: EditableProject,
}

impl ProductiveExportFixture {
    fn persisted_bytes(&self) -> Vec<u8> {
        std::fs::read(self.project.project_path()).expect("the productive Project remains readable")
    }

    fn project_id(&self) -> String {
        self.project.project_id().hyphenated().to_string()
    }
}

fn export_snapshot() -> (ProductiveExportFixture, RenderSnapshot, String) {
    let root = tempfile::tempdir().expect("temporary productive export Project");
    let project_path = root.path().join("RecoveryExport.myalbuns");
    let mut context = OperationPathContext::new();
    context
        .capture(&project_path)
        .expect("the productive Project root is captured");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path, context.freeze()),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project is created through ProjectCore");
    let mut snapshot = project.render_snapshot();
    let sheet = &mut snapshot.composition.sheets[0];
    let draw_rect = sheet.base.draw_rect.clone();
    sheet.frames = vec![ComposedFrame {
        frame_id: "recovery-frame".into(),
        clip_rect: draw_rect.clone(),
        z_index: 1,
        photo: Some(ComposedPhoto {
            media_id: "00000000-0000-4000-8000-000000000010"
                .parse()
                .expect("the recovery media identity is canonical"),
            name: "Recovery source.jpg".into(),
            draw_rect: draw_rect.clone(),
            placement: PhotoPlacementPlan {
                current_pan: NormalizedPan { x: 0.0, y: 0.0 },
                current_zoom: 1.0,
                pan_range: NumberRange {
                    minimum: -1.0,
                    maximum: 1.0,
                },
                zoom_range: NumberRange {
                    minimum: 1.0,
                    maximum: 4.0,
                },
                current: PhotoPlacement {
                    center: VectorUm {
                        x: draw_rect.width as f64 / 2.0,
                        y: draw_rect.height as f64 / 2.0,
                    },
                    size: SizeUm {
                        width: draw_rect.width as f64,
                        height: draw_rect.height as f64,
                    },
                },
                pan_origin: VectorUm {
                    x: draw_rect.width as f64 / 2.0,
                    y: draw_rect.height as f64 / 2.0,
                },
                pan_to_center: Matrix2 {
                    xx: 0.0,
                    xy: 0.0,
                    yx: 0.0,
                    yy: 0.0,
                },
                pan_to_center_per_zoom: Matrix2 {
                    xx: 0.0,
                    xy: 0.0,
                    yx: 0.0,
                    yy: 0.0,
                },
                size_per_zoom: SizeUm {
                    width: draw_rect.width as f64,
                    height: draw_rect.height as f64,
                },
            },
            rotation_degrees: 0.0,
            mirror_x: false,
            palette: ["#112233".into(), "#445566".into(), "#778899".into()],
        }),
    }];
    sheet.overlays.clear();
    let sheet_id = sheet.sheet_id.clone();
    (
        ProductiveExportFixture {
            _root: root,
            project,
        },
        snapshot,
        sheet_id,
    )
}

fn write_evidence(name: &str, value: serde_json::Value) {
    let Some(directory) = evidence_directory() else {
        return;
    };
    let path = directory.join(format!("{name}.json"));
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&value).expect("evidence serializes"),
    )
    .expect("evidence is writable");
}

fn write_evidence_bytes(name: &str, bytes: &[u8]) {
    let Some(directory) = evidence_directory() else {
        return;
    };
    std::fs::write(directory.join(name), bytes).expect("binary evidence is writable");
}

fn evidence_directory() -> Option<PathBuf> {
    std::env::var_os(EVIDENCE_DIRECTORY_ENV).map(PathBuf::from)
}

fn read_test_logs(directory: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return String::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn recovery_export_fixture_references_only_the_generated_source() {
    let (_, snapshot, sheet_id) = export_snapshot();
    let sheet = snapshot
        .composition
        .sheets
        .iter()
        .find(|sheet| sheet.sheet_id == sheet_id)
        .expect("the recovery sheet exists");
    let expected_media_id = sheet.frames[0]
        .photo
        .as_ref()
        .expect("the recovery frame contains a Photo")
        .media_id;

    assert_eq!(
        sheet.referenced_media_ids().collect::<Vec<_>>(),
        vec![expected_media_id]
    );
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
        let project_path = fixture.path().join("Recovery.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the recovery Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(
                fixture.path().join("leases"),
                fixture.path().join("identities"),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, project_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the recovery Project establishes identity authority");
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("identity authority mounts the recovery Cache");
        let project_id = namespace.project_id().to_owned();
        let cache_paths = namespace.paths().clone();
        let cache_request_id = "cache-real-recovery";
        let cache_work = CacheWork::new(
            cache_request_id,
            namespace,
            source.clone(),
            root_bindings(&[cache_paths.root(), source.source_path()]),
        );
        let generation_id = "g-foreign-generation";
        let foreign_process_id = u32::MAX - 7;
        let foreign_temporary = cache_paths
            .preview_temporary_file(
                source.media_id(),
                generation_id,
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

        let cancellation = CacheCancellation::default();
        let cache_owner = CacheEngine::default();
        let cache_execution = cache_owner
            .execute(
                &mut cache_transport,
                &app_paths,
                cache_work,
                &cache_context,
                &cancellation,
            )
            .await
            .expect("the Cache operation restarts the real sidecar once");
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
            .media_id;
        let export_source = RenderSource::new(export_media_id, source.source_path().to_path_buf())
            .expect("the source for Exportação matches the recovery Frame");
        let project_before = session.persisted_bytes();
        let project_sha256_before = format!("{:x}", Sha256::digest(&project_before));
        let output_path = fixture.path().join("recoverable-output.jpg");
        let previous_output = b"previous completed export";
        std::fs::write(&output_path, previous_output).expect("the previous Export is writable");
        let previous_output_sha256 = format!("{:x}", Sha256::digest(previous_output));
        let failed_request_id = "export-real-failure";
        let failed_path_plan = ExportPathPlan::new(output_path.clone(), failed_request_id)
            .expect("the failed Export path is valid");
        let failed_plan = export_plan(
            snapshot.clone(),
            failed_request_id,
            output_path.clone(),
            ExportWriteAuthorization::ReplaceConfirmed,
            sheet_id.clone(),
            vec![export_source.clone()],
        );
        let failed_bindings = root_bindings(&failed_plan.required_paths());
        let failed_context = InvocationContext::new(failed_request_id, Some(project_id.clone()));
        let mut failed_transport =
            RealProcessTransport::new(executable.clone(), log_directory.clone(), CrashNext::Export);
        let failed_cancellation = export_pipeline::ExportExecutionControl::default();
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
        let project_after = session.persisted_bytes();
        let project_sha256_after = format!("{:x}", Sha256::digest(&project_after));
        assert_eq!(project_sha256_after, project_sha256_before);

        let retry_request_id = "export-real-retry";
        let retry_plan = export_plan(
            snapshot,
            retry_request_id,
            output_path.clone(),
            ExportWriteAuthorization::ReplaceConfirmed,
            sheet_id,
            vec![export_source],
        );
        let retry_bindings = root_bindings(&retry_plan.required_paths());
        let retry_context = InvocationContext::new(retry_request_id, Some(project_id.clone()));
        let mut retry_transport =
            RealProcessTransport::new(executable, log_directory, CrashNext::Never);
        let retry_cancellation = export_pipeline::ExportExecutionControl::default();
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
        let project_after_success = session.persisted_bytes();
        let project_sha256_after_success = format!("{:x}", Sha256::digest(&project_after_success));
        assert_eq!(project_sha256_after_success, project_sha256_before);

        write_evidence(
            "export",
            serde_json::json!({
                "failedProcessId": failed_transport.process_ids[0],
                "retryProcessId": retry_transport.process_ids[0],
                "protocolVersion": IMAGING_PROTOCOL_VERSION,
                "processCountBeforeExplicitRetry": failed_transport.process_ids.len(),
                "successResponseBeforeExplicitRetry": false,
                "partialPreparationObserved":
                    failed_transport.partial_preparation_observed,
                "previousOutputSha256BeforeFailure": previous_output_sha256,
                "previousOutputSha256AfterFailure": output_sha256_after_failure,
                "projectSha256BeforeFailure": project_sha256_before,
                "projectSha256AfterFailure": project_sha256_after,
                "projectSha256AfterSuccess": project_sha256_after_success,
                "finalOutputSha256AfterExplicitRetry": final_output_sha256,
            }),
        );

        app_paths
            .clear_project_cache(&cache_paths)
            .expect("the isolated Cache is removed");
    });
}

#[test]
#[ignore = "executed by scripts/Test-ImagingRecovery.ps1 with the real sidecar"]
fn real_obsolete_cache_demand_cancels_and_reaps_the_processor() {
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os(PROCESSOR_ENV)
                .expect("the real imaging executable path is configured"),
        );
        assert!(executable.is_file(), "the real sidecar exists");
        let fixture = tempfile::tempdir().expect("temporary obsolete-demand fixture");
        let app_paths = AppPaths::discover().expect("the application paths are discoverable");
        let log_directory = fixture.path().join("logs");
        std::fs::create_dir_all(&log_directory).expect("sidecar log directory");
        let source = progressive_source(fixture.path());
        let project_path = fixture.path().join("ObsoleteDemand.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the obsolete-demand Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(
                fixture.path().join("leases"),
                fixture.path().join("identities"),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, project_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the obsolete-demand Project establishes identity authority");
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("identity authority mounts the obsolete-demand Cache");
        let project_id = namespace.project_id().to_owned();
        let cache_paths = namespace.paths().clone();
        let bindings = root_bindings(&[cache_paths.root(), source.source_path()]);
        let work = CacheWork::new("cache-real-obsolete-demand", namespace, source, bindings);
        let engine = Arc::new(CacheEngine::default());
        let processor = Arc::new(ImagingProcessor::default());
        let demand = engine.reconcile_demand(&project_id, 1, [work.source.media_id()]);
        let CacheFlightClaim::Owner(owner) = engine
            .claim_demanded(&demand, &work)
            .expect("the current real demand can claim its flight")
        else {
            panic!("the first real obsolete demand owns its flight");
        };
        let CacheFlightClaim::Waiter(waiter) = engine
            .claim_demanded(&demand, &work)
            .expect("the equivalent real demand can share its flight")
        else {
            panic!("the equivalent real demand waits on the same flight");
        };
        let cancellation = owner.cancellation();
        let decode_barrier = fixture.path().join("progressive-obsolete-worker.pid");
        let started_process_id = Arc::new(AtomicU32::new(0));

        let worker_engine = Arc::clone(&engine);
        let worker_processor = Arc::clone(&processor);
        let worker_app_paths = app_paths.clone();
        let worker_cancellation = cancellation.clone();
        let worker_executable = executable;
        let worker_log_directory = log_directory.clone();
        let worker_barrier = decode_barrier.clone();
        let worker_started_process_id = Arc::clone(&started_process_id);
        let worker_context =
            InvocationContext::new(work.request_id.clone(), Some(project_id.clone()));
        let cache_worker = thread::spawn(move || {
            tauri::async_runtime::block_on(async move {
                let permit = worker_engine
                    .begin_cancellable_work(worker_cancellation.clone())
                    .await;
                let reservation = worker_processor
                    .reserve()
                    .await
                    .expect("the obsolete Cache reserves the shared real Processor");
                let mut transport = RealProcessTransport::stable_with_barrier_and_probe(
                    worker_executable,
                    worker_log_directory,
                    worker_barrier,
                    worker_started_process_id,
                );
                let result = worker_engine
                    .execute(
                        &mut transport,
                        &worker_app_paths,
                        work,
                        &worker_context,
                        &worker_cancellation,
                    )
                    .await;
                drop(reservation);
                drop(permit);
                (result, transport)
            })
        });

        let barrier_deadline = Instant::now() + Duration::from_secs(60);
        while !decode_barrier.is_file() {
            if cache_worker.is_finished() {
                let (early_result, early_transport) = cache_worker
                    .join()
                    .expect("the unexpectedly completed obsolete Cache worker joins");
                panic!(
                    "the obsolete Cache ended before the decode barrier: result={early_result:?}, processes={:?}, logs={}",
                    early_transport.process_ids,
                    read_test_logs(&log_directory),
                );
            }
            assert!(
                Instant::now() < barrier_deadline,
                "the obsolete Cache did not reach the progressive decode barrier"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let cancelled_process_id = started_process_id.load(Ordering::Acquire);
        assert_ne!(cancelled_process_id, 0, "the real obsolete process started");

        engine.reconcile_demand(&project_id, 2, std::iter::empty());
        let (obsolete_result, obsolete_transport) = cache_worker
            .join()
            .expect("the obsolete Cache worker joins after cancellation");
        let obsolete_failure = obsolete_result
            .clone()
            .expect_err("the obsolete Cache never publishes a generation");
        assert_eq!(
            obsolete_failure.stage,
            CacheFailureStage::Processor(InvocationFailureStage::Cancelled)
        );
        assert_eq!(
            cancellation.reason(),
            Some(CacheCancellationReason::Obsolete)
        );
        assert!(!cancellation.resume_after_pause());
        assert_eq!(obsolete_transport.process_ids, [cancelled_process_id]);
        assert!(obsolete_transport.cancelled_process_reaped);
        assert!(!cache_paths.metadata_file().exists());
        let shared_result = owner.complete(obsolete_result);
        assert!(shared_result.is_err());
        let waiter_failure = waiter
            .wait()
            .await
            .expect_err("the equivalent waiter observes the obsolete cancellation");
        assert_eq!(waiter_failure.stage, obsolete_failure.stage);
        if decode_barrier.is_file() {
            std::fs::remove_file(&decode_barrier)
                .expect("the obsolete progressive worker barrier is removed");
        }

        write_evidence(
            "obsolete",
            serde_json::json!({
                "cancelledProcessId": cancelled_process_id,
                "cancelledProcessReaped": obsolete_transport.cancelled_process_reaped,
                "cancelledStage": "cancelled",
                "cancellationReason": "obsolete",
                "singleFlightDemandCount": 2,
                "singleFlightProcessorCount": obsolete_transport.process_ids.len(),
                "waiterObservedCancellation": true,
                "resumableAfterCancellation": false,
                "cacheIndexAfterCancellation": cache_paths.metadata_file().exists(),
            }),
        );
        app_paths
            .clear_project_cache(&cache_paths)
            .expect("the isolated obsolete-demand Cache is removed");
    });
}

#[test]
#[ignore = "executed by scripts/Test-ImagingRecovery.ps1 with the real sidecar"]
fn real_cache_is_causally_paused_for_export_and_resumes_after_terminal() {
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os(PROCESSOR_ENV)
                .expect("the real imaging executable path is configured"),
        );
        assert!(executable.is_file(), "the real sidecar exists");
        let fixture = tempfile::tempdir().expect("temporary causal-pause fixture");
        let app_paths = AppPaths::discover().expect("the application paths are discoverable");
        let log_directory = fixture.path().join("logs");
        std::fs::create_dir_all(&log_directory).expect("sidecar log directory");
        let source = progressive_source(fixture.path());
        let project_path = fixture.path().join("CausalPause.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the causal-pause Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(
                fixture.path().join("leases"),
                fixture.path().join("identities"),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, project_context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the causal-pause Project establishes identity authority");
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("identity authority mounts the causal-pause Cache");
        let project_id = namespace.project_id().to_owned();
        let cache_paths = namespace.paths().clone();
        let bindings = root_bindings(&[cache_paths.root(), source.source_path()]);
        let work = CacheWork::new(
            "cache-real-causal-pause",
            namespace.clone(),
            source.clone(),
            bindings.clone(),
        );
        let cancellation = CacheCancellation::default();
        let engine = Arc::new(CacheEngine::default());
        let processor = Arc::new(ImagingProcessor::default());
        let operation_gate = OperationGate::new(&app_paths);
        let decode_barrier = fixture.path().join("progressive-worker.pid");
        let started_process_id = Arc::new(AtomicU32::new(0));

        let worker_engine = Arc::clone(&engine);
        let worker_processor = Arc::clone(&processor);
        let worker_app_paths = app_paths.clone();
        let worker_cancellation = cancellation.clone();
        let worker_executable = executable.clone();
        let worker_log_directory = log_directory.clone();
        let worker_barrier = decode_barrier.clone();
        let worker_started_process_id = Arc::clone(&started_process_id);
        let worker_context =
            InvocationContext::new(work.request_id.clone(), Some(project_id.clone()));
        let cache_worker = thread::spawn(move || {
            tauri::async_runtime::block_on(async move {
                let permit = worker_engine
                    .begin_cancellable_work(worker_cancellation.clone())
                    .await;
                let reservation = worker_processor
                    .reserve()
                    .await
                    .expect("the Cache reserves the shared real Processor");
                let mut transport = RealProcessTransport::stable_with_barrier_and_probe(
                    worker_executable,
                    worker_log_directory,
                    worker_barrier,
                    worker_started_process_id,
                );
                let result = worker_engine
                    .execute(
                        &mut transport,
                        &worker_app_paths,
                        work,
                        &worker_context,
                        &worker_cancellation,
                    )
                    .await;
                drop(reservation);
                drop(permit);
                (result, transport)
            })
        });

        let barrier_deadline = Instant::now() + Duration::from_secs(60);
        while !decode_barrier.is_file() {
            if cache_worker.is_finished() {
                let (early_result, early_transport) = cache_worker
                    .join()
                    .expect("the unexpectedly completed Cache worker joins");
                panic!(
                    "the real Cache ended before the decode barrier: result={early_result:?}, processes={:?}, logs={}",
                    early_transport.process_ids,
                    read_test_logs(&log_directory),
                );
            }
            assert!(
                Instant::now() < barrier_deadline,
                "the real Cache did not reach the progressive decode barrier"
            );
            thread::sleep(Duration::from_millis(5));
        }
        let cancelled_process_id = started_process_id.load(Ordering::Acquire);
        assert_ne!(cancelled_process_id, 0, "the real Cache process started");

        let lease = OperationLease::acquire(&operation_gate, &engine, &processor)
            .await
            .expect("Export causally pauses Cache and owns the Processor exclusively");
        let (cancelled_result, cancelled_transport) = cache_worker
            .join()
            .expect("the causally cancelled Cache worker joins");
        let cancellation_failure =
            cancelled_result.expect_err("the in-flight Cache is cancelled before Export");
        assert_eq!(
            cancellation_failure.stage,
            CacheFailureStage::Processor(InvocationFailureStage::Cancelled)
        );
        assert_eq!(cancellation.reason(), Some(CacheCancellationReason::Paused));
        assert_eq!(cancelled_transport.process_ids, [cancelled_process_id]);
        assert!(cancelled_transport.cancelled_process_reaped);
        assert!(!cache_paths.metadata_file().exists());
        std::fs::remove_file(&decode_barrier)
            .expect("the progressive worker barrier is released after cancellation");

        let mut blocked_cache =
            Box::pin(engine.begin_cancellable_work(CacheCancellation::default()));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut blocked_cache)
                .await
                .is_err(),
            "Cache cannot resume while Export owns the causal pause"
        );
        let mut blocked_processor = Box::pin(processor.reserve());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut blocked_processor)
                .await
                .is_err(),
            "no Cache process can share the Processor reserved by Export"
        );

        drop(blocked_cache);
        drop(blocked_processor);
        drop(lease);
        assert!(
            cancellation.resume_after_pause(),
            "the still-relevant Cache demand resumes after the Export terminal"
        );
        let resumed_permit = engine.begin_cancellable_work(cancellation.clone()).await;
        let resumed_reservation = processor
            .reserve()
            .await
            .expect("the shared Processor is released after Export");
        let resumed_work = CacheWork::new("cache-real-after-export", namespace, source, bindings);
        let resumed_context =
            InvocationContext::new(resumed_work.request_id.clone(), Some(project_id));
        let mut resumed_transport = RealProcessTransport::stable(executable, log_directory);
        let resumed = engine
            .execute(
                &mut resumed_transport,
                &app_paths,
                resumed_work,
                &resumed_context,
                &cancellation,
            )
            .await
            .expect("Cache starts a fresh real process and publishes after Export");
        drop(resumed_reservation);
        drop(resumed_permit);
        let resumed_process_id = resumed_transport.process_ids[0];
        assert_ne!(cancelled_process_id, resumed_process_id);
        assert_eq!(resumed.completion.generated_count, 1);
        assert!(cache_paths.metadata_file().is_file());

        write_evidence(
            "pause",
            serde_json::json!({
                "cancelledProcessId": cancelled_process_id,
                "cancelledProcessReaped": cancelled_transport.cancelled_process_reaped,
                "cancelledStage": "cancelled",
                "cacheIndexAfterCancellation": false,
                "pauseReason": "paused",
                "cacheBlockedWhileExportLease": true,
                "processorExclusiveWhileExportLease": true,
                "resumedAfterExportTerminal": true,
                "resumedProcessId": resumed_process_id,
                "resumedGenerationPublished": cache_paths.metadata_file().is_file(),
            }),
        );
        app_paths
            .clear_project_cache(&cache_paths)
            .expect("the isolated causal-pause Cache is removed");
    });
}

#[test]
#[ignore = "executed by scripts/Test-ImagingRecovery.ps1 with the real sidecar"]
fn real_cache_webview_canvas_reference_matches_background_overlay_export() {
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os(PROCESSOR_ENV)
                .expect("the real imaging executable path is configured"),
        );
        assert!(executable.is_file(), "the real sidecar exists");
        let evidence_directory = evidence_directory()
            .expect("the retained Canvas journey evidence directory is configured");
        let fixture = evidence_directory.join("tauri-canvas-fixture");
        std::fs::create_dir_all(&fixture).expect("retained Canvas journey fixture");
        let log_directory = fixture.join("logs");
        std::fs::create_dir_all(&log_directory).expect("sidecar log directory");
        let background_path = fixture.join("background-original.png");
        let overlay_path = fixture.join("overlay-original.png");
        RgbaImage::from_pixel(64, 32, Rgba([32, 80, 120, 255]))
            .save_with_format(&background_path, ImageFormat::Png)
            .expect("the Background Original is written");
        RgbaImage::from_pixel(64, 32, Rgba([220, 40, 20, 96]))
            .save_with_format(&overlay_path, ImageFormat::Png)
            .expect("the Overlay Original is written");
        let background_original =
            std::fs::read(&background_path).expect("the Background Original is readable");
        let overlay_original =
            std::fs::read(&overlay_path).expect("the Overlay Original is readable");

        let project_path = fixture.join("CanvasJourney.myalbuns");
        let mut project_context = OperationPathContext::new();
        project_context
            .capture(&project_path)
            .expect("the journey Project root is captured");
        let app_paths = AppPaths::discover().expect("isolated application paths are discoverable");
        let initial_project =
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: background_path.clone(),
                    },
                },
                InitialOverlay::BothSides {
                    both: Some(InitialOverlayContent::Media {
                        path: overlay_path.clone(),
                    }),
                },
                InitialFrameBorder::None,
            ));
        let project = ProjectCore::new()
            .with_identity_storage_roots(
                app_paths.project_identity_leases_dir(),
                app_paths.project_identities_dir(),
            )
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), project_context.freeze()),
                initial_project,
                CreateAuthorization::CreateOnly,
            ))
            .expect("the journey Project establishes identity authority");
        let background_media_id = project
            .project()
            .media()
            .iter()
            .find(|media| media.path() == background_path)
            .expect("the persisted Background MediaRef exists")
            .id()
            .hyphenated()
            .to_string();
        let overlay_media_id = project
            .project()
            .media()
            .iter()
            .find(|media| media.path() == overlay_path)
            .expect("the persisted Overlay MediaRef exists")
            .id()
            .hyphenated()
            .to_string();
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("identity authority mounts the journey Cache");
        let project_id = namespace.project_id().to_owned();
        let background = CacheMediaSource::new(
            background_media_id,
            MediaKind::Decorative,
            background_path.clone(),
        )
        .expect("the Background source is valid");
        let overlay = CacheMediaSource::new(
            overlay_media_id,
            MediaKind::Decorative,
            overlay_path.clone(),
        )
        .expect("the Overlay source is valid");
        let engine = CacheEngine::default();
        let cancellation = CacheCancellation::default();
        let mut transport = RealProcessTransport::stable(executable.clone(), log_directory.clone());

        let background_work = CacheWork::new(
            "cache-canvas-background",
            namespace.clone(),
            background.clone(),
            root_bindings(&[namespace.paths().root(), background.source_path()]),
        );
        let demand =
            engine.reconcile_demand(&project_id, 1, [background.media_id(), overlay.media_id()]);
        let CacheFlightClaim::Owner(background_owner) = engine
            .claim_demanded(&demand, &background_work)
            .expect("the current Background demand can claim its flight")
        else {
            panic!("the first equivalent Background demand owns the flight");
        };
        let CacheFlightClaim::Waiter(background_waiter) = engine
            .claim_demanded(&demand, &background_work)
            .expect("the equivalent Background demand can join its flight")
        else {
            panic!("the second equivalent Background demand joins the flight");
        };
        let background_cancellation = background_owner.cancellation();
        let background_execution = engine
            .execute(
                &mut transport,
                &app_paths,
                background_work,
                &InvocationContext::new("cache-canvas-background", Some(project_id.clone())),
                &background_cancellation,
            )
            .await;
        let background_execution = background_owner
            .complete(background_execution)
            .expect("the real Processador publishes the Background representation");
        let joined_background = background_waiter
            .wait()
            .await
            .expect("the equivalent Background demand receives the same result");
        assert_eq!(
            joined_background.artifact().generation_id,
            background_execution.artifact().generation_id,
        );
        assert_eq!(
            transport.process_ids.len(),
            1,
            "two equivalent demands start only one real Processador"
        );
        let overlay_work = CacheWork::new(
            "cache-canvas-overlay",
            namespace.clone(),
            overlay.clone(),
            root_bindings(&[namespace.paths().root(), overlay.source_path()]),
        );
        let overlay_execution = engine
            .execute(
                &mut transport,
                &app_paths,
                overlay_work,
                &InvocationContext::new("cache-canvas-overlay", Some(project_id.clone())),
                &cancellation,
            )
            .await
            .expect("the real Processador publishes the Overlay representation");

        let registry = CachePreviewRegistry::new("project");
        let background_preview = registry
            .publish(&app_paths, &namespace, background_execution.artifact())
            .expect("the Background crosses the opaque WebView boundary");
        let overlay_preview = registry
            .publish(&app_paths, &namespace, overlay_execution.artifact())
            .expect("the Overlay crosses the opaque WebView boundary");
        let background_url = background_preview
            .url
            .expect("Background has an opaque URL");
        let overlay_url = overlay_preview.url.expect("Overlay has an opaque URL");
        let background_preview_bytes = opaque_preview_bytes(&registry, &background_url);
        let overlay_preview_bytes = opaque_preview_bytes(&registry, &overlay_url);
        assert_eq!(
            background_execution.artifact().format,
            CacheArtifactFormat::Jpeg
        );
        assert_eq!(
            overlay_execution.artifact().format,
            CacheArtifactFormat::Png
        );
        write_evidence_bytes("canvas-background-preview.jpg", &background_preview_bytes);
        write_evidence_bytes("canvas-overlay-preview.png", &overlay_preview_bytes);
        for (url, original_path) in [
            (&background_url, &background_path),
            (&overlay_url, &overlay_path),
        ] {
            assert!(url.starts_with("http://myalbuns-cache.localhost/"));
            assert!(!url.contains(original_path.to_string_lossy().as_ref()));
            assert!(!url.contains("original"));
        }
        assert_ne!(background_preview_bytes, background_original);
        assert_ne!(overlay_preview_bytes, overlay_original);

        let (_, mut snapshot, sheet_id) = export_snapshot();
        snapshot.dpi = 10;
        let sheet = snapshot
            .composition
            .sheets
            .iter_mut()
            .find(|sheet| sheet.sheet_id == sheet_id)
            .expect("the journey sheet exists");
        let full_sheet = sheet.base.draw_rect.clone();
        sheet.frames.clear();
        sheet.backgrounds = vec![ComposedBackground::Media {
            media_id: background
                .media_id()
                .parse()
                .expect("the Background media identity is canonical"),
            name: "Background.png".into(),
            draw_rect: full_sheet.clone(),
        }];
        sheet.overlays = vec![ComposedDecorative {
            media_id: overlay
                .media_id()
                .parse()
                .expect("the Overlay media identity is canonical"),
            name: "Overlay.png".into(),
            draw_rect: full_sheet,
        }];
        assert_eq!(
            sheet.referenced_media_ids().collect::<Vec<_>>(),
            [
                background
                    .media_id()
                    .parse()
                    .expect("the Background media identity is canonical"),
                overlay
                    .media_id()
                    .parse()
                    .expect("the Overlay media identity is canonical"),
            ]
        );
        let output_path = fixture.join("canvas-reference-final.jpg");
        let export_sources = vec![
            RenderSource::new(
                background
                    .media_id()
                    .parse()
                    .expect("the Background media identity is canonical"),
                background_path.clone(),
            )
            .expect("the exact Background Original belongs to Exportação"),
            RenderSource::new(
                overlay
                    .media_id()
                    .parse()
                    .expect("the Overlay media identity is canonical"),
                overlay_path.clone(),
            )
            .expect("the exact Overlay Original belongs to Exportação"),
        ];
        let export_plan = export_plan(
            snapshot,
            "export-canvas-reference",
            output_path.clone(),
            ExportWriteAuthorization::CreateOnly,
            sheet_id,
            export_sources,
        );
        let export_bindings = root_bindings(&export_plan.required_paths());
        let export_control = export_pipeline::ExportExecutionControl::default();
        let progress = |_| {};
        let export_execution = export_pipeline::execute(
            &mut transport,
            export_plan,
            &export_bindings,
            &export_control,
            &progress,
            &InvocationContext::new("export-canvas-reference", Some(project_id)),
        )
        .await
        .expect("the final JPEG uses the exact Background and Overlay Originals");
        assert_eq!(export_execution.completion.source_count, 2);

        let background_pixel = image::load_from_memory(&background_preview_bytes)
            .expect("the opaque Background representation decodes")
            .to_rgba8()
            .get_pixel(32, 16)
            .0;
        let overlay_pixel = image::load_from_memory(&overlay_preview_bytes)
            .expect("the opaque Overlay representation decodes")
            .to_rgba8()
            .get_pixel(32, 16)
            .0;
        let canvas_reference = alpha_over(background_pixel, overlay_pixel);
        let final_image = image::open(&output_path)
            .expect("the final JPEG decodes")
            .to_rgb8();
        let final_pixel = final_image
            .get_pixel(final_image.width() / 2, final_image.height() / 2)
            .0;
        let channel_delta = [
            canvas_reference[0].abs_diff(final_pixel[0]),
            canvas_reference[1].abs_diff(final_pixel[1]),
            canvas_reference[2].abs_diff(final_pixel[2]),
        ];
        assert!(
            channel_delta.iter().all(|delta| *delta <= 12),
            "the Canvas reference and final JPEG diverged: reference={canvas_reference:?}, final={final_pixel:?}, delta={channel_delta:?}"
        );
        assert_eq!(transport.process_ids.len(), 3);
        assert_eq!(
            transport
                .process_ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3,
            "Cache Background, Cache Overlay and final Export run in real isolated processes"
        );

        write_evidence(
            "canvas",
            serde_json::json!({
                "tauriProjectPath": project_path,
                "processorIds": transport.process_ids,
                "equivalentBackgroundDemandCount": 2,
                "singleFlightProcessorCount": 1,
                "singleFlightGenerationId": background_execution.artifact().generation_id,
                "compositionMediaOrder": [background.media_id(), overlay.media_id()],
                "backgroundUrlOpaque": true,
                "overlayUrlOpaque": true,
                "originalPathExposedToWebView": false,
                "originalBytesExposedToWebView": false,
                "canvasReferencePixel": canvas_reference,
                "finalJpegPixel": final_pixel,
                "channelDelta": channel_delta,
                "finalSourceCount": export_execution.completion.source_count,
                "finalOutputSha256": export_execution.completion.output_sha256,
            }),
        );
        drop(project);
    });
}

fn opaque_preview_bytes(registry: &CachePreviewRegistry, url: &str) -> Vec<u8> {
    let token = url.rsplit('/').next().expect("the opaque URL has a token");
    let request = tauri::http::Request::builder()
        .method(tauri::http::Method::GET)
        .uri(format!("/{token}"))
        .body(Vec::new())
        .expect("the opaque WebView request is valid");
    let response = registry.serve("project", request);
    assert_eq!(response.status(), tauri::http::StatusCode::OK);
    response.into_body()
}

fn alpha_over(background: [u8; 4], foreground: [u8; 4]) -> [u8; 3] {
    let alpha = f32::from(foreground[3]) / 255.0;
    [
        (f32::from(foreground[0]) * alpha + f32::from(background[0]) * (1.0 - alpha)) as u8,
        (f32::from(foreground[1]) * alpha + f32::from(background[1]) * (1.0 - alpha)) as u8,
        (f32::from(foreground[2]) * alpha + f32::from(background[2]) * (1.0 - alpha)) as u8,
    ]
}

#[test]
#[ignore = "executed by scripts/Test-WindowsPathGate.ps1 with the real sidecar and UNC fixture"]
fn real_processor_consumes_the_frozen_unc_plan_after_the_drive_is_unmapped() {
    tauri::async_runtime::block_on(async {
        let executable = PathBuf::from(
            std::env::var_os(PROCESSOR_ENV)
                .expect("the real imaging executable path is configured"),
        );
        assert!(executable.is_file(), "the real sidecar exists");
        let local_root = PathBuf::from(
            std::env::var_os(PATH_GATE_LOCAL_ROOT_ENV)
                .expect("the local path-gate root is configured"),
        );
        let unc_root = PathBuf::from(
            std::env::var_os(PATH_GATE_UNC_ROOT_ENV).expect("the UNC path-gate root is configured"),
        );
        let drive =
            std::env::var(PATH_GATE_DRIVE_ENV).expect("the temporary drive letter is configured");
        let local_sidecar_root = local_root.join("sidecar");
        let unc_sidecar_root = unc_root.join("sidecar");
        std::fs::create_dir_all(local_sidecar_root.join("exports"))
            .expect("the sidecar fixture is materialized");
        let mapping = TemporaryDriveMapping::new(&drive, &unc_sidecar_root);
        let logical_root = PathBuf::from(format!(r"{drive}\"));
        let logical_exports = logical_root.join("exports");
        assert!(
            logical_exports.is_dir(),
            "the mapped UNC fixture is reachable"
        );
        let source = real_source(&logical_root);
        let (session, snapshot, sheet_id) = export_snapshot();
        let media_id = snapshot.composition.sheets[0].frames[0]
            .photo
            .as_ref()
            .expect("the Export frame contains a Photo")
            .media_id;
        let source = RenderSource::new(media_id, source.source_path().to_path_buf())
            .expect("the mapped source matches the Frame used by Exportação");
        let output_path = logical_exports.join("Album-path-gate.jpg");
        let unavailable_request_id = "export-real-unc-unavailable";
        let unavailable_plan = export_plan(
            snapshot.clone(),
            unavailable_request_id,
            logical_exports.join("Album-unavailable.jpg"),
            ExportWriteAuthorization::CreateOnly,
            sheet_id.clone(),
            vec![source.clone()],
        );
        let unavailable_bindings = root_bindings(&unavailable_plan.required_paths());
        let unavailable_operational_output = unavailable_bindings
            .resolve(&logical_exports.join("Album-unavailable.jpg"))
            .expect("the unavailable output has a frozen UNC binding");
        let unavailable_log_directory = local_sidecar_root.join("unavailable-logs");

        mapping.unmap_and_wait(&logical_root);
        let offline_sidecar_root = local_root.join("sidecar-offline");
        rename_fixture_directory(&local_sidecar_root, &offline_sidecar_root);
        let mut unavailable_transport = RealProcessTransport::new(
            executable.clone(),
            unavailable_log_directory,
            CrashNext::Never,
        );
        let cancellation = export_pipeline::ExportExecutionControl::default();
        let progress = |_| {};
        let unavailable_context =
            InvocationContext::new(unavailable_request_id, Some(session.project_id()));
        let unavailable_failure = export_pipeline::execute(
            &mut unavailable_transport,
            unavailable_plan,
            &unavailable_bindings,
            &cancellation,
            &progress,
            &unavailable_context,
        )
        .await
        .expect_err("an inaccessible frozen binding fails without implicit retry");
        assert_eq!(
            unavailable_failure.stage,
            export_pipeline::ExportFailureStage::Prepare
        );
        assert!(unavailable_transport.process_ids.is_empty());
        assert!(!unavailable_operational_output.exists());
        rename_fixture_directory(&offline_sidecar_root, &local_sidecar_root);

        mapping.map_to(&unc_sidecar_root);
        let request_id = "export-real-unc-plan";
        let plan = export_plan(
            snapshot,
            request_id,
            output_path.clone(),
            ExportWriteAuthorization::CreateOnly,
            sheet_id,
            vec![source],
        );
        let bindings = root_bindings(&plan.required_paths());
        let plan_wire = serde_json::to_vec(&bindings).expect("the frozen plan serializes");
        let plan_sha256 = format!("{:x}", Sha256::digest(&plan_wire));
        let logical_preparation = ExportPathPlan::new(output_path.clone(), request_id)
            .expect("the logical Export path is valid")
            .preparation_directory()
            .to_path_buf();
        let operational_output = bindings
            .resolve(&output_path)
            .expect("the output resolves to its frozen UNC binding");
        let operational_preparation = bindings
            .resolve(&logical_preparation)
            .expect("the staging resolves to the same frozen UNC binding");
        assert!(operational_output.starts_with(&unc_sidecar_root));
        assert!(operational_preparation.starts_with(&unc_sidecar_root));

        mapping.unmap_and_wait(&logical_root);
        assert!(
            !logical_root.exists(),
            "the mapped drive is absent before host preparation and sidecar dispatch"
        );

        let log_directory = local_sidecar_root.join("logs");
        std::fs::create_dir_all(&log_directory).expect("the sidecar log directory exists");
        let mut transport = RealProcessTransport::new(executable, log_directory, CrashNext::Never);
        let context = InvocationContext::new(request_id, Some(session.project_id()));
        let published = export_pipeline::execute(
            &mut transport,
            plan,
            &bindings,
            &cancellation,
            &progress,
            &context,
        )
        .await
        .expect("host and sidecar complete the Export through the frozen UNC plan");

        assert_eq!(transport.process_ids.len(), 1);
        assert!(operational_output.is_file());
        assert!(!operational_preparation.exists());
        let output_bytes = std::fs::read(&operational_output).expect("the UNC output is readable");
        assert_eq!(
            format!("{:x}", Sha256::digest(&output_bytes)),
            published.completion.output_sha256
        );
        let evidence_path = PathBuf::from(
            std::env::var_os(PATH_GATE_SIDECAR_EVIDENCE_ENV)
                .expect("the sidecar evidence path is configured"),
        );
        std::fs::write(
            evidence_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "rootBindingPlanSha256": plan_sha256,
                "mappingRemovedBeforeDispatch": true,
                "processorPid": transport.process_ids[0],
                "processorUsedFrozenOperationalPath": true,
                "outputPublishedOnUnc": true,
                "stagingRemoved": true,
                "unavailableBindingFailedAtPrepare": true,
                "unavailableAttemptStartedProcessor": false,
                "explicitRetryPublished": true,
                "outputSha256": published.completion.output_sha256,
            }))
            .expect("the sidecar path evidence serializes"),
        )
        .expect("the sidecar path evidence is writable");
    });
}

struct TemporaryDriveMapping {
    drive: String,
}

impl TemporaryDriveMapping {
    fn new(drive: &str, remote: &Path) -> Self {
        let mapping = Self {
            drive: drive.to_owned(),
        };
        mapping.map_to(remote);
        mapping
    }

    fn map_to(&self, remote: &Path) {
        self.unmap();
        let status = Command::new("net.exe")
            .arg("use")
            .arg(&self.drive)
            .arg(remote)
            .arg("/persistent:no")
            .status()
            .expect("net.exe starts");
        assert!(
            status.success(),
            "the temporary drive could not target the UNC fixture"
        );
    }

    fn unmap(&self) {
        let _ = Command::new("net.exe")
            .args(["use", &self.drive, "/delete", "/y"])
            .output();
    }

    fn unmap_and_wait(&self, logical_root: &Path) {
        let output = Command::new("net.exe")
            .args(["use", &self.drive, "/delete", "/y"])
            .output()
            .expect("net.exe starts while disconnecting the path-gate drive");
        assert!(
            output.status.success(),
            "the temporary drive could not be disconnected: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match logical_root.try_exists() {
                Ok(false) => break,
                Ok(true) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                Ok(true) => {
                    panic!("the temporary drive remained reachable after the disconnect deadline")
                }
                Err(error) => {
                    panic!("the temporary drive disconnect could not be observed safely: {error}")
                }
            }
        }
    }
}

impl Drop for TemporaryDriveMapping {
    fn drop(&mut self) {
        self.unmap();
    }
}

fn rename_fixture_directory(source: &Path, destination: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match std::fs::rename(source, destination) {
            Ok(()) => return,
            Err(error)
                if matches!(error.raw_os_error(), Some(5 | 32)) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!(
                "the path-gate fixture could not move from {} to {}: {error}",
                source.display(),
                destination.display(),
            ),
        }
    }
}
