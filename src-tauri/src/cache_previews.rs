use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

use myalbuns_imaging_protocol::{CacheArtifact, CacheArtifactFormat};
use myalbuns_paths::AppPaths;
use sha2::{Digest, Sha256};

use crate::{
    cache_engine::{AuthorizedCacheNamespace, CacheSourceBinding},
    ipc_contract::{MediaPreview, MediaPreviewState},
    media_runtime::MediaBinding,
    opaque_image_protocol::{
        ImageFormat, ImagePayload, ImageRequestError, opaque_image_url, read_image,
        respond_to_opaque_image_request, serve_opaque_image,
    },
};

pub(crate) const CACHE_MEDIA_PROTOCOL_SCHEME: &str = "myalbuns-cache";

// Budget for recent previews, excluding the currently demanded set. The decoded
// estimate keeps large previews from making a byte-only LRU too generous; the
// browser and Canvas still own their actual decoded/GPU allocations.
const RECENT_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
const RECENT_PREVIEW_DISPLAY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const RECENT_PREVIEW_COUNT: usize = 512;

#[derive(Clone)]
pub(crate) struct CachePreviewRegistry {
    allowed_webview_label: Arc<str>,
    publication: Arc<Mutex<CachePreviewPublication>>,
}

#[derive(Default)]
struct CachePreviewPublication {
    tokens_by_media: HashMap<String, PublishedCachePreview>,
    previews_by_token: HashMap<String, Arc<PreparedCachePreview>>,
    demanded_media: HashSet<String>,
    access_sequence: u64,
}

