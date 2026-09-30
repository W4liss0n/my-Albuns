//! Every Host access to Arquivos vinculados and to the folders they come from.
//!
//! Callers ask for what they need, many entries at a time, and never decide
//! whether a file sits on a local disk or a network share. On a share each
//! access waits several round trips for the server and every byte crosses the
//! network, so this module owns the policies that make that affordable:
//! observing and reading headers several files at once, reading a JPEG only up
//! to its first scan, listing folders without one more request per entry,
//! failing at once under a server that stopped answering, and timing each
//! batch for the diagnostic log. Fix one of those here and every flow gets it.
//!
//! The Processor reads Originals in its own process under the frozen
//! `RootBindingPlan`; it shares only the pure header functions of
//! `myalbuns-imaging`.
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Cursor, Read, Seek},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation};
use myalbuns_core::{MediaKind, PhotoSourceMetadata};
use myalbuns_imaging::{
    source_header::{JPEG_HEADER_PREFIX_LIMIT, jpeg_header_prefix},
    source_memory::is_sequential_color_jpeg,
};
use myalbuns_paths::{
    ExpectedObject, PhysicalFileIdentity, ResolveError, ResolvedObject, RootBindingPlan,
};

use crate::media_runtime::MediaBinding;

/// How many accesses run at once. Measured on a Wi‑Fi share on 2026-09-29:
/// observing 90 Originals took 4.3–4.5 s one at a time and 1.2–1.35 s eight at
/// a time; local disks showed no loss.
const CONCURRENCY: usize = crate::imaging_processor::IMAGE_PROCESSING_CONCURRENCY;

/// A batch this slow is logged even outside debug logging: it usually means
/// Originals on a network share.
const SLOW_BATCH: Duration = Duration::from_secs(1);

/// Whole Originals read from network shares at once, across import batches
/// and Cache jobs. On the Wi‑Fi share measured on 2026-09-29, one, two and
/// eight readers got the same 29–30 MiB/s together, but with eight the first
/// file arrived after 1.47 s instead of 0.31 s. Reading about 9 MiB takes
/// ~0.3 s there and decoding a 18–24 MP photo ~0.14 s, so three readers keep
/// the decoders busy while the first previews arrive in the order asked.
const REMOTE_FULL_READS: usize = 3;

/// How often an interruptible batch checks whether its caller stopped waiting.
const INTERRUPTION_POLL: Duration = Duration::from_millis(20);

/// Network roots whose server did not answer. Windows waits ~37 s for a server
/// that is switched off and then remembers the failure for only 20–30 s
/// (measured on 2026-09-29), so every later access to that server would wait
/// again. A root stays here until a probe of its own, on a thread nobody
/// waits for, reaches it again; meanwhile every access under it fails at once.
#[derive(Debug)]
struct UnreachableRoots {
    roots: Mutex<Vec<PathBuf>>,
    reachable: fn(&Path) -> bool,
    probe_interval: Duration,
}

static UNREACHABLE_ROOTS: UnreachableRoots = UnreachableRoots {
    roots: Mutex::new(Vec::new()),
    reachable: root_reachable,
    probe_interval: Duration::from_secs(2),
};

fn root_reachable(root: &Path) -> bool {
    std::fs::metadata(root).is_ok()
}

impl UnreachableRoots {
    fn contains(&self, root: &Path) -> bool {
        self.roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|known| known == root)
    }

    fn remember(&'static self, root: &Path) {
        {
            let mut roots = self
                .roots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if roots.iter().any(|known| known == root) {
                return;
            }
            roots.push(root.to_path_buf());
        }
        tracing::info!(target: "myalbuns.desktop", event = "linked_files_root_unreachable");
        let probed = root.to_path_buf();
        let probe = std::thread::Builder::new()
            .name("linked-files-probe".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(self.probe_interval);
                    if (self.reachable)(&probed) {
                        self.forget(&probed);
                        tracing::info!(
                            target: "myalbuns.desktop",
                            event = "linked_files_root_reachable",
                        );
                        return;
                    }
                }
            });
        if probe.is_err() {
            // Without a probe nothing would ever find the root again.
            self.forget(root);
        }
    }

    fn forget(&self, root: &Path) {
        self.roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|known| known != root);
    }
}

