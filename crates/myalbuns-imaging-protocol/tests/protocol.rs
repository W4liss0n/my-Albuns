use std::path::PathBuf;

use myalbuns_core::ProjectCore;
use myalbuns_imaging_protocol::{
    CacheCompletion, CacheJob, CacheRequest, CacheResetRequest, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingFailureStage, ImagingRequest, ImagingResponse, MediaSource,
    RenderCompletion, decode_command, decode_response, encode_command, encode_response,
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
    assert_eq!(
        ImagingFailureStage::from_exit_code(25),
        None,
        "publication belongs to the host ExportPipeline"
    );
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
        PathBuf::from(r"C:\Temp\.myalbuns-export-render-42.tmp\Album_001.png"),
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

    let command = ImagingCommand::render(request.clone());
    let command_payload = encode_command(&command).expect("command serializes");
    assert_eq!(command_payload.last(), Some(&b'\n'));
    let request_json: serde_json::Value =
        serde_json::from_slice(&command_payload).expect("command is JSON");
    assert_eq!(request_json["kind"], "render");
    assert_eq!(
        request_json["request"]["protocolVersion"],
        IMAGING_PROTOCOL_VERSION
    );
    assert_eq!(request_json["request"]["requestId"], "render-42");
    assert_eq!(
        request_json["request"]["preparedOutputPath"],
        r"C:\Temp\.myalbuns-export-render-42.tmp\Album_001.png"
    );
    assert_eq!(request_json["request"]["sheetId"], "lamina-01");
    assert_eq!(request_json["request"]["dpi"], 300);
    assert_eq!(
        request_json["request"]["sources"][0]["mediaId"],
        "media-costa"
    );
    assert_eq!(
        decode_command(&command_payload).expect("command decodes"),
        command
    );
    assert!(
        ImagingRequest::procedural_fixture(
            r"C:\private\operation",
            PathBuf::from(r"C:\Temp\.myalbuns-export-invalid.tmp\invalid.png"),
            request.snapshot.clone(),
            "lamina-01",
            25,
        )
        .is_err(),
        "request identifiers remain safe correlation values"
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
    let response_payload = encode_response(&response).expect("response serializes");
    let response_json: serde_json::Value =
        serde_json::from_slice(&response_payload).expect("response is JSON");
    assert_eq!(response_json["kind"], "completed");
    assert_eq!(response_json["requestId"], "render-42");

    let decoded = decode_response(&response_payload).expect("response decodes");
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
                CacheJob::new(
                    MediaSource::new(
                        "media-42",
                        PathBuf::from(r"C:\Photos\photo.jpg"),
                        1024,
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("the native media source is valid"),
                    "aaaaaaaaaaaaaaaa-v1-1600",
                )
                .expect("the Cache job is valid"),
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
    assert_eq!(
        command_json["request"]["jobs"][0]["source"]["mediaId"],
        "media-42"
    );
    assert_eq!(
        command_json["request"]["jobs"][0]["generationId"],
        "aaaaaaaaaaaaaaaa-v1-1600"
    );

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
