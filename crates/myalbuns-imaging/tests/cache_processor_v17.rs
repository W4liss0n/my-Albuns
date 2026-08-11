use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use myalbuns_core::MediaKind;
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheArtifactProperties, CacheBasicColorProfile, CacheJob,
    CacheMediaSource, CacheRepresentationPolicy, CacheRequest, CacheReusableGeneration,
    ImagingCommand, ImagingFailureStage, decode_event_stream,
};
use myalbuns_paths::{
    AppPaths, CachePathPlan, OperationPathContext, RootBindingPlan, project_data_namespace,
};
use tiff::encoder::{TiffEncoder, colortype};

struct IsolatedCache {
    app_paths: AppPaths,
    paths: CachePathPlan,
    project_id: String,
}

impl IsolatedCache {
    fn new(label: &str) -> Self {
        let app_paths = AppPaths::discover().expect("the test can discover application paths");
        let project_id = format!("v17-{label}-{}", std::process::id());
        let paths = app_paths
            .project_cache(&project_data_namespace(&project_id))
            .expect("the isolated Cache path is valid");
        Self {
            app_paths,
            paths,
            project_id,
        }
    }
}

impl Drop for IsolatedCache {
    fn drop(&mut self) {
        let _ = self.app_paths.clear_project_cache(&self.paths);
    }
}

#[test]
fn real_processor_owns_fingerprint_generation_reuse_and_invalidation() {
    let source_root = tempfile::tempdir().expect("temporary source root");
    let log_root = tempfile::tempdir().expect("temporary log root");
    let cache = IsolatedCache::new("generation");
    let photo_path = source_root.path().join("photo.jpg");
    RgbImage::from_fn(2_400, 1_800, |x, y| {
        Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8])
    })
    .save_with_format(&photo_path, ImageFormat::Jpeg)
    .expect("the Photo fixture is written");

    let first = request(
        &cache,
        "cache-first",
        "photo-a",
        MediaKind::Photo,
        &photo_path,
        "candidate-first",
        None,
    );
    let first_completion = run(&first, log_root.path());
    assert_eq!(first_completion.generated_count, 1);
    assert_eq!(first_completion.reused_count, 0);
    let first_artifact = first_completion.artifacts[0].clone();
    assert_eq!(first_artifact.generation_id, "candidate-first");
    assert_eq!(first_artifact.format, CacheArtifactFormat::Jpeg);
    assert_eq!(first_artifact.fingerprint.algorithm, "sha256-full-file-v1");
    assert!(first_artifact.fingerprint.source_created_unix_ms.is_some());
    assert!(first_artifact.fingerprint.source_modified_unix_ms.is_some());
    assert!(first_artifact.width_px <= 1_600);
    assert!(first_artifact.height_px <= 1_600);

    let second = request(
        &cache,
        "cache-second",
        "photo-a",
        MediaKind::Photo,
        &photo_path,
        "candidate-second",
        Some(reusable(&first_artifact)),
    );
    let second_completion = run(&second, log_root.path());
    assert_eq!(second_completion.generated_count, 0);
    assert_eq!(second_completion.reused_count, 1);
    assert_eq!(
        second_completion.artifacts[0].generation_id,
        "candidate-first"
    );

    RgbImage::from_pixel(2_400, 1_800, Rgb([220, 30, 40]))
        .save_with_format(&photo_path, ImageFormat::Jpeg)
        .expect("the Photo fixture is replaced in place");
    let third = request(
        &cache,
        "cache-third",
        "photo-a",
        MediaKind::Photo,
        &photo_path,
        "candidate-third",
        Some(reusable(&first_artifact)),
    );
    let third_completion = run(&third, log_root.path());
    assert_eq!(third_completion.generated_count, 1);
    assert_eq!(third_completion.reused_count, 0);
    assert_eq!(
        third_completion.artifacts[0].generation_id,
        "candidate-third"
    );
    assert_ne!(
        third_completion.artifacts[0].fingerprint,
        first_artifact.fingerprint
    );
}

#[test]
fn real_processor_supports_decorative_alpha_and_single_page_tiff() {
    let source_root = tempfile::tempdir().expect("temporary source root");
    let log_root = tempfile::tempdir().expect("temporary log root");
    let cache = IsolatedCache::new("formats");
    let alpha_path = source_root.path().join("overlay.png");
    RgbaImage::from_pixel(2_400, 1_800, Rgba([24, 96, 180, 96]))
        .save_with_format(&alpha_path, ImageFormat::Png)
        .expect("the alpha fixture is written");
    let tiff_path = source_root.path().join("background.tif");
    write_tiff(&tiff_path, 2_048, 1_536, 1);

    let mut context = OperationPathContext::new();
    context
        .capture(cache.paths.root())
        .expect("the Cache root is captured");
    context
        .capture(&alpha_path)
        .expect("the PNG root is captured");
    context
        .capture(&tiff_path)
        .expect("the TIFF root is captured");
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-formats",
            cache.project_id.clone(),
            cache.paths.clone(),
            vec![
                CacheJob::new(
                    CacheMediaSource::new("overlay-a", MediaKind::Decorative, alpha_path)
                        .expect("the PNG source is valid"),
                    "candidate-alpha",
                    None,
                )
                .expect("the PNG job is valid"),
                CacheJob::new(
                    CacheMediaSource::new("background-a", MediaKind::Decorative, tiff_path)
                        .expect("the TIFF source is valid"),
                    "candidate-tiff",
                    None,
                )
                .expect("the TIFF job is valid"),
            ],
            CacheRepresentationPolicy::measured_v1(),
            context.freeze(),
        )
        .expect("the mixed-format request is valid"),
    );

    let completion = run(&command, log_root.path());
    assert_eq!(completion.generated_count, 2);
    assert_eq!(completion.artifacts[0].format, CacheArtifactFormat::Png);
    assert_eq!(completion.artifacts[0].exif_orientation, None);
    assert_eq!(completion.artifacts[0].source_page_count, None);
    assert_eq!(
        completion.artifacts[0].basic_color_profile,
        CacheBasicColorProfile::Srgb
    );
    assert_eq!(completion.artifacts[1].format, CacheArtifactFormat::Jpeg);
    assert_eq!(completion.artifacts[1].exif_orientation, Some(1));
    assert_eq!(completion.artifacts[1].source_page_count, Some(1));
    assert_eq!(
        completion.artifacts[1].basic_color_profile,
        CacheBasicColorProfile::Srgb
    );
}

