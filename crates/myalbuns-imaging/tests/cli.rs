use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use image::{ImageFormat, Rgb, RgbImage};
use myalbuns_core::{ProjectCore, ProjectIntent, ProjectSession, RenderSnapshot};
use myalbuns_imaging_protocol::{
    CacheMediaSource, CacheRequest, CacheResetRequest, ImagingCommand, ImagingRequest,
    ImagingResponse,
};
use myalbuns_paths::{AppPaths, CachePathPlan};
use sha2::{Digest, Sha256};

#[path = "../../../tests/support/sample_project.rs"]
mod sample_project;

use sample_project::SampleProject;

static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(1);

struct TestCache {
    paths: CachePathPlan,
    project_id: String,
}

impl TestCache {
    fn new(label: &str) -> Self {
        let app_paths = AppPaths::discover().expect("the test can discover LocalAppData");
        let project_id = format!(
            "test-{label}-{}-{}",
            std::process::id(),
            NEXT_CACHE_ID.fetch_add(1, Ordering::Relaxed)
        );
        let paths = app_paths
            .project_cache(&project_id)
            .expect("the isolated Cache plan is valid");
        Self { paths, project_id }
    }

    fn materialize(&self) {
        let app_paths = AppPaths::discover().expect("the test can discover LocalAppData");
        drop(
            app_paths
                .prepare_cache_storage(&self.paths)
                .expect("the isolated Cache directory is materialized"),
        );
    }

    fn project_root(&self) -> PathBuf {
        self.paths
            .metadata_file()
            .parent()
            .expect("the metadata belongs to the project Cache")
            .to_path_buf()
    }
}

impl Drop for TestCache {
    fn drop(&mut self) {
        if let Ok(app_paths) = AppPaths::discover() {
            let _ = app_paths.clear_project_cache(&self.paths);
        }
    }
}

fn sample_session(sample: SampleProject, sheet_count: usize) -> ProjectSession {
    let source = sample
        .persisted_source(sheet_count)
        .expect("the sample project serializes");
    ProjectCore::open_editable_session(&source)
        .expect("the sample project opens through ProjectCore")
}

#[test]
fn processor_renders_a_png_from_a_validated_snapshot_only() {
    let session = sample_session(SampleProject::Horizon, 12);
    let snapshot = session.render_snapshot();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("lamina-001.png");
    let result = invoke_processor(snapshot, &output_path, "request-001");

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_path.exists());
    let bytes = std::fs::read(output_path).expect("rendered output is readable");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let response: ImagingResponse =
        serde_json::from_slice(&result.stdout).expect("response is valid JSON");
    assert_eq!(
        response.completed_dimensions_for("request-001"),
        Some((600, 300))
    );
}

#[test]
fn processor_builds_one_reduced_representation_per_real_photo() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("build");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    let mut source = RgbImage::new(64, 48);
    for (x, y, pixel) in source.enumerate_pixels_mut() {
        *pixel = Rgb([x as u8 * 3, y as u8 * 4, 96]);
    }
    source
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let source_bytes = std::fs::metadata(&source_path)
        .expect("source metadata is available")
        .len();
    let original_source = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original_source));
    let cache_paths = cache.paths.clone();
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-001",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![
                CacheMediaSource::new(
                    "benchmark-a-001",
                    source_path,
                    source_bytes,
                    source_sha256.clone(),
                )
                .expect("the native source is valid"),
            ],
            32,
        )
        .expect("the Cache request is valid"),
    );

    let result = invoke_imaging_command(&command, Some(log_dir.path()));

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response: ImagingResponse =
        serde_json::from_slice(&result.stdout).expect("response is valid JSON");
    let completed = response
        .cache_completed_for("cache-001")
        .expect("the Cache response is correlated");
    assert_eq!(completed.generated_count, 1);
    assert_eq!(completed.reused_count, 0);
    assert_eq!(completed.artifacts.len(), 1);
    assert!(completed.artifacts[0].width_px <= 32);
    assert!(completed.artifacts[0].height_px <= 32);
    let preview_path = cache_paths
        .preview_file(
            "benchmark-a-001",
            &format!("{}-v1-32", &source_sha256[..16]),
        )
        .expect("the preview path is derived centrally");
    let bytes = std::fs::read(preview_path).expect("the reduced representation is readable");
    assert_eq!(&bytes[..2], b"\xff\xd8");
    assert_eq!(
        std::fs::read(source_dir.path().join("photo.jpg")).expect("the original remains readable"),
        original_source
    );
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(cache_paths.metadata_file()).expect("the disposable index is readable"),
    )
    .expect("the disposable index is valid JSON");
    assert_eq!(metadata["entries"].as_array().map(Vec::len), Some(1));
    assert!(
        !metadata.to_string().contains("sourcePath"),
        "the disposable index must not duplicate original paths"
    );

    let reused_result = invoke_imaging_command(&command, Some(log_dir.path()));
    assert!(reused_result.status.success());
    let reused_response: ImagingResponse =
        serde_json::from_slice(&reused_result.stdout).expect("reuse response is valid JSON");
    let reused = reused_response
        .cache_completed_for("cache-001")
        .expect("the reuse response is correlated");
    assert_eq!(reused.generated_count, 0);
    assert_eq!(reused.reused_count, 1);
}

