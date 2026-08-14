use std::path::PathBuf;

use myalbuns_core::MediaKind;
use myalbuns_imaging_protocol::{
    CacheArtifactFormat, CacheArtifactProperties, CacheBasicColorProfile, CacheFingerprint,
    CacheJob, CacheMediaSource, CacheRepresentationPolicy, CacheRequest, CacheReusableGeneration,
    IMAGING_PROTOCOL_VERSION, ImagingCommand,
};
use myalbuns_paths::{AppPaths, OperationPathContext};

fn cache_paths() -> myalbuns_paths::CachePathPlan {
    AppPaths::from_roots(
        PathBuf::from(r"C:\Roaming").as_path(),
        PathBuf::from(r"C:\Local").as_path(),
    )
    .project_cache("project-a")
    .expect("the project Cache path is valid")
}

#[test]
fn fresh_cache_request_sends_only_media_identity_kind_and_native_path_to_the_processor() {
    let paths = cache_paths();
    let photo = CacheMediaSource::new(
        "photo-a",
        MediaKind::Photo,
        PathBuf::from(r"C:\Photos\original.jpg"),
    )
    .expect("the Photo source is valid");
    let decorative = CacheMediaSource::new(
        "decorative-a",
        MediaKind::Decorative,
        PathBuf::from(r"C:\Decoratives\overlay.tif"),
    )
    .expect("the Decorative source is valid");
    let mut context = OperationPathContext::new();
    context
        .capture(paths.root())
        .expect("the Cache root is captured");
    context
        .capture(photo.source_path())
        .expect("the Photo root is captured");
    context
        .capture(decorative.source_path())
        .expect("the Decorative root is captured");
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-a",
            "opaque-project-a",
            paths,
            vec![
                CacheJob::new(photo, "candidate-photo-a", None).expect("the Photo job is valid"),
                CacheJob::new(decorative, "candidate-decorative-a", None)
                    .expect("the Decorative job is valid"),
            ],
            CacheRepresentationPolicy::measured_v1(),
            context.freeze(),
        )
        .expect("the Cache request is valid"),
    );

    let json = serde_json::to_value(&command).expect("the Cache command serializes");
    assert_eq!(json["request"]["protocolVersion"], IMAGING_PROTOCOL_VERSION);
    assert_eq!(json["request"]["policy"]["maxEdgePx"], 1_600);
    assert_eq!(json["request"]["jobs"][0]["source"]["kind"], "photo");
    assert_eq!(json["request"]["jobs"][1]["source"]["kind"], "decorative");
    for source in [
        &json["request"]["jobs"][0]["source"],
        &json["request"]["jobs"][1]["source"],
    ] {
        assert!(source.get("sourceBytes").is_none());
        assert!(source.get("sourceSha256").is_none());
        assert!(source.get("fingerprint").is_none());
    }
    assert_eq!(
        serde_json::from_value::<ImagingCommand>(json).expect("the command decodes"),
        command
    );
}

#[test]
fn a_reuse_candidate_carries_only_disposable_generation_evidence() {
    let fingerprint = CacheFingerprint::sha256_full_file(
        4_096,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .expect("the versioned fingerprint is valid");
    let reusable = CacheReusableGeneration::new(
        "published-generation-a",
        CacheArtifactProperties::new(
            CacheArtifactFormat::Png,
            1_600,
            1_200,
            2_048,
            None,
            None,
            CacheBasicColorProfile::Srgb,
        ),
        fingerprint.clone(),
    )
    .expect("the reusable generation evidence is valid");
    let source = CacheMediaSource::new(
        "decorative-a",
        MediaKind::Decorative,
        PathBuf::from(r"C:\Decoratives\overlay.png"),
    )
    .expect("the Decorative source is valid");
    let job = CacheJob::new(source, "candidate-generation-b", Some(reusable))
        .expect("the job may compare one published generation");

    assert_eq!(job.reusable.as_ref().unwrap().fingerprint, fingerprint);
    assert_ne!(
        job.candidate_generation_id,
        job.reusable.as_ref().unwrap().generation_id
    );
}

#[test]
fn reusable_generation_carries_the_normative_page_count_and_basic_color_profile() {
    let fingerprint = CacheFingerprint::sha256_full_file(
        4_096,
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )
    .expect("the versioned fingerprint is valid");
    let reusable = CacheReusableGeneration::new(
        "published-tiff-generation",
        CacheArtifactProperties::new(
            CacheArtifactFormat::Jpeg,
            1_600,
            1_200,
            2_048,
            Some(1),
            Some(1),
            CacheBasicColorProfile::Srgb,
        ),
        fingerprint,
    )
    .expect("single-page sRGB metadata is valid");

    let json = serde_json::to_value(&reusable).expect("the generation serializes");
    assert_eq!(json["sourcePageCount"], 1);
    assert_eq!(json["basicColorProfile"], "srgb");
    assert_eq!(
        serde_json::from_value::<CacheReusableGeneration>(json)
            .expect("the generation metadata decodes"),
        reusable
    );
}

#[test]
fn fingerprint_v1_carries_original_size_and_available_file_dates() {
    let fingerprint = CacheFingerprint::sha256_full_file_with_timestamps(
        4_096,
        Some(1_700_000_000_001),
        Some(1_700_000_000_002),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )
    .expect("the observed fingerprint is valid");

    let json = serde_json::to_value(&fingerprint).expect("the fingerprint serializes");
    assert_eq!(json["sourceBytes"], 4_096);
    assert_eq!(json["sourceCreatedUnixMs"], 1_700_000_000_001_u64);
    assert_eq!(json["sourceModifiedUnixMs"], 1_700_000_000_002_u64);
    assert_eq!(
        serde_json::from_value::<CacheFingerprint>(json).expect("the fingerprint decodes"),
        fingerprint
    );
}
