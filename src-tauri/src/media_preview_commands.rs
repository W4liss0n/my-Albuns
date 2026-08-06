use tauri::{State, WebviewWindow};

use crate::{
    ipc_contract::{MediaPreview, MediaPreviewCommandError, MediaPreviewCommandErrorCode},
    linked_media_previews::{LinkedMediaPreviewError, LinkedMediaPreviewRegistry},
    product_runtime::PROJECT_WINDOW_LABEL,
    project_host::ProjectHost,
};

impl From<LinkedMediaPreviewError> for MediaPreviewCommandError {
    fn from(error: LinkedMediaPreviewError) -> Self {
        let code = match error {
            LinkedMediaPreviewError::Unavailable => MediaPreviewCommandErrorCode::Unavailable,
            LinkedMediaPreviewError::UnsupportedImage => {
                MediaPreviewCommandErrorCode::UnsupportedImage
            }
            LinkedMediaPreviewError::ReadFailed => MediaPreviewCommandErrorCode::ReadFailed,
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

impl MediaPreviewCommandError {
    fn read_failed() -> Self {
        LinkedMediaPreviewError::ReadFailed.into()
    }
}

#[tauri::command]
pub(crate) async fn prepare_media_previews(
    window: WebviewWindow,
    state: State<'_, ProjectHost>,
    registry: State<'_, LinkedMediaPreviewRegistry>,
) -> Result<Option<Vec<MediaPreview>>, MediaPreviewCommandError> {
    if window.label() != PROJECT_WINDOW_LABEL {
        return Err(MediaPreviewCommandError::read_failed());
    }
    let sources = state
        .linked_media_sources()
        .map_err(|_| MediaPreviewCommandError::read_failed())?;
    let registry = registry.inner().clone();
    let previews = tauri::async_runtime::spawn_blocking(move || registry.prepare(sources))
        .await
        .map_err(|_| MediaPreviewCommandError::read_failed())?
        .map_err(MediaPreviewCommandError::from)?;
    Ok(Some(previews))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        ipc_contract::MediaPreviewCommandError, linked_media_previews::LinkedMediaPreviewError,
    };

    #[test]
    fn linked_preview_failures_have_one_closed_serialized_contract() {
        for (error, expected) in [
            (
                LinkedMediaPreviewError::Unavailable,
                json!({
                    "code": "unavailable",
                    "message": "Uma Imagem decorativa vinculada não está disponível no local registrado.",
                }),
            ),
            (
                LinkedMediaPreviewError::UnsupportedImage,
                json!({
                    "code": "unsupported_image",
                    "message": "Uma Imagem decorativa vinculada deixou de ser um arquivo JPEG ou PNG.",
                }),
            ),
            (
                LinkedMediaPreviewError::ReadFailed,
                json!({
                    "code": "read_failed",
                    "message": "Não foi possível ler uma Imagem decorativa vinculada.",
                }),
            ),
        ] {
            assert_eq!(
                serde_json::to_value(MediaPreviewCommandError::from(error))
                    .expect("the media preview failure serializes"),
                expected
            );
        }
    }
}
