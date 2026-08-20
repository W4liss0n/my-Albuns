use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::{
    ffi::c_void,
    os::windows::io::AsRawHandle,
    time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat, InitialBackground,
    InitialBackgroundContent, InitialFrameBorder, InitialOverlay, InitialOverlayContent,
    InitialProject, InitialProjectConfiguration, InitialProjectPersonalization, MediaKind,
    ProjectCore, ProjectIntent, ProjectLocation, RenderSnapshot,
};
use myalbuns_imaging_protocol::{
    CacheArtifact, CacheArtifactFormat, CacheArtifactProperties, CacheJob, CacheMediaSource,
    CacheRepresentationPolicy, CacheRequest, CacheReusableGeneration, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingFailure, ImagingFailureCode, ImagingFailureStage, ImagingPathCode,
    ImagingRequest, ImagingResponse, PROCESSOR_HANDSHAKE_CHALLENGE_ENV, RenderSource,
    decode_event_stream, decode_processor_handshake, root_binding_plan_sha256,
};
use myalbuns_paths::{
    AppPaths, CachePathPlan, NativePathDto, OperationPathContext, ProcessInstanceId,
    RootBindingPlan, project_data_namespace,
};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, STILL_ACTIVE},
    System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
};

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
}

impl Drop for TestCache {
    fn drop(&mut self) {
        if let Ok(app_paths) = AppPaths::discover() {
            let _ = app_paths.clear_project_cache(&self.paths);
        }
    }
}

fn cache_job(
    source: CacheMediaSource,
    generation_id: &str,
    reusable: Option<CacheReusableGeneration>,
) -> CacheJob {
    CacheJob::new(source, generation_id, reusable).expect("the Cache job is valid")
}

