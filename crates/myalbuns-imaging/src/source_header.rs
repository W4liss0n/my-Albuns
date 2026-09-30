//! Header bytes the Host inspects without reading an Original's image body.
use std::io::Read;

/// Largest JPEG header the Host reads on its own. Beyond it the caller falls
/// back to reading the Original as a whole, which also judges such a file.
pub const JPEG_HEADER_PREFIX_LIMIT: usize = 4 * 1024 * 1024;

/// The bytes of a JPEG from SOI through its first scan header: everything a
/// decoder reads for dimensions, orientation and colour facts, and none of the
/// entropy-coded body. On a network share the body is most of the transfer.
///
/// Answers `None` when the stream is not such a JPEG or its header exceeds
/// `limit`; the caller then reads the Original as before.
pub fn jpeg_header_prefix(mut reader: impl Read, limit: usize) -> Option<Vec<u8>> {
    let mut prefix = Vec::new();
    if read_into(&mut reader, &mut prefix, 2, limit)? != [0xFF, 0xD8] {
        return None;
    }
    loop {
        if read_into(&mut reader, &mut prefix, 1, limit)? != [0xFF] {
            return None;
        }
        let marker = loop {
            match read_into(&mut reader, &mut prefix, 1, limit)?[0] {
                0xFF => continue,
                marker => break marker,
            }
        };
        match marker {
            // Standalone markers carry no length.
            0x01 | 0xD0..=0xD8 => continue,
            // The image ends before any scan.
            0xD9 => return None,
            _ => {}
        }
        let length = read_into(&mut reader, &mut prefix, 2, limit)?;
        let payload = usize::from(u16::from_be_bytes([length[0], length[1]])).checked_sub(2)?;
        read_into(&mut reader, &mut prefix, payload, limit)?;
        if marker == 0xDA {
            return Some(prefix);
        }
    }
}

/// Appends exactly `count` bytes and returns them, or `None` at the end of the
/// stream or past `limit`.
fn read_into<'a>(
    reader: &mut impl Read,
    prefix: &'a mut Vec<u8>,
    count: usize,
    limit: usize,
) -> Option<&'a [u8]> {
    let start = prefix.len();
    if start.checked_add(count)? > limit {
        return None;
    }
    prefix.resize(start + count, 0);
    reader.read_exact(&mut prefix[start..]).ok()?;
    Some(&prefix[start..])
}

#[cfg(test)]
mod tests {
    use image::{ColorType, ImageDecoder, ImageEncoder, ImageReader, codecs::jpeg::JpegEncoder};

    use super::{JPEG_HEADER_PREFIX_LIMIT, jpeg_header_prefix};

    fn rotated_jpeg(width: u32, height: u32) -> Vec<u8> {
        // Little-endian TIFF with one Orientation entry: 6 (rotate 90).
        let exif = vec![
            b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut bytes = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 90);
        encoder.set_exif_metadata(exif).unwrap();
        let pixels = (0..width * height * 3)
            .map(|index| (index * 7 % 251) as u8)
            .collect::<Vec<_>>();
        encoder
            .write_image(&pixels, width, height, ColorType::Rgb8.into())
            .unwrap();
        bytes
    }

    #[test]
    fn the_prefix_carries_every_header_fact_but_not_the_image_body() {
        let jpeg = rotated_jpeg(640, 480);
        let prefix = jpeg_header_prefix(jpeg.as_slice(), JPEG_HEADER_PREFIX_LIMIT).unwrap();
        assert_eq!(prefix, jpeg[..prefix.len()]);
        assert!(
            prefix.len() * 20 < jpeg.len(),
            "{} header bytes of {}",
            prefix.len(),
            jpeg.len()
        );

        let facts = |bytes: &[u8]| {
            let mut decoder = ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .unwrap()
                .into_decoder()
                .unwrap();
            (decoder.dimensions(), decoder.orientation().unwrap())
        };
        assert_eq!(facts(&prefix), facts(&jpeg));
        assert_eq!(
            facts(&prefix).1,
            image::metadata::Orientation::Rotate90,
            "the EXIF segment precedes the scan"
        );
    }

    #[test]
    fn anything_but_a_complete_jpeg_header_within_the_limit_falls_back() {
        let jpeg = rotated_jpeg(64, 48);
        let prefix = jpeg_header_prefix(jpeg.as_slice(), JPEG_HEADER_PREFIX_LIMIT).unwrap();

        assert!(jpeg_header_prefix(&jpeg[..prefix.len() - 1], JPEG_HEADER_PREFIX_LIMIT).is_none());
        assert!(jpeg_header_prefix(jpeg.as_slice(), prefix.len() - 1).is_none());
        assert_eq!(
            jpeg_header_prefix(jpeg.as_slice(), prefix.len()),
            Some(prefix)
        );
        assert!(jpeg_header_prefix(&b"\x89PNG\r\n\x1a\n"[..], JPEG_HEADER_PREFIX_LIMIT).is_none());
        assert!(
            jpeg_header_prefix(&[0xFF, 0xD8, 0xFF, 0xD9][..], JPEG_HEADER_PREFIX_LIMIT).is_none()
        );
    }
}
