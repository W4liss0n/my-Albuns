use std::path::PathBuf;

use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{
    CacheCompletion, CacheRequest, CacheResetRequest, IMAGING_PROTOCOL_VERSION, ImagingCommand,
    ImagingFailureStage, ImagingRequest, ImagingResponse, MediaSource, RenderCompletion,
};
use myalbuns_paths::AppPaths;

#[path = "../../../tests/support/sample_project.rs"]
mod sample_project;

use sample_project::SampleProject;

#[test]
fn imaging_failure_stages_have_stable_process_exit_codes() {
    let stages = [
        ImagingFailureStage::CacheProcessing,
        ImagingFailureStage::SourceVerification,
        ImagingFailureStage::SourceDecode,
        ImagingFailureStage::Composition,
        ImagingFailureStage::OutputPrepare,
        ImagingFailureStage::OutputEncode,
        ImagingFailureStage::OutputPublish,
        ImagingFailureStage::OutputVerify,
    ];

    for stage in stages {
        assert_eq!(
            ImagingFailureStage::from_exit_code(stage.exit_code().into()),
            Some(stage)
        );
        assert!(!stage.as_str().is_empty());
    }
    assert_eq!(ImagingFailureStage::from_exit_code(1), None);
}

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
        "lamina-01",
        300,
        vec![
            MediaSource::new(
                "media-costa",
                PathBuf::from(r"C:\Photos\costa.jpg"),
                1024,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("the first native source is valid"),
            MediaSource::new(
                "media-campo",
                PathBuf::from(r"C:\Photos\campo.jpg"),
                2048,
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
            .expect("the second native source is valid"),
        ],
    )
    .expect("the render request is valid");

    let request_json = serde_json::to_value(&request).expect("request serializes");
    assert_eq!(request_json["protocolVersion"], IMAGING_PROTOCOL_VERSION);
    assert_eq!(request_json["requestId"], "render-42");
    assert_eq!(request_json["sheetId"], "lamina-01");
    assert_eq!(request_json["dpi"], 300);
    assert_eq!(request_json["sources"][0]["mediaId"], "media-costa");
    let decoded_request: ImagingRequest =
        serde_json::from_value(request_json).expect("request decodes");
    assert_eq!(decoded_request, request);
    assert_eq!(
        request
            .temporary_output_path()
            .expect("the temporary path is derived from validated protocol fields"),
        PathBuf::from(r"C:\Temp\.Album_001.png.render-42.tmp")
    );
    assert!(
        ImagingRequest::procedural_fixture(
            r"C:\private\operation",
            PathBuf::from(r"C:\Temp\invalid.png"),
            request.snapshot.clone(),
            "lamina-01",
            25,
        )
        .is_err(),
        "request identifiers cannot become temporary-file path fragments"
    );

    let response = ImagingResponse::completed(
        "render-42",
        RenderCompletion {
            width_px: 7087,
            height_px: 3543,
            dpi: 300,
            source_count: 2,
            source_bytes: 3072,
            output_bytes: 4096,
            output_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .into(),
        },
    );
    let response_json = serde_json::to_value(&response).expect("response serializes");
    assert_eq!(response_json["kind"], "completed");
    assert_eq!(response_json["requestId"], "render-42");

    let decoded: ImagingResponse = serde_json::from_value(response_json).expect("response decodes");
    assert_eq!(
        decoded
            .completed_for("render-42")
            .map(|completion| (completion.width_px, completion.height_px)),
        Some((7087, 3543))
    );
    assert_eq!(decoded.completed_for("another"), None);
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
                MediaSource::new(
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
