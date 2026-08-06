use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use myalbuns_paths::{
    ExpectedObject, OperationPathContext, PhysicalIdentityEvidence, ResolveError, ResolvedObject,
    RootBindingPlan,
};

use crate::{
    ipc_contract::MediaPreview,
    opaque_image_protocol::{
        ImagePayload, ImageReadError, ImageRequestError, opaque_image_url, read_image,
        respond_to_opaque_image_request, serve_opaque_image, sniff_image,
    },
};

pub(crate) const PROJECT_MEDIA_PROTOCOL_SCHEME: &str = "myalbuns-media";

#[derive(Clone)]
pub(crate) struct LinkedMediaPreviewRegistry {
    allowed_webview_label: Arc<str>,
    preparation: Arc<Mutex<()>>,
    publication: Arc<Mutex<LinkedMediaPublication>>,
}

#[derive(Default)]
struct LinkedMediaPublication {
    catalog: Option<Vec<(String, PathBuf)>>,
    tokens: Vec<String>,
    prepared: HashMap<String, Arc<PreparedLinkedMedia>>,
}

struct PreparedLinkedMedia {
    logical_path: PathBuf,
    root_bindings: RootBindingPlan,
    resolved: ResolvedObject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinkedMediaPreviewError {
    Unavailable,
    UnsupportedImage,
    ReadFailed,
}

impl std::fmt::Display for LinkedMediaPreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str(
                "Uma Imagem decorativa vinculada não está disponível no local registrado.",
            ),
            Self::UnsupportedImage => formatter
                .write_str("Uma Imagem decorativa vinculada deixou de ser um arquivo JPEG ou PNG."),
            Self::ReadFailed => {
                formatter.write_str("Não foi possível ler uma Imagem decorativa vinculada.")
            }
        }
    }
}

impl LinkedMediaPreviewRegistry {
    pub(crate) fn new(allowed_webview_label: impl Into<Arc<str>>) -> Self {
        Self {
            allowed_webview_label: allowed_webview_label.into(),
            preparation: Arc::new(Mutex::new(())),
            publication: Arc::new(Mutex::new(LinkedMediaPublication::default())),
        }
    }

    pub(crate) fn prepare(
        &self,
        sources: Vec<(String, PathBuf)>,
    ) -> Result<Vec<MediaPreview>, LinkedMediaPreviewError> {
        let _preparation = self
            .preparation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reusable_tokens = {
            let publication = self
                .publication
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if publication.catalog.as_ref() == Some(&sources) {
                Some(publication.tokens.clone())
            } else {
                None
            }
        };

        let catalog = sources.clone();
        let mut path_context = OperationPathContext::new();
        let mut opened = Vec::with_capacity(sources.len());
        for (media_id, logical_path) in sources {
            let resolved = path_context
                .resolve_existing(&logical_path, ExpectedObject::RegularFile)
                .map_err(map_resolve_error)?;
            sniff_resolved(&resolved)?;
            opened.push((media_id, logical_path, resolved));
        }
        let root_bindings = path_context.freeze();
        let mut prepared = HashMap::with_capacity(opened.len());
        let mut tokens = Vec::with_capacity(opened.len());
        let mut previews = Vec::with_capacity(opened.len());
        for (index, (media_id, logical_path, resolved)) in opened.into_iter().enumerate() {
            let token = reusable_tokens
                .as_ref()
                .and_then(|tokens| tokens.get(index))
                .cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().hyphenated().to_string());
            previews.push(MediaPreview {
                media_id,
                url: preview_url(&token),
            });
            prepared.insert(
                token.clone(),
                Arc::new(PreparedLinkedMedia {
                    logical_path,
                    root_bindings: root_bindings.clone(),
                    resolved,
                }),
            );
            tokens.push(token);
        }
        *self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = LinkedMediaPublication {
            catalog: Some(catalog),
            tokens,
            prepared,
        };
        Ok(previews)
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
                let source = self
                    .publication
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .prepared
                    .get(token)
                    .cloned()
                    .ok_or(ImageRequestError::NotFound)?;
                read_source(&source, include_body).map_err(|error| match error {
                    LinkedMediaPreviewError::UnsupportedImage => {
                        ImageRequestError::UnsupportedImage
                    }
                    LinkedMediaPreviewError::Unavailable | LinkedMediaPreviewError::ReadFailed => {
                        ImageRequestError::NotFound
                    }
                })
            },
        )
    }
}

fn sniff_resolved(resolved: &ResolvedObject) -> Result<(), LinkedMediaPreviewError> {
    let readable = resolved
        .reopen_for_read()
        .map_err(|_| LinkedMediaPreviewError::ReadFailed)?;
    sniff_image(&readable)
        .map(|_| ())
        .map_err(map_image_read_error)
}