static REMOTE_READS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| {
        std::sync::Arc::new(tokio::sync::Semaphore::new(REMOTE_FULL_READS))
    });

/// A turn to read Originals from a network share; local reads need none.
pub(crate) struct RemoteReadTurn {
    _permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

#[cfg(test)]
std::thread_local! {
    static PHOTO_SOURCE_DECODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Full decodes of Originals made by the calling thread, for tests that prove
/// a flow does not decode again.
#[cfg(test)]
pub(crate) fn photo_source_decode_count() -> usize {
    PHOTO_SOURCE_DECODES.get()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaAvailability {
    Candidate,
    Absent,
    Unavailable,
}

/// What one look at an Arquivo vinculado established: whether it exists and can
/// be read, its physical identity, size and dates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MediaObservation {
    pub(crate) media_id: String,
    pub(crate) kind: MediaKind,
    logical_path: PathBuf,
    pub(crate) availability: MediaAvailability,
    physical_identity: Option<PhysicalFileIdentity>,
    source_bytes: Option<u64>,
    source_created_unix_ms: Option<u64>,
    source_modified_unix_ms: Option<u64>,
}

impl MediaObservation {
    pub(crate) fn same_source(&self, current: &Self) -> bool {
        self.availability == MediaAvailability::Candidate
            && current.availability == MediaAvailability::Candidate
            && self.physical_identity.is_some()
            && self.source_modified_unix_ms.is_some()
            && self.kind == current.kind
            && self.logical_path == current.logical_path
            && self.physical_identity == current.physical_identity
            && self.source_bytes == current.source_bytes
            && self.source_created_unix_ms == current.source_created_unix_ms
            && self.source_modified_unix_ms == current.source_modified_unix_ms
    }

    pub(crate) fn matches_fingerprint(
        &self,
        fingerprint: &myalbuns_imaging_protocol::CacheFingerprint,
    ) -> bool {
        self.availability == MediaAvailability::Candidate
            && self.source_bytes == Some(fingerprint.source_bytes)
            && self.source_created_unix_ms == fingerprint.source_created_unix_ms
            && self.source_modified_unix_ms == fingerprint.source_modified_unix_ms
    }

    /// Whether this readable observation shows other content than `previous`.
    pub(crate) fn changes_content_of(&self, previous: &Self) -> bool {
        self.availability == MediaAvailability::Candidate
            && (previous.availability != MediaAvailability::Candidate
                || previous.kind != self.kind
                || previous.logical_path != self.logical_path
                || previous.physical_identity != self.physical_identity
                || previous.source_bytes != self.source_bytes
                || previous.source_created_unix_ms != self.source_created_unix_ms
                || previous.source_modified_unix_ms != self.source_modified_unix_ms)
    }

    pub(crate) fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    pub(crate) fn source_bytes(&self) -> Option<u64> {
        self.source_bytes
    }

    pub(crate) fn created_unix_ms(&self) -> Option<u64> {
        self.source_created_unix_ms
    }

    pub(crate) fn modified_unix_ms(&self) -> Option<u64> {
        self.source_modified_unix_ms
    }

    #[cfg(test)]
    pub(crate) fn unavailable_for_test(binding: &MediaBinding) -> Self {
        observation(binding, MediaAvailability::Unavailable, None, None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceFormat {
    Jpeg,
    Png,
    Tiff,
}

/// Format, presented dimensions and memory facts of an Original. One read of
/// its header serves both the Photo's metadata and the memory admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceHeader {
    pub(crate) format: SourceFormat,
    /// Dimensions after the EXIF/TIFF orientation.
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// A baseline or extended colour JPEG, which the Processor decodes straight
    /// to RGB with far less memory per pixel.
    pub(crate) sequential_color_jpeg: bool,
    pub(crate) bytes: u64,
}

impl SourceHeader {
    pub(crate) fn pixels(&self) -> Option<u64> {
        u64::from(self.width).checked_mul(u64::from(self.height))
    }

    pub(crate) fn photo_metadata(&self) -> Result<PhotoSourceMetadata, String> {
        PhotoSourceMetadata::new(
            self.width,
            self.height,
            ["#D8DEE2".into(), "#BBC4CA".into(), "#929EA6".into()],
        )
        .map_err(crate::project_error_message::project_error_message)
    }
}

/// One entry of a listed folder. Size, dates and kind come from the listing
/// itself; only a link is followed, at the cost of one more request.
#[derive(Clone, Debug)]
pub(crate) struct ListedEntry {
    pub(crate) name: OsString,
    /// A symbolic link, junction or other reparse point.
    pub(crate) link: bool,
    /// Whether the entry, or the target of a link, is a folder.
    pub(crate) directory: bool,
    pub(crate) bytes: u64,
    pub(crate) created: Option<SystemTime>,
    pub(crate) modified: Option<SystemTime>,
}

pub(crate) struct ListedFolder {
    directory: ResolvedObject,
    pub(crate) entries: Vec<ListedEntry>,
}

impl ListedFolder {
    pub(crate) fn physical_identity(&self) -> Option<PhysicalFileIdentity> {
        self.directory.physical_identity()
    }
}

#[derive(Debug)]
pub(crate) enum ListFolderError {
    /// The path is not an existing folder reachable through the plan.
    Resolve(ResolveError),
    /// The folder could not be listed completely.
    Io(std::io::Error),
}

impl std::fmt::Display for ListFolderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolve(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

/// The Host's access to Arquivos vinculados. Every operation takes the attempt's
/// `RootBindingPlan`, accepts many entries, answers in the order asked and blocks:
/// callers run it away from the interface thread.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LinkedFiles {
    unreachable: &'static UnreachableRoots,
    /// Test adapter: each access waits this long first, like a slow share.
    #[cfg(test)]
    latency: Option<Duration>,
}

impl Default for LinkedFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkedFiles {
    pub(crate) const fn new() -> Self {
        Self {
            unreachable: &UNREACHABLE_ROOTS,
            #[cfg(test)]
            latency: None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn with_latency(latency: Duration) -> Self {
        Self {
            unreachable: &UNREACHABLE_ROOTS,
            latency: Some(latency),
        }
    }

    /// Test adapter: remembers unreachable roots on its own and probes them
    /// with `reachable`, so a test decides when a server answers again.
    #[cfg(test)]
    fn with_unreachable_roots(
        reachable: fn(&Path) -> bool,
        probe_interval: Duration,
        latency: Duration,
    ) -> Self {
        Self {
            unreachable: Box::leak(Box::new(UnreachableRoots {
                roots: Mutex::new(Vec::new()),
                reachable,
                probe_interval,
            })),
            latency: Some(latency),
        }
    }

    pub(crate) fn observe_one(
        &self,
        plan: &RootBindingPlan,
        binding: &MediaBinding,
    ) -> MediaObservation {
        observe_resolved(
            binding,
            self.resolve(plan, &binding.logical_path, ExpectedObject::RegularFile),
        )
    }

    pub(crate) fn observe<'a>(
        &self,
        plan: &RootBindingPlan,
        bindings: impl IntoIterator<Item = &'a MediaBinding>,
    ) -> Vec<MediaObservation> {
        self.observe_unless(plan, bindings, &AtomicBool::new(false))
            .expect("an observation nobody interrupts completes")
    }

    /// Answers `None` soon after `interrupted` is set, without waiting for
    /// the observations already running: they finish on their own and are
    /// dropped. One of them may be waiting for a server that is switched off.
    pub(crate) fn observe_unless<'a>(
        &self,
        plan: &RootBindingPlan,
        bindings: impl IntoIterator<Item = &'a MediaBinding>,
        interrupted: &AtomicBool,
    ) -> Option<Vec<MediaObservation>> {
        let bindings = bindings.into_iter().cloned().collect::<Vec<_>>();
        let remote = bindings
            .iter()
            .any(|binding| plan.is_remote(&binding.logical_path));
        let (files, plan) = (*self, plan.clone());
        timed("observe", remote, bindings.len(), || {
            map_concurrently_unless(bindings, interrupted, move |binding| {
                files.observe_one(&plan, &binding)
            })
        })
    }

    /// Observes bindings that belong to different attempts, each in its own plan.
    pub(crate) fn observe_in_plans<'a>(
        &self,
        requests: impl IntoIterator<Item = (&'a RootBindingPlan, &'a MediaBinding)>,
    ) -> Vec<MediaObservation> {
        let requests = requests.into_iter().collect::<Vec<_>>();
        let remote = requests
            .iter()
            .any(|(plan, binding)| plan.is_remote(&binding.logical_path));
        timed("observe", remote, requests.len(), || {
            map_concurrently(
                requests,
                |(plan, binding)| self.observe_one(plan, binding),
                |_| {},
            )
        })
    }

    /// Format, presented dimensions and memory facts, reading a JPEG only up to
    /// its first scan. It proves nothing about the image body: only for
    /// Originals the Processor decodes afterwards.
    pub(crate) fn header(
        &self,
        plan: &RootBindingPlan,
        path: &Path,
    ) -> Result<SourceHeader, String> {
        let (file, bytes) = self.open_source(plan, path)?;
        read_source(BufReader::new(file), bytes, false)
    }

    pub(crate) fn headers<'a>(
        &self,
        plan: &RootBindingPlan,
        paths: impl IntoIterator<Item = &'a Path>,
    ) -> Vec<Result<SourceHeader, String>> {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let remote = paths.iter().any(|path| plan.is_remote(path));
        timed("headers", remote, paths.len(), || {
            map_concurrently(paths, |path| self.header(plan, path), |_| {})
        })
    }

    /// Decodes the whole Original, proving its content is readable.
    pub(crate) fn inspect_decoded(
        &self,
        plan: &RootBindingPlan,
        path: &Path,
    ) -> Result<SourceHeader, String> {
        let (file, bytes) = self.open_source(plan, path)?;
        read_source(BufReader::new(file), bytes, true)
    }

    /// Lists a folder reached through the plan.
    pub(crate) fn list_folder(
        &self,
        plan: &RootBindingPlan,
        folder: &Path,
    ) -> Result<ListedFolder, ListFolderError> {
        let directory = self
            .resolve(plan, folder, ExpectedObject::Directory)
            .map_err(ListFolderError::Resolve)?;
        let started = Instant::now();
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(directory.operational_path()).map_err(ListFolderError::Io)? {
            entries.push(
                listed_entry(entry.map_err(ListFolderError::Io)?).map_err(ListFolderError::Io)?,
            );
        }
        log_batch(
            "list_folder",
            plan.is_remote(folder),
            entries.len(),
            started.elapsed(),
        );
        Ok(ListedFolder { directory, entries })
    }

    /// Lists several paths at once; a path that is not a reachable folder
    /// answers `ListFolderError::Resolve`.
    pub(crate) fn list_folders<'a>(
        &self,
        plan: &RootBindingPlan,
        folders: impl IntoIterator<Item = &'a Path>,
    ) -> Vec<Result<ListedFolder, ListFolderError>> {
        map_concurrently(
            folders.into_iter().collect(),
            |folder| self.list_folder(plan, folder),
            |_| {},
        )
    }

    /// How many processes may read `paths` in full at once: `capacity`, or
    /// fewer when any of them is on a network share.
    pub(crate) fn full_read_capacity<'a>(
        &self,
        plan: &RootBindingPlan,
        paths: impl IntoIterator<Item = &'a Path>,
        capacity: usize,
    ) -> usize {
        if paths.into_iter().any(|path| plan.is_remote(path)) {
            capacity.clamp(1, REMOTE_FULL_READS)
        } else {
            capacity
        }
    }

