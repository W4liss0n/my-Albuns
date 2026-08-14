use std::path::PathBuf;

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialBackground, InitialBackgroundContent,
    InitialFrameBorder, InitialOverlay, InitialProject, InitialProjectPersonalization, MediaKind,
    ProjectCore, ProjectLocation,
};
use myalbuns_imaging_protocol::{
    CacheCompletion, CacheJob, CacheMediaSource, CacheRepresentationPolicy, CacheRequest,
    IMAGING_PROTOCOL_VERSION, ImagingCommand, ImagingEvent, ImagingEventStreamDecoder,
    ImagingFailureCode, ImagingFailureStage, ImagingPathCode, ImagingProgress,
    ImagingProgressStage, ImagingRequest, ImagingResponse, MediaSource, RenderCompletion,
    RenderSource, decode_command, decode_event_stream, encode_command, encode_event,
    root_binding_plan_sha256,
};
use myalbuns_paths::{
    AppPaths, CacheArtifactFormat, NativePathDto, OperationPathContext, ResolveError,
    RootBindingPlan,
};

fn empty_cache_response(request_id: &str) -> ImagingResponse {
    ImagingResponse::cache_completed(
        request_id,
        CacheCompletion {
            artifacts: vec![],
            generated_count: 0,
            reused_count: 0,
            source_bytes: 0,
            preview_bytes: 0,
        },
    )
}

