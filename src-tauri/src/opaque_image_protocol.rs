use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use tauri::{Runtime, UriSchemeContext, UriSchemeResponder};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageFormat {
    Jpeg,
    Png,
}

impl ImageFormat {
    const fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageReadError {
    UnsupportedImage,
    ReadFailed,
}

pub(crate) struct ImagePayload {
    format: ImageFormat,
    source_bytes: u64,
    body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageRequestError {
    NotFound,
    UnsupportedImage,
}

pub(crate) fn sniff_image(file: &File) -> Result<ImageFormat, ImageReadError> {
    let mut readable = file.try_clone().map_err(|_| ImageReadError::ReadFailed)?;
    sniff_reader(&mut readable)
}

pub(crate) fn read_image(
    mut readable: File,
    include_body: bool,
) -> Result<ImagePayload, ImageReadError> {
    let source_bytes = readable
        .metadata()
        .map_err(|_| ImageReadError::ReadFailed)?
        .len();
    let format = sniff_reader(&mut readable)?;
    if !include_body {
        return Ok(ImagePayload {
            format,
            source_bytes,
            body: Vec::new(),
        });
    }

    readable
        .seek(SeekFrom::Start(0))
        .map_err(|_| ImageReadError::ReadFailed)?;
    let source_len = usize::try_from(source_bytes).map_err(|_| ImageReadError::ReadFailed)?;
    let mut body = Vec::new();
    body.try_reserve_exact(source_len)
        .map_err(|_| ImageReadError::ReadFailed)?;
    body.resize(source_len, 0);
    readable
        .read_exact(&mut body)
        .map_err(|_| ImageReadError::ReadFailed)?;
    let mut unexpected_growth = [0_u8; 1];
    if readable
        .read(&mut unexpected_growth)
        .map_err(|_| ImageReadError::ReadFailed)?
        != 0
    {
        return Err(ImageReadError::ReadFailed);
    }

    Ok(ImagePayload {
        format,
        source_bytes,
        body,
    })
}

fn sniff_reader(readable: &mut File) -> Result<ImageFormat, ImageReadError> {
    readable
        .seek(SeekFrom::Start(0))
        .map_err(|_| ImageReadError::ReadFailed)?;
    let mut signature = [0_u8; 8];
    let read = readable
        .read(&mut signature)
        .map_err(|_| ImageReadError::ReadFailed)?;
    if read >= PNG_SIGNATURE.len() && &signature == PNG_SIGNATURE {
        return Ok(ImageFormat::Png);
    }
    if read >= 3 && signature[..3] == [0xff, 0xd8, 0xff] {
        return Ok(ImageFormat::Jpeg);
    }
    Err(ImageReadError::UnsupportedImage)
}

pub(crate) fn serve_opaque_image(
    allowed_webview_label: &str,
    webview_label: &str,
    request: tauri::http::Request<Vec<u8>>,
    read_source: impl FnOnce(&str, bool) -> Result<ImagePayload, ImageRequestError>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Method, StatusCode};

    if !matches!(request.method(), &Method::GET | &Method::HEAD) {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED, Some(("allow", "GET, HEAD")));
    }
    if webview_label != allowed_webview_label {
        return empty_response(StatusCode::NOT_FOUND, None);
    }
    let Some(token) = token_from_path(request.uri().path()) else {
        return empty_response(StatusCode::NOT_FOUND, None);
    };
    let include_body = request.method() == Method::GET;
    match read_source(token, include_body) {
        Ok(payload) => success_response(payload),
        Err(ImageRequestError::UnsupportedImage) => {
            empty_response(StatusCode::UNSUPPORTED_MEDIA_TYPE, None)
        }
        Err(ImageRequestError::NotFound) => empty_response(StatusCode::NOT_FOUND, None),
    }
}

pub(crate) fn opaque_image_url(scheme: &str, token: &str) -> String {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        format!("http://{scheme}.localhost/{token}")
    } else {
        format!("{scheme}://localhost/{token}")
    }
}

pub(crate) fn respond_to_opaque_image_request<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
    serve: impl FnOnce(&str, tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>>
    + Send
    + 'static,
) {
    let webview_label = context.webview_label().to_owned();
    let _image_response = std::thread::spawn(move || {
        responder.respond(serve(&webview_label, request));
    });
}

fn token_from_path(path: &str) -> Option<&str> {
    let token = path.strip_prefix('/')?;
    (!token.is_empty() && !token.contains('/')).then_some(token)
}

fn success_response(payload: ImagePayload) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(tauri::http::StatusCode::OK)
        .header("content-type", payload.format.content_type())
        .header("content-length", payload.source_bytes.to_string())
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(payload.body)
        .expect("the fixed opaque image response is valid")
}

fn empty_response(
    status: tauri::http::StatusCode,
    header: Option<(&'static str, &'static str)>,
) -> tauri::http::Response<Vec<u8>> {
    let mut response = tauri::http::Response::builder().status(status);
    if let Some((name, value)) = header {
        response = response.header(name, value);
    }
    response
        .header("content-length", "0")
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Vec::new())
        .expect("the fixed empty opaque image response is valid")
}
