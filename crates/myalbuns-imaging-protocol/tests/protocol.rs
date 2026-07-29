use std::path::PathBuf;

use myalbuns_core::{ProjectCore, SampleProject};
use myalbuns_imaging_protocol::{IMAGING_PROTOCOL_VERSION, ImagingRequest, ImagingResponse};

#[test]
fn host_and_processor_share_one_serialized_protocol() {
    let snapshot = ProjectCore::open_sample_project(2, SampleProject::Horizon).render_snapshot();
    let request = ImagingRequest::new(
        "render-42",
        PathBuf::from(r"C:\Temp\Album_001.png"),
        snapshot,
    );

    let request_json = serde_json::to_value(&request).expect("request serializes");
    assert_eq!(request_json["protocolVersion"], IMAGING_PROTOCOL_VERSION);
    assert_eq!(request_json["requestId"], "render-42");
    let decoded_request: ImagingRequest =
        serde_json::from_value(request_json).expect("request decodes");
    assert_eq!(decoded_request, request);

    let response = ImagingResponse::completed("render-42", 600, 300);
    let response_json = serde_json::to_value(&response).expect("response serializes");
    assert_eq!(response_json["kind"], "completed");
    assert_eq!(response_json["requestId"], "render-42");

    let decoded: ImagingResponse = serde_json::from_value(response_json).expect("response decodes");
    assert_eq!(
        decoded.completed_dimensions_for("render-42"),
        Some((600, 300))
    );
    assert_eq!(decoded.completed_dimensions_for("another"), None);
}
