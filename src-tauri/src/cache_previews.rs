use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use myalbuns_imaging_protocol::{CacheArtifact, CacheArtifactFormat};
use myalbuns_paths::AppPaths;

use crate::{
    cache_engine::{AuthorizedCacheNamespace, CacheSourceBinding},
    ipc_contract::{MediaPreview, MediaPreviewState},
    opaque_image_protocol::{
        ImageFormat, ImagePayload, ImageRequestError, opaque_image_url, read_image,
        respond_to_opaque_image_request, serve_opaque_image,
    },
};

pub(crate) const CACHE_MEDIA_PROTOCOL_SCHEME: &str = "myalbuns-cache";

#[derive(Clone)]
pub(crate) struct CachePreviewRegistry {
    allowed_webview_label: Arc<str>,
    publication: Arc<Mutex<CachePreviewPublication>>,
}

#[derive(Default)]
struct CachePreviewPublication {
    tokens_by_media: HashMap<String, (String, CacheSourceBinding, String)>,
    previews_by_token: HashMap<String, Arc<PreparedCachePreview>>,
}

struct PreparedCachePreview {
    format: ImageFormat,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CachePreviewError {
    Unavailable,
    InvalidDerivedArtifact,
}

impl std::fmt::Display for CachePreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter
                .write_str("A representação reduzida não está disponível no Cache autorizado."),
            Self::InvalidDerivedArtifact => formatter
                .write_str("A representação reduzida publicada pelo Processador é inválida."),
        }
    }
}

impl CachePreviewRegistry {
    pub(crate) fn new(allowed_webview_label: impl Into<Arc<str>>) -> Self {
        Self {
            allowed_webview_label: allowed_webview_label.into(),
            publication: Arc::new(Mutex::new(CachePreviewPublication::default())),
        }
    }

    pub(crate) fn publish(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        artifact: &CacheArtifact,
        source_path: &Path,
    ) -> Result<MediaPreview, CachePreviewError> {
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|_| CachePreviewError::Unavailable)?;
        let path = namespace
            .paths()
            .preview_file(&artifact.media_id, &artifact.generation_id, artifact.format)
            .map_err(|_| CachePreviewError::Unavailable)?;
        let file = storage
            .open_existing_file(&path)
            .map_err(|_| CachePreviewError::Unavailable)?
            .ok_or(CachePreviewError::Unavailable)?;
        let payload =
            read_image(file, true).map_err(|_| CachePreviewError::InvalidDerivedArtifact)?;
        let expected_format = match artifact.format {
            CacheArtifactFormat::Jpeg => ImageFormat::Jpeg,
            CacheArtifactFormat::Png => ImageFormat::Png,
        };
        if payload.format != expected_format
            || payload.source_bytes != artifact.preview_bytes
            || payload.body.len() as u64 != artifact.preview_bytes
        {
            return Err(CachePreviewError::InvalidDerivedArtifact);
        }

        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_binding = CacheSourceBinding::for_path(source_path);
        if let Some((generation_id, published_binding, token)) =
            publication.tokens_by_media.get(&artifact.media_id)
            && generation_id == &artifact.generation_id
            && published_binding == &source_binding
        {
            return Ok(MediaPreview {
                media_id: artifact.media_id.clone(),
                state: MediaPreviewState::Ready,
                url: Some(opaque_image_url(CACHE_MEDIA_PROTOCOL_SCHEME, token)),
            });
        }
        if let Some((_, _, previous_token)) = publication.tokens_by_media.remove(&artifact.media_id)
        {
            publication.previews_by_token.remove(&previous_token);
        }
        let token = format!(
            "{}.{}",
            uuid::Uuid::new_v4().hyphenated(),
            expected_format.extension()
        );
        publication.tokens_by_media.insert(
            artifact.media_id.clone(),
            (
                artifact.generation_id.clone(),
                source_binding,
                token.clone(),
            ),
        );
        publication.previews_by_token.insert(
            token.clone(),
            Arc::new(PreparedCachePreview {
                format: payload.format,
                bytes: payload.body,
            }),
        );
        Ok(MediaPreview {
            media_id: artifact.media_id.clone(),
            state: MediaPreviewState::Ready,
            url: Some(opaque_image_url(CACHE_MEDIA_PROTOCOL_SCHEME, &token)),
        })
    }

    pub(crate) fn invalidate_media<I, S>(&self, media_ids: I) -> usize
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut removed = 0;
        for media_id in media_ids {
            if let Some((_, _, token)) = publication.tokens_by_media.remove(media_id.as_ref()) {
                publication.previews_by_token.remove(&token);
                removed += 1;
            }
        }
        removed
    }

    pub(crate) fn retained_preview(
        &self,
        media_id: &str,
        source_path: &Path,
        state: MediaPreviewState,
    ) -> Option<MediaPreview> {
        let publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (_, source_binding, token) = publication.tokens_by_media.get(media_id)?;
        if !source_binding.matches_source_path(source_path) {
            return None;
        }
        Some(MediaPreview {
            media_id: media_id.to_owned(),
            state,
            url: Some(opaque_image_url(CACHE_MEDIA_PROTOCOL_SCHEME, token)),
        })
    }

    pub(crate) fn serve(
        &self,
        webview_label: &str,
        request: tauri::http::Request<Vec<u8>>,
    ) -> tauri::http::Response<Vec<u8>> {
        serve_opaque_image(
            self.allowed_webview_label.as_ref(),
            webview_label,
            request,
            |token, include_body| {
                let preview = self
                    .publication
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .previews_by_token
                    .get(token)
                    .cloned()
                    .ok_or(ImageRequestError::NotFound)?;
                Ok(ImagePayload {
                    format: preview.format,
                    source_bytes: preview.bytes.len() as u64,
                    body: if include_body {
                        preview.bytes.clone()
                    } else {
                        Vec::new()
                    },
                })
            },
        )
    }
}

