use std::path::PathBuf;

use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{
    CacheCompletion, CacheMediaSource, CacheRequest, CacheResetRequest, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingRequest, ImagingResponse,
};
use myalbuns_paths::AppPaths;

#[path = "../../../tests/support/sample_project.rs"]
mod sample_project;

use sample_project::SampleProject;

#[test]
fn host_and_processor_share_one_serialized_protocol() {
    let source = SampleProject::Horizon
        .persisted_source(2)
        .expect("the sample project serializes");
    let snapshot = ProjectCore::open_editable_session(&source)
        .expect("the sample project opens through ProjectCore")
        .render_snapshot();
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

#[test]
fn cache_command_keeps_source_paths_inside_the_native_protocol() {
    let cache_paths = AppPaths::from_known_folders(
        PathBuf::from(r"C:\Roaming").as_path(),
        PathBuf::from(r"C:\Local").as_path(),
    )
    .project_cache("project-42")
    .expect("the project Cache namespace is safe");
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-42",
            "project-42",
            cache_paths,
            vec![
                CacheMediaSource::new(
                    "media-42",
                    PathBuf::from(r"C:\Photos\photo.jpg"),
                    1024,
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("the native media source is valid"),
            ],
            1600,
        )
        .expect("the Cache request is valid"),
    );

    let command_json = serde_json::to_value(&command).expect("command serializes");
    assert_eq!(command_json["kind"], "buildCache");
    assert_eq!(
        command_json["request"]["protocolVersion"],
        IMAGING_PROTOCOL_VERSION
    );
    assert_eq!(command_json["request"]["requestId"], "cache-42");
    assert_eq!(command_json["request"]["sources"][0]["mediaId"], "media-42");

    let decoded: ImagingCommand = serde_json::from_value(command_json).expect("command decodes");
    assert_eq!(decoded, command);

    let response = ImagingResponse::cache_completed(
        "cache-42",
        CacheCompletion {
            artifacts: vec![],
            generated_count: 0,
            reused_count: 1,
            source_bytes: 1024,
            preview_bytes: 128,
        },
    );
    assert!(response.cache_completed_for("cache-42").is_some());
    assert!(response.cache_completed_for("another").is_none());

    let reset = ImagingCommand::reset_cache(
        CacheResetRequest::new("reset-42", vec!["project-42".into(), "project-43".into()])
            .expect("the reset request is valid"),
    );
    let reset_json = serde_json::to_value(&reset).expect("reset command serializes");
    assert_eq!(reset_json["kind"], "resetCache");
    assert_eq!(reset_json["request"]["projectIds"][1], "project-43");
    let response = ImagingResponse::cache_reset("reset-42", 2);
    assert_eq!(response.cache_reset_for("reset-42"), Some(2));
    assert_eq!(response.cache_reset_for("another"), None);
}
