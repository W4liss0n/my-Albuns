use std::{
    io::{BufReader, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
};

use myalbuns_imaging_protocol::{ImagingFailureCode, ImagingPathCode, MediaSource};
use myalbuns_paths::{AppPathsError, ExpectedObject, OperationPathContext, RootBindingPlan};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub(crate) struct SourceFingerprintFailure {
    pub(crate) code: ImagingFailureCode,
    pub(crate) media_id: Option<String>,
    pub(crate) path_code: Option<ImagingPathCode>,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) enum SourceFingerprintError {
    Cancelled,
    Failed(SourceFingerprintFailure),
}

pub(crate) async fn capture_root_bindings(
    paths: Vec<PathBuf>,
) -> Result<RootBindingPlan, AppPathsError> {
    capture_root_bindings_with_thread(paths)
        .await
        .map(|(bindings, _)| bindings)
}

async fn capture_root_bindings_with_thread(
    paths: Vec<PathBuf>,
) -> Result<(RootBindingPlan, ThreadId), AppPathsError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut owner = OperationPathContext::new();
        for path in paths {
            owner.capture(&path)?;
        }
        Ok((owner.freeze(), std::thread::current().id()))
    })
    .await
    .map_err(|_| AppPathsError::OperationPathIoFailure)?
}

#[cfg(test)]
pub(crate) async fn fingerprint_media_sources(
    bindings: RootBindingPlan,
    frozen_sources: Vec<(String, PathBuf)>,
) -> Result<Vec<MediaSource>, SourceFingerprintFailure> {
    match fingerprint_media_sources_cancellable(
        bindings,
        frozen_sources,
        Arc::new(AtomicBool::new(false)),
    )
    .await
    {
        Ok(sources) => Ok(sources),
        Err(SourceFingerprintError::Failed(failure)) => Err(failure),
        Err(SourceFingerprintError::Cancelled) => {
            unreachable!("a private uncancelled fingerprint token cannot change")
        }
    }
}

pub(crate) async fn fingerprint_media_sources_cancellable(
    bindings: RootBindingPlan,
    frozen_sources: Vec<(String, PathBuf)>,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<MediaSource>, SourceFingerprintError> {
    tauri::async_runtime::spawn_blocking(move || {
        fingerprint_sources(&bindings, frozen_sources, &cancelled, || {})
    })
    .await
    .map_err(|_| {
        SourceFingerprintError::Failed(SourceFingerprintFailure {
            code: ImagingFailureCode::SourceUnavailable,
            media_id: None,
            path_code: Some(ImagingPathCode::IoFailure),
            message: "Não foi possível verificar as fontes originais da Exportação.".into(),
        })
    })?
}

fn fingerprint_sources<F>(
    bindings: &RootBindingPlan,
    frozen_sources: Vec<(String, PathBuf)>,
    cancelled: &AtomicBool,
    mut on_chunk: F,
) -> Result<Vec<MediaSource>, SourceFingerprintError>
where
    F: FnMut(),
{
    ensure_not_cancelled(cancelled)?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(frozen_sources.len())
        .map_err(|_| {
            SourceFingerprintError::Failed(SourceFingerprintFailure {
                code: ImagingFailureCode::ResourceLimitExceeded,
                media_id: None,
                path_code: None,
                message: "Não há recursos suficientes para preparar as fontes da Exportação."
                    .into(),
            })
        })?;
    for (media_id, source_path) in frozen_sources {
        ensure_not_cancelled(cancelled)?;
        let resolved = bindings
            .resolve_existing(&source_path, ExpectedObject::RegularFile)
            .map_err(|error| {
                source_failure(&media_id, ImagingPathCode::from_resolve_error(error))
            })?;
        let file = resolved
            .reopen_for_read()
            .map_err(|error| source_failure(&media_id, ImagingPathCode::from_io_error(&error)))?;
        let source_bytes = file
            .metadata()
            .map_err(|error| source_failure(&media_id, ImagingPathCode::from_io_error(&error)))?
            .len();
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut measured_bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            ensure_not_cancelled(cancelled)?;
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(source_failure(
                        &media_id,
                        ImagingPathCode::from_io_error(&error),
                    ));
                }
            };
            if read == 0 {
                break;
            }
            measured_bytes = measured_bytes
                .checked_add(read as u64)
                .ok_or_else(|| source_failure(&media_id, ImagingPathCode::IoFailure))?;
            hasher.update(&buffer[..read]);
            on_chunk();
        }
        ensure_not_cancelled(cancelled)?;
        if measured_bytes != source_bytes {
            return Err(source_failure(&media_id, ImagingPathCode::Conflict));
        }
        let source = MediaSource::new(
            media_id.clone(),
            source_path,
            source_bytes,
            format!("{:x}", hasher.finalize()),
        )
        .map_err(|_| source_failure(&media_id, ImagingPathCode::IoFailure))?;
        sources.push(source);
    }
    Ok(sources)
}

