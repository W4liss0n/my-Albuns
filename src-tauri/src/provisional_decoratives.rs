use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use myalbuns_paths::{
    ExpectedObject, NativePathDto, OperationPathContext, ResolveError, RootBindingPlan,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State, UriSchemeContext, UriSchemeResponder, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    global_runtime::GLOBAL_WINDOW_LABEL,
    opaque_image_protocol::{
        ImagePayload, ImageReadError, ImageRequestError, opaque_image_url, read_image,
        respond_to_opaque_image_request, serve_opaque_image, sniff_image,
    },
    project_bootstrap::{
        InitialBackground, InitialBackgroundContent, InitialDocumentConfiguration,
        InitialFrameBorder, InitialOverlay, InitialOverlayContent,
        InitialProjectCreationConfiguration, InitialStructureConfiguration, InitialVisualDefaults,
    },
};

pub(crate) const PREVIEW_PROTOCOL_SCHEME: &str = "myalbuns-preview";

#[derive(Clone, Debug)]
struct ProvisionalDecorativeSource {
    native_path: NativePathDto,
    root_bindings: RootBindingPlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProvisionalDecorativeSelection {
    pub(crate) selection_id: String,
    display_name: String,
    preview_url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ProvisionalBackgroundContent {
    Color { rgb: String },
    Image { selection_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ProvisionalBackground {
    BothSides {
        both: ProvisionalBackgroundContent,
    },
    PerSide {
        left: ProvisionalBackgroundContent,
        right: ProvisionalBackgroundContent,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ProvisionalOverlayContent {
    Image { selection_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ProvisionalOverlay {
    BothSides {
        both: Option<ProvisionalOverlayContent>,
    },
    PerSide {
        left: Option<ProvisionalOverlayContent>,
        right: Option<ProvisionalOverlayContent>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProvisionalVisualDefaults {
    pub(crate) background: ProvisionalBackground,
    pub(crate) overlay: ProvisionalOverlay,
    pub(crate) frame_border: InitialFrameBorder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProvisionalProjectCreationConfiguration {
    pub(crate) document: InitialDocumentConfiguration,
    pub(crate) structure: InitialStructureConfiguration,
    pub(crate) visual_defaults: ProvisionalVisualDefaults,
}

#[derive(Clone, Default)]
pub(crate) struct ProvisionalDecorativeRegistry {
    selections: Arc<Mutex<HashMap<String, ProvisionalDecorativeSource>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProvisionalDecorativeError {
    UnknownSelection,
    InvalidPath,
    Unavailable,
    UnsupportedImage,
    ReadFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProvisionalDecorativeFailure {
    code: &'static str,
    message: &'static str,
    action: &'static str,
}

impl ProvisionalDecorativeFailure {
    const fn invalid_surface() -> Self {
        Self {
            code: "invalid_creation_surface",
            message: "A seleção pertence apenas à janela de Novo Projeto.",
            action: "Feche esta janela e tente novamente.",
        }
    }

    const fn dialog_unavailable() -> Self {
        Self {
            code: "decorative_picker_unavailable",
            message: "Não foi possível concluir o seletor de Imagem decorativa.",
            action: "Tente novamente.",
        }
    }

    const fn invalid_selection() -> Self {
        Self {
            code: "invalid_decorative_selection",
            message: "O item escolhido não é um arquivo local válido.",
            action: "Escolha uma imagem JPEG ou PNG.",
        }
    }

    const fn from_registration(error: ProvisionalDecorativeError) -> Self {
        match error {
            ProvisionalDecorativeError::UnknownSelection => Self {
                code: "image_selection_expired",
                message: "Uma Imagem decorativa selecionada n\u{e3}o est\u{e1} mais dispon\u{ed}vel.",
                action: "Escolha novamente a imagem antes de criar o Projeto.",
            },
            ProvisionalDecorativeError::InvalidPath => Self {
                code: "invalid_image_path",
                message: "O caminho da Imagem decorativa não é válido.",
                action: "Escolha outro arquivo JPEG ou PNG.",
            },
            ProvisionalDecorativeError::Unavailable => Self {
                code: "image_unavailable",
                message: "A Imagem decorativa não está disponível.",
                action: "Reconecte o local ou escolha outro arquivo.",
            },
            ProvisionalDecorativeError::UnsupportedImage => Self {
                code: "unsupported_image",
                message: "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
                action: "Escolha outro arquivo JPEG ou PNG.",
            },
            ProvisionalDecorativeError::ReadFailed => Self {
                code: "image_read_failed",
                message: "Não foi possível ler a Imagem decorativa.",
                action: "Confirme o acesso ao arquivo ou escolha outro.",
            },
        }
    }
}

impl ProvisionalDecorativeRegistry {
    pub(crate) fn register_dialog_selection(
        &self,
        path: Option<PathBuf>,
    ) -> Result<Option<ProvisionalDecorativeSelection>, ProvisionalDecorativeError> {
        let Some(path) = path else {
            return Ok(None);
        };
        let mut path_context = OperationPathContext::new();
        let resolved = path_context
            .resolve_existing(&path, ExpectedObject::RegularFile)
            .map_err(map_resolve_error)?;
        let readable = resolved
            .reopen_for_read()
            .map_err(|_| ProvisionalDecorativeError::ReadFailed)?;
        sniff_image(&readable).map_err(map_image_read_error)?;
        let display_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .ok_or(ProvisionalDecorativeError::InvalidPath)?;
        let selection_id = uuid::Uuid::new_v4().hyphenated().to_string();
        self.selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                selection_id.clone(),
                ProvisionalDecorativeSource {
                    native_path: NativePathDto::from(path),
                    root_bindings: path_context.freeze(),
                },
            );
        Ok(Some(ProvisionalDecorativeSelection {
            preview_url: preview_url(&selection_id),
            selection_id,
            display_name,
        }))
    }

    pub(crate) fn resolve_creation_configuration(
        &self,
        configuration: ProvisionalProjectCreationConfiguration,
    ) -> Result<InitialProjectCreationConfiguration, ProvisionalDecorativeError> {
        let mut resolved_paths = HashMap::new();
        let background = match configuration.visual_defaults.background {
            ProvisionalBackground::BothSides { both } => InitialBackground::BothSides {
                both: self.resolve_background_content(both, &mut resolved_paths)?,
            },
            ProvisionalBackground::PerSide { left, right } => InitialBackground::PerSide {
                left: self.resolve_background_content(left, &mut resolved_paths)?,
                right: self.resolve_background_content(right, &mut resolved_paths)?,
            },
        };
        let overlay = match configuration.visual_defaults.overlay {
            ProvisionalOverlay::BothSides { both } => InitialOverlay::BothSides {
                both: both
                    .map(|content| self.resolve_overlay_content(content, &mut resolved_paths))
                    .transpose()?,
            },
            ProvisionalOverlay::PerSide { left, right } => InitialOverlay::PerSide {
                left: left
                    .map(|content| self.resolve_overlay_content(content, &mut resolved_paths))
                    .transpose()?,
                right: right
                    .map(|content| self.resolve_overlay_content(content, &mut resolved_paths))
                    .transpose()?,
            },
        };

        Ok(InitialProjectCreationConfiguration {
            document: configuration.document,
            structure: configuration.structure,
            visual_defaults: InitialVisualDefaults {
                background,
                overlay,
                frame_border: configuration.visual_defaults.frame_border,
            },
        })
    }

    fn resolve_background_content(
        &self,
        content: ProvisionalBackgroundContent,
        resolved_paths: &mut HashMap<String, NativePathDto>,
    ) -> Result<InitialBackgroundContent, ProvisionalDecorativeError> {
        Ok(match content {
            ProvisionalBackgroundContent::Color { rgb } => InitialBackgroundContent::Color { rgb },
            ProvisionalBackgroundContent::Image { selection_id } => {
                InitialBackgroundContent::Image {
                    native_path: self.resolve_native_path(&selection_id, resolved_paths)?,
                }
            }
        })
    }

    fn resolve_overlay_content(
        &self,
        content: ProvisionalOverlayContent,
        resolved_paths: &mut HashMap<String, NativePathDto>,
    ) -> Result<InitialOverlayContent, ProvisionalDecorativeError> {
        match content {
            ProvisionalOverlayContent::Image { selection_id } => Ok(InitialOverlayContent::Image {
                native_path: self.resolve_native_path(&selection_id, resolved_paths)?,
            }),
        }
    }

    fn resolve_native_path(
        &self,
        selection_id: &str,
        resolved_paths: &mut HashMap<String, NativePathDto>,
    ) -> Result<NativePathDto, ProvisionalDecorativeError> {
        if let Some(path) = resolved_paths.get(selection_id) {
            return Ok(path.clone());
        }
        let source = self
            .selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(selection_id)
            .cloned()
            .ok_or(ProvisionalDecorativeError::UnknownSelection)?;
        read_source(&source, false)?;
        let native_path = source.native_path;
        resolved_paths.insert(selection_id.to_owned(), native_path.clone());
        Ok(native_path)
    }

    pub(crate) fn release(&self, selection_id: &str) -> bool {
        self.selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(selection_id)
            .is_some()
    }

    pub(crate) fn clear(&self) -> usize {
        let mut selections = self
            .selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed = selections.len();
        selections.clear();
        removed
    }

    pub(crate) fn serve(
        &self,
        webview_label: &str,
        request: tauri::http::Request<Vec<u8>>,
    ) -> tauri::http::Response<Vec<u8>> {
        serve_opaque_image(
            GLOBAL_WINDOW_LABEL,
            webview_label,
            request,
            |selection_id, include_body| {
                let source = self
                    .selections
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(selection_id)
                    .cloned()
                    .ok_or(ImageRequestError::NotFound)?;
                read_source(&source, include_body).map_err(|error| match error {
                    ProvisionalDecorativeError::UnsupportedImage => {
                        ImageRequestError::UnsupportedImage
                    }
                    ProvisionalDecorativeError::UnknownSelection
                    | ProvisionalDecorativeError::InvalidPath
                    | ProvisionalDecorativeError::Unavailable
                    | ProvisionalDecorativeError::ReadFailed => ImageRequestError::NotFound,
                })
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, selection_id: &str) -> bool {
        self.selections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(selection_id)
    }
}

#[tauri::command]
pub(crate) async fn choose_provisional_decorative(
    app: AppHandle,
    window: WebviewWindow,
    registry: State<'_, ProvisionalDecorativeRegistry>,
) -> Result<Option<ProvisionalDecorativeSelection>, ProvisionalDecorativeFailure> {
    if window.label() != GLOBAL_WINDOW_LABEL {
        return Err(ProvisionalDecorativeFailure::invalid_surface());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(&window)
        .add_filter("Imagens JPEG e PNG", &["jpg", "jpeg", "png"])
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| ProvisionalDecorativeFailure::dialog_unavailable())?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let FilePath::Path(path) = selection else {
        return Err(ProvisionalDecorativeFailure::invalid_selection());
    };
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn_blocking(move || registry.register_dialog_selection(Some(path)))
        .await
        .map_err(|_| ProvisionalDecorativeFailure::dialog_unavailable())?
        .map_err(ProvisionalDecorativeFailure::from_registration)
}

#[tauri::command]
pub(crate) fn release_provisional_decorative(
    window: WebviewWindow,
    selection_id: String,
    registry: State<'_, ProvisionalDecorativeRegistry>,
) -> Result<bool, String> {
    if window.label() != GLOBAL_WINDOW_LABEL {
        return Err("only the global New Project flow can release provisional selections".into());
    }
    Ok(registry.release(&selection_id))
}

pub(crate) fn respond_to_preview_request<R: Runtime>(
    registry: ProvisionalDecorativeRegistry,
    context: UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    respond_to_opaque_image_request(
        context,
        request,
        responder,
        move |webview_label, request| registry.serve(webview_label, request),
    );
}

fn read_source(
    source: &ProvisionalDecorativeSource,
    include_body: bool,
) -> Result<ImagePayload, ProvisionalDecorativeError> {
    let resolved = source
        .root_bindings
        .resolve_existing(source.native_path.as_path(), ExpectedObject::RegularFile)
        .map_err(map_resolve_error)?;
    let readable = resolved
        .reopen_for_read()
        .map_err(|_| ProvisionalDecorativeError::ReadFailed)?;
    read_image(readable, include_body).map_err(map_image_read_error)
}

fn map_image_read_error(error: ImageReadError) -> ProvisionalDecorativeError {
    match error {
        ImageReadError::UnsupportedImage => ProvisionalDecorativeError::UnsupportedImage,
        ImageReadError::ReadFailed => ProvisionalDecorativeError::ReadFailed,
    }
}

fn map_resolve_error(error: ResolveError) -> ProvisionalDecorativeError {
    match error {
        ResolveError::InvalidPath
        | ResolveError::UnsupportedNamespace
        | ResolveError::UnboundRoot
        | ResolveError::UnexpectedObjectType { .. } => ProvisionalDecorativeError::InvalidPath,
        ResolveError::NotFound | ResolveError::Unavailable => {
            ProvisionalDecorativeError::Unavailable
        }
        ResolveError::AccessDenied | ResolveError::IoFailure => {
            ProvisionalDecorativeError::ReadFailed
        }
    }
}

fn preview_url(selection_id: &str) -> String {
    opaque_image_url(PREVIEW_PROTOCOL_SCHEME, selection_id)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tauri::http::{Method, Request, StatusCode};

    use super::*;

    const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\npreview";
    const JPEG_BYTES: &[u8] = b"\xff\xd8\xffpreview\xff\xd9";

    fn write_source(directory: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = directory.join(name);
        fs::write(&path, bytes).expect("the linked image fixture is writable");
        path
    }

    fn request(method: Method, uri: &str) -> Request<Vec<u8>> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Vec::new())
            .expect("the opaque preview URI is valid")
    }

    #[test]
    fn cancelling_the_native_picker_does_not_change_registered_selections() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let selected = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Background.png",
                PNG_BYTES,
            )))
            .expect("a selected image is registered")
            .expect("the picker returned one image");

        assert_eq!(registry.len(), 1);
        assert!(
            registry
                .register_dialog_selection(None)
                .expect("picker cancellation is a valid outcome")
                .is_none()
        );
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(&selected.selection_id));
    }

    #[test]
    fn creation_resolution_replaces_tokens_with_reversible_paths_without_consuming_them() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let source_path = write_source(directory.path(), "Background.png", PNG_BYTES);
        let registry = ProvisionalDecorativeRegistry::default();
        let selected = registry
            .register_dialog_selection(Some(source_path.clone()))
            .expect("selection is accepted")
            .expect("an image was selected");
        let configuration = ProvisionalProjectCreationConfiguration {
            document: InitialDocumentConfiguration {
                display_unit: crate::project_bootstrap::InitialDisplayUnit::Cm,
                sheet_width_um: 600_000,
                sheet_height_um: 300_000,
                dpi: 300,
                bleed_um: 3_000,
                safety_um: 3_000,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 2,
                first_sheet: crate::project_bootstrap::InitialSheetFormat::Double,
                last_sheet: crate::project_bootstrap::InitialSheetFormat::Double,
            },
            visual_defaults: ProvisionalVisualDefaults {
                background: ProvisionalBackground::BothSides {
                    both: ProvisionalBackgroundContent::Image {
                        selection_id: selected.selection_id.clone(),
                    },
                },
                overlay: ProvisionalOverlay::BothSides {
                    both: Some(ProvisionalOverlayContent::Image {
                        selection_id: selected.selection_id.clone(),
                    }),
                },
                frame_border: InitialFrameBorder::None,
            },
        };

        let resolved = registry
            .resolve_creation_configuration(configuration)
            .expect("registered tokens resolve");
        let InitialBackground::BothSides {
            both: InitialBackgroundContent::Image { native_path },
        } = resolved.visual_defaults.background
        else {
            panic!("the resolved background keeps its scope and image kind");
        };
        assert_eq!(native_path.as_path(), source_path);
        let InitialOverlay::BothSides {
            both: Some(InitialOverlayContent::Image { native_path }),
        } = resolved.visual_defaults.overlay
        else {
            panic!("the resolved overlay keeps its scope and image kind");
        };
        assert_eq!(native_path.as_path(), source_path);
        assert!(registry.contains(&selected.selection_id));
    }

    #[test]
    fn creation_resolution_rejects_unknown_tokens_without_mutating_the_registry() {
        let registry = ProvisionalDecorativeRegistry::default();
        let configuration = ProvisionalProjectCreationConfiguration {
            document: InitialDocumentConfiguration {
                display_unit: crate::project_bootstrap::InitialDisplayUnit::Cm,
                sheet_width_um: 600_000,
                sheet_height_um: 300_000,
                dpi: 300,
                bleed_um: 3_000,
                safety_um: 3_000,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 2,
                first_sheet: crate::project_bootstrap::InitialSheetFormat::Double,
                last_sheet: crate::project_bootstrap::InitialSheetFormat::Double,
            },
            visual_defaults: ProvisionalVisualDefaults {
                background: ProvisionalBackground::BothSides {
                    both: ProvisionalBackgroundContent::Image {
                        selection_id: "released-token".into(),
                    },
                },
                overlay: ProvisionalOverlay::BothSides { both: None },
                frame_border: InitialFrameBorder::None,
            },
        };

        assert_eq!(
            registry.resolve_creation_configuration(configuration),
            Err(ProvisionalDecorativeError::UnknownSelection)
        );
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn selected_images_expose_only_an_opaque_token_name_and_preview_url() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let selected = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Árvore.png",
                PNG_BYTES,
            )))
            .expect("a selected image is registered")
            .expect("the picker returned one image");

        let encoded = serde_json::to_value(&selected).expect("selection serializes");
        assert_eq!(encoded["displayName"], "Árvore.png");
        assert_eq!(encoded["selectionId"], selected.selection_id);
        assert_eq!(encoded["previewUrl"], selected.preview_url);
        assert_eq!(
            encoded.as_object().expect("selection is an object").len(),
            3
        );
        assert!(encoded.get("path").is_none());
        assert!(encoded.get("nativePath").is_none());
        assert!(
            !encoded
                .to_string()
                .contains(directory.path().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn opaque_protocol_serves_jpeg_and_png_for_get_and_head_without_creating_files() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let png = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Overlay.png",
                PNG_BYTES,
            )))
            .expect("PNG selection is accepted")
            .expect("PNG was selected");
        let jpeg = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Background.jpg",
                JPEG_BYTES,
            )))
            .expect("JPEG selection is accepted")
            .expect("JPEG was selected");
        let original_entries = fs::read_dir(directory.path())
            .expect("source directory is readable")
            .count();

