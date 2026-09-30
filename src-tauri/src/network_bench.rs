//! Host + real Processor measurements with Originals on a network share, run
//! once per process so the caller can alternate builds (see
//! `.scratch/rede-20260929/tools/run-bench.ps1`). No WebView and no dialogs:
//! these are the native seams of each flow, not window latency.
//!
//! Environment: MYALBUNS_TEST_IMAGING_PROCESSOR, MYALBUNS_NET_SCENARIO
//! (`open`, `export`, `offline` or `watch`), MYALBUNS_NET_PHOTOS (folder of JPEGs),
//! MYALBUNS_NET_DESTINATION (export only), MYALBUNS_NET_INPUTS (offline and
//! watch: JSON list of Original paths on a share) and
//! MYALBUNS_NET_OUTPUT (JSON file).
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, DisplayUnit, EndSheetFormat, InitialProject,
    InitialProjectConfiguration, MediaKind, PhotoPlacementMode, ProjectCore, ProjectIntent,
    ProjectLocation,
};
use myalbuns_imaging_protocol::CacheMediaSource;
use myalbuns_paths::{AppPaths, ExportWriteAuthorization, OperationPathContext, RootBindingPlan};

use crate::imaging_processor::InvocationContext;
use crate::{
    cache_engine::{AuthorizedCacheNamespace, CacheEngine, CacheFlightClaim, CacheWork},
    export_pipeline,
    imaging_processor::ImagingProcessor,
    imaging_recovery_integration::RealProcessTransport,
    media_runtime::{MediaBinding, MediaMonitor, MediaRuntime},
    project_host::ProjectHost,
};

// SHIM-START: the calls that differ between the builds being compared.
mod shim {
    use super::*;
    use crate::linked_files::LinkedFiles;

    pub(super) const BUILD: &str = "current";

    pub(super) fn observe_all(roots: &RootBindingPlan, bindings: &[MediaBinding]) -> usize {
        LinkedFiles::new().observe(roots, bindings).len()
    }

    pub(super) fn header(roots: &RootBindingPlan, binding: &MediaBinding) -> bool {
        LinkedFiles::new()
            .header(roots, &binding.logical_path)
            .and_then(|header| header.photo_metadata())
            .is_ok()
    }

    /// What `prepare_owned_cache` does before reserving the Processor.
    pub(super) async fn before_cache(work: &CacheWork) -> Box<dyn std::any::Any + Send> {
        let files = LinkedFiles::new();
        let _estimate = crate::imaging_processor::ImageMemoryEstimate::from_headers(
            &files.headers(&work.root_bindings, [work.source.source_path()]),
        );
        // MYALBUNS_NET_REMOTE_TURNS=off measures the same build without P5.
        if std::env::var("MYALBUNS_NET_REMOTE_TURNS").as_deref() == Ok("off") {
            return Box::new(());
        }
        Box::new(
            files
                .remote_read_turn(&work.root_bindings, [work.source.source_path()], || false)
                .await,
        )
    }
}
// SHIM-END

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn photos() -> Vec<PathBuf> {
    let mut photos = std::fs::read_dir(env_path("MYALBUNS_NET_PHOTOS"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    photos.sort();
    photos
}

/// Dimensions after EXIF orientation, from the first MiB of the file only.
fn presented_dimensions(path: &Path) -> (u32, u32) {
    use image::ImageDecoder;
    use std::io::Read;
    let mut prefix = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(1 << 20)
        .read_to_end(&mut prefix)
        .unwrap();
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(prefix))
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    let (width, height) = decoder.dimensions();
    if matches!(
        decoder.orientation().unwrap(),
        image::metadata::Orientation::Rotate90
            | image::metadata::Orientation::Rotate270
            | image::metadata::Orientation::Rotate90FlipH
            | image::metadata::Orientation::Rotate270FlipH
    ) {
        (height, width)
    } else {
        (width, height)
    }
}

fn capture(paths: impl IntoIterator<Item = PathBuf>) -> RootBindingPlan {
    let mut context = OperationPathContext::new();
    for path in paths {
        let _ = context.capture(&path);
    }
    context.freeze()
}

fn in_parallel<T: Sync>(items: &[T], workers: usize, work: impl Fn(&T) + Sync) {
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.min(items.len()).max(1) {
            scope.spawn(|| {
                while let Some(item) = items.get(next.fetch_add(1, Ordering::Relaxed)) {
                    work(item);
                }
            });
        }
    });
}

