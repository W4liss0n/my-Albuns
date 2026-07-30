use std::{
    fs,
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use myalbuns_imaging_protocol::MediaSource;
use sha2::{Digest, Sha256};

pub(crate) fn read_verified_source(source: &MediaSource) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(source.source_path()).map_err(|error| {
        format!(
            "não foi possível abrir a mídia {}: {error}",
            source.media_id()
        )
    })?;
    if !metadata.is_file() || metadata.len() != source.source_bytes() {
        return Err(source_changed_error(source));
    }
    let bytes = fs::read(source.source_path())
        .map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    if bytes.len() as u64 != source.source_bytes()
        || !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(bytes)
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let file =
        File::open(path).map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn verify_source_current(source: &MediaSource) -> Result<(), String> {
    let metadata = fs::metadata(source.source_path()).map_err(|_| source_changed_error(source))?;
    if !metadata.is_file()
        || metadata.len() != source.source_bytes()
        || !sha256_file(source.source_path())?.eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(())
}

pub(crate) fn decode_jpeg(media_id: &str, verified_bytes: &[u8]) -> Result<RgbaImage, String> {
    let reader = ImageReader::new(Cursor::new(verified_bytes))
        .with_guessed_format()
        .map_err(|error| format!("não foi possível inspecionar a Foto {media_id}: {error}"))?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Err(format!("a mídia {media_id} não é JPEG"));
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| format!("não foi possível preparar o decoder JPEG: {error}"))?;
    let orientation = decoder
        .orientation()
        .map_err(|error| format!("não foi possível ler a orientação EXIF: {error}"))?;
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("não foi possível decodificar a Foto: {error}"))?;
    decoded.apply_orientation(orientation);
    Ok(decoded.to_rgba8())
}

fn source_changed_error(source: &MediaSource) -> String {
    format!(
        "a mídia {} mudou desde o planejamento do Cache",
        source.media_id()
    )
}
