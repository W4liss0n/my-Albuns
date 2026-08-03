use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use myalbuns_core::{EditableProject, ProjectCore, ProjectIntent, RenderSnapshot};
use myalbuns_imaging_protocol::{
    CacheArtifactFormat, CacheJob, CacheRequest, CacheResetRequest, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingFailureStage, ImagingProgressStage, ImagingRequest, ImagingResponse,
    MediaSource, decode_event_stream, root_binding_plan_sha256,
};
use myalbuns_paths::{
    AppPaths, CachePathPlan, OperationPathContext, RootBindingPlan, project_data_namespace,
};
use sha2::{Digest, Sha256};

#[path = "../../../tests/support/sample_project.rs"]
mod sample_project;

use sample_project::SampleProject;

static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(1);

struct TestCache {
    paths: CachePathPlan,
    project_id: String,
}

impl TestCache {
    fn new(label: &str) -> Self {
        let app_paths = AppPaths::discover().expect("the test can discover LocalAppData");
        let project_id = format!(
            "test-{label}-{}-{}",
            std::process::id(),
            NEXT_CACHE_ID.fetch_add(1, Ordering::Relaxed)
        );
        let paths = app_paths
            .project_cache(&project_data_namespace(&project_id))
            .expect("the isolated Cache plan is valid");
        Self { paths, project_id }
    }

    fn materialize(&self) {
        let app_paths = AppPaths::discover().expect("the test can discover LocalAppData");
        drop(
            app_paths
                .prepare_cache_storage(&self.paths)
                .expect("the isolated Cache directory is materialized"),
        );
    }

    fn project_root(&self) -> PathBuf {
        self.paths
            .metadata_file()
            .parent()
            .expect("the metadata belongs to the project Cache")
            .to_path_buf()
    }
}

impl Drop for TestCache {
    fn drop(&mut self) {
        if let Ok(app_paths) = AppPaths::discover() {
            let _ = app_paths.clear_project_cache(&self.paths);
        }
    }
}

fn cache_job(source: MediaSource, max_edge_px: u32) -> CacheJob {
    let generation_id = format!(
        "{}-v1-{max_edge_px}",
        source.source_sha256()[..16].to_ascii_lowercase()
    );
    CacheJob::new(source, generation_id).expect("the Cache job is valid")
}

fn root_bindings(paths: &[&Path]) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in paths {
        context
            .capture(path)
            .expect("the operation path root is captured");
    }
    context.freeze()
}

fn sample_session(sample: SampleProject, sheet_count: usize) -> EditableProject {
    let source = sample
        .persisted_source(sheet_count)
        .expect("the sample project serializes");
    ProjectCore::new()
        .open_editable_session(&source)
        .expect("the sample project opens through ProjectCore")
}