    /// Waits for a turn to read `paths` in full when any is on a network
    /// share, first come first served, so previews arrive in the order they
    /// were asked. Local paths get their turn at once. Answers `None` once
    /// `abandoned` says the caller no longer needs it.
    pub(crate) async fn remote_read_turn<'a>(
        &self,
        plan: &RootBindingPlan,
        paths: impl IntoIterator<Item = &'a Path>,
        abandoned: impl Fn() -> bool,
    ) -> Option<RemoteReadTurn> {
        if !paths.into_iter().any(|path| plan.is_remote(path)) {
            return Some(RemoteReadTurn { _permit: None });
        }
        let acquire = std::sync::Arc::clone(&REMOTE_READS).acquire_owned();
        tokio::pin!(acquire);
        loop {
            tokio::select! {
                permit = &mut acquire => {
                    return Some(RemoteReadTurn { _permit: permit.ok() });
                }
                () = tokio::time::sleep(Duration::from_millis(100)) => {
                    if abandoned() {
                        return None;
                    }
                }
            }
        }
    }

    /// Every access to an Arquivo vinculado starts here. Under a network root
    /// whose server stopped answering it fails at once instead of waiting for
    /// the server again.
    fn resolve(
        &self,
        plan: &RootBindingPlan,
        path: &Path,
        expected: ExpectedObject,
    ) -> Result<ResolvedObject, ResolveError> {
        let remote_root = plan.remote_root(path);
        if remote_root.is_some_and(|root| self.unreachable.contains(root)) {
            return Err(ResolveError::Unavailable);
        }
        self.wait();
        let resolved = plan.resolve_existing(path, expected);
        // `Unavailable` is a network failure or a root that no longer
        // answers, never a file held by another program.
        if let (Some(root), Err(ResolveError::Unavailable)) = (remote_root, &resolved) {
            self.unreachable.remember(root);
        }
        resolved
    }

    fn open_source(
        &self,
        plan: &RootBindingPlan,
        path: &Path,
    ) -> Result<(std::fs::File, u64), String> {
        let resolved = self
            .resolve(plan, path, ExpectedObject::RegularFile)
            .map_err(|error| inspection_failure(error, "O arquivo escolhido não está disponível. Confira se ele continua no mesmo local e pode ser aberto."))?;
        let file = resolved.reopen_for_read().map_err(|error| {
            inspection_failure(error, "Não foi possível abrir o arquivo escolhido.")
        })?;
        let bytes = file
            .metadata()
            .map_err(|error| {
                inspection_failure(error, "Não foi possível abrir o arquivo escolhido.")
            })?
            .len();
        Ok((file, bytes))
    }

    fn wait(&self) {
        #[cfg(test)]
        if let Some(latency) = self.latency {
            std::thread::sleep(latency);
        }
    }
}