#[test]
fn processor_rejects_a_same_length_change_before_reusing_a_preview() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("integrity");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    RgbImage::from_pixel(32, 24, Rgb([24, 96, 180]))
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let original = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original));
    let cache_paths = cache.paths.clone();
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-integrity",
            cache.project_id.clone(),
            cache_paths,
            vec![
                CacheMediaSource::new(
                    "benchmark-a-001",
                    source_path.clone(),
                    original.len() as u64,
                    source_sha256,
                )
                .expect("the source is valid"),
            ],
            32,
        )
        .expect("the Cache request is valid"),
    );
    let generated = invoke_imaging_command(&command, Some(log_dir.path()));
    assert!(generated.status.success());

    std::fs::write(&source_path, vec![0x5a; original.len()])
        .expect("the source changes without changing its length");
    let changed = invoke_imaging_command(&command, Some(log_dir.path()));

    assert!(
        !changed.status.success(),
        "a stale generation must not be reused for changed source bytes"
    );
}

#[test]
fn processor_resets_only_native_project_cache_namespaces() {
    let first = TestCache::new("reset-a");
    let second = TestCache::new("reset-b");
    first.materialize();
    second.materialize();
    let command = ImagingCommand::reset_cache(
        CacheResetRequest::new(
            "reset-cache",
            vec![first.project_id.clone(), second.project_id.clone()],
        )
        .expect("the reset request is valid"),
    );

    let result = invoke_imaging_command(&command, None);

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response: ImagingResponse =
        serde_json::from_slice(&result.stdout).expect("response is valid JSON");
    assert_eq!(response.cache_reset_for("reset-cache"), Some(2));
    assert!(!first.project_root().exists());
    assert!(!second.project_root().exists());
}

