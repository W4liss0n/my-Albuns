use std::{
    fs,
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use myalbuns_imaging_protocol::MediaSource;
use sha2::{Digest, Sha256};

struct DecodedJpeg {
    image: RgbaImage,
    exif_orientation: u8,
}

pub(crate) struct DecodedSource {
    pub(crate) image: RgbaImage,
    pub(crate) exif_orientation: Option<u8>,
}

pub(crate) fn read_verified_source(
    source: &MediaSource,
    operational_path: &Path,
) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(operational_path).map_err(|error| {
        format!(
            "não foi possível abrir a mídia {}: {error}",
            source.media_id()
        )
    })?;
    if !metadata.is_file() || metadata.len() != source.source_bytes() {
        return Err(source_changed_error(source));
    }
    let bytes = fs::read(operational_path)
        .map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    if bytes.len() as u64 != source.source_bytes()
        || !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(bytes)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let file =
        File::open(path).map_err(|error| format!("não foi possível verificar a Foto: {error}"))?;
    sha256_reader(&mut BufReader::new(file))
        .map_err(|error| format!("não foi possível verificar a Foto: {error}"))
}

fn sha256_reader(reader: &mut impl Read) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn verify_source_current(
    source: &MediaSource,
    operational_path: &Path,
) -> Result<(), String> {
    let metadata = fs::metadata(operational_path).map_err(|_| source_changed_error(source))?;
    if !metadata.is_file()
        || metadata.len() != source.source_bytes()
        || !sha256_file(operational_path)?.eq_ignore_ascii_case(source.source_sha256())
    {
        return Err(source_changed_error(source));
    }
    Ok(())
}

fn decode_jpeg(media_id: &str, verified_bytes: &[u8]) -> Result<DecodedJpeg, String> {
    let mut decoder = jpeg_decoder(media_id, verified_bytes)?;
    let orientation = decoder
        .orientation()
        .map_err(|error| format!("não foi possível ler a orientação EXIF: {error}"))?;
    let exif_orientation = orientation.to_exif();
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("não foi possível decodificar a Foto: {error}"))?;
    decoded.apply_orientation(orientation);
    Ok(DecodedJpeg {
        image: decoded.to_rgba8(),
        exif_orientation,
    })
}

pub(crate) fn cache_source_orientation(
    media_id: &str,
    verified_bytes: &[u8],
) -> Result<Option<u8>, String> {
    match source_format(media_id, verified_bytes)? {
        ImageFormat::Jpeg => jpeg_orientation(media_id, verified_bytes).map(Some),
        ImageFormat::Png => Ok(None),
        _ => Err(format!(
            "a mídia {media_id} não usa um formato aceito pelo Cache"
        )),
    }
}

pub(crate) fn decode_source(
    media_id: &str,
    verified_bytes: &[u8],
) -> Result<DecodedSource, String> {
    match source_format(media_id, verified_bytes)? {
        ImageFormat::Jpeg => {
            let decoded = decode_jpeg(media_id, verified_bytes)?;
            Ok(DecodedSource {
                image: decoded.image,
                exif_orientation: Some(decoded.exif_orientation),
            })
        }
        ImageFormat::Png => {
            let decoded = ImageReader::new(Cursor::new(verified_bytes))
                .with_guessed_format()
                .map_err(|error| {
                    format!("não foi possível inspecionar a mídia {media_id}: {error}")
                })?
                .decode()
                .map_err(|error| format!("não foi possível decodificar a mídia: {error}"))?;
            Ok(DecodedSource {
                image: decoded.to_rgba8(),
                exif_orientation: None,
            })
        }
        _ => Err(format!(
            "a mídia {media_id} não usa um formato aceito pelo Cache"
        )),
    }
}

fn jpeg_orientation(media_id: &str, verified_bytes: &[u8]) -> Result<u8, String> {
    let mut decoder = jpeg_decoder(media_id, verified_bytes)?;
    decoder
        .orientation()
        .map(|orientation| orientation.to_exif())
        .map_err(|error| format!("não foi possível ler a orientação EXIF: {error}"))
}

fn source_format(media_id: &str, verified_bytes: &[u8]) -> Result<ImageFormat, String> {
    ImageReader::new(Cursor::new(verified_bytes))
        .with_guessed_format()
        .map_err(|error| format!("não foi possível inspecionar a mídia {media_id}: {error}"))?
        .format()
        .ok_or_else(|| format!("não foi possível identificar o formato da mídia {media_id}"))
}

fn jpeg_decoder<'a>(
    media_id: &str,
    verified_bytes: &'a [u8],
) -> Result<impl ImageDecoder + 'a, String> {
    let reader = ImageReader::new(Cursor::new(verified_bytes))
        .with_guessed_format()
        .map_err(|error| format!("não foi possível inspecionar a Foto {media_id}: {error}"))?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Err(format!("a mídia {media_id} não é JPEG"));
    }
    reader
        .into_decoder()
        .map_err(|error| format!("não foi possível preparar o decoder JPEG: {error}"))
}

fn source_changed_error(source: &MediaSource) -> String {
    format!(
        "a mídia {} mudou desde o planejamento da operação",
        source.media_id()
    )
}