fn reusable_generation(artifact: &CacheArtifact) -> CacheReusableGeneration {
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
    .expect("the generated artifact is valid reuse evidence")
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

#[test]
fn processor_exports_the_neutral_project_as_the_canonical_jpeg() {
    let project_directory = tempfile::tempdir().expect("temporary neutral Project directory");
    let project_path = project_directory.path().join("Projeto.myalbuns");
    let mut project_context = OperationPathContext::new();
    project_context
        .capture(&project_path)
        .expect("the neutral Project root is captured");
    let project = ProjectCore::new()
        .with_identity_storage_roots(
            project_directory.path().join("leases"),
            project_directory.path().join("identities"),
        )
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path, project_context.freeze()),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the neutral Project is created");
    let snapshot = project.render_snapshot();
    let unit = snapshot
        .output_unit(&snapshot.composition.sheets[0].sheet_id)
        .expect("the first neutral Sheet becomes an output unit");
    let output_directory = tempfile::tempdir().expect("temporary JPEG output directory");
    let output_path = output_directory.path().join("Projeto_001.jpg");
    let bindings = root_bindings(&[&output_path]);
    let request = ImagingRequest::new(
        "neutral-jpeg-001",
        snapshot.project_id.clone(),
        snapshot.revision,
        NativePathDto::from(output_path.as_path()),
        unit,
        snapshot.dpi,
        Vec::new(),
        bindings,
    )
    .expect("the source-free neutral render request is valid");

    let log_directory = tempfile::tempdir().expect("temporary Processor log directory");
    let result = invoke_render_request(&request, Some(log_directory.path()));

    assert!(
        result.status.success(),
        "processor failed: {}\n{}",
        String::from_utf8_lossy(&result.stderr),
        read_logs(log_directory.path())
    );
    let bytes = std::fs::read(&output_path).expect("the exported JPEG is readable");
    assert_eq!(&bytes[..2], b"\xFF\xD8");
    assert_eq!(&bytes[bytes.len() - 2..], b"\xFF\xD9");
    let markers = jpeg_header_markers(&bytes);
    assert_eq!(markers[0].0, 0xE0, "JFIF must immediately follow SOI");
    let jfif = markers[0].1;
    assert_eq!(&jfif[..5], b"JFIF\0");
    assert_eq!(jfif[7], 1, "JFIF density units must be DPI");
    assert_eq!(u16::from_be_bytes([jfif[8], jfif[9]]), 300);
    assert_eq!(u16::from_be_bytes([jfif[10], jfif[11]]), 300);
    assert_eq!(&jfif[12..14], &[0, 0], "JFIF thumbnail must be absent");
    assert!(markers.iter().all(|(marker, _)| *marker != 0xE1));
    assert!(markers.iter().all(|(marker, _)| *marker != 0xFE));

    let sof0 = markers
        .iter()
        .find_map(|(marker, payload)| (*marker == 0xC0).then_some(*payload))
        .expect("the JPEG uses baseline SOF0");
    assert_eq!(sof0[0], 8);
    assert_eq!(u16::from_be_bytes([sof0[1], sof0[2]]), 3_543);
    assert_eq!(u16::from_be_bytes([sof0[3], sof0[4]]), 7_087);
    assert_eq!(sof0[5], 3);

    let mut icc_segments = markers
        .iter()
        .filter_map(|(marker, payload)| {
            (*marker == 0xE2 && payload.starts_with(b"ICC_PROFILE\0")).then_some((
                payload[12],
                payload[13],
                &payload[14..],
            ))
        })
        .collect::<Vec<_>>();
    icc_segments.sort_by_key(|(sequence, _, _)| *sequence);
    assert!(!icc_segments.is_empty());
    let segment_count = icc_segments[0].1;
    assert_eq!(usize::from(segment_count), icc_segments.len());
    assert!(
        icc_segments
            .iter()
            .enumerate()
            .all(|(index, (sequence, total, _))| {
                usize::from(*sequence) == index + 1 && *total == segment_count
            })
    );
    let profile = icc_segments
        .into_iter()
        .flat_map(|(_, _, payload)| payload.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(profile.len(), 3_024);
    assert_eq!(
        format!("{:x}", Sha256::digest(&profile)),
        "384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a"
    );

    let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
        .expect("the golden JPEG decodes")
        .to_rgb8();
    assert_eq!(decoded.dimensions(), (7_087, 3_543));
    assert!(decoded.pixels().all(|pixel| pixel.0 == [255, 255, 255]));
    let (_, response) =
        decode_event_stream(&result.stdout).expect("the processor output is a valid event stream");
    let completion = response
        .completed_for("neutral-jpeg-001")
        .expect("the neutral render completes");
    assert_eq!(completion.source_count, 0);
    assert_eq!(completion.source_bytes, 0);
    assert_eq!(completion.width_px, 7_087);
    assert_eq!(completion.height_px, 3_543);
    assert_eq!(completion.dpi, 300);
}

#[test]
fn visible_noninitial_sheet_uses_unsaved_dpi_personalization_and_exact_originals_end_to_end() {
    let root = tempfile::tempdir().expect("temporary visible-state Projeto fixture");
    let project_path = root.path().join("Visivel.myalbuns");
    let left_path = root.path().join("left-background.png");
    let right_path = root.path().join("right-background.png");
    let overlay_path = root.path().join("both-overlay.png");
    RgbaImage::from_pixel(4, 4, Rgba([240, 20, 20, 128]))
        .save_with_format(&left_path, ImageFormat::Png)
        .expect("the translucent left Background is written");
    RgbaImage::from_pixel(4, 4, Rgba([20, 40, 220, 255]))
        .save_with_format(&right_path, ImageFormat::Png)
        .expect("the opaque right Background is written");
    RgbaImage::from_pixel(4, 4, Rgba([20, 220, 20, 128]))
        .save_with_format(&overlay_path, ImageFormat::Png)
        .expect("the translucent Overlay with Ambos os lados scope is written");
    let initial = InitialProject::configured(InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        25_400,
        12_700,
        300,
        0,
        0,
        3,
        EndSheetFormat::SinglePage,
        EndSheetFormat::SinglePage,
    ))
    .with_personalization(InitialProjectPersonalization::new(
        InitialBackground::PerSide {
            left: InitialBackgroundContent::Media {
                path: left_path.clone(),
            },
            right: InitialBackgroundContent::Media {
                path: right_path.clone(),
            },
        },
        InitialOverlay::BothSides {
            both: Some(InitialOverlayContent::Media {
                path: overlay_path.clone(),
            }),
        },
        InitialFrameBorder::None,
    ));
    let mut project_context = OperationPathContext::new();
    project_context
        .capture(&project_path)
        .expect("the Projeto root is captured");
    let mut project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path.clone(), project_context.freeze()),
            initial,
            CreateAuthorization::CreateOnly,
        ))
        .expect("the personalized Projeto is created");
    let persisted_before =
        std::fs::read(&project_path).expect("the persisted Projeto baseline is readable");
    let visible = project
        .apply(ProjectIntent::SetDpi { dpi: 200 })
        .expect("the visible DPI changes without saving");
    let snapshot = project.render_snapshot();

    assert_eq!(snapshot.revision, visible.state.revision);
    assert_eq!(snapshot.dpi, 200);
    assert!(visible.state.dirty);
    assert!(visible.state.can_undo);
    assert!(!visible.state.can_redo);
    assert!(
        snapshot
            .composition
            .sheets
            .iter()
            .all(|sheet| sheet.frames.is_empty()),
        "the real v1 Projeto contributes no demonstration Frames"
    );

    let expected_widths = [100, 200, 100];
    let expected_source_counts = [2, 3, 2];
    let mut rendered_sheets = Vec::new();
    for (index, sheet) in snapshot.composition.sheets.iter().enumerate() {
        let output_path = root.path().join(format!("visible-sheet-{index}.jpg"));
        let unit = snapshot
            .output_unit(&sheet.sheet_id)
            .expect("the chosen visible Lâmina becomes one output unit");
        let mut media_ids = Vec::new();
        for media_id in unit.sheet.referenced_media_ids() {
            if !media_ids.contains(&media_id) {
                media_ids.push(media_id.to_owned());
            }
        }
        let mut sources = Vec::new();
        for media_id in media_ids {
            let media = project
                .project()
                .media()
                .iter()
                .find(|media| media.id() == media_id.into_uuid())
                .expect("every composed Imagem decorativa belongs to the frozen Revisão");
            sources.push(
                RenderSource::new(media_id, media.path().to_path_buf())
                    .expect("the exact original descriptor is valid"),
            );
        }
        let mut paths = Vec::with_capacity(sources.len() + 1);
        paths.push(output_path.as_path());
        paths.extend(sources.iter().map(RenderSource::source_path));
        let bindings = root_bindings(&paths);
        let request = ImagingRequest::new(
            format!("visible-revision-{index}"),
            snapshot.project_id.clone(),
            snapshot.revision,
            NativePathDto::from(output_path.as_path()),
            unit,
            snapshot.dpi,
            sources,
            bindings,
        )
        .expect("the exact visible-state request is valid");
        assert_eq!(request.revision, visible.state.revision);
        assert_eq!(request.unit.sheet.sheet_id, sheet.sheet_id);

        let result = invoke_render_request(&request, None);

        assert!(
            result.status.success(),
            "Processador failed for Lâmina {index}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let response = processor_response(&result.stdout);
        let completion = response
            .completed_for(&request.request_id)
            .expect("the visible-state terminal is correlated");
        assert_eq!(completion.width_px, expected_widths[index]);
        assert_eq!(completion.height_px, 100);
        assert_eq!(completion.dpi, 200);
        assert_eq!(completion.source_count, expected_source_counts[index]);
        let bytes = std::fs::read(&output_path).expect("the visible JPEG is readable");
        let jfif = jpeg_header_markers(&bytes)[0].1;
        assert_eq!(u16::from_be_bytes([jfif[8], jfif[9]]), 200);
        rendered_sheets.push(
            image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
                .expect("the visible JPEG decodes")
                .to_rgb8(),
        );
    }

    let right_only = rendered_sheets[0].get_pixel(50, 50);
    let both_left = rendered_sheets[1].get_pixel(40, 50);
    let both_right = rendered_sheets[1].get_pixel(160, 50);
    let left_only = rendered_sheets[2].get_pixel(50, 50);
    assert!(both_left[0] > both_right[0] + 70, "left scope remains red");
    assert!(
        both_right[2] > both_left[2] + 30,
        "right scope remains blue"
    );
    assert!(
        both_left[1] > 100 && both_right[1] > 100,
        "the Overlay with Ambos os lados scope remains visible"
    );
    assert_eq!(
        right_only.0, both_right.0,
        "the right-side Lâmina de página única normalizes its personalization"
    );
    assert_eq!(
        left_only.0, both_left.0,
        "the left-side Lâmina de página única normalizes its personalization"
    );
    assert!(
        both_left[2] > 50,
        "the translucent Background was composed over canonical white"
    );
    assert_eq!(
        project.projection(),
        visible,
        "Exportação leaves Revisão, dirty state and Undo/Redo untouched"
    );
    assert_eq!(
        std::fs::read(&project_path).expect("the Projeto file remains readable"),
        persisted_before,
        "the unsaved visible Revisão is never persisted by Exportação"
    );
}