        let png_response =
            registry.serve(GLOBAL_WINDOW_LABEL, request(Method::GET, &png.preview_url));
        assert_eq!(png_response.status(), StatusCode::OK);
        assert_eq!(png_response.headers()["content-type"], "image/png");
        assert_eq!(png_response.body(), PNG_BYTES);

        let jpeg_response = registry.serve(
            GLOBAL_WINDOW_LABEL,
            request(Method::HEAD, &jpeg.preview_url),
        );
        assert_eq!(jpeg_response.status(), StatusCode::OK);
        assert_eq!(jpeg_response.headers()["content-type"], "image/jpeg");
        assert_eq!(
            jpeg_response.headers()["content-length"]
                .to_str()
                .expect("content length is textual"),
            JPEG_BYTES.len().to_string()
        );
        assert!(jpeg_response.body().is_empty());
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("source directory remains readable")
                .count(),
            original_entries,
            "serving a provisional preview must not create a copy, Cache or temporary file"
        );
    }

    #[test]
    fn opaque_protocol_hides_tokens_from_other_windows_and_after_release() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let selected = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Background.png",
                PNG_BYTES,
            )))
            .expect("selection is accepted")
            .expect("an image was selected");

        assert_eq!(
            registry
                .serve(
                    GLOBAL_WINDOW_LABEL,
                    request(Method::GET, &selected.preview_url)
                )
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            registry
                .serve("project", request(Method::GET, &selected.preview_url))
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            registry
                .serve(
                    GLOBAL_WINDOW_LABEL,
                    request(
                        Method::GET,
                        "http://myalbuns-preview.localhost/forged-token"
                    )
                )
                .status(),
            StatusCode::NOT_FOUND
        );
        assert!(registry.release(&selected.selection_id));
        assert_eq!(
            registry
                .serve(
                    GLOBAL_WINDOW_LABEL,
                    request(Method::GET, &selected.preview_url)
                )
                .status(),
            StatusCode::NOT_FOUND
        );
        assert!(!registry.release(&selected.selection_id));
    }

    #[test]
    fn clearing_the_registry_revokes_every_provisional_preview() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let first = registry
            .register_dialog_selection(Some(write_source(directory.path(), "First.png", PNG_BYTES)))
            .expect("first selection is accepted")
            .expect("first image was selected");
        let second = registry
            .register_dialog_selection(Some(write_source(
                directory.path(),
                "Second.jpg",
                JPEG_BYTES,
            )))
            .expect("second selection is accepted")
            .expect("second image was selected");

        assert_eq!(registry.clear(), 2);
        for selection in [first, second] {
            assert_eq!(
                registry
                    .serve(
                        GLOBAL_WINDOW_LABEL,
                        request(Method::GET, &selection.preview_url)
                    )
                    .status(),
                StatusCode::NOT_FOUND
            );
        }
        assert_eq!(registry.clear(), 0);
    }

    #[test]
    fn unsupported_content_is_rejected_without_authorizing_a_preview() {
        let directory = tempfile::tempdir().expect("temporary linked image directory");
        let registry = ProvisionalDecorativeRegistry::default();
        let disguised = write_source(directory.path(), "Not-an-image.png", b"<html></html>");

        let error = registry
            .register_dialog_selection(Some(disguised))
            .expect_err("content, rather than the extension, authorizes a preview");

        assert_eq!(error, ProvisionalDecorativeError::UnsupportedImage);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn non_get_methods_are_refused_without_revealing_registry_membership() {
        let registry = ProvisionalDecorativeRegistry::default();
        let response = registry.serve(
            GLOBAL_WINDOW_LABEL,
            request(Method::POST, "http://myalbuns-preview.localhost/any-token"),
        );

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()["allow"], "GET, HEAD");
        assert!(response.body().is_empty());
    }
}
