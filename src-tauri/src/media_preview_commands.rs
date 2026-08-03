use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use myalbuns_core::MediaKind;
use myalbuns_imaging_protocol::{CacheArtifactFormat, IMAGING_PROTOCOL_VERSION};
use myalbuns_logging::{ProcessRole, safe_log_identifier};
use myalbuns_paths::{AppPaths, project_data_namespace};
use tauri::{AppHandle, State, WebviewWindow};

use crate::{
    cache_engine::{self, CacheEngine, CacheWork},
    imaging_processor::{ImagingProcessor, InvocationContext, TauriImagingTransport},
    ipc_contract::MediaPreview,
    logging::{LoggingState, log_imaging_failure},
    path_io,
    project_host::ProjectHost,
};

static CACHE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const CACHE_PREVIEW_MAX_EDGE_PX: u32 = 1600;

#[tauri::command]
pub(crate) async fn prepare_media_previews(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    app_paths: State<'_, AppPaths>,
    logging: State<'_, LoggingState>,
    cache: State<'_, CacheEngine>,
    processor: State<'_, ImagingProcessor>,
) -> Result<Option<Vec<MediaPreview>>, String> {
    let cache_sequence = CACHE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_id = format!("cache-{}-{cache_sequence}", std::process::id());
    let projection = state.projection()?;
    let decorative_media_ids = projection
        .state
        .album
        .media
        .iter()
        .filter(|media| media.kind == MediaKind::Decorative)
        .map(|media| media.id.clone())
        .collect::<HashSet<_>>();
    let project_id = projection.state.project_id;
    let cache_paths = app_paths
        .project_cache(&project_data_namespace(&project_id))
        .map_err(|error| error.to_string())?;
    let Some(sources) = state.cache_sources() else {
        return Ok(None);
    };
    let mut operation_paths = Vec::with_capacity(sources.len() + 1);
    operation_paths.push(cache_paths.root().to_path_buf());
    operation_paths.extend(
        sources
            .iter()
            .map(|source| source.source_path().to_path_buf()),
    );
    let root_bindings = path_io::capture_root_bindings(operation_paths).await?;
    let work = CacheWork::new(
        request_id.clone(),
        project_id.clone(),
        cache_paths.clone(),
        sources,
        CACHE_PREVIEW_MAX_EDGE_PX,
        root_bindings,
    );
    let safe_project_id = safe_log_identifier(&project_id);
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        media_count = work.sources.len(),
        event = "media_cache_started",
    );
    let started = Instant::now();
    let context = InvocationContext::new(request_id.clone(), safe_project_id);
    let _cache_activity = cache.begin_work().await;
    let processor_reservation = processor
        .reserve()
        .await
        .map_err(|error| error.to_string())?;
    let mut transport = TauriImagingTransport::new(&app, &logging, &processor_reservation);
    let execution = cache_engine::execute(&mut transport, &app_paths, work, &context)
        .await
        .map_err(|failure| {
            log_imaging_failure(
                "media_cache_failed",
                &request_id,
                safe_project_id,
                failure.stage.as_str(),
                failure.exit_code,
            );
            failure.message
        })?;
    let recovered_process_id = execution
        .recovery
        .map(|recovery| recovery.failed_process_id);
    let removed_recovery_temporary_count = execution
        .recovery
        .map_or(0, |recovery| recovery.removed_temporary_count);
    let completed = execution.completion;
    let decorative_artifact_count = completed
        .artifacts
        .iter()
        .filter(|artifact| decorative_media_ids.contains(&artifact.media_id))
        .count();
    let decorative_png_artifact_count = completed
        .artifacts
        .iter()
        .filter(|artifact| {
            decorative_media_ids.contains(&artifact.media_id)
                && artifact.format == CacheArtifactFormat::Png
        })
        .count();
    let previews = completed
        .artifacts
        .iter()
        .map(|artifact| -> Result<MediaPreview, String> {
            let preview_path = cache_paths
                .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
                .map_err(|error| error.to_string())?;
            Ok(MediaPreview {
                media_id: artifact.media_id.clone(),
                url: cache_asset_url(&app_paths, &preview_path)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    tracing::info!(
        target: "myalbuns.desktop",
        process_role = ProcessRole::DesktopHost.as_str(),
        protocol_version = IMAGING_PROTOCOL_VERSION,
        operation_id = request_id.as_str(),
        project_id = safe_project_id,
        window_label = window.label(),
        generated_count = completed.generated_count,
        reused_count = completed.reused_count,
        decorative_media_count = decorative_media_ids.len(),
        decorative_artifact_count,
        decorative_png_artifact_count,
        source_bytes = completed.source_bytes,
        preview_bytes = completed.preview_bytes,
        recovered_process_id,
        removed_recovery_temporary_count,
        elapsed_ms = started.elapsed().as_millis(),
        event = "media_cache_completed",
    );
    Ok(Some(previews))
}

fn cache_asset_url(app_paths: &AppPaths, cache_file: &Path) -> Result<String, String> {
    app_paths
        .validate_cache_artifact(cache_file)
        .map_err(|error| error.to_string())?;
    let path = cache_file
        .to_str()
        .ok_or_else(|| "o caminho do Cache não pode ser representado pelo WebView".to_owned())?;
    let protocol = if cfg!(any(target_os = "windows", target_os = "android")) {
        "http://asset.localhost/"
    } else {
        "asset://localhost/"
    };
    Ok(format!("{protocol}{}", encode_uri_component(path)))
}

fn encode_uri_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use myalbuns_paths::{AppPaths, CacheArtifactFormat};

    use super::cache_asset_url;

    #[test]
    fn encodes_an_authorized_cache_artifact_for_the_tauri_asset_protocol() {
        let paths = AppPaths::from_roots(
            Path::new(r"C:\Roaming"),
            Path::new(r"C:\Local"),
            Path::new(r"C:\Temp"),
        );
        let preview = paths
            .project_cache("project-01")
            .expect("project namespace is safe")
            .preview_file(
                "media-001",
                "0123456789abcdef-v1-1600",
                CacheArtifactFormat::Jpeg,
            )
            .expect("artifact identity is safe");

        assert_eq!(
            cache_asset_url(&paths, &preview)
                .expect("the authorized Cache path becomes an asset URL"),
            "http://asset.localhost/C%3A%5CLocal%5CMyAlbuns2%5CCache%5Cproject-01%5CMedia%5Cmedia-ddedb0a5b1fd0e11bd569d4b06eec63d02c0e5a272186ce3e2ef6529439afafa.0123456789abcdef-v1-1600.jpg"
        );
    }

    #[test]
    fn encodes_an_authorized_png_cache_artifact_for_the_tauri_asset_protocol() {
        let paths = AppPaths::from_roots(
            Path::new(r"C:\Roaming"),
            Path::new(r"C:\Local"),
            Path::new(r"C:\Temp"),
        );
        let preview = paths
            .project_cache("project-01")
            .expect("project namespace is safe")
            .preview_file(
                "decorative-001",
                "0123456789abcdef-v1-1600",
                CacheArtifactFormat::Png,
            )
            .expect("PNG artifact identity is safe");

        assert_eq!(
            cache_asset_url(&paths, &preview)
                .expect("the authorized PNG Cache path becomes an asset URL"),
            "http://asset.localhost/C%3A%5CLocal%5CMyAlbuns2%5CCache%5Cproject-01%5CMedia%5Cmedia-12a79c4913ad160c8f60a357adb14fa9ea6a07a156d2feee32b8312e9c00da19.0123456789abcdef-v1-1600.png"
        );
    }

    #[test]
    fn refuses_to_expose_a_file_outside_the_authorized_cache_root() {
        let paths = AppPaths::from_roots(
            Path::new(r"C:\Roaming"),
            Path::new(r"C:\Local"),
            Path::new(r"C:\Temp"),
        );

        assert!(cache_asset_url(&paths, Path::new(r"C:\Photos\private.jpg")).is_err());
    }
}
