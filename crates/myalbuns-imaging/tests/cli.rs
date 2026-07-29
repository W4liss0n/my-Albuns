use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use myalbuns_core::{ProjectCore, ProjectIntent, RenderSnapshot};
use myalbuns_imaging_protocol::{ImagingRequest, ImagingResponse};

#[test]
fn processor_renders_a_png_from_a_validated_snapshot_only() {
    let session = ProjectCore::open_sample_project(12);
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
    let mut session = ProjectCore::open_sample_project(12);
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
    let session = ProjectCore::open_sample_project(12);
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
        String::from_utf8_lossy(&result.stderr).contains("snapshot inválido"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn invoke_processor(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
) -> std::process::Output {
    let request = ImagingRequest::new(request_id, output_path.to_path_buf(), snapshot);
    let mut child = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("processor starts");
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