#[test]
fn real_processor_rejects_multi_page_tiff_without_publishing_a_generation() {
    let source_root = tempfile::tempdir().expect("temporary TIFF source root");
    let log_root = tempfile::tempdir().expect("temporary TIFF log root");
    let cache = IsolatedCache::new("multi-page-tiff");
    let tiff_path = source_root.path().join("album-pages.tif");
    write_tiff(&tiff_path, 32, 24, 2);
    let command = request(
        &cache,
        "cache-reject-multi-page",
        "background-pages",
        MediaKind::Decorative,
        &tiff_path,
        "candidate-rejected",
        None,
    );

    let output = invoke(&command, log_root.path());

    assert!(!output.status.success());
    assert_eq!(
        output.status.code(),
        Some(ImagingFailureStage::CacheProcessing.exit_code().into())
    );
    for format in [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png] {
        let artifact = cache
            .paths
            .preview_file("background-pages", "candidate-rejected", format)
            .expect("the candidate artifact path is valid");
        assert!(
            !artifact.exists(),
            "a rejected TIFF cannot publish Cache pixels"
        );
    }
}

fn request(
    cache: &IsolatedCache,
    request_id: &str,
    media_id: &str,
    kind: MediaKind,
    path: &Path,
    candidate_generation_id: &str,
    reusable: Option<CacheReusableGeneration>,
) -> ImagingCommand {
    let source = CacheMediaSource::new(media_id, kind, path.to_path_buf())
        .expect("the Cache source is valid");
    let bindings = root_bindings(&[cache.paths.root(), source.source_path()]);
    ImagingCommand::build_cache(
        CacheRequest::new(
            request_id,
            cache.project_id.clone(),
            cache.paths.clone(),
            vec![
                CacheJob::new(source, candidate_generation_id, reusable)
                    .expect("the Cache job is valid"),
            ],
            CacheRepresentationPolicy::measured_v1(),
            bindings,
        )
        .expect("the Cache request is valid"),
    )
}

fn reusable(artifact: &CacheArtifact) -> CacheReusableGeneration {
    CacheReusableGeneration::new(
        artifact.generation_id.clone(),
        CacheArtifactProperties::new(
            artifact.format,
            artifact.width_px,
            artifact.height_px,
            artifact.preview_bytes,
            artifact.exif_orientation,
            artifact.source_page_count,
            artifact.basic_color_profile,
        ),
        artifact.fingerprint.clone(),
    )
    .expect("the published artifact is valid reuse evidence")
}

fn root_bindings(paths: &[&Path]) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in paths {
        context
            .capture(path)
            .expect("the operation root is captured");
    }
    context.freeze()
}

fn run(command: &ImagingCommand, log_root: &Path) -> myalbuns_imaging_protocol::CacheCompletion {
    let output = invoke(command, log_root);
    assert!(
        output.status.success(),
        "Processador failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let (_, response) =
        decode_event_stream(&output.stdout).expect("the Processador response is valid");
    let request_id = match command {
        ImagingCommand::BuildCache(request) => &request.request_id,
        ImagingCommand::Render(_) => panic!("the fixture requires a Cache command"),
    };
    response
        .cache_completed_for(request_id)
        .expect("the response is correlated")
        .clone()
}

fn invoke(command: &ImagingCommand, log_root: &Path) -> std::process::Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    process
        .env("MYALBUNS_LOG_DIR", log_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process.spawn().expect("the real Processador starts");
    let mut payload = serde_json::to_vec(command).expect("the command serializes");
    payload.push(b'\n');
    child
        .stdin
        .take()
        .expect("the Processador stdin is available")
        .write_all(&payload)
        .expect("the Cache command is sent");
    child.wait_with_output().expect("the Processador exits")
}

fn write_tiff(path: &Path, width: u32, height: u32, pages: usize) {
    let pixels = vec![128_u8; width as usize * height as usize * 3];
    let file = std::fs::File::create(path).expect("the TIFF fixture is writable");
    let mut encoder = TiffEncoder::new(file).expect("the TIFF encoder is prepared");
    for _ in 0..pages {
        encoder
            .write_image::<colortype::RGB8>(width, height, &pixels)
            .expect("the TIFF page is written");
    }
}