pub(crate) fn respond_to_cache_media_request<R: tauri::Runtime>(
    registry: CachePreviewRegistry,
    context: tauri::UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    respond_to_opaque_image_request(
        context,
        request,
        responder,
        move |webview_label, request| registry.serve(webview_label, request),
    );
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat as EncoderFormat, Rgba, RgbaImage};
    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, InitialProject, ProjectCore, ProjectLocation,
    };
    use myalbuns_imaging_protocol::{CacheArtifact, CacheBasicColorProfile, CacheFingerprint};
    use myalbuns_paths::{AppPaths, CacheArtifactFormat, OperationPathContext};
    use tauri::http::{Method, Request, StatusCode};

    use crate::cache_engine::AuthorizedCacheNamespace;

    use super::CachePreviewRegistry;

    #[test]
    fn webview_receives_only_the_published_derived_bytes_behind_an_opaque_token() {
        let root = tempfile::tempdir().expect("temporary opaque Cache fixture");
        let project_path = root.path().join("Projeto.myalbuns");
        let original_path = root.path().join("Original-secreto.png");
        let original_bytes = b"bytes exclusivos do Original";
        std::fs::write(&original_path, original_bytes).expect("the Original fixture is writable");
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the Project root is captured");
        let project = ProjectCore::new()
            .with_identity_storage_roots(root.path().join("leases"), root.path().join("identities"))
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path, context.freeze()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("the editable Project establishes identity authority");
        let roaming_root = root.path().join("roaming");
        let local_root = root.path().join("local");
        std::fs::create_dir_all(&roaming_root).expect("the roaming root is available");
        std::fs::create_dir_all(&local_root).expect("the local root is available");
        let app_paths = AppPaths::from_roots(&roaming_root, &local_root);
        let namespace = AuthorizedCacheNamespace::mount(&app_paths, project.identity_authority())
            .expect("the authority mounts the Cache namespace");
        let derived_path = namespace
            .paths()
            .preview_file("media-photo", "g-derived-one", CacheArtifactFormat::Png)
            .expect("the derived path is central");
        let storage = app_paths
            .prepare_cache_storage(namespace.paths())
            .expect("the Cache storage is prepared");
        RgbaImage::from_pixel(2, 1, Rgba([20, 40, 60, 128]))
            .save_with_format(&derived_path, EncoderFormat::Png)
            .expect("the derived PNG is written");
        drop(storage);
        let derived_bytes = std::fs::read(&derived_path).expect("the derived PNG is readable");
        let artifact = CacheArtifact {
            media_id: "media-photo".into(),
            generation_id: "g-derived-one".into(),
            width_px: 2,
            height_px: 1,
            preview_bytes: derived_bytes.len() as u64,
            format: CacheArtifactFormat::Png,
            exif_orientation: None,
            source_page_count: None,
            basic_color_profile: CacheBasicColorProfile::Srgb,
            fingerprint: CacheFingerprint::sha256_full_file(27, "0".repeat(64))
                .expect("the fixture fingerprint is valid"),
        };
        let registry = CachePreviewRegistry::new("main");

        let preview = registry
            .publish(&app_paths, &namespace, &artifact, &original_path)
            .expect("the validated derived artifact is published");
        let url = preview.url.expect("a ready preview has one opaque URL");
        assert!(url.starts_with("http://myalbuns-cache.localhost/"));
        assert!(url.ends_with(".png"), "Pixi can select its texture loader");
        assert!(!url.contains("Original-secreto"));
        assert!(!url.contains(root.path().to_string_lossy().as_ref()));
        let token = url.rsplit('/').next().expect("the URL contains a token");
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{token}"))
            .header("origin", "http://tauri.localhost")
            .body(Vec::new())
            .expect("the opaque request is valid");

        let response = registry.serve("main", request);

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("access-control-allow-origin"),
            Some(&tauri::http::HeaderValue::from_static(
                "http://tauri.localhost"
            ))
        );
        assert_eq!(response.body(), &derived_bytes);
        assert_ne!(response.body().as_slice(), original_bytes);
        assert!(
            registry
                .retained_preview(
                    "media-photo",
                    &root.path().join("Outro Original.png"),
                    crate::ipc_contract::MediaPreviewState::Unavailable,
                )
                .is_none(),
            "resident bytes cannot cross a relink, Undo, or discarded binding"
        );

        assert_eq!(registry.invalidate_media(["media-photo"]), 1);
        let revoked_request = Request::builder()
            .method(Method::GET)
            .uri(format!("/{token}"))
            .body(Vec::new())
            .expect("the revoked opaque request is valid");
        let revoked_response = registry.serve("main", revoked_request);
        assert_eq!(revoked_response.status(), StatusCode::NOT_FOUND);
        assert!(revoked_response.body().is_empty());
    }
}