fn jpeg_header_markers(bytes: &[u8]) -> Vec<(u8, &[u8])> {
    assert_eq!(&bytes[..2], b"\xFF\xD8");
    let mut markers = Vec::new();
    let mut cursor = 2;
    while cursor < bytes.len() {
        assert_eq!(bytes[cursor], 0xFF, "expected a JPEG marker prefix");
        while bytes[cursor] == 0xFF {
            cursor += 1;
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]));
        assert!(length >= 2);
        cursor += 2;
        let payload_length = length - 2;
        markers.push((marker, &bytes[cursor..cursor + payload_length]));
        cursor += payload_length;
    }
    markers
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

#[cfg(windows)]
#[test]
fn processor_handshake_reports_the_exact_instance_seen_through_the_spawned_child_handle() {
    let challenge = "cli_launch_exact_instance";
    let child = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"))
        .env(PROCESSOR_HANDSHAKE_CHALLENGE_ENV, challenge)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the Processor starts");
    let expected =
        ProcessInstanceId::from_process_handle(child.id(), child.as_raw_handle().cast::<c_void>())
            .expect("the causal child handle exposes its exact identity");
    let process_id = child.id();
    let output = child
        .wait_with_output()
        .expect("the Processor exits after its empty input");
    let line_end = output
        .stdout
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("the handshake line is present");

    assert_eq!(
        decode_processor_handshake(&output.stdout[..=line_end], challenge, process_id)
            .expect("the handshake is bound to this launch"),
        expected
    );
    assert_eq!(line_end + 1, output.stdout.len());
}