fn listed_entry(entry: std::fs::DirEntry) -> std::io::Result<ListedEntry> {
    // On Windows the listing already carries each entry's attributes, size and
    // dates; asking the server again per entry costs a round trip each.
    let own = entry.metadata()?;
    let link = is_link(&own);
    // A broken link keeps its own facts instead of failing the whole folder.
    let metadata = if link {
        std::fs::metadata(entry.path()).unwrap_or(own)
    } else {
        own
    };
    Ok(ListedEntry {
        name: entry.file_name(),
        link,
        directory: metadata.is_dir(),
        bytes: metadata.len(),
        created: metadata.created().ok(),
        modified: metadata.modified().ok(),
    })
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn observation(
    binding: &MediaBinding,
    availability: MediaAvailability,
    physical_identity: Option<PhysicalFileIdentity>,
    metadata: Option<&std::fs::Metadata>,
) -> MediaObservation {
    MediaObservation {
        media_id: binding.media_id.clone(),
        kind: binding.kind,
        logical_path: binding.logical_path.clone(),
        availability,
        physical_identity,
        source_bytes: metadata.map(std::fs::Metadata::len),
        source_created_unix_ms: metadata.and_then(|metadata| file_time_millis(metadata.created())),
        source_modified_unix_ms: metadata
            .and_then(|metadata| file_time_millis(metadata.modified())),
    }
}

fn observe_resolved(
    binding: &MediaBinding,
    resolved: Result<ResolvedObject, ResolveError>,
) -> MediaObservation {
    match resolved {
        Ok(resolved) => match readable_source_metadata(&resolved) {
            Ok(metadata) => observation(
                binding,
                MediaAvailability::Candidate,
                resolved.physical_identity(),
                Some(&metadata),
            ),
            Err(_) => observation(binding, MediaAvailability::Unavailable, None, None),
        },
        Err(ResolveError::NotFound) => observation(binding, MediaAvailability::Absent, None, None),
        Err(
            ResolveError::InvalidPath
            | ResolveError::UnsupportedNamespace
            | ResolveError::UnboundRoot
            | ResolveError::AccessDenied
            | ResolveError::Unavailable
            | ResolveError::UnexpectedObjectType { .. }
            | ResolveError::IoFailure,
        ) => observation(binding, MediaAvailability::Unavailable, None, None),
    }
}

fn readable_source_metadata(resolved: &ResolvedObject) -> std::io::Result<std::fs::Metadata> {
    // A metadata-only handle may succeed while Photoshop holds the original
    // against readers. Such a sample cannot revoke the last usable preview.
    let mut file = resolved.reopen_for_read()?;
    file.read_exact(&mut [0u8; 1])?;
    file.metadata()
}

fn file_time_millis(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

/// Without pixels, a JPEG is judged from its header segments alone. The image
/// crate reads a whole JPEG before its header, which on a network share is a
/// second transfer of every Original the Processor reads right after.
fn read_source(
    mut source: impl BufRead + Seek,
    bytes: u64,
    decode_pixels: bool,
) -> Result<SourceHeader, String> {
    if !decode_pixels
        && let Some(header) = jpeg_header_prefix(&mut source, JPEG_HEADER_PREFIX_LIMIT)
    {
        let sequential_color_jpeg = is_sequential_color_jpeg(header.as_slice());
        return read_image(Cursor::new(header), bytes, sequential_color_jpeg, false);
    }
    let rewind = |error| inspection_failure(error, "Não foi possível ler a imagem escolhida.");
    source.rewind().map_err(rewind)?;
    let sequential_color_jpeg = is_sequential_color_jpeg(&mut source);
    source.rewind().map_err(rewind)?;
    read_image(source, bytes, sequential_color_jpeg, decode_pixels)
}

fn read_image(
    source: impl BufRead + Seek,
    bytes: u64,
    sequential_color_jpeg: bool,
    decode_pixels: bool,
) -> Result<SourceHeader, String> {
    let reader = ImageReader::new(source)
        .with_guessed_format()
        .map_err(|error| inspection_failure(error, "Não foi possível ler a imagem escolhida."))?;
    let format = match reader.format() {
        Some(ImageFormat::Jpeg) => SourceFormat::Jpeg,
        Some(ImageFormat::Png) => SourceFormat::Png,
        Some(ImageFormat::Tiff) => SourceFormat::Tiff,
        _ => return Err("O arquivo escolhido não usa um formato de mídia compatível.".into()),
    };
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| inspection_failure(error, "Não foi possível ler a imagem escolhida."))?;
    let (mut width, mut height) = decoder.dimensions();
    let orientation = decoder.orientation().map_err(|error| {
        inspection_failure(
            error,
            "Não foi possível ler a orientação da imagem escolhida.",
        )
    })?;
    if matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        std::mem::swap(&mut width, &mut height);
    }
    if decode_pixels {
        #[cfg(test)]
        PHOTO_SOURCE_DECODES.set(PHOTO_SOURCE_DECODES.get() + 1);
        DynamicImage::from_decoder(decoder).map_err(|_| {
            "Não foi possível ler a imagem. O arquivo pode estar danificado.".to_string()
        })?;
    }
    Ok(SourceHeader {
        format,
        width,
        height,
        sequential_color_jpeg,
        bytes,
    })
}