fn write_report(report: serde_json::Value) {
    let output = env_path("MYALBUNS_NET_OUTPUT");
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!("{report}");
}

#[test]
#[ignore = "network measurement; run by run-bench.ps1"]
fn network_measurement() {
    match std::env::var("MYALBUNS_NET_SCENARIO").as_deref() {
        Ok("open") => open_scenario(),
        Ok("export") => export_scenario(),
        Ok("offline") => offline_scenario(),
        Ok("watch") => watch_scenario(),
        other => panic!("unknown scenario {other:?}"),
    }
}

/// First opening without Cache, reopening with Cache and one Monitor sweep,
/// following the order of `product_runtime` and `image_processing`.
fn open_scenario() {
    let executable = env_path("MYALBUNS_TEST_IMAGING_PROCESSOR");
    let root = tempfile::tempdir().unwrap();
    let data_root = root.path().join("data");
    for name in ["Roaming", "Local", "logs"] {
        std::fs::create_dir_all(data_root.join(name)).unwrap();
    }
    let app_paths = AppPaths::from_roots(&data_root.join("Roaming"), &data_root.join("Local"));
    let project_path = root.path().join("Project.myalbuns");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path.clone(), capture([project_path])),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let namespace =
        AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority()).unwrap();
    let bindings = photos()
        .into_iter()
        .map(|logical_path| MediaBinding {
            media_id: uuid::Uuid::new_v4().hyphenated().to_string(),
            kind: MediaKind::Photo,
            logical_path,
        })
        .collect::<Vec<_>>();
    let processor = ImagingProcessor::with_available_memory_for_test(16 * 1024, 16 * 1024);
    let capacity = std::env::var("MYALBUNS_NET_WORKERS")
        .ok()
        .map_or(processor.cache_capacity(), |value| value.parse().unwrap());
    let engine = CacheEngine::default();
    let monitor = MediaMonitor::default();
    let runtime = MediaRuntime::default();

    let started = Instant::now();
    let roots = capture(
        std::iter::once(namespace.paths().root().to_path_buf())
            .chain(bindings.iter().map(|binding| binding.logical_path.clone())),
    );
    // images_requiring_startup_preparation
    let phase = Instant::now();
    assert_eq!(shim::observe_all(&roots, &bindings), bindings.len());
    let startup_observation = phase.elapsed();
    // image_processing: two samples stabilize the Monitor
    let phase = Instant::now();
    monitor.prepare_in_plan(&runtime, &bindings, &roots);
    assert!(
        monitor
            .prepare_in_plan(&runtime, &bindings, &roots)
            .is_some()
    );
    let stabilization = phase.elapsed();
    // media_confirmation: the header of every Original the Processor decodes next
    let phase = Instant::now();
    let headers = AtomicUsize::new(0);
    in_parallel(&bindings, capacity, |binding| {
        if shim::header(&roots, binding) {
            headers.fetch_add(1, Ordering::Relaxed);
        }
    });
    assert_eq!(headers.load(Ordering::Relaxed), bindings.len());
    let header_confirmation = phase.elapsed();
    // Cache jobs: estimate, turn and the real Processor, then publication
    let phase = Instant::now();
    let works = engine
        .plan_works(
            &app_paths,
            &namespace,
            &roots,
            bindings.iter().map(|binding| {
                CacheMediaSource::new(
                    binding.media_id.clone(),
                    binding.kind,
                    binding.logical_path.clone(),
                )
                .unwrap()
            }),
        )
        .unwrap();
    // As in ImageProcessingBatch: claim every flight, prepare, then publish
    // the batch; publication keeps only the candidates of claimed flights.
    let owners = works
        .iter()
        .map(|work| match engine.claim_for_processing(work) {
            CacheFlightClaim::Owner(owner) => owner,
            CacheFlightClaim::Waiter(_) => panic!("each Original is claimed once"),
        })
        .collect::<Vec<_>>();
    let jobs = works
        .iter()
        .zip(&owners)
        .enumerate()
        .map(|(index, (work, owner))| (index, work, owner.cancellation()))
        .collect::<Vec<_>>();
    let completions = Mutex::new(Vec::new());
    let pendings = Mutex::new(Vec::new());
    in_parallel(&jobs, capacity, |(index, work, cancellation)| {
        tauri::async_runtime::block_on(async {
            let _turn = shim::before_cache(work).await;
            let mut transport =
                RealProcessTransport::in_data_root(executable.clone(), data_root.clone());
            let pending = engine
                .prepare(
                    &mut transport,
                    &app_paths,
                    (*work).clone(),
                    &InvocationContext::new(&work.request_id, Some(work.namespace.project_id())),
                    cancellation,
                )
                .await
                .unwrap();
            completions.lock().unwrap().push(ms(started.elapsed()));
            pendings.lock().unwrap().push((*index, pending));
        });
    });
    let mut pendings = pendings.into_inner().unwrap();
    pendings.sort_by_key(|(index, _)| *index);
    let results =
        engine.publish_prepared_batch(pendings.into_iter().map(|(_, pending)| pending).collect());
    for (owner, result) in owners.into_iter().zip(results) {
        owner.complete(result).unwrap();
    }
    let cache_jobs = phase.elapsed();
    let first_open = started.elapsed();
    let mut completions = completions.into_inner().unwrap();
    completions.sort_by(f64::total_cmp);

    // Reopening with Cache: recovered previews, then adopted inspections.
    std::thread::sleep(Duration::from_secs(32));
    let phase = Instant::now();
    shim::observe_all(&roots, &bindings);
    let reopen_recovery = phase.elapsed();
    let phase = Instant::now();
    shim::observe_all(&roots, &bindings);
    let reopen_adoption = phase.elapsed();

    // One periodic Monitor sweep, 30 s after the previous look.
    std::thread::sleep(Duration::from_secs(32));
    let phase = Instant::now();
    monitor.prepare_in_plan(&runtime, &bindings, &roots);
    let monitor_sweep = phase.elapsed();

    write_report(serde_json::json!({
        "scenario": "open", "build": shim::BUILD, "photos": bindings.len(),
        "remote": roots.is_remote_for_bench(&bindings[0].logical_path), "capacity": capacity,
        "firstOpenMs": ms(first_open),
        "startupObservationMs": ms(startup_observation),
        "stabilizationMs": ms(stabilization),
        "headerConfirmationMs": ms(header_confirmation),
        "cacheJobsMs": ms(cache_jobs),
        "firstPreviewMs": completions.first(),
        "tenthPreviewMs": completions.get(9),
        "reopenRecoveryMs": ms(reopen_recovery),
        "reopenAdoptionMs": ms(reopen_adoption),
        "monitorSweepMs": ms(monitor_sweep),
    }));
}