fn read_source(
    source: &PreparedLinkedMedia,
    include_body: bool,
) -> Result<ImagePayload, LinkedMediaPreviewError> {
    let current = source
        .root_bindings
        .resolve_existing(&source.logical_path, ExpectedObject::RegularFile)
        .map_err(map_resolve_error)?;
    if source.resolved.compare_physical(&current) != PhysicalIdentityEvidence::Same {
        return Err(LinkedMediaPreviewError::Unavailable);
    }
    let readable = source
        .resolved
        .reopen_for_read()
        .map_err(|_| LinkedMediaPreviewError::ReadFailed)?;
    read_image(readable, include_body).map_err(map_image_read_error)
}

fn map_image_read_error(error: ImageReadError) -> LinkedMediaPreviewError {
    match error {
        ImageReadError::UnsupportedImage => LinkedMediaPreviewError::UnsupportedImage,
        ImageReadError::ReadFailed => LinkedMediaPreviewError::ReadFailed,
    }
}

fn map_resolve_error(error: ResolveError) -> LinkedMediaPreviewError {
    match error {
        ResolveError::NotFound | ResolveError::Unavailable => LinkedMediaPreviewError::Unavailable,
        ResolveError::InvalidPath
        | ResolveError::UnsupportedNamespace
        | ResolveError::UnboundRoot
        | ResolveError::AccessDenied
        | ResolveError::UnexpectedObjectType { .. }
        | ResolveError::IoFailure => LinkedMediaPreviewError::ReadFailed,
    }
}

fn preview_url(token: &str) -> String {
    opaque_image_url(PROJECT_MEDIA_PROTOCOL_SCHEME, token)
}