#[test]
fn processor_never_replaces_an_existing_preparation() {
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("existing-output.jpg");
    std::fs::write(&output_path, b"previous export").expect("the previous output is writable");
    let request = neutral_render_request(output_path.clone(), "replacement-request", 25);

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "replacement-request").code,
        ImagingFailureCode::EncodeFailed
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
    let mut source = RgbImage::new(2_000, 1_500);
    for (x, y, pixel) in source.enumerate_pixels_mut() {
        *pixel = Rgb([((x * 3) % 256) as u8, ((y * 4) % 256) as u8, 96]);
    }
    source
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the real JPEG fixture is written");
    let original_source = std::fs::read(&source_path).expect("the source is readable");
    let source_sha256 = format!("{:x}", Sha256::digest(&original_source));
    let cache_paths = cache.paths.clone();
    let media_source = CacheMediaSource::new("benchmark-a-001", MediaKind::Photo, source_path)
        .expect("the native source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-001",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source.clone(), "g-build-one", None)],
            CacheRepresentationPolicy::measured_v1(),
            bindings.clone(),
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
    assert_eq!(completed.artifacts[0].width_px, 1_600);
    assert_eq!(completed.artifacts[0].height_px, 1_200);
    assert_eq!(completed.artifacts[0].fingerprint.value, source_sha256);
    let preview_path = cache_paths
        .preview_file("benchmark-a-001", "g-build-one", CacheArtifactFormat::Jpeg)
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

    let reuse_command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-002",
            cache.project_id.clone(),
            cache_paths,
            vec![cache_job(
                media_source,
                "g-build-two",
                Some(reusable_generation(&completed.artifacts[0])),
            )],
            CacheRepresentationPolicy::measured_v1(),
            bindings.clone(),
        )
        .expect("the reuse request is valid"),
    );
    let reused_result = invoke_imaging_command(&reuse_command, Some(log_dir.path()));
    assert!(reused_result.status.success());
    let reused_response = processor_response(&reused_result.stdout);
    let reused = reused_response
        .cache_completed_for("cache-002")
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
    let media_source =
        CacheMediaSource::new("decorative-overlay-001", MediaKind::Decorative, source_path)
            .expect("the transparent decorative source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-transparent-decorative",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source, "g-transparent-one", None)],
            CacheRepresentationPolicy::measured_v1(),
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
    assert_eq!(artifact.fingerprint.value, source_sha256);
    assert_eq!(artifact.format, CacheArtifactFormat::Png);
    assert_eq!(artifact.exif_orientation, None);
    assert_eq!((artifact.width_px, artifact.height_px), (1_600, 1_200));
    let preview_path = cache_paths
        .preview_file(
            "decorative-overlay-001",
            "g-transparent-one",
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
    let media_source =
        CacheMediaSource::new("decorative-opaque-001", MediaKind::Decorative, source_path)
            .expect("the opaque source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-opaque-png",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source, "g-opaque-one", None)],
            CacheRepresentationPolicy::measured_v1(),
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
    assert_eq!(artifact.fingerprint.value, source_sha256);
    assert_eq!(artifact.format, CacheArtifactFormat::Jpeg);
    assert_eq!(artifact.exif_orientation, None);
    let preview_path = cache_paths
        .preview_file(
            "decorative-opaque-001",
            "g-opaque-one",
            CacheArtifactFormat::Jpeg,
        )
        .expect("the JPEG preview path is derived centrally");
    let bytes = std::fs::read(preview_path).expect("the reduced representation is readable");
    assert_eq!(&bytes[..2], b"\xff\xd8");
}

#[test]
fn processor_opens_the_current_original_once_and_classifies_its_content() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("photo.jpg");
    RgbImage::from_pixel(16, 12, Rgb([24, 96, 180]))
        .save_with_format(&source_path, ImageFormat::Jpeg)
        .expect("the linked JPEG fixture is written");
    let original = std::fs::read(&source_path).expect("the source is readable");
    let request = single_source_render_request(
        output_dir.path().join("source-verification.jpg"),
        "source-verification",
        &source_path,
    );
    let mut changed = original;
    changed[0] ^= 0xff;
    std::fs::write(&source_path, changed).expect("the source is changed after planning");

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        render_failure_stage(&result, "source-verification"),
        ImagingFailureStage::SourceDecode
    );
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"source_decode\""));
}

#[test]
fn processor_identifies_a_missing_original_as_a_source_verification_failure() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("missing-photo.jpg");
    let output_path = output_dir.path().join("missing-original.jpg");
    let request =
        single_source_render_request(output_path.clone(), "missing-original", &source_path);

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        render_failure_stage(&result, "missing-original"),
        ImagingFailureStage::SourceVerification
    );
    assert_eq!(
        render_failure(&result, "missing-original").path_code,
        Some(ImagingPathCode::NotFound)
    );
    assert!(!output_path.exists());
    let logs = read_logs(log_dir.path());
    assert!(logs.contains("\"stage\":\"source_verification\""));
    assert!(logs.contains("\"failure_code\":\"source_unavailable\""));
    assert!(logs.contains("\"path_code\":\"not_found\""));
}

