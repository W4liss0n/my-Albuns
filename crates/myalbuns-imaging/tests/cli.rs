use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use myalbuns_core::{ProjectCore, ProjectIntent, RenderSnapshot, SampleProject};
use myalbuns_imaging_protocol::{ImagingRequest, ImagingResponse};

#[test]
fn processor_renders_a_png_from_a_validated_snapshot_only() {
    let session = ProjectCore::open_sample_project(12, SampleProject::Horizon);
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
fn processor_uses_the_composed_media_transform() {
    let mut session = ProjectCore::open_sample_project(12, SampleProject::Horizon);
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
    let session = ProjectCore::open_sample_project(12, SampleProject::Horizon);
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
    let session = ProjectCore::open_sample_project(12, SampleProject::Horizon);
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
    let mut snapshot =
        ProjectCore::open_sample_project(12, SampleProject::Horizon).render_snapshot();
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