fn ensure_not_cancelled(cancelled: &AtomicBool) -> Result<(), SourceFingerprintError> {
    if cancelled.load(Ordering::Acquire) {
        Err(SourceFingerprintError::Cancelled)
    } else {
        Ok(())
    }
}

fn source_failure(media_id: &str, path_code: ImagingPathCode) -> SourceFingerprintError {
    SourceFingerprintError::Failed(SourceFingerprintFailure {
        code: ImagingFailureCode::SourceUnavailable,
        media_id: Some(media_id.to_owned()),
        path_code: Some(path_code),
        message: "Uma fonte original necessária não está disponível para a Exportação.".into(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, Ordering},
        thread,
    };

    use myalbuns_imaging_protocol::{ImagingFailureCode, ImagingPathCode};

    #[test]
    fn path_binding_capture_runs_on_the_blocking_pool() {
        let root = tempfile::tempdir().expect("temporary path-I/O fixture");
        let path = root.path().join("Foto.jpg");
        std::fs::write(&path, b"photo").expect("the path-I/O fixture is writable");
        let caller_thread = thread::current().id();

        let (bindings, io_thread) =
            tauri::async_runtime::block_on(super::capture_root_bindings_with_thread(vec![
                path.clone(),
            ]))
            .expect("the native binding is captured");

        assert!(bindings.covers(&path));
        assert_ne!(
            io_thread, caller_thread,
            "potentially remote path I/O must not run on the caller/UI thread"
        );
    }

    #[test]
    fn fingerprints_only_the_frozen_originals_through_the_captured_plan() {
        let root = tempfile::tempdir().expect("temporary source fixture");
        let first = root.path().join("first.png");
        let second = root.path().join("second.jpg");
        std::fs::write(&first, b"first original").expect("the first original is writable");
        std::fs::write(&second, b"second original").expect("the second original is writable");
        let paths = vec![first.clone(), second.clone()];
        let bindings = tauri::async_runtime::block_on(super::capture_root_bindings(paths))
            .expect("the source roots are captured");

        let sources = tauri::async_runtime::block_on(super::fingerprint_media_sources(
            bindings,
            vec![
                ("media-first".into(), first.clone()),
                ("media-second".into(), second.clone()),
            ],
        ))
        .expect("the frozen originals are fingerprinted");

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].media_id(), "media-first");
        assert_eq!(sources[0].source_path(), first);
        assert_eq!(sources[0].source_bytes(), 14);
        assert_eq!(sources[1].media_id(), "media-second");
        assert_eq!(sources[1].source_path(), second);
        assert_eq!(sources[1].source_bytes(), 15);
    }

    #[test]
    fn fingerprint_preserves_an_empty_original_for_format_classification() {
        let root = tempfile::tempdir().expect("temporary source fixture");
        let source_path = root.path().join("empty.png");
        std::fs::write(&source_path, []).expect("the empty original is writable");
        let bindings =
            tauri::async_runtime::block_on(super::capture_root_bindings(vec![source_path.clone()]))
                .expect("the source root is captured");

        let sources = tauri::async_runtime::block_on(super::fingerprint_media_sources(
            bindings,
            vec![("media-empty".into(), source_path)],
        ))
        .expect("the empty original is fingerprinted before processor classification");

        assert_eq!(sources[0].source_bytes(), 0);
        assert_eq!(
            sources[0].source_sha256(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn fingerprint_stops_cooperatively_between_source_reads() {
        let root = tempfile::tempdir().expect("temporary source fixture");
        let source_path = root.path().join("large-original.jpg");
        std::fs::write(&source_path, vec![7_u8; 128 * 1024])
            .expect("the multi-chunk original is writable");
        let bindings =
            tauri::async_runtime::block_on(super::capture_root_bindings(vec![source_path.clone()]))
                .expect("the source root is captured");
        let cancelled = AtomicBool::new(false);

        let failure = super::fingerprint_sources(
            &bindings,
            vec![("media-large".into(), source_path)],
            &cancelled,
            || cancelled.store(true, Ordering::Release),
        )
        .expect_err("cancellation stops hashing after the current read completes");

        assert!(matches!(failure, super::SourceFingerprintError::Cancelled));
    }

    #[test]
    fn missing_frozen_original_keeps_media_and_path_failure_context() {
        let missing = PathBuf::from(r"C:\definitely-missing-myalbuns\source.png");
        let bindings =
            tauri::async_runtime::block_on(super::capture_root_bindings(vec![missing.clone()]))
                .expect("the missing source root is still captured");

        let failure = tauri::async_runtime::block_on(super::fingerprint_media_sources(
            bindings,
            vec![("media-missing".into(), missing)],
        ))
        .expect_err("the unavailable original is refused");

        assert_eq!(failure.code, ImagingFailureCode::SourceUnavailable);
        assert_eq!(failure.media_id.as_deref(), Some("media-missing"));
        assert_eq!(failure.path_code, Some(ImagingPathCode::NotFound));
    }
}