#[test]
fn processor_uses_the_composed_media_transform() {
    let mut session = sample_session(SampleProject::Horizon, 12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let original_path = output_dir.path().join("original.png");
    let transformed_path = output_dir.path().join("transformed.png");

    let original = invoke_processor(session.render_snapshot(), &original_path, "original");
    assert!(original.status.success());

    session
        .apply(ProjectIntent::TransformPhoto {
            frame_id: "frame-01-a".into(),
            delta_pan_x: 0.65,
            delta_pan_y: -0.25,
            delta_zoom: 0.4,
        })
        .expect("the Photo transform is valid");
    let transformed = invoke_processor(session.render_snapshot(), &transformed_path, "transformed");
    assert!(transformed.status.success());

    assert_ne!(
        std::fs::read(original_path).expect("original output is readable"),
        std::fs::read(transformed_path).expect("transformed output is readable"),
        "Pan and Zoom from CompositionCore must affect final pixels"
    );
}

#[test]
fn processor_rejects_an_invalid_snapshot() {
    let session = sample_session(SampleProject::Horizon, 12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("invalid.png");
    let mut snapshot =
        serde_json::to_value(session.render_snapshot()).expect("snapshot is serializable");
    snapshot["composition"]["sheets"][0]["widthUm"] = serde_json::json!(0);
    let snapshot: RenderSnapshot =
        serde_json::from_value(snapshot).expect("modified snapshot retains its shape");

    let result = invoke_processor(snapshot, &output_path, "invalid");

    assert!(!result.status.success());
    assert!(!output_path.exists());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("não concluiu a solicitação"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn processor_writes_correlated_logs_without_exposing_the_output_path() {
    let session = sample_session(SampleProject::Horizon, 12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir.path().join("private-album-name.png");

    let result = invoke_processor_with_log_dir(
        session.render_snapshot(),
        &output_path,
        "logged-request-001",
        Some(log_dir.path()),
    );

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let logs = std::fs::read_dir(log_dir.path())
        .expect("log directory is readable")
        .map(|entry| {
            let path = entry.expect("log entry is valid").path();
            std::fs::read_to_string(path).expect("log file is readable")
        })
        .collect::<String>();
    assert!(logs.contains("imaging_request_started"));
    assert!(logs.contains("imaging_request_completed"));
    assert!(logs.contains("\"process_role\":\"imaging\""));
    assert!(logs.contains("\"protocol_version\":1"));
    assert!(logs.contains("\"operation_id\":\"logged-request-001\""));
    assert!(
        !logs.contains(&output_path.to_string_lossy().into_owned()),
        "the output path must not be written to logs"
    );
}

#[test]
fn processor_redacts_path_shaped_identifiers_and_output_failures() {
    let mut snapshot = sample_session(SampleProject::Horizon, 12).render_snapshot();
    snapshot.project_id = r"c:\users\person\private-project".into();
    let request_id = r"c:\users\person\private-operation";
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir
        .path()
        .join("missing-parent")
        .join("private-album-name.png");

    let result =
        invoke_processor_with_log_dir(snapshot, &output_path, request_id, Some(log_dir.path()));

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("não concluiu a solicitação"));
    assert!(!stderr.contains(request_id));
    assert!(!stderr.contains(&output_path.to_string_lossy().into_owned()));

    let logs = std::fs::read_dir(log_dir.path())
        .expect("log directory is readable")
        .map(|entry| {
            let path = entry.expect("log entry is valid").path();
            std::fs::read_to_string(path).expect("log file is readable")
        })
        .collect::<String>();
    assert!(logs.contains("imaging_request_started"));
    assert!(logs.contains("imaging_render_failed"));
    assert!(!logs.contains(request_id));
    assert!(!logs.contains(r"c:\users\person\private-project"));
    assert!(!logs.contains(&output_path.to_string_lossy().into_owned()));
}

fn invoke_processor(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
) -> std::process::Output {
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    invoke_processor_with_log_dir(snapshot, output_path, request_id, Some(log_dir.path()))
}

fn invoke_processor_with_log_dir(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
    log_dir: Option<&Path>,
) -> std::process::Output {
    let request = ImagingRequest::new(request_id, output_path.to_path_buf(), snapshot);
    let mut command = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(log_dir) = log_dir {
        command.env("MYALBUNS_LOG_DIR", log_dir);
    }
    let mut child = command.spawn().expect("processor starts");
    let mut payload = serde_json::to_vec(&request).expect("request is serializable");
    payload.push(b'\n');
    child
        .stdin
        .take()
        .expect("stdin is available")
        .write_all(&payload)
        .expect("request is sent");

    child.wait_with_output().expect("processor exits")
}

fn invoke_imaging_command(
    command: &ImagingCommand,
    log_dir: Option<&Path>,
) -> std::process::Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(log_dir) = log_dir {
        process.env("MYALBUNS_LOG_DIR", log_dir);
    }
    let mut child = process.spawn().expect("processor starts");
    let mut payload = serde_json::to_vec(command).expect("command is serializable");
    payload.push(b'\n');
    child
        .stdin
        .take()
        .expect("stdin is available")
        .write_all(&payload)
        .expect("command is sent");
    child.wait_with_output().expect("processor exits")
}
