use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use myalbuns_core::{ProjectCore, ProjectIntent, RenderSnapshot};

#[test]
fn processor_renders_a_png_from_a_validated_snapshot_only() {
    let session = ProjectCore::open_sample_project(12);
    let snapshot = session.render_snapshot();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("lamina-001.png");
    let result = invoke_processor(
        serde_json::to_value(snapshot).expect("snapshot is serializable"),
        &output_path,
        "request-001",
    );

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_path.exists());
    let bytes = std::fs::read(output_path).expect("rendered output is readable");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let response: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("response is valid JSON");
    assert_eq!(response["requestId"], "request-001");
    assert_eq!(response["kind"], "completed");
}

#[test]
fn processor_uses_the_composed_media_transform() {
    let mut session = ProjectCore::open_sample_project(12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let original_path = output_dir.path().join("original.png");
    let transformed_path = output_dir.path().join("transformed.png");

    let original = invoke_processor(
        snapshot_value(session.render_snapshot()),
        &original_path,
        "original",
    );
    assert!(original.status.success());

    session
        .apply(ProjectIntent::PanPhoto {
            frame_id: "frame-01-a".into(),
            delta_x: 0.65,
            delta_y: -0.25,
        })
        .expect("pan is valid");
    session
        .apply(ProjectIntent::ZoomPhoto {
            frame_id: "frame-01-a".into(),
            delta: 0.4,
        })
        .expect("zoom is valid");
    let transformed = invoke_processor(
        snapshot_value(session.render_snapshot()),
        &transformed_path,
        "transformed",
    );
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
    let mut snapshot = snapshot_value(session.render_snapshot());
    snapshot["composition"]["sheets"][0]["widthUm"] = serde_json::json!(0);

    let result = invoke_processor(snapshot, &output_path, "invalid");

    assert!(!result.status.success());
    assert!(!output_path.exists());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("snapshot inválido"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn snapshot_value(snapshot: RenderSnapshot) -> serde_json::Value {
    serde_json::to_value(snapshot).expect("snapshot is serializable")
}

fn invoke_processor(
    snapshot: serde_json::Value,
    output_path: &Path,
    request_id: &str,
) -> std::process::Output {
    let request = serde_json::json!({
        "protocolVersion": 1,
        "requestId": request_id,
        "outputPath": output_path,
        "snapshot": snapshot,
    });
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