struct PublishedCachePreview {
    generation_id: String,
    source_binding: CacheSourceBinding,
    token: String,
    source_verified: bool,
    state: MediaPreviewState,
    last_used: u64,
    encoded_bytes: u64,
    display_bytes: u64,
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
        self.publish_with_digest(app_paths, namespace, artifact, source_path, None)
    }

    pub(crate) fn publish_verified(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        artifact: &CacheArtifact,
        source_path: &Path,
        preview_sha256: &[u8; 32],
    ) -> Result<MediaPreview, CachePreviewError> {
        self.publish_with_digest(
            app_paths,
            namespace,
            artifact,
            source_path,
            Some(preview_sha256),
        )
    }

    fn publish_with_digest(
        &self,
        app_paths: &AppPaths,
        namespace: &AuthorizedCacheNamespace,
        artifact: &CacheArtifact,
        source_path: &Path,
        expected_digest: Option<&[u8; 32]>,
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
        if let Some(expected) = expected_digest {
            let actual: [u8; 32] = Sha256::digest(&payload.body).into();
            if actual != *expected {
                return Err(CachePreviewError::InvalidDerivedArtifact);
            }
        }

        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_binding = CacheSourceBinding::for_path(source_path);
        publication.access_sequence += 1;
        let last_used = publication.access_sequence;
        if let Some(published) = publication.tokens_by_media.get_mut(&artifact.media_id)
            && published.generation_id == artifact.generation_id
            && published.source_binding == source_binding
        {
            published.source_verified = true;
            published.state = MediaPreviewState::Ready;
            published.last_used = last_used;
            return Ok(MediaPreview {
                media_id: artifact.media_id.clone(),
                state: MediaPreviewState::Ready,
                url: Some(opaque_image_url(
                    CACHE_MEDIA_PROTOCOL_SCHEME,
                    &published.token,
                )),
            });
        }
        if let Some(previous) = publication.tokens_by_media.remove(&artifact.media_id) {
            publication.previews_by_token.remove(&previous.token);
        }
        let token = format!(
            "{}.{}",
            uuid::Uuid::new_v4().hyphenated(),
            expected_format.extension()
        );
        publication.tokens_by_media.insert(
            artifact.media_id.clone(),
            PublishedCachePreview {
                generation_id: artifact.generation_id.clone(),
                source_binding,
                token: token.clone(),
                source_verified: true,
                state: MediaPreviewState::Ready,
                last_used,
                encoded_bytes: artifact.preview_bytes,
                display_bytes: artifact.preview_bytes.saturating_add(
                    u64::from(artifact.width_px) * u64::from(artifact.height_px) * 4,
                ),
            },
        );
        publication.previews_by_token.insert(
            token.clone(),
            Arc::new(PreparedCachePreview {
                format: payload.format,
                bytes: payload.body,
            }),
        );
        publication.trim_recent();
        Ok(MediaPreview {
            media_id: artifact.media_id.clone(),
            state: MediaPreviewState::Ready,
            url: Some(opaque_image_url(CACHE_MEDIA_PROTOCOL_SCHEME, &token)),
        })
    }

    pub(crate) fn retain_catalog(&self, bindings: &[MediaBinding]) {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let bindings = bindings
            .iter()
            .map(|binding| (binding.media_id.as_str(), binding))
            .collect::<HashMap<_, _>>();
        let removed = publication
            .tokens_by_media
            .iter()
            .filter(|(id, preview)| {
                !bindings.get(id.as_str()).is_some_and(|binding| {
                    preview
                        .source_binding
                        .matches_source_path(&binding.logical_path)
                })
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in removed {
            publication.remove(&id);
        }
    }

    pub(crate) fn retain_demand(&self, media_ids: &HashSet<String>) {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        publication.demanded_media.clone_from(media_ids);
        publication.access_sequence += 1;
        let last_used = publication.access_sequence;
        for id in media_ids {
            if let Some(preview) = publication.tokens_by_media.get_mut(id) {
                preview.last_used = last_used;
            }
        }
        publication.trim_recent();
    }

    // Completion replaces the frontend's snapshot, so include warm residents as
    // well as demanded outcomes. An explicit failure always wins over old bytes.
    pub(crate) fn presentation_snapshot(&self, outcomes: Vec<MediaPreview>) -> Vec<MediaPreview> {
        let publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut previews = publication
            .tokens_by_media
            .iter()
            .filter(|(_, preview)| {
                preview.source_verified || preview.state != MediaPreviewState::Ready
            })
            .map(|(id, preview)| {
                (
                    id.clone(),
                    MediaPreview {
                        media_id: id.clone(),
                        state: preview.state,
                        url: Some(opaque_image_url(
                            CACHE_MEDIA_PROTOCOL_SCHEME,
                            &preview.token,
                        )),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        for preview in outcomes {
            previews.insert(preview.media_id.clone(), preview);
        }
        let mut previews = previews.into_values().collect::<Vec<_>>();
        previews.sort_unstable_by(|left, right| left.media_id.cmp(&right.media_id));
        previews
    }

    pub(crate) fn mark_sources_changed<I, S>(&self, media_ids: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for media_id in media_ids {
            if let Some(published) = publication.tokens_by_media.get_mut(media_id.as_ref()) {
                published.source_verified = false;
            }
        }
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
            if publication.remove(media_id.as_ref()) {
                removed += 1;
            }
        }
        removed
    }

    pub(crate) fn revoke_all(&self) -> usize {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed = publication.previews_by_token.len();
        *publication = CachePreviewPublication::default();
        removed
    }

    pub(crate) fn retained_preview(
        &self,
        media_id: &str,
        source_path: &Path,
        state: MediaPreviewState,
    ) -> Option<MediaPreview> {
        let mut publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        publication.access_sequence += 1;
        let last_used = publication.access_sequence;
        let published = publication.tokens_by_media.get_mut(media_id)?;
        if !published.source_binding.matches_source_path(source_path)
            || (state == MediaPreviewState::Ready && !published.source_verified)
        {
            return None;
        }
        // Context retained after a read/cache failure is not proof that these
        // bytes represent the current Original. Only verified publication is.
        if state != MediaPreviewState::Ready {
            published.source_verified = false;
        }
        published.state = state;
        published.last_used = last_used;
        Some(MediaPreview {
            media_id: media_id.to_owned(),
            state,
            url: Some(opaque_image_url(
                CACHE_MEDIA_PROTOCOL_SCHEME,
                &published.token,
            )),
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

impl CachePreviewPublication {
    fn remove(&mut self, media_id: &str) -> bool {
        if let Some(preview) = self.tokens_by_media.remove(media_id) {
            self.previews_by_token.remove(&preview.token);
            true
        } else {
            false
        }
    }

    fn trim_recent(&mut self) {
        let mut recent = self
            .tokens_by_media
            .iter()
            .filter(|(id, _)| !self.demanded_media.contains(*id))
            .map(|(id, preview)| {
                (
                    id.clone(),
                    preview.last_used,
                    preview.encoded_bytes,
                    preview.display_bytes,
                )
            })
            .collect::<Vec<_>>();
        let mut encoded = recent.iter().map(|entry| entry.2).sum::<u64>();
        let mut display = recent.iter().map(|entry| entry.3).sum::<u64>();
        let mut count = recent.len();
        recent.sort_unstable_by(|left, right| (left.1, &left.0).cmp(&(right.1, &right.0)));
        for (id, _, bytes, display_bytes) in recent {
            if encoded <= RECENT_PREVIEW_BYTES
                && display <= RECENT_PREVIEW_DISPLAY_BYTES
                && count <= RECENT_PREVIEW_COUNT
            {
                break;
            }
            self.remove(&id);
            encoded -= bytes;
            display -= display_bytes;
            count -= 1;
        }
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

    fn resident_registry(
        count: usize,
        encoded_bytes: u64,
        display_bytes: u64,
    ) -> CachePreviewRegistry {
        let registry = CachePreviewRegistry::new("project");
        let mut publication = registry.publication.lock().unwrap();
        for index in 0..count {
            let id = format!("photo-{index:03}");
            let token = format!("{id}.jpg");
            publication.tokens_by_media.insert(
                id,
                super::PublishedCachePreview {
                    generation_id: "verified-generation".into(),
                    source_binding: crate::cache_engine::CacheSourceBinding::for_path(
                        std::path::Path::new("original.jpg"),
                    ),
                    token: token.clone(),
                    source_verified: true,
                    state: crate::ipc_contract::MediaPreviewState::Ready,
                    last_used: index as u64,
                    encoded_bytes,
                    display_bytes,
                },
            );
            publication.previews_by_token.insert(
                token,
                std::sync::Arc::new(super::PreparedCachePreview {
                    format: crate::opaque_image_protocol::ImageFormat::Jpeg,
                    bytes: vec![0; 4],
                }),
            );
        }
        publication.access_sequence = count as u64;
        drop(publication);
        registry
    }

    #[test]
    fn scrolling_134_previews_keeps_the_same_urls_in_the_presentation_snapshot() {
        let registry = resident_registry(134, 90_000, 1600 * 1067 * 4 + 90_000);
        let original = registry.presentation_snapshot(Vec::new());
        for index in 0..134 {
            registry.retain_demand(&[format!("photo-{index:03}")].into());
        }
        registry.retain_demand(&Default::default());
        let returned = registry.presentation_snapshot(Vec::new());
        assert_eq!(returned.len(), 134);
        assert_eq!(
            returned
                .iter()
                .map(|preview| &preview.url)
                .collect::<Vec<_>>(),
            original
                .iter()
                .map(|preview| &preview.url)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn recent_residency_evicts_the_least_used_for_each_budget_and_pins_active_demand() {
        for (count, encoded, display) in [
            (3, super::RECENT_PREVIEW_BYTES / 2, 1),
            (3, 1, super::RECENT_PREVIEW_DISPLAY_BYTES / 2),
            (super::RECENT_PREVIEW_COUNT + 1, 1, 1),
        ] {
            let registry = resident_registry(count, encoded, display);
            registry.retain_demand(&["photo-000".to_owned()].into());
            assert_eq!(registry.presentation_snapshot(Vec::new()).len(), count);
            registry.retain_demand(&Default::default());
            let snapshot = registry.presentation_snapshot(Vec::new());
            assert_eq!(snapshot.len(), count - 1);
            assert!(
                snapshot
                    .iter()
                    .any(|preview| preview.media_id == "photo-000")
            );
            assert!(
                !snapshot
                    .iter()
                    .any(|preview| preview.media_id == "photo-001")
            );
            let revoked = registry.serve(
                "project",
                Request::builder()
                    .uri("/photo-001.jpg")
                    .body(Vec::new())
                    .unwrap(),
            );
            assert_eq!(revoked.status(), StatusCode::NOT_FOUND);
        }
        let registry = resident_registry(
            2,
            super::RECENT_PREVIEW_BYTES + 1,
            super::RECENT_PREVIEW_DISPLAY_BYTES + 1,
        );
        registry.retain_demand(&["photo-000".to_owned()].into());
        assert_eq!(registry.presentation_snapshot(Vec::new()).len(), 1);
        registry.retain_demand(&Default::default());
        assert!(registry.presentation_snapshot(Vec::new()).is_empty());
    }

    #[test]
    fn snapshots_do_not_restore_invalidated_relinked_removed_or_failed_previews() {
        use crate::ipc_contract::{MediaPreview, MediaPreviewState};
        let registry = resident_registry(4, 1, 1);
        registry.mark_sources_changed(["photo-000"]);
        assert!(
            registry
                .presentation_snapshot(Vec::new())
                .iter()
                .all(|preview| preview.media_id != "photo-000")
        );
        registry
            .retained_preview(
                "photo-000",
                std::path::Path::new("original.jpg"),
                MediaPreviewState::Absent,
            )
            .unwrap();
        let snapshot = registry.presentation_snapshot(vec![MediaPreview {
            media_id: "photo-001".into(),
            state: MediaPreviewState::CacheUnavailable,
            url: None,
        }]);
        assert_eq!(snapshot[0].state, MediaPreviewState::Absent);
        assert_eq!(snapshot[1].state, MediaPreviewState::CacheUnavailable);
        assert!(snapshot[1].url.is_none());
        registry.retain_catalog(&[
            crate::media_runtime::MediaBinding {
                media_id: "photo-000".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: "original.jpg".into(),
            },
            crate::media_runtime::MediaBinding {
                media_id: "photo-001".into(),
                kind: myalbuns_core::MediaKind::Photo,
                logical_path: "relinked.jpg".into(),
            },
        ]);
        let snapshot = registry.presentation_snapshot(Vec::new());
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].media_id, "photo-000");
        assert_eq!(registry.revoke_all(), 1);
        assert!(registry.presentation_snapshot(Vec::new()).is_empty());
    }

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

        registry.mark_sources_changed(["media-photo"]);
        assert!(
            registry
                .retained_preview(
                    "media-photo",
                    &original_path,
                    crate::ipc_contract::MediaPreviewState::Ready
                )
                .is_none()
        );
        let retained = registry.serve(
            "main",
            Request::builder()
                .uri(format!("/{token}"))
                .body(Vec::new())
                .unwrap(),
        );
        assert_eq!(retained.status(), StatusCode::OK);
        assert_eq!(retained.body(), &derived_bytes);
        let mut successor = artifact.clone();
        successor.generation_id = "g-derived-two".into();
        let successor_path = namespace
            .paths()
            .preview_file(
                &successor.media_id,
                &successor.generation_id,
                successor.format,
            )
            .unwrap();
        std::fs::write(successor_path, &derived_bytes).unwrap();
        let replacement = registry
            .publish(&app_paths, &namespace, &successor, &original_path)
            .unwrap();
        assert_ne!(replacement.url.as_ref(), Some(&url));
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
