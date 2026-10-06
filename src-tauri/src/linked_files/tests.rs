use std::{
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use image::{ImageFormat, Rgb, RgbImage};
use myalbuns_core::MediaKind;
use myalbuns_paths::{OperationPathContext, ResolveError};

use super::{
    CONCURRENCY, LinkedFiles, ListFolderError, MediaAvailability, SourceFormat, map_concurrently,
    map_concurrently_unless, read_source,
};
use crate::media_runtime::MediaBinding;

struct Counting {
    inner: Cursor<Vec<u8>>,
    read: usize,
}

impl Read for Counting {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.read += read;
        Ok(read)
    }
}

impl Seek for Counting {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

/// A noisy 480 × 320 JPEG rotated 90° by EXIF, so its body dwarfs its header.
fn rotated_jpeg() -> Vec<u8> {
    // Little-endian TIFF with one Orientation entry: 6 (rotate 90).
    let exif = vec![
        b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    ];
    let (width, height) = (480, 320);
    let pixels = (0..width * height * 3)
        .map(|index| (index * 7 % 251) as u8)
        .collect::<Vec<_>>();
    let mut jpeg = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95);
    image::ImageEncoder::set_exif_metadata(&mut encoder, exif).unwrap();
    image::ImageEncoder::write_image(
        encoder,
        &pixels,
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
    jpeg
}

fn encoded(format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    RgbImage::from_pixel(37, 23, Rgb([20, 80, 160]))
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn a_header_reads_only_the_jpeg_header_and_agrees_with_a_full_decode() {
    let jpeg = rotated_jpeg();
    let read = |decode_pixels| {
        let mut counting = Counting {
            inner: Cursor::new(jpeg.clone()),
            read: 0,
        };
        let header = read_source(
            BufReader::new(&mut counting),
            jpeg.len() as u64,
            decode_pixels,
        )
        .unwrap();
        (header, counting.read)
    };
    let (header, header_bytes) = read(false);
    let (decoded, _) = read(true);
    assert_eq!(header, decoded);
    assert_eq!(header.format, SourceFormat::Jpeg);
    assert!(header.sequential_color_jpeg);
    assert_eq!(
        (header.width, header.height),
        (320, 480),
        "the EXIF rotation swaps the presented dimensions"
    );
    assert!(
        header_bytes * 4 < jpeg.len(),
        "read {header_bytes} of {} bytes",
        jpeg.len()
    );
}

#[test]
fn a_header_still_judges_png_tiff_and_foreign_bytes() {
    for (format, expected) in [
        (ImageFormat::Png, SourceFormat::Png),
        (ImageFormat::Tiff, SourceFormat::Tiff),
    ] {
        let bytes = encoded(format);
        let header = read_source(Cursor::new(bytes.clone()), bytes.len() as u64, false).unwrap();
        assert_eq!(header.format, expected);
        assert_eq!((header.width, header.height), (37, 23));
        assert!(!header.sequential_color_jpeg);
    }
    assert!(read_source(Cursor::new(b"GIF89a unsupported image".to_vec()), 24, false).is_err());
}

#[test]
fn work_overlaps_and_keeps_the_input_order() {
    let active = AtomicUsize::new(0);
    let peak = AtomicUsize::new(0);
    let first_pair = std::sync::Barrier::new(2);
    let mut progress = Vec::new();
    let results = map_concurrently(
        vec![0, 1, 2, 3, 4],
        |index| {
            let count = active.fetch_add(1, Ordering::AcqRel) + 1;
            peak.fetch_max(count, Ordering::AcqRel);
            if index < 2 {
                first_pair.wait();
            }
            active.fetch_sub(1, Ordering::AcqRel);
            if index == 1 { Err(index) } else { Ok(index) }
        },
        |completed| progress.push(completed),
    );
    assert!(peak.load(Ordering::Acquire) >= 2);
    assert_eq!(results, [Ok(0), Err(1), Ok(2), Ok(3), Ok(4)]);
    assert_eq!(progress, [1, 2, 3, 4, 5]);
}

#[test]
fn interrupted_work_answers_at_once_and_starts_no_further_item() {
    let interrupted = AtomicBool::new(false);
    let started = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let item = Duration::from_millis(400);
    let (result, answered) = std::thread::scope(|scope| {
        // Interrupt only once every worker holds a running item.
        let interruption = scope.spawn(|| {
            while started.load(Ordering::Acquire) < CONCURRENCY {
                std::thread::yield_now();
            }
            interrupted.store(true, Ordering::Release);
            Instant::now()
        });
        let (started, finished) = (Arc::clone(&started), Arc::clone(&finished));
        let result = map_concurrently_unless((0..400).collect(), &interrupted, move |index| {
            started.fetch_add(1, Ordering::AcqRel);
            std::thread::sleep(item);
            finished.fetch_add(1, Ordering::AcqRel);
            index
        });
        (result, interruption.join().unwrap().elapsed())
    });
    assert_eq!(result, None);
    assert!(
        answered < item / 2,
        "answered {answered:?} after the interruption, while the running items take {item:?}"
    );
    while finished.load(Ordering::Acquire) < CONCURRENCY {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        started.load(Ordering::Acquire),
        CONCURRENCY,
        "the running items finish on their own and no further one starts"
    );
    assert_eq!(
        map_concurrently_unless(vec![1, 2, 3], &AtomicBool::new(false), |item| item * 2),
        Some(vec![2, 4, 6])
    );
}

fn photos(
    count: usize,
) -> (
    tempfile::TempDir,
    Vec<MediaBinding>,
    myalbuns_paths::RootBindingPlan,
) {
    let root = tempfile::tempdir().unwrap();
    let bindings = (0..count)
        .map(|index| {
            let logical_path = root.path().join(format!("{index}.jpg"));
            std::fs::write(&logical_path, b"photo").unwrap();
            MediaBinding {
                media_id: format!("photo-{index}"),
                kind: MediaKind::Photo,
                logical_path,
            }
        })
        .collect::<Vec<_>>();
    let mut paths = OperationPathContext::new();
    paths.capture(root.path()).unwrap();
    (root, bindings, paths.freeze())
}

#[test]
fn a_slow_share_is_observed_several_files_at_once_in_the_given_order() {
    let (_root, bindings, plan) = photos(32);
    let latency = Duration::from_millis(40);
    let started = Instant::now();
    let observations = LinkedFiles::with_latency(latency).observe(&plan, &bindings);
    let elapsed = started.elapsed();

    assert_eq!(
        observations
            .iter()
            .map(|observation| observation.media_id.as_str())
            .collect::<Vec<_>>(),
        bindings
            .iter()
            .map(|binding| binding.media_id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        observations
            .iter()
            .all(|observation| observation.availability == MediaAvailability::Candidate)
    );
    assert!(
        elapsed < latency * 32 / 2,
        "32 accesses of {latency:?} took {elapsed:?}"
    );
}

#[test]
fn an_interrupted_observation_does_not_wait_for_a_slow_access() {
    let (_root, bindings, plan) = photos(64);
    // Like a server that is switched off: each access waits a long time.
    let latency = Duration::from_millis(800);
    let files = LinkedFiles::with_latency(latency);
    let interrupted = AtomicBool::new(false);
    let started = Instant::now();
    let observed = std::thread::scope(|scope| {
        let observing = scope.spawn(|| files.observe_unless(&plan, &bindings, &interrupted));
        std::thread::sleep(Duration::from_millis(45));
        interrupted.store(true, Ordering::Release);
        observing.join().unwrap()
    });
    let elapsed = started.elapsed();
    assert_eq!(observed, None);
    assert!(
        elapsed < latency / 2,
        "answered after {elapsed:?}, while one access takes {latency:?}"
    );
}

static SHARE_ANSWERS: AtomicBool = AtomicBool::new(false);

#[test]
fn a_share_that_stopped_answering_fails_at_once_until_it_answers_again() {
    let latency = Duration::from_secs(5);
    let files = LinkedFiles::with_unreachable_roots(
        |_| SHARE_ANSWERS.load(Ordering::Acquire),
        Duration::from_millis(10),
        latency,
    );
    let shared = PathBuf::from(r"\\servidor\Fotos\Cliente\a.jpg");
    let mut paths = OperationPathContext::new();
    paths
        .capture_with_binding(&shared, Path::new(r"\\servidor\Fotos\"))
        .unwrap();
    let plan = paths.freeze();
    let root = plan.remote_root(&shared).unwrap().to_path_buf();
    // What an access that found the server switched off leaves behind.
    files.unreachable.remember(&root);
    let binding = MediaBinding {
        media_id: "photo".into(),
        kind: MediaKind::Photo,
        logical_path: shared.clone(),
    };

    let started = Instant::now();
    assert_eq!(
        files.observe_one(&plan, &binding).availability,
        MediaAvailability::Unavailable
    );
    assert!(files.header(&plan, &shared).is_err());
    assert!(matches!(
        files.list_folder(&plan, Path::new(r"\\servidor\Fotos\Cliente")),
        Err(ListFolderError::Resolve(ResolveError::Unavailable))
    ));
    assert!(
        started.elapsed() < latency,
        "no access waited for the server"
    );

    SHARE_ANSWERS.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(5);
    while files.unreachable.contains(&root) {
        assert!(
            Instant::now() < deadline,
            "the probe finds the server again"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

static UNANSWERED_PROBES: AtomicUsize = AtomicUsize::new(0);

#[test]
fn an_access_that_finds_a_share_unreachable_remembers_its_root() {
    let files = LinkedFiles::with_unreachable_roots(
        |_| {
            UNANSWERED_PROBES.fetch_add(1, Ordering::AcqRel);
            false
        },
        Duration::from_secs(3600),
        Duration::ZERO,
    );
    // The loopback address refuses a share that does not exist at once,
    // without leaving this computer.
    let share = format!(r"\\127.0.0.1\myalbuns-{}\", uuid::Uuid::new_v4().simple());
    let shared = PathBuf::from(format!(r"{share}Cliente\a.jpg"));
    let mut paths = OperationPathContext::new();
    paths
        .capture_with_binding(&shared, Path::new(&share))
        .unwrap();
    let plan = paths.freeze();
    let root = plan.remote_root(&shared).unwrap().to_path_buf();
    let binding = MediaBinding {
        media_id: "photo".into(),
        kind: MediaKind::Photo,
        logical_path: shared.clone(),
    };
    assert!(!files.unreachable.contains(&root));

    assert_eq!(
        files.observe_one(&plan, &binding).availability,
        MediaAvailability::Unavailable
    );
    assert!(
        files.unreachable.contains(&root),
        "the access itself records that the server did not answer"
    );

    // The same memory behind a slow share: an access that reached the
    // server again would wait for it.
    let latency = Duration::from_secs(5);
    let slow = LinkedFiles {
        unreachable: files.unreachable,
        latency: Some(latency),
    };
    let started = Instant::now();
    assert_eq!(
        slow.observe_one(&plan, &binding).availability,
        MediaAvailability::Unavailable
    );
    assert!(matches!(
        slow.list_folder(&plan, shared.parent().unwrap()),
        Err(ListFolderError::Resolve(ResolveError::Unavailable))
    ));
    assert!(
        started.elapsed() < latency,
        "no later access waited for the server"
    );
    assert_eq!(
        UNANSWERED_PROBES.load(Ordering::Acquire),
        0,
        "the root is probed again only after the probe interval"
    );
}

#[test]
fn a_local_root_that_disappears_is_never_remembered_as_unreachable() {
    let files =
        LinkedFiles::with_unreachable_roots(|_| false, Duration::from_secs(60), Duration::ZERO);
    let (root, bindings, plan) = photos(1);
    drop(root);
    assert_ne!(
        files.observe_one(&plan, &bindings[0]).availability,
        MediaAvailability::Candidate
    );
    assert!(
        files.unreachable.roots.lock().unwrap().is_empty(),
        "a local disk answers at once; only a network root is remembered"
    );
}

#[test]
fn a_folder_lists_its_entries_without_following_anything_but_links() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.jpg"), b"12345").unwrap();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    let mut paths = OperationPathContext::new();
    paths.capture(root.path()).unwrap();
    let plan = paths.freeze();
    let files = LinkedFiles::new();

    let listed = files.list_folder(&plan, root.path()).unwrap();
    let mut entries = listed
        .entries
        .iter()
        .map(|entry| {
            (
                entry.name.to_string_lossy().into_owned(),
                entry.directory,
                entry.link,
                entry.bytes,
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        [
            ("a.jpg".to_owned(), false, false, 5),
            ("sub".to_owned(), true, false, entries[1].3)
        ]
    );
    assert!(listed.physical_identity().is_some());

    let results = files.list_folders(
        &plan,
        [
            root.path().join("a.jpg").as_path(),
            root.path().join("sub").as_path(),
        ],
    );
    assert!(matches!(
        results[0],
        Err(super::ListFolderError::Resolve(_))
    ));
    assert!(
        results[1]
            .as_ref()
            .is_ok_and(|folder| folder.entries.is_empty())
    );
}

/// A junction redirects a folder without the privilege a symbolic link needs.
#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) {
    let output = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
#[test]
fn a_listed_link_is_marked_and_followed_and_a_broken_one_does_not_fail_the_folder() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Fotos");
    let target = root.path().join("destino");
    let removed = root.path().join("removido");
    for directory in [&folder, &target, &removed] {
        std::fs::create_dir(directory).unwrap();
    }
    std::fs::write(folder.join("a.jpg"), b"12345").unwrap();
    create_junction(&folder.join("redirecionada"), &target);
    create_junction(&folder.join("quebrada"), &removed);
    std::fs::remove_dir(&removed).unwrap();
    let mut paths = OperationPathContext::new();
    paths.capture(&folder).unwrap();

    let listed = LinkedFiles::new()
        .list_folder(&paths.freeze(), &folder)
        .unwrap();
    let mut entries = listed
        .entries
        .iter()
        .map(|entry| {
            (
                entry.name.to_string_lossy().into_owned(),
                entry.link,
                entry.directory,
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        [
            ("a.jpg".to_owned(), false, false),
            // Nothing answers behind the broken link, so it is not a folder.
            ("quebrada".to_owned(), true, false),
            ("redirecionada".to_owned(), true, true),
        ]
    );
}

#[test]
fn full_reads_from_a_share_take_turns_in_the_order_asked() {
    let shared = std::path::PathBuf::from(r"\\servidor\Fotos\Cliente\a.jpg");
    let local_root = tempfile::tempdir().unwrap();
    let local = local_root.path().join("a.jpg");
    let mut paths = OperationPathContext::new();
    paths
        .capture_with_binding(&shared, std::path::Path::new(r"\\servidor\Fotos\"))
        .unwrap();
    paths.capture(&local).unwrap();
    let plan = paths.freeze();
    let files = LinkedFiles::new();

    assert_eq!(files.full_read_capacity(&plan, [shared.as_path()], 8), 3);
    assert_eq!(files.full_read_capacity(&plan, [local.as_path()], 8), 8);
    assert_eq!(files.full_read_capacity(&plan, [shared.as_path()], 2), 2);

    tauri::async_runtime::block_on(async {
        let turn = || files.remote_read_turn(&plan, [shared.as_path()], || false);
        let first = turn().await.unwrap();
        let _second = turn().await.unwrap();
        let _third = turn().await.unwrap();
        let waiting = turn();
        tokio::pin!(waiting);
        assert!(
            tokio::time::timeout(Duration::from_millis(250), &mut waiting)
                .await
                .is_err(),
            "a fourth reader waits for a turn"
        );
        assert!(
            files
                .remote_read_turn(&plan, [shared.as_path()], || true)
                .await
                .is_none(),
            "an abandoned request gives up its place"
        );
        let mut local_turns = Vec::new();
        for _ in 0..8 {
            local_turns.push(
                files
                    .remote_read_turn(&plan, [local.as_path()], || false)
                    .await
                    .unwrap(),
            );
        }
        drop(first);
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .expect("the first waiting reader gets the released turn")
            .unwrap();
    });
}