#[test]
fn processor_identifies_source_decode_failures() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let source_path = source_dir.path().join("invalid.jpg");
    let bytes = b"this is not a JPEG";
    std::fs::write(&source_path, bytes).expect("the invalid source is written");
    let request = single_source_render_request(
        output_dir.path().join("source-decode.jpg"),
        "source-decode",
        &source_path,
    );

    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        render_failure_stage(&result, "source-decode"),
        ImagingFailureStage::SourceDecode
    );
    assert!(read_logs(log_dir.path()).contains("\"stage\":\"source_decode\""));
}

#[test]
fn processor_decodes_a_supported_progressive_jpeg_in_the_isolated_worker() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("progressive.jpg");
    let output_path = output_dir.path().join("progressive-output.jpg");
    let bytes = include_bytes!("fixtures/progressive-420-dri.jpg");
    std::fs::write(&source_path, bytes).expect("the progressive JPEG fixture is written");
    let request =
        single_source_render_request(output_path.clone(), "progressive-worker", &source_path);

    let result = invoke_render_request(&request, None);

    assert!(
        result.status.success(),
        "Processador failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    processor_response(&result.stdout)
        .completed_for("progressive-worker")
        .expect("the successful terminal is correlated");
    image::open(output_path).expect("the JPEG produced by Exportação decodes");
}

#[cfg(windows)]
#[test]
fn cancelling_processor_during_progressive_decode_leaves_no_worker_process() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let barrier_dir = tempfile::tempdir().expect("temporary worker barrier directory");
    let source_path = source_dir.path().join("progressive.jpg");
    let output_path = output_dir.path().join("cancelled-progressive.jpg");
    let barrier_path = barrier_dir.path().join("worker.pid");
    let bytes = include_bytes!("fixtures/progressive-420-dri.jpg");
    std::fs::write(&source_path, bytes).expect("the progressive JPEG fixture is written");
    let request =
        single_source_render_request(output_path, "cancel-progressive-worker", &source_path);
    let command = ImagingCommand::render(request);
    let mut processor = spawn_imaging_command_with_barrier(&command, &barrier_path);
    let worker_pid = wait_for_worker_pid(&barrier_path);
    assert!(
        process_is_running(worker_pid),
        "the worker reached progressive decode"
    );

    processor
        .kill()
        .expect("cancellation terminates the Processador");
    processor
        .wait()
        .expect("the cancelled Processador is reaped");
    let worker_stopped = wait_for_process_exit(worker_pid, Duration::from_secs(2));

    let _ = std::fs::remove_file(&barrier_path);
    if !worker_stopped {
        assert!(
            wait_for_process_exit(worker_pid, Duration::from_secs(5)),
            "the orphaned worker must exit after test cleanup"
        );
    }
    assert!(
        worker_stopped,
        "cancelling the Processador must terminate its progressive JPEG worker"
    );
}

#[test]
fn processor_rejects_a_progressive_jpeg_decoder_budget_before_publication() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("hostile-progressive.jpg");
    let output_path = output_dir.path().join("hostile-progressive-output.jpg");
    let bytes = progressive_jpeg_header(12_000, 10_000);
    std::fs::write(&source_path, &bytes).expect("the hostile progressive header is written");
    let request =
        single_source_render_request(output_path.clone(), "progressive-budget", &source_path);

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "progressive-budget").code,
        ImagingFailureCode::ResourceLimitExceeded
    );
    assert!(!output_path.exists());
}

#[test]
fn processor_rejects_tiff_by_detected_content_with_a_stable_code() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("misleading.jpg");
    let bytes = b"II\x2a\x00\x08\x00\x00\x00";
    std::fs::write(&source_path, bytes).expect("the TIFF signature is written");
    let request = single_source_render_request(
        output_dir.path().join("unsupported-format.jpg"),
        "unsupported-format",
        &source_path,
    );
    let media_id = request.sources[0].media_id().to_owned();

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "unsupported-format"),
        ImagingFailure {
            code: ImagingFailureCode::UnsupportedSourceFormat,
            media_id: Some(media_id.to_string()),
            path_code: None,
        }
    );
}

#[test]
fn processor_classifies_a_short_unknown_source_by_format_not_decoder_failure() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("short.png");
    let bytes = b"GIF";
    std::fs::write(&source_path, bytes).expect("the short unknown source is written");
    let request = single_source_render_request(
        output_dir.path().join("short-unknown.jpg"),
        "short-unknown",
        &source_path,
    );

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "short-unknown").code,
        ImagingFailureCode::UnsupportedSourceFormat
    );
}