#[test]
fn processor_advertises_the_protocol_version_used_by_external_runners() {
    let output = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"))
        .arg("--protocol-version")
        .output()
        .expect("the Processor reports its protocol version");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("the version is UTF-8")
            .trim(),
        IMAGING_PROTOCOL_VERSION.to_string()
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn processor_renders_a_png_from_a_validated_snapshot_only() {
    let session = sample_session(SampleProject::Horizon, 12);
    let snapshot = session.render_snapshot();
    let expected_source_count = snapshot.composition.sheets[0]
        .referenced_media_ids()
        .count();
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
    let (progress, response) =
        decode_event_stream(&result.stdout).expect("the processor output is a valid event stream");
    let stage_transitions =
        progress
            .iter()
            .map(|event| event.stage)
            .fold(Vec::new(), |mut stages, stage| {
                if stages.last() != Some(&stage) {
                    stages.push(stage);
                }
                stages
            });
    assert_eq!(
        stage_transitions,
        [
            ImagingProgressStage::LoadingSources,
            ImagingProgressStage::Composing,
            ImagingProgressStage::EncodingOutput,
        ]
    );
    assert!(
        progress
            .iter()
            .all(|event| event.completed_units <= event.total_units)
    );
    let completion = response
        .completed_for("request-001")
        .expect("the response is correlated");
    assert_eq!((completion.width_px, completion.height_px), (591, 295));
    assert_eq!(completion.dpi, 25);
    assert_eq!(completion.source_count, expected_source_count);
    assert!(completion.source_bytes > 0);
}

#[test]
fn processor_never_replaces_an_existing_preparation() {
    let session = sample_session(SampleProject::Horizon, 2);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("existing-output.png");
    std::fs::write(&output_path, b"previous export").expect("the previous output is writable");

    let result = invoke_processor(
        session.render_snapshot(),
        &output_path,
        "replacement-request",
    );

    assert_eq!(
        result.status.code(),
        Some(ImagingFailureStage::OutputPrepare.exit_code().into())
    );
    assert_eq!(
        std::fs::read(output_path).expect("the existing preparation remains readable"),
        b"previous export"
    );
}

#[test]
fn processor_builds_one_reduced_representation_per_real_photo() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("build");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    let mut source = RgbImage::new(64, 48);
    for (x, y, pixel) in source.enumerate_pixels_mut() {
        *pixel = Rgb([x as u8 * 3, y as u8 * 4, 96]);
    }
    source
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let source_bytes = std::fs::metadata(&source_path)
        .expect("source metadata is available")
        .len();
    let original_source = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original_source));
    let cache_paths = cache.paths.clone();
    let media_source = MediaSource::new(
        "benchmark-a-001",
        source_path,
        source_bytes,
        source_sha256.clone(),
    )
    .expect("the native source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-001",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source, 32)],
            32,
            bindings,
        )
        .expect("the Cache request is valid"),
    );

    let result = invoke_imaging_command(&command, Some(log_dir.path()));

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    let completed = response
        .cache_completed_for("cache-001")
        .expect("the Cache response is correlated");
    assert_eq!(completed.generated_count, 1);
    assert_eq!(completed.reused_count, 0);
    assert_eq!(completed.artifacts.len(), 1);
    assert!(completed.artifacts[0].width_px <= 32);
    assert!(completed.artifacts[0].height_px <= 32);
    let preview_path = cache_paths
        .preview_file(
            "benchmark-a-001",
            &format!("{}-v1-32", &source_sha256[..16]),
            CacheArtifactFormat::Jpeg,
        )
        .expect("the preview path is derived centrally");
    let bytes = std::fs::read(preview_path).expect("the reduced representation is readable");
    assert_eq!(&bytes[..2], b"\xff\xd8");
    assert_eq!(
        std::fs::read(source_dir.path().join("photo.jpg")).expect("the original remains readable"),
        original_source
    );
    assert!(
        !cache_paths.metadata_file().exists(),
        "only CacheEngine publishes the disposable index after validating the response"
    );

    let reused_result = invoke_imaging_command(&command, Some(log_dir.path()));
    assert!(reused_result.status.success());
    let reused_response = processor_response(&reused_result.stdout);
    let reused = reused_response
        .cache_completed_for("cache-001")
        .expect("the reuse response is correlated");
    assert_eq!(reused.generated_count, 0);
    assert_eq!(reused.reused_count, 1);
}