pub(crate) fn respond_to_media_request<R: tauri::Runtime>(
    registry: LinkedMediaPreviewRegistry,
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
    use std::{
        fs,
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;
    use tauri::http::{Method, Request, StatusCode};

    const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nlinked-background";
    const JPEG_BYTES: &[u8] = b"\xff\xd8\xfflinked-overlay\xff\xd9";

    fn request(method: Method, uri: &str) -> Request<Vec<u8>> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Vec::new())
            .expect("the opaque linked-media URI is valid")
    }

    #[test]
    fn preparing_linked_media_returns_opaque_urls_without_materializing_files() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let source = directory.path().join("Background familiar.png");
        fs::write(&source, PNG_BYTES).expect("the linked PNG fixture is writable");
        let original_entries = fs::read_dir(directory.path())
            .expect("the fixture directory is readable")
            .count();
        let registry = LinkedMediaPreviewRegistry::new("project");

        let previews = registry
            .prepare(vec![("decorative-001".into(), source.clone())])
            .expect("the linked original can be prepared");

        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].media_id, "decorative-001");
        assert!(
            previews[0]
                .url
                .starts_with("http://myalbuns-media.localhost/")
        );
        assert!(!previews[0].url.contains("Background"));
        assert!(!previews[0].url.contains(source.to_string_lossy().as_ref()));
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("the fixture directory remains readable")
                .count(),
            original_entries,
            "preparing a direct preview must not create a copy, Cache or temporary file"
        );
    }

    #[test]
    fn concurrent_preparations_of_the_same_catalog_keep_one_stable_publication() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let source = directory.path().join("Background.png");
        fs::write(&source, PNG_BYTES).expect("the linked PNG fixture is writable");
        let registry = LinkedMediaPreviewRegistry::new("project");
        let start = Arc::new(Barrier::new(2));

        let first_registry = registry.clone();
        let first_source = source.clone();
        let first_start = Arc::clone(&start);
        let first = thread::spawn(move || {
            first_start.wait();
            first_registry
                .prepare(vec![("background".into(), first_source)])
                .expect("the first preparation succeeds")
                .remove(0)
        });
        let second_registry = registry.clone();
        let second = thread::spawn(move || {
            start.wait();
            second_registry
                .prepare(vec![("background".into(), source)])
                .expect("the second preparation succeeds")
                .remove(0)
        });

        let first_preview = first.join().expect("the first preparation finishes");
        let second_preview = second.join().expect("the second preparation finishes");

        assert_eq!(first_preview.media_id, second_preview.media_id);
        assert_eq!(first_preview.url, second_preview.url);
        assert_eq!(
            registry
                .serve("project", request(Method::GET, &first_preview.url))
                .status(),
            StatusCode::OK
        );
    }

    #[test]
    fn project_window_serves_sniffed_png_and_jpeg_for_get_and_head() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let png = directory.path().join("Background.png");
        let jpeg = directory.path().join("Overlay.jpg");
        fs::write(&png, PNG_BYTES).expect("the linked PNG fixture is writable");
        fs::write(&jpeg, JPEG_BYTES).expect("the linked JPEG fixture is writable");
        let registry = LinkedMediaPreviewRegistry::new("project");
        let previews = registry
            .prepare(vec![("background".into(), png), ("overlay".into(), jpeg)])
            .expect("both linked originals can be prepared");

        let png_response = registry.serve("project", request(Method::GET, &previews[0].url));
        assert_eq!(png_response.status(), StatusCode::OK);
        assert_eq!(png_response.headers()["content-type"], "image/png");
        assert_eq!(png_response.headers()["cache-control"], "no-store");
        assert_eq!(png_response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(png_response.body(), PNG_BYTES);

        let jpeg_response = registry.serve("project", request(Method::HEAD, &previews[1].url));
        assert_eq!(jpeg_response.status(), StatusCode::OK);
        assert_eq!(jpeg_response.headers()["content-type"], "image/jpeg");
        assert_eq!(
            jpeg_response.headers()["content-length"],
            JPEG_BYTES.len().to_string()
        );
        assert!(jpeg_response.body().is_empty());
    }

    #[test]
    fn opaque_protocol_rejects_other_windows_unknown_tokens_and_non_read_methods() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let png = directory.path().join("Background.png");
        fs::write(&png, PNG_BYTES).expect("the linked PNG fixture is writable");
        let registry = LinkedMediaPreviewRegistry::new("project");
        let preview = registry
            .prepare(vec![("background".into(), png)])
            .expect("the linked original can be prepared")
            .remove(0);

        assert_eq!(
            registry
                .serve("global", request(Method::GET, &preview.url))
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            registry
                .serve(
                    "project",
                    request(Method::GET, "http://myalbuns-media.localhost/unknown"),
                )
                .status(),
            StatusCode::NOT_FOUND
        );
        let method_response = registry.serve("project", request(Method::POST, &preview.url));
        assert_eq!(method_response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(method_response.headers()["allow"], "GET, HEAD");
    }

    #[test]
    fn serving_fails_closed_when_the_bound_origin_is_replaced_or_unavailable() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let source = directory.path().join("Background.png");
        let moved = directory.path().join("Background-original.png");
        fs::write(&source, PNG_BYTES).expect("the linked PNG fixture is writable");
        let registry = LinkedMediaPreviewRegistry::new("project");
        let preview = registry
            .prepare(vec![("background".into(), source.clone())])
            .expect("the linked original can be prepared")
            .remove(0);

        fs::rename(&source, &moved).expect("the prepared source can move while shared");
        fs::write(&source, JPEG_BYTES).expect("a different object can occupy the same pathname");
        assert_eq!(
            registry
                .serve("project", request(Method::GET, &preview.url))
                .status(),
            StatusCode::NOT_FOUND
        );

        let refreshed = registry
            .prepare(vec![("background".into(), source.clone())])
            .expect("the replacement can be prepared explicitly")
            .remove(0);
        assert_eq!(refreshed.url, preview.url);
        let refreshed_response = registry.serve("project", request(Method::GET, &refreshed.url));
        assert_eq!(refreshed_response.status(), StatusCode::OK);
        assert_eq!(refreshed_response.headers()["content-type"], "image/jpeg");
        assert_eq!(refreshed_response.body(), JPEG_BYTES);

        fs::remove_file(&source).expect("the replacement can become unavailable");
        assert_eq!(
            registry
                .serve("project", request(Method::GET, &preview.url))
                .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn preparation_rejects_an_extension_disguising_unsupported_content() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let source = directory.path().join("Not-an-image.png");
        fs::write(&source, b"<html></html>").expect("the disguised fixture is writable");
        let registry = LinkedMediaPreviewRegistry::new("project");

        let result = registry.prepare(vec![("background".into(), source)]);
        assert!(matches!(
            result,
            Err(LinkedMediaPreviewError::UnsupportedImage)
        ));
    }

    #[test]
    fn preparation_distinguishes_unavailable_sources_from_unreadable_sources() {
        let directory = tempfile::tempdir().expect("temporary linked media directory");
        let missing = directory.path().join("Missing.png");
        let registry = LinkedMediaPreviewRegistry::new("project");

        assert!(matches!(
            registry.prepare(vec![("missing".into(), missing)]),
            Err(LinkedMediaPreviewError::Unavailable)
        ));
        assert!(matches!(
            registry.prepare(vec![("directory".into(), directory.path().to_path_buf())]),
            Err(LinkedMediaPreviewError::ReadFailed)
        ));
    }
}