#[test]
fn processor_classifies_an_empty_source_by_detected_format() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("empty.png");
    std::fs::write(&source_path, []).expect("the empty source is written");
    let output_path = output_dir.path().join("empty-source.jpg");
    let request = single_source_render_request(output_path.clone(), "empty-source", &source_path);

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "empty-source").code,
        ImagingFailureCode::UnsupportedSourceFormat
    );
    assert!(!output_path.exists());
}

#[test]
fn processor_rejects_apng_before_choosing_a_frame() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("animated.png");
    RgbaImage::from_pixel(1, 1, Rgba([20, 40, 60, 255]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the PNG baseline is written");
    let mut bytes = std::fs::read(&source_path).expect("the PNG baseline is readable");
    insert_png_chunk_after_ihdr(&mut bytes, *b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
    std::fs::write(&source_path, &bytes).expect("the APNG marker is written");
    let request = single_source_render_request(
        output_dir.path().join("animated.jpg"),
        "animated-source",
        &source_path,
    );

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "animated-source").code,
        ImagingFailureCode::UnsupportedSourceVariant
    );
}

#[test]
fn processor_rejects_a_malformed_png_profile_before_publication() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("bad-profile.png");
    RgbaImage::from_pixel(1, 1, Rgba([20, 40, 60, 255]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the PNG baseline is written");
    let mut bytes = std::fs::read(&source_path).expect("the PNG baseline is readable");
    insert_png_chunk_after_ihdr(&mut bytes, *b"iCCP", b"broken\0\0not-zlib");
    std::fs::write(&source_path, &bytes).expect("the malformed profile is written");
    let output_path = output_dir.path().join("bad-profile.jpg");
    let request = single_source_render_request(output_path.clone(), "bad-profile", &source_path);

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "bad-profile").code,
        ImagingFailureCode::UnsupportedColorProfile
    );
    assert!(!output_path.exists());
}

#[test]
fn processor_checks_unique_source_pixels_separately_from_output_pixels() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("oversized-source.png");
    RgbaImage::from_pixel(1, 1, Rgba([20, 40, 60, 255]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the PNG baseline is written");
    let mut bytes = std::fs::read(&source_path).expect("the PNG baseline is readable");
    set_png_dimensions(&mut bytes, 16_384, 16_384);
    std::fs::write(&source_path, &bytes).expect("the oversized header is written");
    let output_path = output_dir.path().join("source-limit.jpg");
    let request = single_source_render_request(output_path.clone(), "source-limit", &source_path);

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "source-limit").code,
        ImagingFailureCode::ResourceLimitExceeded
    );
    assert!(!output_path.exists());
}

#[test]
fn processor_enforces_the_measured_decoder_allocation_budget_below_the_pixel_ceiling() {
    let source_dir = tempfile::tempdir().expect("temporary source directory");
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let source_path = source_dir.path().join("decoder-allocation-limit.png");
    RgbaImage::from_pixel(1, 1, Rgba([20, 40, 60, 255]))
        .save_with_format(&source_path, ImageFormat::Png)
        .expect("the PNG baseline is written");
    let mut bytes = std::fs::read(&source_path).expect("the PNG baseline is readable");
    // 8,192² stays below the measured pixel ceiling, but the conservative
    // PNG decoder working plan (8 bytes/pixel plus headroom) exceeds 512 MiB.
    set_png_dimensions(&mut bytes, 8_192, 8_192);
    std::fs::write(&source_path, &bytes).expect("the allocation header is written");
    let output_path = output_dir.path().join("decoder-allocation-limit.jpg");
    let request = single_source_render_request(
        output_path.clone(),
        "decoder-allocation-limit",
        &source_path,
    );

    let result = invoke_render_request(&request, None);

    assert_eq!(
        render_failure(&result, "decoder-allocation-limit").code,
        ImagingFailureCode::ResourceLimitExceeded
    );
    assert!(!output_path.exists());
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
    let media_source =
        CacheMediaSource::new("benchmark-a-001", MediaKind::Photo, source_path.clone())
            .expect("the source is valid");
    let bindings = root_bindings(&[cache_paths.root(), media_source.source_path()]);
    let command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-integrity",
            cache.project_id.clone(),
            cache_paths.clone(),
            vec![cache_job(media_source.clone(), "g-integrity-one", None)],
            CacheRepresentationPolicy::measured_v1(),
            bindings.clone(),
        )
        .expect("the Cache request is valid"),
    );
    let generated = invoke_imaging_command(&command, Some(log_dir.path()));
    assert!(generated.status.success());
    let generated_response = processor_response(&generated.stdout);
    let generated_artifact = generated_response
        .cache_completed_for("cache-integrity")
        .expect("the generated response is correlated")
        .artifacts[0]
        .clone();
    assert_eq!(generated_artifact.fingerprint.value, source_sha256);

    std::fs::write(&source_path, vec![0x5a; original.len()])
        .expect("the source changes without changing its length");
    let changed_command = ImagingCommand::build_cache(
        CacheRequest::new(
            "cache-integrity-changed",
            cache.project_id.clone(),
            cache_paths,
            vec![cache_job(
                media_source,
                "g-integrity-two",
                Some(reusable_generation(&generated_artifact)),
            )],
            CacheRepresentationPolicy::measured_v1(),
            bindings,
        )
        .expect("the changed-source request is valid"),
    );
    let changed = invoke_imaging_command(&changed_command, Some(log_dir.path()));

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
fn processor_rejects_an_invalid_output_unit() {
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("invalid.jpg");
    let request = neutral_render_request(output_path.clone(), "invalid", 25);
    let mut request = serde_json::to_value(request).expect("request is serializable");
    request["unit"]["sheet"]["widthUm"] = serde_json::json!(0);

    let command = serde_json::json!({
        "kind": "render",
        "request": request,
    });
    let result = invoke_render_payload(
        serde_json::to_vec(&command).expect("modified command is serializable"),
        None,
    );

    assert_eq!(
        render_failure_stage(&result, "invalid"),
        ImagingFailureStage::InvalidRenderRequest
    );
    assert!(!output_path.exists());
}