#[test]
fn processor_preserves_transparency_in_one_reduced_png_representation() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("transparent-decorative");
    let source_path = source_dir.path().join("overlay.png");
    RgbaImage::from_pixel(2_400, 1_800, Rgba([24, 96, 180, 96]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the transparent decorative fixture is written");
    let original_source = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original_source));
    let cache_paths = cache.paths.clone();
    let media_source = MediaSource::new(
        "decorative-overlay-001",
        source_path,
        original_source.len() as u64,
        source_sha256.clone(),
    )
    .expect("the transparent decorative source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-transparent-decorative",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source, 1_600)],
            1_600,
            bindings,
        )
        .expect("the Cache request is valid"),
    );

    let result = invoke_imaging_command(&command, None);

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    let completed = response
        .cache_completed_for("cache-transparent-decorative")
        .expect("the Cache response is correlated");
    assert_eq!(completed.generated_count, 1);
    assert_eq!(completed.reused_count, 0);
    assert_eq!(completed.artifacts.len(), 1);
    let artifact = &completed.artifacts[0];
    assert_eq!(artifact.format, CacheArtifactFormat::Png);
    assert_eq!(artifact.exif_orientation, None);
    assert_eq!((artifact.width_px, artifact.height_px), (1_600, 1_200));
    let preview_path = cache_paths
        .preview_file(
            "decorative-overlay-001",
            &format!("{}-v1-1600", &source_sha256[..16]),
            CacheArtifactFormat::Png,
        )
        .expect("the PNG preview path is derived centrally");
    let bytes = std::fs::read(&preview_path).expect("the reduced representation is readable");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let preview = image::open(preview_path)
        .expect("the reduced representation decodes")
        .to_rgba8();
    assert_eq!((preview.width(), preview.height()), (1_600, 1_200));
    assert_eq!(preview.get_pixel(800, 600)[3], 96);
    assert_eq!(
        std::fs::read(source_dir.path().join("overlay.png"))
            .expect("the original remains readable"),
        original_source
    );
}

#[test]
fn processor_keeps_an_opaque_png_source_in_the_jpeg_cache_baseline() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("opaque-png");
    let source_path = source_dir.path().join("opaque-decorative.png");
    RgbaImage::from_pixel(64, 48, Rgba([24, 96, 180, u8::MAX]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the opaque PNG fixture is written");
    let original_source = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original_source));
    let cache_paths = cache.paths.clone();
    let media_source = MediaSource::new(
        "decorative-opaque-001",
        source_path,
        original_source.len() as u64,
        source_sha256.clone(),
    )
    .expect("the opaque source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-opaque-png",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source, 32)],
            32,
            bindings,
        )
        .expect("the Cache request is valid"),
    );

    let result = invoke_imaging_command(&command, None);

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    let artifact = &response
        .cache_completed_for("cache-opaque-png")
        .expect("the Cache response is correlated")
        .artifacts[0];
    assert_eq!(artifact.format, CacheArtifactFormat::Jpeg);
    assert_eq!(artifact.exif_orientation, None);
    let preview_path = cache_paths
        .preview_file(
            "decorative-opaque-001",
            &format!("{}-v1-32", &source_sha256[..16]),
            CacheArtifactFormat::Jpeg,
        )
        .expect("the JPEG preview path is derived centrally");
    let bytes = std::fs::read(preview_path).expect("the reduced representation is readable");
    assert_eq!(&bytes[..2], b"\xff\xd8");
}