trait RemoteForBench {
    fn is_remote_for_bench(&self, path: &Path) -> bool;
}

impl RemoteForBench for RootBindingPlan {
    fn is_remote_for_bench(&self, path: &Path) -> bool {
        path.to_string_lossy().starts_with(r"\\")
    }
}

/// Export of six 600 × 300 mm Sheets at 300 DPI, five Photos each, as JPEG.
fn export_scenario() {
    let executable = env_path("MYALBUNS_TEST_IMAGING_PROCESSOR");
    let destination_parent = env_path("MYALBUNS_NET_DESTINATION");
    let root = tempfile::tempdir().unwrap();
    let project_path = root.path().join("Exportacao.myalbuns");
    let project = ProjectCore::new()
        .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
        .create_editable(CreateProjectRequest::new(
            ProjectLocation::new(project_path.clone(), capture([project_path])),
            InitialProject::configured(InitialProjectConfiguration::new(
                DisplayUnit::Mm,
                600_000,
                300_000,
                300,
                0,
                0,
                6,
                EndSheetFormat::Double,
                EndSheetFormat::Double,
            )),
            CreateAuthorization::CreateOnly,
        ))
        .unwrap();
    let host = ProjectHost::new(project);
    // Only each header is read here, so the measured export still finds its
    // Originals cold on the share, as a real export after opening would.
    let commands = photos()
        .into_iter()
        .take(30)
        .map(|path| {
            let (width, height) = presented_dimensions(&path);
            myalbuns_core::ImportPhoto::new(
                path,
                myalbuns_core::PhotoSourceMetadata::new(
                    width,
                    height,
                    ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
                )
                .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let project_id = host.projection().unwrap().state.project_id;
    let crate::ipc_contract::ImportMediaResult::Completed { media_ids, .. } = host
        .commit_photo_import_proposal(
            &project_id,
            crate::media_runtime::PhotoImportsProposal {
                kind: MediaKind::Photo,
                commands,
                problems: Vec::new(),
                operation_problem: None,
                inspections: Vec::new(),
            },
        )
        .unwrap()
    else {
        panic!("the Photos are imported")
    };
    let sheet_ids = host.projection().unwrap().state.album.sheets[..6]
        .iter()
        .map(|sheet| sheet.id.clone())
        .collect::<Vec<_>>();
    for (index, media_id) in media_ids.iter().enumerate() {
        host.apply_with_outcome(ProjectIntent::AddPhoto {
            sheet_id: sheet_ids[index / 5].clone(),
            media_id: media_id.parse().unwrap(),
            mode: PhotoPlacementMode::Normal,
        })
        .unwrap();
    }
    let destination = destination_parent.join(format!(
        "export-{}-{}",
        shim::BUILD,
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&destination).unwrap();

    let started = Instant::now();
    let problems = crate::export_media::inspect_selection(&host, &sheet_ids).unwrap();
    assert!(problems.is_empty());
    let precheck = started.elapsed();
    let (snapshot, sources) = host.freeze_export(&sheet_ids).unwrap();
    let request_id = format!("export-{}", uuid::Uuid::new_v4());
    let planned = export_pipeline::plan_album(
        snapshot,
        export_pipeline::AlbumExportOptions {
            request_id: request_id.clone(),
            destination: destination.clone(),
            authorization: ExportWriteAuthorization::CreateOnly,
            sheet_ids: sheet_ids.clone(),
            sources,
            protected_originals: vec![],
            whole_album: false,
            mode: myalbuns_core::ExportMode::Sheet,
            format: myalbuns_core::ExportFormat::Jpeg { quality: 100 },
        },
    )
    .unwrap();
    let log_directory = root.path().join("processor-logs");
    std::fs::create_dir(&log_directory).unwrap();
    let execution = Instant::now();
    let published = tauri::async_runtime::block_on(async {
        let roots = crate::path_io::capture_root_bindings(planned.required_paths())
            .await
            .unwrap();
        let mut transport = RealProcessTransport::stable(executable, log_directory);
        export_pipeline::execute_album(
            &mut transport,
            planned,
            &roots,
            &export_pipeline::ExportExecutionControl::default(),
            &|_| {},
            &InvocationContext::new(&request_id, None::<&str>),
        )
        .await
        .unwrap()
    });
    let execute = execution.elapsed();
    let total = started.elapsed();
    let outputs = std::fs::read_dir(&destination)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .collect::<Vec<_>>();
    std::fs::remove_dir_all(&destination).unwrap();
    let _ = published;

    write_report(serde_json::json!({
        "scenario": "export", "build": shim::BUILD,
        "remoteDestination": destination.to_string_lossy().starts_with(r"\\"),
        "sheets": sheet_ids.len(), "photos": media_ids.len(),
        "outputs": outputs.len(), "outputBytes": outputs.iter().sum::<u64>(),
        "precheckMs": ms(precheck), "executeMs": ms(execute), "totalMs": ms(total),
    }));
}

fn offline_bindings() -> Vec<MediaBinding> {
    let inputs: Vec<PathBuf> =
        serde_json::from_slice(&std::fs::read(env_path("MYALBUNS_NET_INPUTS")).unwrap()).unwrap();
    inputs
        .into_iter()
        .map(|logical_path| MediaBinding {
            media_id: uuid::Uuid::new_v4().hyphenated().to_string(),
            kind: MediaKind::Photo,
            logical_path,
        })
        .collect()
}

fn availability(observations: &[crate::media_runtime::MediaObservation]) -> serde_json::Value {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for observation in observations {
        *counts
            .entry(format!("{:?}", observation.availability))
            .or_default() += 1;
    }
    serde_json::json!(counts)
}

/// Originals on a server that is switched off, in a new process. Windows
/// waits for the server once and then remembers the failure for 20–30 s, so
/// each later step starts MYALBUNS_NET_GAP seconds after the previous one,
/// past that memory.
fn offline_scenario() {
    use crate::{cache_activity_gate::CacheCancellation, linked_files::LinkedFiles};
    let gap = Duration::from_secs(
        std::env::var("MYALBUNS_NET_GAP").map_or(45, |value| value.parse().unwrap()),
    );
    let bindings = offline_bindings();
    let files = LinkedFiles::new();
    let mut report = serde_json::Map::new();
    let started = Instant::now();
    let roots = capture(bindings.iter().map(|binding| binding.logical_path.clone()));
    report.insert(
        "captureMs".to_owned(),
        serde_json::json!(ms(started.elapsed())),
    );

    // The first contact with the switched-off server happens inside a Monitor
    // observation that holds Cache work; a Cache pause (Export, Save As) is
    // asked 0.5 s later.
    let engine = CacheEngine::default();
    let monitor = MediaMonitor::default();
    let runtime = MediaRuntime::default();
    let (waited, tick) = tauri::async_runtime::block_on(async {
        let cancellation = CacheCancellation::default();
        let permit = engine.begin_cancellable_work(cancellation.clone()).await;
        let (roots, bindings, monitor, runtime) = (
            roots.clone(),
            bindings.clone(),
            monitor.clone(),
            runtime.clone(),
        );
        let tick_started = Instant::now();
        let tick = tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let _ =
                monitor.prepare_in_plan_unless(&runtime, &bindings, &roots, cancellation.flag());
            tick_started.elapsed()
        });
        tokio::time::sleep(Duration::from_millis(500)).await;
        let asked = Instant::now();
        let _pause = engine.pause().await;
        let waited = asked.elapsed();
        drop(_pause);
        (waited, tick.await.unwrap())
    });
    eprintln!(
        "pauseDuringFirstContact: {:.0} ms (tick {:.0} ms)",
        ms(waited),
        ms(tick)
    );
    report.insert(
        "pauseDuringFirstContact".to_owned(),
        serde_json::json!({ "ms": ms(waited), "tickMs": ms(tick) }),
    );

    let mut step = |name: &str, work: &mut dyn FnMut() -> serde_json::Value| {
        std::thread::sleep(gap);
        let started = Instant::now();
        let detail = work();
        let elapsed = ms(started.elapsed());
        eprintln!("{name}: {elapsed:.0} ms {detail}");
        report.insert(
            name.to_owned(),
            serde_json::json!({ "ms": elapsed, "detail": detail }),
        );
    };
    step("observeOne", &mut || {
        availability(&[files.observe_one(&roots, &bindings[0])])
    });
    step("observeAll", &mut || {
        availability(&files.observe(&roots, &bindings))
    });
    step("listingHint", &mut || {
        crate::media_runtime::MediaListingHint::read(&files, &roots, &bindings);
        serde_json::Value::Null
    });
    step("monitorStabilization", &mut || {
        monitor.prepare_in_plan(&runtime, &bindings, &roots);
        let proposal = monitor.prepare_in_plan(&runtime, &bindings, &roots);
        serde_json::json!(proposal.is_some())
    });
    report.insert("scenario".to_owned(), serde_json::json!("offline"));
    report.insert("photos".to_owned(), serde_json::json!(bindings.len()));
    report.insert("gapS".to_owned(), serde_json::json!(gap.as_secs()));
    write_report(serde_json::Value::Object(report));
}

/// Looks at the Originals once a second, like the Monitor (listing, then
/// observation), for MYALBUNS_NET_WATCH_S seconds, and logs every tick that
/// changes or takes over a second: whether the Originals are all available
/// with the server on, how long a tick takes while it is off, and how soon the
/// Originals come back once it answers again. Stops once they came back.
fn watch_scenario() {
    use crate::linked_files::LinkedFiles;
    let limit = Duration::from_secs(
        std::env::var("MYALBUNS_NET_WATCH_S").map_or(600, |value| value.parse().unwrap()),
    );
    let bindings = offline_bindings();
    let files = LinkedFiles::new();
    let roots = capture(bindings.iter().map(|binding| binding.logical_path.clone()));
    let started = Instant::now();
    let mut ticks = Vec::new();
    let mut last = serde_json::Value::Null;
    let mut seen_unavailable = false;
    while started.elapsed() < limit {
        let tick = Instant::now();
        crate::media_runtime::MediaListingHint::read(&files, &roots, &bindings);
        let listing = tick.elapsed();
        let counts = availability(&files.observe(&roots, &bindings));
        let elapsed = tick.elapsed();
        let all_available = counts.get("Candidate").and_then(serde_json::Value::as_u64)
            == Some(bindings.len() as u64);
        if counts != last || elapsed > Duration::from_secs(1) {
            let at = started.elapsed().as_secs_f64();
            eprintln!(
                "t={at:.1} s listing {:.0} ms tick {:.0} ms {counts}",
                ms(listing),
                ms(elapsed)
            );
            ticks.push(serde_json::json!({
                "atS": at, "listingMs": ms(listing), "tickMs": ms(elapsed), "availability": counts,
            }));
        }
        seen_unavailable |= !all_available;
        if seen_unavailable && all_available {
            break;
        }
        last = counts;
        std::thread::sleep(Duration::from_secs(1));
    }
    write_report(serde_json::json!({
        "scenario": "watch", "photos": bindings.len(), "ticks": ticks,
    }));
}