pub(crate) fn inspection_failure(error: impl std::fmt::Display, message: &str) -> String {
    tracing::warn!(target: "myalbuns.desktop", %error, event = "media_inspection_failed");
    message.into()
}

fn timed<R>(operation: &'static str, remote: bool, count: usize, work: impl FnOnce() -> R) -> R {
    let started = Instant::now();
    let result = work();
    log_batch(operation, remote, count, started.elapsed());
    result
}

fn log_batch(operation: &'static str, remote: bool, count: usize, elapsed: Duration) {
    if count == 0 {
        return;
    }
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    if elapsed >= SLOW_BATCH {
        tracing::info!(
            target: "myalbuns.desktop",
            operation,
            remote,
            count,
            elapsed_ms,
            event = "linked_files_batch_completed",
        );
    } else {
        tracing::debug!(
            target: "myalbuns.desktop",
            operation,
            remote,
            count,
            elapsed_ms,
            event = "linked_files_batch_completed",
        );
    }
}

/// Runs `work` on several items at once and keeps the input order in the
/// result. Work on Originals mostly waits for the disk or the server, so
/// running it in sequence adds up every wait.
pub(crate) fn map_concurrently<T: Send, R: Send>(
    items: Vec<T>,
    work: impl Fn(T) -> R + Sync,
    mut completed: impl FnMut(u32),
) -> Vec<R> {
    let total = items.len();
    if total <= 1 {
        return items
            .into_iter()
            .map(|item| {
                let result = work(item);
                completed(1);
                result
            })
            .collect();
    }
    let items = Mutex::new(items.into_iter().enumerate());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..total.min(CONCURRENCY) {
            let items = &items;
            let work = &work;
            let sender = sender.clone();
            scope.spawn(move || {
                loop {
                    let Some((index, item)) = items
                        .lock()
                        .expect("the concurrent work queue is healthy")
                        .next()
                    else {
                        break;
                    };
                    if sender.send((index, work(item))).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut results = Vec::with_capacity(total);
        for result in receiver {
            results.push(result);
            completed(results.len() as u32);
        }
        results.sort_unstable_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, result)| result).collect()
    })
}