#[test]
fn imaging_failure_stages_have_stable_process_exit_codes() {
    let stages = [
        ImagingFailureStage::InvalidRenderRequest,
        ImagingFailureStage::CacheProcessing,
        ImagingFailureStage::SourceVerification,
        ImagingFailureStage::SourceDecode,
        ImagingFailureStage::Composition,
        ImagingFailureStage::OutputPrepare,
        ImagingFailureStage::OutputEncode,
        ImagingFailureStage::OutputVerify,
        ImagingFailureStage::ResourceLimitExceeded,
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
fn imaging_path_codes_centralize_resolver_and_io_taxonomy() {
    assert_eq!(
        ImagingPathCode::from_resolve_error(ResolveError::NotFound),
        ImagingPathCode::NotFound
    );
    assert_eq!(
        ImagingPathCode::from_resolve_error(ResolveError::UnsupportedNamespace),
        ImagingPathCode::InvalidPath
    );
    assert_eq!(
        ImagingPathCode::from_io_error(&std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        ImagingPathCode::AccessDenied
    );
    assert_eq!(
        ImagingPathCode::from_io_error(&std::io::Error::from(std::io::ErrorKind::TimedOut)),
        ImagingPathCode::Unavailable
    );
}

#[test]
fn deterministic_failure_is_one_correlated_terminal_even_mid_progress() {
    let progress =
        ImagingProgress::new("render-failed", ImagingProgressStage::LoadingSources, 0, 1)
            .expect("the partial progress is valid");
    let failure = ImagingResponse::failed(
        "render-failed",
        ImagingFailureCode::ResourceLimitExceeded,
        Some("media-42"),
        Some(ImagingPathCode::Unavailable),
    );
    let mut stream =
        encode_event(&ImagingEvent::Progress(progress.clone())).expect("progress serializes");
    stream.extend(
        encode_event(&ImagingEvent::Response(failure.clone())).expect("failure serializes"),
    );

    let (decoded_progress, decoded_terminal) =
        decode_event_stream(&stream).expect("a known failure may terminate partial progress");
    assert_eq!(decoded_progress, vec![progress]);
    assert_eq!(
        decoded_terminal.failure_for("render-failed"),
        Some(myalbuns_imaging_protocol::ImagingFailure {
            code: ImagingFailureCode::ResourceLimitExceeded,
            media_id: Some("media-42".into()),
            path_code: Some(ImagingPathCode::Unavailable),
        })
    );
    assert_eq!(decoded_terminal.failure_for("another"), None);
    assert_eq!(decoded_terminal, failure);

    stream.extend(
        encode_event(&ImagingEvent::Response(ImagingResponse::failed(
            "render-failed",
            ImagingFailureCode::CompositionFailed,
            None::<String>,
            None,
        )))
        .expect("the duplicate terminal fixture serializes"),
    );
    assert!(
        decode_event_stream(&stream).is_err(),
        "the stream rejects a second terminal"
    );
}

#[test]
fn terminal_absence_malformed_payload_and_other_attempt_are_protocol_failures() {
    let absent = ImagingEventStreamDecoder::for_request("render-current")
        .expect("the expected correlation is valid");
    assert!(absent.finish().is_err(), "a terminal is mandatory");

    let mut malformed = ImagingEventStreamDecoder::for_request("render-current")
        .expect("the expected correlation is valid");
    assert!(
        malformed.push(b"{not-json}\n").is_err(),
        "a malformed terminal is rejected"
    );

    let other_attempt = ImagingResponse::failed(
        "render-previous",
        ImagingFailureCode::CompositionFailed,
        None::<String>,
        None,
    );
    let payload = encode_event(&ImagingEvent::Response(other_attempt))
        .expect("the other attempt terminal serializes");
    let mut correlated = ImagingEventStreamDecoder::for_request("render-current")
        .expect("the expected correlation is valid");
    assert!(
        correlated.push(&payload).is_err(),
        "a terminal from another attempt is rejected"
    );
}

#[test]
fn deterministic_failure_serializes_stable_code_and_optional_context() {
    let response = ImagingResponse::failed(
        "render-profile",
        ImagingFailureCode::UnsupportedColorProfile,
        Some("overlay-01"),
        Some(ImagingPathCode::AccessDenied),
    );

    assert_eq!(
        serde_json::to_value(response).expect("the terminal serializes"),
        serde_json::json!({
            "kind": "failed",
            "requestId": "render-profile",
            "code": "unsupportedColorProfile",
            "mediaId": "overlay-01",
            "pathCode": "accessDenied"
        })
    );
}

#[test]
fn host_and_processor_share_one_serialized_protocol() {
    let fixture = tempfile::tempdir().expect("temporary productive protocol fixture");
    let project_path = fixture.path().join("Protocol.myalbuns");
    let original_path = fixture.path().join("background.jpg");
    std::fs::write(&original_path, b"original owned by the Processor")
        .expect("the linked original fixture is written");
    let mut project_context = OperationPathContext::new();
    project_context
        .capture(&project_path)
        .expect("the Project root is captured");
    let project = ProjectCore::new()
        .with_identity_storage_roots(
            fixture.path().join("leases"),
            fixture.path().join("identities"),
        )
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path, project_context.freeze()),
            InitialProject::neutral().with_personalization(InitialProjectPersonalization::new(
                InitialBackground::BothSides {
                    both: InitialBackgroundContent::Media {
                        path: original_path,
                    },
                },
                InitialOverlay::BothSides { both: None },
                InitialFrameBorder::None,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project is created through ProjectCore");
    let frozen = project.freeze_rendering();
    let sheet_id = frozen.render_snapshot().composition.sheets[0]
        .sheet_id
        .clone();
    let (snapshot, unit, frozen_sources) = frozen
        .into_sheet(&sheet_id)
        .expect("the selected sheet is frozen with exact originals")
        .into_parts();
    let sources = frozen_sources
        .into_iter()
        .map(|source| {
            RenderSource::new(
                source.id().hyphenated().to_string(),
                source.path().to_path_buf(),
            )
            .expect("the exact native source is valid")
        })
        .collect::<Vec<_>>();
    let source_media_id = sources[0].media_id().to_owned();
    let prepared_output_path = fixture
        .path()
        .join(".myalbuns-export-render-42.tmp")
        .join("Album_001.jpg");
    let mut path_context = OperationPathContext::new();
    path_context
        .capture(&prepared_output_path)
        .expect("the output root is captured");
    for source in &sources {
        path_context
            .capture(source.source_path())
            .expect("the source root is captured");
    }
    let root_bindings = path_context.freeze();
    let request = ImagingRequest::new(
        "render-42",
        snapshot.project_id.clone(),
        snapshot.revision,
        NativePathDto::from(prepared_output_path),
        unit,
        snapshot.dpi,
        sources,
        root_bindings.clone(),
    )
    .expect("the render request is valid");

    let command = ImagingCommand::render(request.clone());
    let owner_plan_digest =
        root_binding_plan_sha256(&root_bindings).expect("the frozen plan has a stable digest");
    assert_eq!(owner_plan_digest.len(), 64);
    assert_eq!(command.root_bindings(), &root_bindings);
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
        request_json["request"]["projectId"], snapshot.project_id,
        "the frozen Identidade do Projeto remains opaque render context"
    );
    assert_eq!(
        request_json["request"]["revision"], snapshot.revision,
        "the visible Revisão crosses the process boundary with its composition"
    );
    assert_eq!(
        request_json["request"]["preparedOutputPath"]["encoding"],
        "windowsUtf16"
    );
    assert!(
        request_json["request"]["preparedOutputPath"]["units"]
            .as_array()
            .is_some_and(|units| !units.is_empty()),
        "the output path uses only the reversible native Windows wire form"
    );
    assert!(
        request_json["request"].get("sheetId").is_none(),
        "selection belongs to the host; the Processor receives one output unit"
    );
    assert_eq!(
        request_json["request"]["unit"]["sheet"]["sheetId"],
        sheet_id
    );
    assert_eq!(request_json["request"]["dpi"], 300);
    assert!(
        request_json["request"].get("sourcePolicy").is_none(),
        "the production protocol has one source contract: linked originals"
    );
    assert_eq!(
        request_json["request"]["sources"][0]["mediaId"],
        source_media_id
    );
    assert!(
        request_json["request"]["sources"][0]
            .get("sourceBytes")
            .is_none(),
        "The Processador opens and validates the original instead of receiving a Host measurement"
    );
    assert!(
        request_json["request"]["sources"][0]
            .get("sourceSha256")
            .is_none(),
        "The Processador must not require the Host to read and hash the original first"
    );
    assert_eq!(
        request_json["request"]["rootBindings"]["bindings"][0]["kind"],
        "disk"
    );
    assert_eq!(
        request_json["request"]["rootBindings"]["bindings"][0]["logicalRoot"]["encoding"],
        "windowsUtf16"
    );
    assert!(
        request_json["request"]["rootBindings"]["bindings"][0]["logicalRoot"]["units"]
            .as_array()
            .is_some_and(|units| !units.is_empty()),
        "root bindings use the reversible native Windows wire form"
    );
    let decoded_command = decode_command(&command_payload).expect("command decodes");
    assert_eq!(decoded_command, command);
    assert_eq!(
        root_binding_plan_sha256(decoded_command.root_bindings())
            .expect("the received plan has a stable digest"),
        owner_plan_digest,
        "the Processor observes exactly the plan frozen by the operation owner"
    );
    let mut legacy_json = request_json.clone();
    legacy_json["request"]["protocolVersion"] = serde_json::json!(9);
    let legacy_command: ImagingCommand =
        serde_json::from_value(legacy_json).expect("the old wire remains syntactically JSON");
    let ImagingCommand::Render(legacy_request) = legacy_command else {
        panic!("the fixture remains a Render command");
    };
    assert!(
        legacy_request.validate().is_err(),
        "protocol 9 is rejected after the render contract became original-only"
    );
    let mut unbound_request = request.clone();
    unbound_request.root_bindings = RootBindingPlan::default();
    assert!(
        unbound_request.validate().is_err(),
        "a worker must never resolve a root omitted by the operation owner"
    );
    let mut non_jpeg_request = request.clone();
    non_jpeg_request.prepared_output_path = NativePathDto::from(PathBuf::from(
        r"C:\Temp\.myalbuns-export-render-42.tmp\Album_001.png",
    ));
    assert!(
        non_jpeg_request.validate().is_err(),
        "the Processor rejects a JPEG payload prepared under a misleading extension"
    );
    let mut uppercase_jpeg_request = request.clone();
    uppercase_jpeg_request.prepared_output_path = NativePathDto::from(PathBuf::from(
        r"C:\Temp\.myalbuns-export-render-42.tmp\Album_001.JPG",
    ));
    assert!(
        uppercase_jpeg_request.validate().is_ok(),
        "Windows extension casing must not make the host and Processor disagree"
    );
    assert!(
        ImagingRequest::new(
            r"C:\private\operation",
            request.project_id.clone(),
            request.revision,
            NativePathDto::from(PathBuf::from(
                r"C:\Temp\.myalbuns-export-invalid.tmp\invalid.jpg",
            )),
            request.unit.clone(),
            25,
            request.sources.clone(),
            root_bindings,
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
            source_count: 1,
            source_bytes: 3072,
            output_bytes: 4096,
            output_sha256: "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .into(),
        },
    );
    let progress = [
        ImagingProgress::new("render-42", ImagingProgressStage::LoadingSources, 1, 1)
            .expect("source progress is valid"),
        ImagingProgress::new("render-42", ImagingProgressStage::Composing, 2, 2)
            .expect("composition progress is valid"),
        ImagingProgress::new("render-42", ImagingProgressStage::EncodingOutput, 1, 1)
            .expect("encoding progress is valid"),
    ];
    let mut event_stream = Vec::new();
    for event in &progress {
        event_stream.extend(
            encode_event(&ImagingEvent::Progress(event.clone())).expect("progress serializes"),
        );
    }
    event_stream.extend(
        encode_event(&ImagingEvent::Response(response.clone())).expect("response event serializes"),
    );
    let (decoded_progress, decoded_response) =
        decode_event_stream(&event_stream).expect("the event stream decodes");
    assert_eq!(decoded_progress, progress);
    assert_eq!(
        decoded_response
            .completed_for("render-42")
            .map(|completion| (completion.width_px, completion.height_px)),
        Some((7087, 3543))
    );
    assert_eq!(decoded_response.completed_for("another"), None);
    assert_eq!(decoded_response, response);
}

#[test]
fn processor_event_stream_rejects_invalid_or_out_of_order_progress() {
    let invalid_progress = ImagingProgress {
        request_id: "render-42".into(),
        stage: ImagingProgressStage::Composing,
        completed_units: 2,
        total_units: 1,
    };
    let invalid_stream = encode_event(&ImagingEvent::Progress(invalid_progress))
        .expect("the malformed fixture serializes");
    assert!(
        decode_event_stream(&invalid_stream).is_err(),
        "deserialization validates progress even when its constructor was bypassed"
    );

    let response = empty_cache_response("render-42");
    let progress = ImagingProgress::new("render-42", ImagingProgressStage::EncodingOutput, 1, 1)
        .expect("the progress fixture is valid");
    let mut out_of_order =
        encode_event(&ImagingEvent::Response(response)).expect("the final response serializes");
    out_of_order.extend(
        encode_event(&ImagingEvent::Progress(progress)).expect("the progress event serializes"),
    );
    assert!(
        decode_event_stream(&out_of_order).is_err(),
        "progress cannot be emitted after the final response"
    );
}

#[test]
fn processor_event_stream_rejects_regressive_progress() {
    let response = empty_cache_response("render-42");
    let cases = [
        vec![
            (ImagingProgressStage::LoadingSources, 1, 1),
            (ImagingProgressStage::EncodingOutput, 1, 1),
        ],
        vec![
            (ImagingProgressStage::LoadingSources, 1, 1),
            (ImagingProgressStage::Composing, 1, 2),
            (ImagingProgressStage::Composing, 0, 2),
        ],
        vec![
            (ImagingProgressStage::LoadingSources, 1, 1),
            (ImagingProgressStage::Composing, 1, 1),
            (ImagingProgressStage::LoadingSources, 1, 1),
        ],
    ];

    for events in cases {
        let mut stream = Vec::new();
        for (stage, completed, total) in events {
            let progress = ImagingProgress::new("render-42", stage, completed, total)
                .expect("each individual progress event is valid");
            stream.extend(
                encode_event(&ImagingEvent::Progress(progress))
                    .expect("the progress event serializes"),
            );
        }
        stream.extend(
            encode_event(&ImagingEvent::Response(response.clone()))
                .expect("the final response serializes"),
        );
        assert!(
            decode_event_stream(&stream).is_err(),
            "the stream-level progress invariant rejects regressions"
        );
    }
}

#[test]
fn media_identity_stays_opaque_across_the_protocol_and_cache_paths() {
    let paths = AppPaths::from_roots(
        PathBuf::from(r"C:\Roaming").as_path(),
        PathBuf::from(r"C:\Local").as_path(),
        PathBuf::from(r"C:\Temp").as_path(),
    );
    let source = MediaSource::new(
        "Foto/\u{00c1}rvore CON",
        PathBuf::from(r"C:\Photos\photo.jpg"),
        1024,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .expect("the domain Media identity is not constrained by path syntax");
    let artifact = paths
        .project_cache("project-42")
        .expect("the project Cache namespace is safe")
        .preview_file(
            source.media_id(),
            "aaaaaaaaaaaaaaaa-v1-1600",
            CacheArtifactFormat::Jpeg,
        )
        .expect("the path layer derives an opaque Media artifact key");
    let artifact_name = artifact
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("the artifact name is textual");

    assert!(artifact_name.starts_with("media-"));
    assert!(!artifact_name.contains(source.media_id()));
}

#[test]
fn cache_command_keeps_project_identity_opaque_and_source_paths_native() {
    let cache_paths = AppPaths::from_roots(
        PathBuf::from(r"C:\Roaming").as_path(),
        PathBuf::from(r"C:\Local").as_path(),
        PathBuf::from(r"C:\Temp").as_path(),
    )
    .project_cache("project-42")
    .expect("the project Cache namespace is safe");
    let source = CacheMediaSource::new(
        "media-42",
        MediaKind::Photo,
        PathBuf::from(r"C:\Photos\photo.jpg"),
    )
    .expect("the native media source is valid");
    let mut path_context = OperationPathContext::new();
    path_context
        .capture(cache_paths.root())
        .expect("the Cache root is captured");
    path_context
        .capture(source.source_path())
        .expect("the source root is captured");
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-42",
            "Projeto/\u{00c1}rvore CON",
            cache_paths,
            vec![
                CacheJob::new(source, "aaaaaaaaaaaaaaaa-v1-1600", None)
                    .expect("the Cache job is valid"),
            ],
            CacheRepresentationPolicy::measured_v1(),
            path_context.freeze(),
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
        command_json["request"]["projectId"],
        "Projeto/\u{00c1}rvore CON"
    );
    assert_eq!(
        command_json["request"]["jobs"][0]["source"]["mediaId"],
        "media-42"
    );
    assert_eq!(
        command_json["request"]["jobs"][0]["candidateGenerationId"],
        "aaaaaaaaaaaaaaaa-v1-1600"
    );

    let decoded: ImagingCommand = serde_json::from_value(command_json).expect("command decodes");
    assert_eq!(decoded, command);
    let ImagingCommand::BuildCache(mut invalid_project) = decoded.clone() else {
        panic!("the decoded command remains a Cache request");
    };
    invalid_project.project_id = " \t".into();
    assert!(
        invalid_project.validate().is_err(),
        "an empty Project identity is rejected without constraining opaque identities"
    );

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
}
