mod cache;
mod command;
mod event_stream;
mod processor_handshake;
mod render;
mod response;

pub use cache::{
    CACHE_FINGERPRINT_VERSION, CACHE_JPEG_QUALITY, CACHE_MAX_DECODED_PIXELS,
    CACHE_MAX_DECODER_ALLOC_BYTES, CACHE_MAX_EDGE_PX, CACHE_REPRESENTATION_VERSION, CacheArtifact,
    CacheArtifactProperties, CacheBasicColorProfile, CacheCompletion, CacheFingerprint, CacheJob,
    CacheMediaSource, CacheRepresentationPolicy, CacheRequest, CacheReusableGeneration,
};
pub use command::{
    ImagingCommand, ImagingFailure, ImagingFailureCode, ImagingFailureStage, ImagingPathCode,
    decode_command, encode_command, root_binding_plan_sha256,
};
pub use event_stream::{
    ImagingEvent, ImagingEventStreamDecoder, ImagingProgress, ImagingProgressStage, decode_event,
    decode_event_stream, encode_event,
};
pub use myalbuns_paths::CacheArtifactFormat;
pub use processor_handshake::{
    PROCESSOR_HANDSHAKE_CHALLENGE_ENV, PROCESSOR_HANDSHAKE_MAX_BYTES, decode_processor_handshake,
    encode_processor_handshake,
};
pub use render::{
    ImagingRequest, MediaSource, RenderCompletion, RenderSource, has_jpeg_extension,
    validate_render_content,
};
pub use response::ImagingResponse;

pub const IMAGING_PROTOCOL_VERSION: u32 = 17;

pub(crate) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
}

#[cfg(test)]
mod processor_handshake_contract_tests {
    use myalbuns_paths::ProcessInstanceId;

    use super::{decode_processor_handshake, encode_processor_handshake};

    fn process() -> ProcessInstanceId {
        ProcessInstanceId::from_wire(42, 17).expect("the fixture identity is valid")
    }

    #[test]
    fn processor_handshake_binds_challenge_and_exact_process_instance() {
        let encoded = encode_processor_handshake("host_challenge-1", process())
            .expect("the handshake is encodable");

        assert_eq!(
            decode_processor_handshake(&encoded, "host_challenge-1", 42)
                .expect("the exact handshake is accepted"),
            process()
        );
        assert!(decode_processor_handshake(&encoded, "another_challenge", 42).is_err());
        assert!(decode_processor_handshake(&encoded, "host_challenge-1", 41).is_err());
    }

    #[test]
    fn processor_handshake_rejects_schema_shape_and_stream_ambiguity() {
        let valid = encode_processor_handshake("host_challenge-1", process())
            .expect("the handshake is encodable");
        let wrong_schema = valid
            .split(|byte| *byte == b'\n')
            .next()
            .expect("the line exists")
            .to_vec();
        let wrong_schema = String::from_utf8(wrong_schema)
            .expect("the JSON is UTF-8")
            .replace("\"schemaVersion\":1", "\"schemaVersion\":2")
            + "\n";
        assert!(
            decode_processor_handshake(wrong_schema.as_bytes(), "host_challenge-1", 42).is_err()
        );

        let unknown = String::from_utf8(valid.clone())
            .expect("the JSON is UTF-8")
            .replace("}\n", ",\"unknown\":true}\n");
        assert!(decode_processor_handshake(unknown.as_bytes(), "host_challenge-1", 42).is_err());

        let mut trailing = valid;
        trailing.extend_from_slice(b"{}\n");
        assert!(decode_processor_handshake(&trailing, "host_challenge-1", 42).is_err());
        assert!(decode_processor_handshake(b"{}", "host_challenge-1", 42).is_err());
    }
}
