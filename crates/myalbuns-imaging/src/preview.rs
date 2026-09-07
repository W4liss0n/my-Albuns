//! Canonical validation of the reduced representation, independent of its owner.
use std::io::{BufRead, Seek, SeekFrom};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use myalbuns_imaging_protocol::{CACHE_MAX_EDGE_PX, CacheArtifactFormat};

pub const SRGB_PROFILE: &[u8] = include_bytes!("../assets/sRGB2014.icc");

#[derive(Clone, Copy, Debug)]
pub struct CachePreviewSpec {
    pub format: CacheArtifactFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub bytes: u64,
}

/// Validates the bytes opened by the caller. The caller retains authority over
/// the opening, source binding, digest, publication and recovery decision.
pub fn validate_cache_preview(
    mut reader: impl BufRead + Seek,
    expected: CachePreviewSpec,
) -> Result<(), String> {
    if expected.width_px == 0
        || expected.height_px == 0
        || expected.width_px > CACHE_MAX_EDGE_PX
        || expected.height_px > CACHE_MAX_EDGE_PX
        || expected.bytes == 0
    {
        return Err("as propriedades da representação reduzida são inválidas".into());
    }
    let bytes = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| error.to_string())?;
    reader.rewind().map_err(|error| error.to_string())?;
    if bytes != expected.bytes {
        return Err("o tamanho da representação reduzida não corresponde ao esperado".into());
    }
    let reader = ImageReader::new(reader)
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let format = match expected.format {
        CacheArtifactFormat::Jpeg => ImageFormat::Jpeg,
        CacheArtifactFormat::Png => ImageFormat::Png,
    };
    if reader.format() != Some(format) {
        return Err("o formato da representação reduzida não corresponde ao esperado".into());
    }
    let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    if decoder.dimensions() != (expected.width_px, expected.height_px)
        || decoder
            .icc_profile()
            .map_err(|error| error.to_string())?
            .as_deref()
            != Some(SRGB_PROFILE)
    {
        return Err("dimensões ou perfil sRGB da representação reduzida são inválidos".into());
    }
    DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor, Write};

    use image::{
        ExtendedColorType, ImageEncoder, RgbImage,
        codecs::{jpeg::JpegEncoder, png::PngEncoder},
    };

    use super::{CacheArtifactFormat, CachePreviewSpec, SRGB_PROFILE, validate_cache_preview};

    fn encoded(format: CacheArtifactFormat, profile: bool) -> Vec<u8> {
        let image = RgbImage::from_pixel(19, 11, image::Rgb([30, 90, 180]));
        let mut bytes = Vec::new();
        match format {
            CacheArtifactFormat::Jpeg => {
                let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 84);
                if profile {
                    encoder.set_icc_profile(SRGB_PROFILE.to_vec()).unwrap();
                }
                encoder.encode_image(&image).unwrap();
            }
            CacheArtifactFormat::Png => {
                let mut encoder = PngEncoder::new(&mut bytes);
                if profile {
                    encoder.set_icc_profile(SRGB_PROFILE.to_vec()).unwrap();
                }
                encoder
                    .write_image(image.as_raw(), 19, 11, ExtendedColorType::Rgb8)
                    .unwrap();
            }
        }
        bytes
    }

    #[test]
    fn both_opening_boundaries_enforce_the_same_preview_policy() {
        for format in [CacheArtifactFormat::Jpeg, CacheArtifactFormat::Png] {
            let valid = encoded(format, true);
            let expected = CachePreviewSpec {
                format,
                width_px: 19,
                height_px: 11,
                bytes: valid.len() as u64,
            };
            let wrong_format = match format {
                CacheArtifactFormat::Jpeg => CacheArtifactFormat::Png,
                CacheArtifactFormat::Png => CacheArtifactFormat::Jpeg,
            };
            let mut corrupt = valid.clone();
            corrupt[0] = 0;
            let unprofiled = encoded(format, false);
            for (bytes, spec, accepted) in [
                (valid.clone(), expected, true),
                (
                    valid.clone(),
                    CachePreviewSpec {
                        format: wrong_format,
                        ..expected
                    },
                    false,
                ),
                (
                    valid.clone(),
                    CachePreviewSpec {
                        width_px: 18,
                        ..expected
                    },
                    false,
                ),
                (
                    valid.clone(),
                    CachePreviewSpec {
                        bytes: expected.bytes + 1,
                        ..expected
                    },
                    false,
                ),
                (
                    unprofiled.clone(),
                    CachePreviewSpec {
                        bytes: unprofiled.len() as u64,
                        ..expected
                    },
                    false,
                ),
                (corrupt, expected, false),
                (valid[..valid.len() / 2].to_vec(), expected, false),
            ] {
                // Host owns verified in-memory bytes; Processor owns an authorized file.
                assert_eq!(
                    validate_cache_preview(Cursor::new(&bytes), spec).is_ok(),
                    accepted
                );
                let mut file = tempfile::tempfile().unwrap();
                file.write_all(&bytes).unwrap();
                assert_eq!(
                    validate_cache_preview(BufReader::new(file), spec).is_ok(),
                    accepted
                );
            }
        }
    }
}
