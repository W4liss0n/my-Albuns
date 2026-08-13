mod cache;
mod command;
mod event_stream;
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
