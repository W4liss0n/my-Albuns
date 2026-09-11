//! Real-process verification of the production import seams, without WebView or
//! native dialogs. Optional explicit inputs also produce repeatable measurements.
use super::*;
use crate::imaging_recovery_integration::RealProcessTransport;
use image::{ImageEncoder, ImageFormat, Rgb, RgbImage, codecs::jpeg::JpegEncoder};
use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
};
use myalbuns_imaging_protocol::CacheMediaSource;
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

fn digest(path: &Path) -> String {
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

struct InputSet {
    paths: Vec<PathBuf>,
    imported: usize,
    previews: usize,
    rejected: usize,
    host_decodes: usize,
}

fn inputs(root: &Path) -> InputSet {
    if let Some(path) = std::env::var_os("MYALBUNS_IMPORT_MEASUREMENT_INPUTS") {
        let paths: Vec<PathBuf> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(!paths.is_empty());
        return InputSet {
            imported: paths.len(),
            previews: paths.len(),
            paths,
            rejected: 0,
            host_decodes: 0,
        };
    }
    let mut paths = Vec::new();
    for index in 0..40 {
        let path = root.join(format!("photo-{index:03}.jpg"));
        RgbImage::from_pixel(37, 23, Rgb([index, 80, 160]))
            .save_with_format(&path, ImageFormat::Jpeg)
            .unwrap();
        paths.push(path);
    }
    let progressive = root.join("progressive.jpg");
    std::fs::write(
        &progressive,
        include_bytes!("../../../crates/myalbuns-imaging/tests/fixtures/progressive-420-dri.jpg"),
    )
    .unwrap();
    paths.push(progressive);
    let profile = root.join("unknown-profile.jpg");
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new(&mut encoded);
    encoder
        .set_icc_profile(b"unrecognized ICC profile".to_vec())
        .unwrap();
    encoder
        .encode_image(&RgbImage::from_pixel(37, 23, Rgb([40, 80, 160])))
        .unwrap();
    std::fs::write(&profile, encoded).unwrap();
    paths.push(profile);
    let bad = root.join("corrupted.jpg");
    std::fs::write(&bad, b"invalid JPEG").unwrap();
    paths.push(bad);
    let png = root.join("png-with-jpeg-extension.jpg");
    RgbImage::from_pixel(37, 23, Rgb([40, 80, 160]))
        .save_with_format(&png, ImageFormat::Png)
        .unwrap();
    paths.push(png);
    let tiff = root.join("overlay.tiff");
    image::RgbaImage::from_pixel(37, 23, image::Rgba([40, 80, 160, 100]))
        .save_with_format(&tiff, ImageFormat::Tiff)
        .unwrap();
    paths.push(tiff);
    paths.push(paths[0].clone());
    InputSet {
        paths,
        imported: 44,
        previews: 43,
        rejected: 1,
        host_decodes: 1,
    }
}

#[test]
#[ignore = "executed by scripts/Test-Rust.ps1 with the real debug sidecar"]
fn real_import_flow() {
    run_real_import_flow(MediaKind::Photo, inputs, false);
    if std::env::var_os("MYALBUNS_IMPORT_MEASUREMENT_INPUTS").is_none() {
        run_real_import_flow(MediaKind::Decorative, inputs, false);
    }
}

#[test]
#[ignore = "executed with the real debug sidecar"]
fn real_import_flow_reports_each_alternate_inspection_for_files_and_folder() {
    fn rejected_inputs(root: &Path) -> InputSet {
        let paths = (0..2)
            .map(|index| {
                let path = root.join(format!("unreadable-{index}.jpg"));
                std::fs::write(&path, b"invalid JPEG").unwrap();
                path
            })
            .collect();
        InputSet {
            paths,
            imported: 0,
            previews: 0,
            rejected: 2,
            host_decodes: 0,
        }
    }
    for by_folder in [false, true] {
        run_real_import_flow(MediaKind::Photo, rejected_inputs, by_folder);
    }
}

fn run_real_import_flow(kind: MediaKind, build_inputs: fn(&Path) -> InputSet, by_folder: bool) {
    run_real_import_flow_with_memory(kind, build_inputs, by_folder, None);
}

#[test]
#[ignore = "executed with the real debug sidecar"]
fn real_import_flow_finishes_serially_under_memory_pressure() {
    fn small_inputs(root: &Path) -> InputSet {
        let paths = (0..8)
            .map(|index| {
                let path = root.join(format!("serial-{index}.jpg"));
                RgbImage::from_pixel(37, 23, Rgb([index, 80, 160]))
                    .save(&path)
                    .unwrap();
                path
            })
            .collect();
        InputSet {
            paths,
            imported: 8,
            previews: 8,
            rejected: 0,
            host_decodes: 0,
        }
    }
    for by_folder in [false, true] {
        run_real_import_flow_with_memory(
            MediaKind::Photo,
            small_inputs,
            by_folder,
            Some((350, 7168)),
        );
    }
}

fn run_real_import_flow_with_memory(
    kind: MediaKind,
    build_inputs: fn(&Path) -> InputSet,
    by_folder: bool,
    available_memory_mib: Option<(u64, u64)>,
) {
    let executable = PathBuf::from(
        std::env::var_os("MYALBUNS_TEST_IMAGING_PROCESSOR").expect("real Processor path"),
    );
    let fixture = tempfile::tempdir().unwrap();
    let source_directory = fixture.path().join("originals");
    std::fs::create_dir(&source_directory).unwrap();
    let inputs = build_inputs(&source_directory);
    let original_hashes = inputs
        .paths
        .iter()
        .map(|path| digest(path))
        .collect::<Vec<_>>();
    let rounds: usize = std::env::var("MYALBUNS_IMPORT_MEASUREMENT_ROUNDS")
        .ok()
        .map(|value| value.parse().unwrap())
        .unwrap_or(1);
    assert!((1..=10).contains(&rounds));
    let mut measurements = Vec::new();
    let mut previous_previews = None;
    for round in 0..rounds {
        let root = tempfile::tempdir_in(fixture.path()).unwrap();
        let data_root = root.path().join("data");
        for name in ["Roaming", "Local", "logs"] {
            std::fs::create_dir_all(data_root.join(name)).unwrap();
        }
        let app_paths = AppPaths::from_roots(&data_root.join("Roaming"), &data_root.join("Local"));
        let project_path = root.path().join("Project.myalbuns");
        let mut context = OperationPathContext::new();
        context.capture(&project_path).unwrap();
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .unwrap();
        let namespace =
            AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority()).unwrap();
        let host = ProjectHost::new(project);
        let engine = CacheEngine::default();
        // Success-path tests must not depend on other applications' memory use.
        // The pressure scenario supplies its own constrained snapshot.
        let (physical, commit) = available_memory_mib.unwrap_or((16 * 1024, 16 * 1024));
        let processor = ImagingProcessor::with_available_memory_for_test(physical, commit);
        let registry = CachePreviewRegistry::new("import-flow");
        let monitor = MediaMonitor::default();
        let runtime = MediaRuntime::default();
        let total_started = Instant::now();
        let attempt = PhotoImportAttempt::capture_for_kind(
            kind,
            host.authorized_media_catalog().unwrap(),
            namespace.clone(),
            if by_folder {
                vec![source_directory.clone()]
            } else {
                inputs.paths.clone()
            },
        )
        .unwrap();
        let candidates = attempt
            .sources
            .iter()
            .map(|source| source.candidate.clone())
            .collect::<Vec<_>>();
        let stage = engine
            .begin_import_stage(
                &app_paths,
                namespace.clone(),
                attempt.id.clone(),
                &candidates,
            )
            .unwrap();
        let requests = attempt.requests(processor.cache_capacity());
        let process_count = requests.len();
        let capture_ms = milliseconds(total_started);
        let native_started = Instant::now();
        let active = AtomicUsize::new(0);
        let peak_active = AtomicUsize::new(0);
        let progress_samples = std::sync::Mutex::new(Vec::new());
        let unique_count = attempt.paths.len() as u32;
        let new_source_count = attempt.sources.len() as u32;
        let import_progress = NativeImportProgress::new(
            ImageProcessingBatch::new(unique_count, |event| {
                progress_samples.lock().unwrap().push(serde_json::json!({
                    "completed": event.completed_files,
                    "total": event.total_files,
                    "elapsedMs": milliseconds(total_started),
                    "activeJobs": active.load(Ordering::Acquire),
                    "problem": event.problem,
                }));
            }),
            &requests,
        );
        let completions = std::thread::scope(|scope| {
            let tasks = requests
                .into_iter()
                .map(|request| {
                    let (
                        engine,
                        processor,
                        app_paths,
                        executable,
                        data_root,
                        active,
                        peak_active,
                        import_progress,
                    ) = (
                        &engine,
                        &processor,
                        &app_paths,
                        &executable,
                        &data_root,
                        &active,
                        &peak_active,
                        &import_progress,
                    );
                    scope.spawn(move || {
                        let estimate = ImageMemoryEstimate::in_plan(
                            &request.root_bindings,
                            request.candidates.iter().map(PhotoImportCandidate::path),
                        );
                        tauri::async_runtime::block_on(async {
                            let cancellation = CacheCancellation::default();
                            let _permit = engine.begin_cancellable_work(cancellation.clone()).await;
                            let _reservation = processor
                                .reserve_cache_for(estimate, cancellation.flag())
                                .await
                                .unwrap();
                            let running = active.fetch_add(1, Ordering::AcqRel) + 1;
                            peak_active.fetch_max(running, Ordering::AcqRel);
                            let mut transport = RealProcessTransport::in_data_root(
                                executable.clone(),
                                data_root.clone(),
                            );
                            let (response, recovery) = engine
                                .invoke_cache_command(
                                    &mut transport,
                                    app_paths,
                                    &ImagingCommand::PreparePhotoImport(request.clone()),
                                    &InvocationContext::new(
                                        &request.request_id,
                                        Some(&request.project_id),
                                    ),
                                    InvocationControl::controlled(cancellation.flag(), &|event| {
                                        import_progress.report(event)
                                    }),
                                )
                                .await
                                .unwrap();
                            active.fetch_sub(1, Ordering::AcqRel);
                            assert!(recovery.is_none());
                            let ImagingResponse::PhotoImportCompleted {
                                request_id,
                                completion,
                            } = response
                            else {
                                panic!("typed completion")
                            };
                            assert_eq!(request_id, request.request_id);
                            completion.validate_for(&request).unwrap();
                            completion
                        })
                    })
                })
                .collect::<Vec<_>>();
            tasks
                .into_iter()
                .map(|task| task.join().unwrap())
                .collect::<Vec<_>>()
        });
        let native_ms = milliseconds(native_started);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(peak_active.load(Ordering::Acquire) <= processor.cache_capacity());
        if available_memory_mib.is_some() {
            assert_eq!(
                peak_active.load(Ordering::Acquire),
                1,
                "memory pressure keeps the real workers serial"
            );
        }
        assert!(!namespace.paths().metadata_file().exists());
        let outcomes: HashMap<_, _> = completions
            .into_iter()
            .flat_map(|completion| completion.photos)
            .map(|photo| (photo.source_id, photo.outcome))
            .collect();
        let inspection_started = Instant::now();
        let bindings = attempt.catalog.bindings.clone();
        let roots = attempt.roots.clone();
        let decoded_before = crate::media_runtime::photo_source_decode_count();
        import_progress.begin_inspection(outcomes.iter().filter_map(|(source, outcome)| {
            matches!(outcome, PhotoImportOutcome::Validated { .. }).then_some(source)
        }));
        active.fetch_add(1, Ordering::AcqRel);
        let prepared = prepare_proposal_with_inspection(
            attempt,
            Some(stage),
            outcomes,
            HashMap::new(),
            None,
            Vec::new(),
            |candidate| {
                let proposal = inspect_with_capacity(
                    kind,
                    &engine,
                    &processor,
                    candidate.path(),
                    &bindings,
                    &roots,
                );
                import_progress.complete_inspection(&candidate.source_id);
                proposal
            },
        )
        .unwrap();
        active.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(
            crate::media_runtime::photo_source_decode_count() - decoded_before,
            inputs.host_decodes
        );
        let inspection_ms = milliseconds(inspection_started);
        let commit_started = Instant::now();
        let committed =
            commit_prepared_import(&host, &engine, &monitor, &runtime, &registry, prepared)
                .unwrap();
        let commit_ms = milliseconds(commit_started);
        let mut progress = import_progress.finish(new_source_count);
        for (path, reason) in &committed.cache_problems {
            progress.report_problem(cache_problem(path, reason.clone()));
        }
        let progress_samples = progress_samples.into_inner().unwrap();
        assert_eq!(
            progress_samples
                .iter()
                .filter(|sample| sample["problem"].is_null())
                .map(|sample| sample["completed"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            (0..=u64::from(unique_count)).collect::<Vec<_>>(),
            "the real import must advance once per selected source"
        );
        assert_eq!(
            progress_samples
                .iter()
                .filter(|sample| !sample["problem"].is_null())
                .count(),
            inputs.imported - inputs.previews,
            "late Cache problems must be reported without counting a source twice: {:?}",
            committed
                .cache_problems
                .values()
                .take(3)
                .collect::<Vec<_>>()
        );
        assert!(
            progress_samples.iter().any(|sample| {
                let completed = sample["completed"].as_u64().unwrap();
                completed > 0
                    && completed < u64::from(unique_count)
                    && sample["activeJobs"].as_u64().unwrap() > 0
            }),
            "intermediate progress must arrive while native preparation or alternate inspection is still active (folder: {by_folder})"
        );
        assert!(committed.existing.is_empty());
        assert_eq!(committed.new_paths.len(), inputs.imported);
        assert_eq!(
            committed.cache_problems.len(),
            inputs.imported - inputs.previews
        );
        let ImportMediaResult::Completed {
            imported_count,
            problems,
            projection,
            ..
        } = committed.result
        else {
            panic!("completed import")
        };
        assert_eq!(imported_count as usize, inputs.imported);
        assert_eq!(problems.len(), inputs.rejected);
        assert_eq!(projection.state.revision, u64::from(inputs.imported > 0));
        assert!(
            projection
                .state
                .album
                .media
                .iter()
                .all(|media| media.kind == kind)
        );
        let catalog = host.authorized_media_catalog().unwrap();
        assert!(
            monitor
                .poll_in_plan(&runtime, &catalog.bindings, &roots)
                .update()
                .is_none()
        );
        let handoff_started = Instant::now();
        let sources = catalog.bindings.iter().map(|binding| {
            CacheMediaSource::new(
                binding.media_id.clone(),
                binding.kind,
                binding.logical_path.clone(),
            )
            .unwrap()
        });
        let works = engine
            .plan_works(&app_paths, &namespace, &roots, sources)
            .unwrap();
        let demand = engine.reconcile_preview_demand(
            &registry,
            namespace.project_id(),
            1,
            works.iter().map(|work| work.source.media_id()),
        );
        let previews =
            engine.publish_prepared_for_demand(&app_paths, &namespace, &registry, &demand, &works);
        assert_eq!(previews.len(), inputs.previews);
        let mut preview_hashes = Vec::new();
        for path in &committed.new_paths {
            let binding = catalog
                .bindings
                .iter()
                .find(|binding| &binding.logical_path == path)
                .unwrap();
            let Some(preview) = previews.get(&binding.media_id) else {
                continue;
            };
            let response = registry.serve(
                "import-flow",
                tauri::http::Request::builder()
                    .uri(preview.url.as_ref().unwrap())
                    .body(Vec::new())
                    .unwrap(),
            );
            assert_eq!(response.status(), 200);
            preview_hashes.push(format!("{:x}", Sha256::digest(response.body())));
        }
        let handoff_ms = milliseconds(handoff_started);
        let total_ms = milliseconds(total_started);
        let revisit_started = Instant::now();
        // Scroll through individual rows, including an empty observation between
        // distant rows. Returning must reuse the live URL without a cache job.
        for (index, binding) in catalog.bindings.iter().enumerate() {
            engine.reconcile_preview_demand(
                &registry,
                namespace.project_id(),
                index as u64 + 2,
                [binding.media_id.as_str()],
            );
        }
        engine.reconcile_preview_demand(
            &registry,
            namespace.project_id(),
            catalog.bindings.len() as u64 + 2,
            std::iter::empty(),
        );
        let returned = engine.reconcile_preview_demand(
            &registry,
            namespace.project_id(),
            catalog.bindings.len() as u64 + 3,
            catalog
                .bindings
                .iter()
                .map(|binding| binding.media_id.as_str()),
        );
        let mut reused_previews = 0;
        for binding in &catalog.bindings {
            let Some(previous) = previews.get(&binding.media_id) else {
                continue;
            };
            let reused = engine
                .commit_preview_if_demanded(&returned, &binding.media_id, || {
                    registry.retained_preview(
                        &binding.media_id,
                        &binding.logical_path,
                        crate::ipc_contract::MediaPreviewState::Ready,
                    )
                })
                .flatten()
                .unwrap();
            assert_eq!(reused.url, previous.url);
            reused_previews += 1;
        }
        assert_eq!(reused_previews, inputs.previews);
        assert_eq!(
            registry.presentation_snapshot(Vec::new()).len(),
            inputs.previews
        );
        let revisit_ms = milliseconds(revisit_started);
        if std::env::var_os("MYALBUNS_IMPORT_MEASUREMENT_OUTPUT").is_some() {
            eprintln!(
                "Import round {}/{}: {} photos, {:.2}s total, {:.2}s native, {} processes, peak {}",
                round + 1,
                rounds,
                inputs.imported,
                total_ms / 1000.0,
                native_ms / 1000.0,
                process_count,
                peak_active.load(Ordering::Acquire),
            );
        }
        if let Some(previous) = &previous_previews {
            assert_eq!(&preview_hashes, previous);
        }
        previous_previews = Some(preview_hashes.clone());
        assert_eq!(
            std::fs::read_dir(namespace.paths().media_directory())
                .unwrap()
                .count(),
            inputs.previews
        );
        assert_eq!(
            crate::media_runtime::photo_source_decode_count() - decoded_before,
            inputs.host_decodes
        );
        if inputs.imported > 0 {
            assert!(host.undo().unwrap().state.album.media.is_empty());
        }
        measurements.push(serde_json::json!({
            "round": round, "imported": inputs.imported, "previews": inputs.previews, "rejected": inputs.rejected,
            "capacity": processor.cache_capacity(), "peakActiveJobs": peak_active.load(Ordering::Acquire),
            "processes": process_count, "hostOriginalDecodes": inputs.host_decodes,
            "captureMs": capture_ms, "nativeMs": native_ms, "inspectionMs": inspection_ms,
            "commitMs": commit_ms, "firstDemandMs": handoff_ms, "totalMs": total_ms,
            "scrollRevisitMs": revisit_ms, "reusedPreviews": reused_previews,
            "previewSha256": preview_hashes, "progress": progress_samples
        }));
    }
    assert_eq!(
        inputs
            .paths
            .iter()
            .map(|path| digest(path))
            .collect::<Vec<_>>(),
        original_hashes
    );
    if let Some(output) = std::env::var_os("MYALBUNS_IMPORT_MEASUREMENT_OUTPUT") {
        let report = serde_json::json!({ "schemaVersion": 1, "protocolVersion": IMAGING_PROTOCOL_VERSION,
            "profile": "debug", "processorSha256": digest(&executable), "originalsUnchanged": true,
            "sourceSha256": original_hashes, "runs": measurements });
        std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