#[test]
fn processor_rejects_a_render_root_omitted_by_the_operation_owner() {
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output_path = output_dir.path().join("unbound.jpg");
    let request = neutral_render_request(output_path.clone(), "unbound-root", 25);
    let mut command =
        serde_json::to_value(ImagingCommand::render(request)).expect("the command is serializable");
    command["request"]["rootBindings"] = serde_json::json!({ "bindings": [] });

    let result = invoke_render_payload(
        serde_json::to_vec(&command).expect("the altered command is serializable"),
        None,
    );

    assert_eq!(
        render_failure_stage(&result, "unbound-root"),
        ImagingFailureStage::InvalidRenderRequest
    );
    assert!(
        !output_path.exists(),
        "the Processor must reject the request before creating a preparation"
    );
}

#[test]
fn processor_writes_correlated_logs_without_exposing_the_output_path() {
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir.path().join("private-album-name.jpg");

    let request = neutral_render_request(output_path.clone(), "logged-request-001", 25);
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
    let process_started = events
        .iter()
        .find(|event| event["event"] == "imaging_process_started")
        .expect("the Processor start is logged");
    let process_stopped = events
        .iter()
        .find(|event| event["event"] == "imaging_process_stopped")
        .expect("the Processor terminal is logged");
    assert_eq!(
        completed["process_id"], started["process_id"],
        "the terminal event keeps the Processor PID used by the host correlation"
    );
    assert_eq!(
        process_stopped["process_id"], process_started["process_id"],
        "the process terminal keeps the exact Processor PID"
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
    let mut snapshot = productive_snapshot(InitialProject::neutral());
    snapshot.project_id = r"c:\users\person\private-project".into();
    let request_id = "private-operation";
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let log_dir = tempfile::tempdir().expect("temporary log directory");
    let output_path = output_dir
        .path()
        .join("missing-parent")
        .join("private-album-name.jpg");

    let request = render_request(snapshot, output_path.clone(), request_id, 300, Vec::new());
    let result = invoke_render_request(&request, Some(log_dir.path()));

    assert_eq!(
        render_failure(&result, request_id).code,
        ImagingFailureCode::EncodeFailed
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!stderr.contains(request_id));
    assert!(!stderr.contains(&output_path.to_string_lossy().into_owned()));

    let logs = read_logs(log_dir.path());
    assert!(logs.contains("imaging_request_started"));
    assert!(logs.contains("imaging_render_failed"));
    assert!(logs.contains("\"stage\":\"output_encode\""));
    assert!(logs.contains("\"failure_code\":\"encode_failed\""));
    assert!(logs.contains("\"reason\":"));
    assert!(logs.contains(request_id));
    assert!(!logs.contains(r"c:\users\person\private-project"));
    assert!(!logs.contains(&output_path.to_string_lossy().into_owned()));
}

fn single_source_render_request(
    output_path: PathBuf,
    request_id: &str,
    source_path: &Path,
) -> ImagingRequest {
    let initial =
        small_initial_project(25).with_personalization(InitialProjectPersonalization::new(
            InitialBackground::BothSides {
                both: InitialBackgroundContent::Media {
                    path: source_path.to_path_buf(),
                },
            },
            InitialOverlay::BothSides { both: None },
            InitialFrameBorder::None,
        ));
    let snapshot = productive_snapshot(initial);
    let sheet = snapshot
        .composition
        .sheets
        .first()
        .expect("the fixture contains a sheet");
    let media_id = sheet
        .referenced_media_ids()
        .next()
        .expect("the productive fixture references its decorative original");
    let sheet_id = sheet.sheet_id.clone();
    let source =
        RenderSource::new(media_id, source_path.to_path_buf()).expect("the linked source is valid");
    render_request_for_sheet(
        snapshot,
        &sheet_id,
        output_path,
        request_id,
        25,
        vec![source],
    )
}

fn neutral_render_request(output_path: PathBuf, request_id: &str, dpi: u32) -> ImagingRequest {
    let snapshot = productive_snapshot(small_initial_project(i64::from(dpi)));
    render_request(snapshot, output_path, request_id, dpi, Vec::new())
}

fn small_initial_project(dpi: i64) -> InitialProject {
    InitialProject::configured(InitialProjectConfiguration::new(
        DisplayUnit::Mm,
        25_400,
        12_700,
        dpi,
        0,
        0,
        2,
        EndSheetFormat::Double,
        EndSheetFormat::Double,
    ))
}

fn productive_snapshot(initial: InitialProject) -> RenderSnapshot {
    let root = tempfile::tempdir().expect("temporary productive Project directory");
    let project_path = root.path().join("ProcessorFixture.myalbuns");
    let mut context = OperationPathContext::new();
    context
        .capture(&project_path)
        .expect("the productive Project root is captured");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path, context.freeze()),
            initial,
            CreateAuthorization::CreateOnly,
        ))
        .expect("the productive Project is created through ProjectCore");
    project.render_snapshot()
}