/// `map_concurrently` for a caller that may stop waiting. Soon after
/// `interrupted` is set it answers `None`: no further item starts, and the
/// items already running finish on their own threads, their results dropped.
/// The caller may be holding up a Cache pause while an item waits for a
/// server that is switched off.
fn map_concurrently_unless<T: Send + 'static, R: Send + 'static>(
    items: Vec<T>,
    interrupted: &AtomicBool,
    work: impl Fn(T) -> R + Send + Sync + 'static,
) -> Option<Vec<R>> {
    let total = items.len();
    let items = Arc::new(Mutex::new(items.into_iter().enumerate()));
    let work = Arc::new(work);
    let abandoned = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let workers = (0..total.min(CONCURRENCY))
        .map(|_| {
            let (items, work, abandoned, sender) = (
                Arc::clone(&items),
                Arc::clone(&work),
                Arc::clone(&abandoned),
                sender.clone(),
            );
            std::thread::spawn(move || {
                while !abandoned.load(Ordering::Acquire) {
                    let Some((index, item)) = items
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .next()
                    else {
                        break;
                    };
                    if sender.send((index, work(item))).is_err() {
                        break;
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    drop(sender);
    let mut results = Vec::with_capacity(total);
    while results.len() < total {
        if interrupted.load(Ordering::Acquire) {
            abandoned.store(true, Ordering::Release);
            return None;
        }
        match receiver.recv_timeout(INTERRUPTION_POLL) {
            Ok(result) => results.push(result),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    for worker in workers {
        if let Err(panic) = worker.join() {
            std::panic::resume_unwind(panic);
        }
    }
    results.sort_unstable_by_key(|(index, _)| *index);
    Some(results.into_iter().map(|(_, result)| result).collect())
}

#[cfg(test)]
mod architecture_tests;
#[cfg(test)]
mod tests;