#[test]
fn processor_renders_linked_original_pixels_at_the_requested_dpi() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("photo.jpg");
    let output_path = output_dir.path().join("real-sheet.png");
    let mut source = RgbImage::new(40, 20);
    for (x, _, pixel) in source.enumerate_pixels_mut() {
        *pixel = if x < 20 {
            Rgb([240, 16, 16])
        } else {
            Rgb([16, 32, 240])
        };
    }
    source
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the linked JPEG is written");
    let source_bytes = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
    let mut snapshot = sample_session(SampleProject::Horizon, 2).render_snapshot();
    let sheet = &mut snapshot.composition.sheets[0];
    sheet.overlay = None;
    sheet.width_um = 25_400;
    sheet.height_um = 12_700;
    sheet.frames.truncate(1);
    let frame = &mut sheet.frames[0];
    frame.clip_rect.x = 0;
    frame.clip_rect.y = 0;
    frame.clip_rect.width = sheet.width_um;
    frame.clip_rect.height = sheet.height_um;
    let photo = frame.photo.as_mut().expect("the fixture frame has a Photo");
    photo.draw_rect = frame.clip_rect.clone();
    photo.rotation_degrees = 0.0;
    photo.mirror_x = false;
    let source = MediaSource::new(
        photo.media_id.clone(),
        source_path,
        source_bytes.len() as u64,
        source_sha256,
    )
    .expect("the linked source is valid");

    let result = invoke_real_processor(
        snapshot,
        &output_path,
        "real-request-001",
        100,
        vec![source],
    );

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    let completion = response
        .completed_for("real-request-001")
        .expect("the response is correlated");
    assert_eq!((completion.width_px, completion.height_px), (100, 50));
    assert_eq!(completion.dpi, 100);
    assert_eq!(completion.source_count, 1);
    assert_eq!(completion.source_bytes, source_bytes.len() as u64);
    assert_eq!(
        completion.output_sha256,
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(&output_path).expect("output is readable"))
        )
    );
    let rendered = image::open(&output_path)
        .expect("the output decodes")
        .to_rgb8();
    let left = rendered.get_pixel(10, 25);
    let right = rendered.get_pixel(90, 25);
    assert!(left[0] > left[2] * 3, "the left source half remains red");
    assert!(
        right[2] > right[0] * 3,
        "the right source half remains blue"
    );
}

#[test]
fn processor_composites_a_transparent_decorative_from_its_original_png() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("decorative-sheet.png");
    let photo_path = source_dir.path().join("photo.jpg");
    let decorative_path = source_dir.path().join("overlay.png");
    RgbImage::from_pixel(40, 20, Rgb([20, 40, 220]))
        .save_with_format(&photo_path, ImageFormat::Jpeg)
        .expect("the linked Photo is written");
    RgbaImage::from_pixel(40, 20, Rgba([220, 20, 20, 128]))
        .save_with_format(&decorative_path, ImageFormat::Png)
        .expect("the transparent Decorative is written");
    let photo_bytes = std::fs::read(&photo_path).expect("the Photo is readable");
    let decorative_bytes = std::fs::read(&decorative_path).expect("the Decorative is readable");

    let mut snapshot = sample_session(SampleProject::Horizon, 3).render_snapshot();
    let overlay = snapshot.composition.sheets[0]
        .overlay
        .clone()
        .expect("the representative fixture contains an Overlay");
    let sheet = &mut snapshot.composition.sheets[0];
    sheet.width_um = 25_400;
    sheet.height_um = 12_700;
    sheet.frames.truncate(1);
    let (photo_media_id, draw_rect) = {
        let frame = &mut sheet.frames[0];
        frame.clip_rect.x = 0;
        frame.clip_rect.y = 0;
        frame.clip_rect.width = sheet.width_um;
        frame.clip_rect.height = sheet.height_um;
        let photo = frame.photo.as_mut().expect("the fixture Frame has a Photo");
        photo.draw_rect = frame.clip_rect.clone();
        photo.rotation_degrees = 0.0;
        photo.mirror_x = false;
        (photo.media_id.clone(), frame.clip_rect.clone())
    };
    sheet.overlay = Some(myalbuns_core::ComposedDecorative {
        draw_rect,
        ..overlay
    });

    let photo_source = MediaSource::new(
        photo_media_id,
        photo_path,
        photo_bytes.len() as u64,
        format!("{:x}", Sha256::digest(&photo_bytes)),
    )
    .expect("the Photo source is valid");
    let decorative_source = MediaSource::new(
        "decorative-overlay",
        decorative_path.clone(),
        decorative_bytes.len() as u64,
        format!("{:x}", Sha256::digest(&decorative_bytes)),
    )
    .expect("the Decorative source is valid");

    let result = invoke_real_processor(
        snapshot,
        &output_path,
        "decorative-original-001",
        100,
        vec![photo_source, decorative_source],
    );

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    let completion = response
        .completed_for("decorative-original-001")
        .expect("the response is correlated");
    assert_eq!(completion.source_count, 2);
    let rendered = image::open(output_path)
        .expect("the output decodes")
        .to_rgba8();
    let blended = rendered.get_pixel(25, 25);
    assert!(blended[0] > 90, "the Overlay contributes red");
    assert!(blended[2] > 90, "the Photo remains visible through alpha");
    assert!(blended[1] < 70, "the blend preserves the expected tint");
    assert_eq!(
        std::fs::read(decorative_path).expect("the original remains readable"),
        decorative_bytes
    );
}

