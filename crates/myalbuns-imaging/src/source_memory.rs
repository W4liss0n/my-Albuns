//! Header facts the Host uses to size memory admission before the Processor
//! decodes an Original.
use std::io::Read;

/// Whether the source is a sequential (baseline or extended) JPEG with three
/// colour components. The Processor decodes those straight to RGB before
/// reducing them, which needs far less memory per pixel than the progressive,
/// grayscale, PNG and TIFF paths. Anything unexpected answers `false`, so the
/// caller keeps its conservative estimate.
pub fn is_sequential_color_jpeg(mut reader: impl Read) -> bool {
    let mut read_byte = || {
        let mut byte = [0_u8; 1];
        reader.read_exact(&mut byte).ok().map(|()| byte[0])
    };
    if (read_byte(), read_byte()) != (Some(0xFF), Some(0xD8)) {
        return false;
    }
    loop {
        if read_byte() != Some(0xFF) {
            return false;
        }
        let marker = loop {
            match read_byte() {
                Some(0xFF) => continue,
                Some(marker) => break marker,
                None => return false,
            }
        };
        match marker {
            // Standalone markers carry no length.
            0x01 | 0xD0..=0xD8 => continue,
            // Image data or its end before any frame header.
            0xD9 | 0xDA => return false,
            _ => {}
        }
        let (Some(high), Some(low)) = (read_byte(), read_byte()) else {
            return false;
        };
        let Some(payload) = (usize::from(high) << 8 | usize::from(low)).checked_sub(2) else {
            return false;
        };
        let frame = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if frame {
            // Precision, height and width precede the component count.
            for _ in 0..5 {
                if read_byte().is_none() {
                    return false;
                }
            }
            return matches!(marker, 0xC0 | 0xC1) && read_byte() == Some(3);
        }
        for _ in 0..payload {
            if read_byte().is_none() {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{ColorType, ImageEncoder, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};

    use super::is_sequential_color_jpeg;

    fn jpeg(color: ColorType, channels: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        JpegEncoder::new(&mut bytes)
            .write_image(&vec![90; 32 * 24 * channels], 32, 24, color.into())
            .unwrap();
        bytes
    }

    #[test]
    fn only_sequential_color_jpeg_takes_the_lighter_memory_estimate() {
        assert!(is_sequential_color_jpeg(
            jpeg(ColorType::Rgb8, 3).as_slice()
        ));
        assert!(!is_sequential_color_jpeg(jpeg(ColorType::L8, 1).as_slice()));
        assert!(!is_sequential_color_jpeg(
            include_bytes!("../tests/fixtures/progressive-420-dri.jpg").as_slice()
        ));

        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&[90; 32 * 24 * 3], 32, 24, ColorType::Rgb8.into())
            .unwrap();
        assert!(!is_sequential_color_jpeg(png.as_slice()));
        let truncated = jpeg(ColorType::Rgb8, 3);
        assert!(!is_sequential_color_jpeg(&truncated[..6]));
    }
}