fn render_request(
    snapshot: RenderSnapshot,
    output_path: PathBuf,
    request_id: &str,
    dpi: u32,
    sources: Vec<RenderSource>,
) -> ImagingRequest {
    let sheet_id = snapshot
        .composition
        .sheets
        .first()
        .expect("the productive fixture contains a Sheet")
        .sheet_id
        .clone();
    render_request_for_sheet(snapshot, &sheet_id, output_path, request_id, dpi, sources)
}

fn render_request_for_sheet(
    snapshot: RenderSnapshot,
    sheet_id: &str,
    output_path: PathBuf,
    request_id: &str,
    dpi: u32,
    sources: Vec<RenderSource>,
) -> ImagingRequest {
    let mut paths = Vec::with_capacity(sources.len() + 1);
    paths.push(output_path.as_path());
    paths.extend(sources.iter().map(RenderSource::source_path));
    let bindings = root_bindings(&paths);
    let unit = snapshot
        .output_unit(sheet_id)
        .expect("the selected Sheet becomes an output unit");
    ImagingRequest::new(
        request_id,
        snapshot.project_id.clone(),
        snapshot.revision,
        NativePathDto::from(output_path),
        unit,
        dpi,
        sources,
        bindings,
    )
    .expect("the productive render request is valid")
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

fn render_failure_stage(result: &Output, request_id: &str) -> ImagingFailureStage {
    render_failure(result, request_id).code.stage()
}

fn render_failure(result: &Output, request_id: &str) -> ImagingFailure {
    assert!(
        result.status.success(),
        "known render failures must use a structured terminal: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let (_, response) = decode_event_stream(&result.stdout).expect("the failure stream is valid");
    response
        .failure_for(request_id)
        .expect("the failure terminal is correlated")
}

fn insert_png_chunk_after_ihdr(bytes: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(&kind);
    chunk.extend_from_slice(data);
    let mut crc_input = Vec::from(kind);
    crc_input.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    bytes.splice(33..33, chunk);
}

fn progressive_jpeg_header(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xc2, 0, 17, 8];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 0, 3, 0x11, 0]);
    bytes.extend_from_slice(&[0xff, 0xda, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]);
    bytes
}

fn set_png_dimensions(bytes: &mut [u8], width: u32, height: u32) {
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    let crc = crc32(&bytes[12..29]);
    bytes[29..33].copy_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
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
    spawn_imaging_command_with_options(command, log_dir, None)
}

#[cfg(windows)]
fn spawn_imaging_command_with_barrier(command: &ImagingCommand, barrier: &Path) -> Child {
    spawn_imaging_command_with_options(command, None, Some(barrier))
}

fn spawn_imaging_command_with_options(
    command: &ImagingCommand,
    log_dir: Option<&Path>,
    progressive_decode_barrier: Option<&Path>,
) -> Child {
    let mut process = Command::new(env!("CARGO_BIN_EXE_myalbuns-imaging"));
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(log_dir) = log_dir {
        process.env("MYALBUNS_LOG_DIR", log_dir);
    }
    if let Some(barrier) = progressive_decode_barrier {
        process.env("MYALBUNS_TEST_PROGRESSIVE_DECODE_BARRIER", barrier);
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

#[cfg(windows)]
fn wait_for_worker_pid(barrier: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(pid) = std::fs::read_to_string(barrier)
            && let Ok(pid) = pid.parse()
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "the progressive worker did not reach the decode barrier"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn wait_for_process_exit(process_id: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !process_is_running(process_id) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    !process_is_running(process_id)
}

#[cfg(windows)]
fn process_is_running(process_id: u32) -> bool {
    // SAFETY: the handle is checked before use and closed on every path.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return false;
        }
        let mut exit_code = 0;
        let queried = GetExitCodeProcess(process, &mut exit_code) != 0;
        CloseHandle(process);
        queried && exit_code == STILL_ACTIVE as u32
    }
}