#[test]
fn processor_identifies_source_verification_failures() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    RgbImage::from_pixel(16, 12, Rgb([24, 96, 180]))
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the linked JPEG fixture is written");
    let original = std::fs::read(&source_path).expect("the source is readable");
    let request = single_photo_render_request(
        output_dir.path().join("source-verification.png"),
        "source-verification",
        &source_path,
        original.len() as u64,
        format!("{:x}", Sha256::digest(&original)),
    );
    let mut changed = original;
    changed[0] ^= 0xff;
    std::fs::write(&source_path, changed).expect("the source is changed after planning");

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        result.status.code(),
        Some(ImagingFailureStage::SourceVerification.exit_code().into())
    );
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"source_verification\""));
}

#[test]
fn processor_identifies_a_missing_original_as_a_source_verification_failure() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("missing-photo.jpg");
    let output_path = output_dir.path().join("missing-original.png");
    let request = single_photo_render_request(
        output_path.clone(),
        "missing-original",
        &source_path,
        1024,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
    );

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        result.status.code(),
        Some(ImagingFailureStage::SourceVerification.exit_code().into())
    );
    assert!(!output_path.exists());
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"source_verification\""));
}

#[test]
fn processor_identifies_source_decode_failures() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("invalid.jpg");
    let bytes = b"this is not a JPEG";
    std::fs::write(&source_path, bytes).expect("the invalid source is written");
    let request = single_photo_render_request(
        output_dir.path().join("source-decode.png"),
        "source-decode",
        &source_path,
        bytes.len() as u64,
        format!("{:x}", Sha256::digest(bytes)),
    );

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        result.status.code(),
        Some(ImagingFailureStage::SourceDecode.exit_code().into())
    );
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"source_decode\""));
}

#[test]
fn processor_rejects_a_same_length_change_before_reusing_a_preview() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let cache = TestCache::new("integrity");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    RgbImage::from_pixel(32, 24, Rgb([24, 96, 180]))
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let original = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original));
    let cache_paths = cache.paths.clone();
    let media_source = MediaSource::new(
        "benchmark-a-001",
        source_path.clone(),
        original.len() as u64,
        source_sha256,
    )
    .expect("the source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-integrity",
            cache.project_id.clone(),
            cache_paths,
            vec![cache_job(media_source, 32)],
            32,
            bindings,
        )
        .expect("the Cache request is valid"),
    );
    let generated = invoke_imaging_command(&command, Some(log_dir.path()));
    assert!(generated.status.success());

    std::fs::write(&source_path, vec![0x5a; original.len()])
        .expect("the source changes without changing its length");
    let changed = invoke_imaging_command(&command, Some(log_dir.path()));

    assert!(
        !changed.status.success(),
        "a stale generation must not be reused for changed source bytes"
    );
    assert_eq!(
        changed.status.code(),
        Some(ImagingFailureStage::CacheProcessing.exit_code().into())
    );
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"cache_processing\""));
}

#[test]
fn processor_resets_only_native_project_cache_namespaces() {
    let first = TestCache::new("reset-a");
    let second = TestCache::new("reset-b");
    first.materialize();
    second.materialize();
    let command = ImagingCommand::reset_cache(
        CacheResetRequest::new(
            "reset-cache",
            vec![first.project_id.clone(), second.project_id.clone()],
        )
        .expect("the reset request is valid"),
    );

    let result = invoke_imaging_command(&command, None);

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response = processor_response(&result.stdout);
    assert_eq!(response.cache_reset_for("reset-cache"), Some(2));
    assert!(!first.project_root().exists());
    assert!(!second.project_root().exists());
}

#[test]
fn processor_uses_the_composed_media_transform() {
    let mut session = sample_session(SampleProject::Horizon, 12);
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
    let session = sample_session(SampleProject::Horizon, 12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("invalid.png");
    let snapshot = session.render_snapshot();
    let (_source_dir, request) =
        render_request_with_temp_originals(snapshot, &output_path, "invalid", 25);
    let mut request = serde_json::to_value(request).expect("request is serializable");
    request["snapshot"]["composition"]["sheets"][0]["widthUm"] = serde_json::json!(0);

    let command = serde_json::json!({
        "kind": "render",
        "request": request,
    });
    let result = invoke_render_payload(
        serde_json::to_vec(&command).expect("modified command is serializable"),
        None,
    );

    assert!(!result.status.success());
    assert!(!output_path.exists());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("não concluiu a solicitação"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn processor_rejects_a_render_root_omitted_by_the_operation_owner() {
    let session = sample_session(SampleProject::Horizon, 2);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("unbound.png");
    let snapshot = session.render_snapshot();
    let (_source_dir, request) =
        render_request_with_temp_originals(snapshot, &output_path, "unbound-root", 25);
    let mut command =
        serde_json::to_value(ImagingCommand::render(request)).expect("the command is serializable");
    command["request"]["rootBindings"] = serde_json::json!({ "bindings": [] });

    let result = invoke_render_payload(
        serde_json::to_vec(&command).expect("the altered command is serializable"),
        None,
    );

    assert!(!result.status.success());
    assert!(
        !output_path.exists(),
        "the Processor must reject the request before creating a preparation"
    );
}

#[test]
fn processor_writes_correlated_logs_without_exposing_the_output_path() {
    let session = sample_session(SampleProject::Horizon, 12);
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir.path().join("private-album-name.png");

    let (_source_dir, request) = render_request_with_temp_originals(
        session.render_snapshot(),
        &output_path,
        "logged-request-001",
        25,
    );
    let expected_plan_sha256 =
        root_binding_plan_sha256(&request.root_bindings).expect("the expected plan has a digest");
    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert!(
        result.status.success(),
        "processor failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let logs = read_logs(log_dir.path());
    assert!(logs.contains("imaging_request_started"));
    assert!(logs.contains("imaging_request_completed"));
    assert!(logs.contains("\"process_role\":\"imaging\""));
    assert!(logs.contains(&format!("\"protocol_version\":{IMAGING_PROTOCOL_VERSION}")));
    assert!(logs.contains("\"operation_id\":\"logged-request-001\""));
    let events = logs
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("the log line is JSON"))
        .collect::<Vec<_>>();
    let started = events
        .iter()
        .find(|event| event["event"] == "imaging_request_started")
        .expect("the request start is logged");
    let completed = events
        .iter()
        .find(|event| event["event"] == "imaging_request_completed")
        .expect("the request completion is logged");
    assert_eq!(
        completed["process_id"], started["process_id"],
        "the terminal event keeps the Processor PID used by the host correlation"
    );
    assert!(
        logs.contains(&format!(
            "\"root_binding_plan_sha256\":\"{expected_plan_sha256}\""
        )),
        "expected RootBindingPlan digest {expected_plan_sha256} in logs: {logs}"
    );
    assert!(
        !logs.contains(&output_path.to_string_lossy().into_owned()),
        "the output path must not be written to logs"
    );
}

#[test]
fn processor_redacts_path_shaped_project_identifiers_and_output_failures() {
    let mut snapshot = sample_session(SampleProject::Horizon, 12).render_snapshot();
    snapshot.project_id = r"c:\users\person\private-project".into();
    let request_id = "private-operation";
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir
        .path()
        .join("missing-parent")
        .join("private-album-name.png");

    let result =
        invoke_processor_with_log_dir(snapshot, &output_path, request_id, Some(log_dir.path()));

    assert!(!result.status.success());
    assert_eq!(
        result.status.code(),
        Some(ImagingFailureStage::OutputPrepare.exit_code().into())
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("não concluiu a solicitação"));
    assert!(!stderr.contains(request_id));
    assert!(!stderr.contains(&output_path.to_string_lossy().into_owned()));

    let logs = read_logs(log_dir.path());
    assert!(logs.contains("imaging_request_started"));
    assert!(logs.contains("imaging_render_failed"));
    assert!(logs.contains("\"stage\":\"output_prepare\""));
    assert!(logs.contains(request_id));
    assert!(!logs.contains(r"c:\users\person\private-project"));
    assert!(!logs.contains(&output_path.to_string_lossy().into_owned()));
}

fn invoke_processor(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
) -> std::process::Output {
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    invoke_processor_with_log_dir(snapshot, output_path, request_id, Some(log_dir.path()))
}

fn single_photo_render_request(
    output_path: PathBuf,
    request_id: &str,
    source_path: &Path,
    source_bytes: u64,
    source_sha256: String,
) -> ImagingRequest {
    let mut snapshot = sample_session(SampleProject::Horizon, 2).render_snapshot();
    let sheet = snapshot
        .composition
        .sheets
        .first_mut()
        .expect("the fixture contains a sheet");
    sheet.overlay = None;
    sheet.frames.truncate(1);
    let media_id = sheet.frames[0]
        .photo
        .as_ref()
        .expect("the fixture frame contains a Photo")
        .media_id
        .clone();
    let sheet_id = sheet.sheet_id.clone();
    let source = MediaSource::new(
        media_id,
        source_path.to_path_buf(),
        source_bytes,
        source_sha256,
    )
    .expect("the linked source is valid");
    let bindings = root_bindings(&[&output_path, source.source_path()]);
    ImagingRequest::new(
        request_id,
        output_path,
        snapshot,
        sheet_id,
        25,
        vec![source],
        bindings,
    )
    .expect("the linked-original render request is valid")
}

fn read_logs(log_dir: &Path) -> String {
    std::fs::read_dir(log_dir)
        .expect("log directory is readable")
        .map(|entry| {
            let path = entry.expect("log entry is valid").path();
            std::fs::read_to_string(path).expect("log file is readable")
        })
        .collect()
}

fn processor_response(stdout: &[u8]) -> ImagingResponse {
    decode_event_stream(stdout)
        .expect("the processor output is a valid event stream")
        .1
}

fn invoke_processor_with_log_dir(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
    log_dir: Option<&Path>,
) -> std::process::Output {
    let (_source_dir, request) =
        render_request_with_temp_originals(snapshot, output_path, request_id, 25);
    invoke_render_request(&request, log_dir)
}

fn render_request_with_temp_originals(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
    dpi: u32,
) -> (tempfile::TempDir, ImagingRequest) {
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .expect("the fixture contains a sheet")
        .sheet_id
        .clone();
    let sheet = snapshot
        .composition
        .sheets
        .first()
        .expect("the fixture contains a sheet");
    let overlay_id = sheet
        .overlay
        .as_ref()
        .map(|overlay| overlay.media_id.as_str());
    let mut media_ids = Vec::new();
    for media_id in sheet.referenced_media_ids() {
        if !media_ids.iter().any(|known| known == media_id) {
            media_ids.push(media_id.to_owned());
        }
    }
    let source_dir = tempfile::tempdir().expect("temporary original-source directory");
    let mut sources = Vec::with_capacity(media_ids.len());
    for (index, media_id) in media_ids.into_iter().enumerate() {
        let source_path = source_dir.path().join(format!("original-{index}.png"));
        let alpha = if overlay_id == Some(media_id.as_str()) {
            144
        } else {
            u8::MAX
        };
        RgbaImage::from_fn(64, 48, |x, y| {
            let horizontal = (x * 3) as u8;
            let vertical = (y * 4) as u8;
            let accent = (index as u8).wrapping_mul(53).wrapping_add(32);
            Rgba([horizontal, vertical, accent, alpha])
        })
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the temporary original is written");
        let source_bytes = std::fs::read(&source_path).expect("the original is readable");
        sources.push(
            MediaSource::new(
                media_id,
                source_path,
                source_bytes.len() as u64,
                format!("{:x}", Sha256::digest(&source_bytes)),
            )
            .expect("the temporary original source is valid"),
        );
    }
    let mut paths = Vec::with_capacity(sources.len() + 1);
    paths.push(output_path);
    paths.extend(sources.iter().map(MediaSource::source_path));
    let bindings = root_bindings(&paths);
    drop(paths);
    let request = ImagingRequest::new(
        request_id,
        output_path.to_path_buf(),
        snapshot,
        sheet_id,
        dpi,
        sources,
        bindings,
    )
    .expect("the original-only render request is valid");
    (source_dir, request)
}

fn invoke_real_processor(
    snapshot: RenderSnapshot,
    output_path: &Path,
    request_id: &str,
    dpi: u32,
    sources: Vec<MediaSource>,
) -> std::process::Output {
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .expect("the fixture contains a sheet")
        .sheet_id
        .clone();
    let mut paths = Vec::with_capacity(sources.len() + 1);
    paths.push(output_path);
    paths.extend(sources.iter().map(MediaSource::source_path));
    let bindings = root_bindings(&paths);
    drop(paths);
    let request = ImagingRequest::new(
        request_id,
        output_path.to_path_buf(),
        snapshot,
        sheet_id,
        dpi,
        sources,
        bindings,
    )
    .expect("the linked-original render request is valid");
    invoke_render_request(&request, None)
}

fn invoke_render_request(request: &ImagingRequest, log_dir: Option<&Path>) -> std::process::Output {
    spawn_render_request(request, log_dir)
        .wait_with_output()
        .expect("processor exits")
}

fn invoke_render_payload(mut payload: Vec<u8>, log_dir: Option<&Path>) -> std::process::Output {
    spawn_render_payload(&mut payload, log_dir)
        .wait_with_output()
        .expect("processor exits")
}

fn spawn_render_request(request: &ImagingRequest, log_dir: Option<&Path>) -> Child {
    spawn_imaging_command(&ImagingCommand::render(request.clone()), log_dir)
}

fn spawn_render_payload(payload: &mut Vec<u8>, log_dir: Option<&Path>) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(log_dir) = log_dir {
        command.env("MYALBUNS_LOG_DIR", log_dir);
    }
    let mut child = command.spawn().expect("processor starts");
    payload.push(b'\n');
    child
        .stdin
        .take()
        .expect("stdin is available")
        .write_all(payload)
        .expect("request is sent");
    child
}

fn invoke_imaging_command(
    command: &ImagingCommand,
    log_dir: Option<&Path>,
) -> std::process::Output {
    spawn_imaging_command(command, log_dir)
        .wait_with_output()
        .expect("processor exits")
}

fn spawn_imaging_command(command: &ImagingCommand, log_dir: Option<&Path>) -> Child {
    let mut process = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(log_dir) = log_dir {
        process.env("MYALBUNS_LOG_DIR", log_dir);
    }
    let mut child = process.spawn().expect("processor starts");
    let mut payload = serde_json::to_vec(command).expect("command is serializable");
    payload.push(b'\n');
    child
        .stdin
        .take()
        .expect("stdin is available")
        .write_all(&payload)
        .expect("command is sent");
    child
}
