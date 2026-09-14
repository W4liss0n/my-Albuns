use crate::jpeg_output::{JpegFailure, SRGB_2014, VerifiedJpeg, opaque_rgb_bytes};
use image::RgbaImage;
use myalbuns_imaging_protocol::ImagingFailureCode;
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::Path,
};

fn failed(error: impl std::fmt::Display) -> JpegFailure {
    JpegFailure::new(ImagingFailureCode::EncodeFailed, error.to_string())
}

fn io_failed(error: std::io::Error) -> JpegFailure {
    JpegFailure::io("não foi possível preparar a Exportação", &error)
}

fn png_failed(error: png::EncodingError) -> JpegFailure {
    match error {
        png::EncodingError::IoError(error) => io_failed(error),
        other => failed(other),
    }
}

pub(crate) fn receipt(path: &Path) -> Result<VerifiedJpeg, JpegFailure> {
    let mut reader = BufReader::new(File::open(path).map_err(failed)?);
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0_u8; 65536];
    loop {
        let count = reader.read(&mut buffer).map_err(failed)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(VerifiedJpeg {
        output_bytes: bytes,
        output_sha256: format!("{:x}", hash.finalize()),
    })
}

pub(crate) fn write_png(
    image: &RgbaImage,
    path: &Path,
    dpi: u32,
) -> Result<VerifiedJpeg, JpegFailure> {
    let rgb = opaque_rgb_bytes(image)?;
    let file = crate::export_output::create_output(path).map_err(io_failed)?;
    let mut writer = BufWriter::new(file);
    let mut info = png::Info::with_size(image.width(), image.height());
    info.color_type = png::ColorType::Rgb;
    info.bit_depth = png::BitDepth::Eight;
    info.icc_profile = Some(Cow::Borrowed(SRGB_2014));
    let ppm = (dpi * 10000 + 127) / 254;
    info.pixel_dims = Some(png::PixelDimensions {
        xppu: ppm,
        yppu: ppm,
        unit: png::Unit::Meter,
    });
    let mut encoder = png::Encoder::with_info(&mut writer, info)
        .map_err(png_failed)?
        .write_header()
        .map_err(png_failed)?;
    encoder.write_image_data(&rgb).map_err(png_failed)?;
    encoder.finish().map_err(png_failed)?;
    writer.flush().map_err(io_failed)?;
    writer.get_ref().sync_all().map_err(io_failed)?;
    drop(writer);
    let mut decoder = png::Decoder::new(BufReader::new(File::open(path).map_err(failed)?))
        .read_info()
        .map_err(failed)?;
    let header = decoder.info();
    if header.width != image.width()
        || header.height != image.height()
        || header.icc_profile.as_deref() != Some(SRGB_2014)
        || !header.pixel_dims.is_some_and(|density| {
            density.xppu == ppm && density.yppu == ppm && density.unit == png::Unit::Meter
        })
    {
        return Err(JpegFailure::new(
            ImagingFailureCode::VerificationFailed,
            "o PNG não conservou dimensões, perfil e DPI",
        ));
    }
    let mut decoded = Vec::new();
    decoded.try_reserve_exact(rgb.len()).map_err(failed)?;
    decoded.resize(rgb.len(), 0);
    let frame = decoder.next_frame(&mut decoded).map_err(failed)?;
    if frame.color_type != png::ColorType::Rgb || decoded != rgb {
        return Err(JpegFailure::new(
            ImagingFailureCode::VerificationFailed,
            "o PNG não reproduziu o raster canônico",
        ));
    }
    receipt(path)
}

/// A deliberately small PDF writer: one lossless, ICCBased raster per physical page.
/// Streams are written page by page; the document never retains the album's rasters.
pub(crate) struct PdfOutput {
    writer: BufWriter<crate::export_output::OutputFile>,
    offsets: Vec<u64>,
    pages: Vec<usize>,
}

impl PdfOutput {
    pub(crate) fn create(path: &Path) -> Result<Self, JpegFailure> {
        let file = crate::export_output::create_output(path).map_err(io_failed)?;
        let mut result = Self {
            writer: BufWriter::new(file),
            offsets: vec![0; 4],
            pages: Vec::new(),
        };
        result
            .writer
            .write_all(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
            .map_err(io_failed)?;
        result.object(1, b"<< /Type /Catalog /Pages 2 0 R >>")?;
        result.stream(3, "/N 3 /Alternate /DeviceRGB", SRGB_2014)?;
        Ok(result)
    }
    fn begin_object(&mut self, id: usize) -> Result<(), JpegFailure> {
        self.offsets.resize(self.offsets.len().max(id + 1), 0);
        self.offsets[id] = self.writer.stream_position().map_err(io_failed)?;
        writeln!(self.writer, "{id} 0 obj").map_err(io_failed)
    }
    fn object(&mut self, id: usize, contents: &[u8]) -> Result<(), JpegFailure> {
        self.begin_object(id)?;
        self.writer.write_all(contents).map_err(io_failed)?;
        self.writer.write_all(b"\nendobj\n").map_err(io_failed)
    }
    fn stream(&mut self, id: usize, dictionary: &str, data: &[u8]) -> Result<(), JpegFailure> {
        self.begin_object(id)?;
        writeln!(
            self.writer,
            "<< {dictionary} /Length {} >>\nstream",
            data.len()
        )
        .map_err(io_failed)?;
        self.writer.write_all(data).map_err(io_failed)?;
        self.writer
            .write_all(b"\nendstream\nendobj\n")
            .map_err(io_failed)
    }
    pub(crate) fn add_page(
        &mut self,
        image: &RgbaImage,
        width_um: i64,
        height_um: i64,
    ) -> Result<(), JpegFailure> {
        let rgb = opaque_rgb_bytes(image)?;
        let mut compressed =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(&rgb).map_err(io_failed)?;
        let compressed = compressed.finish().map_err(io_failed)?;
        // Verify the lossless payload before it becomes part of a prepared document.
        let mut decoded = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let mut buffer = [0_u8; 65536];
        let mut offset = 0;
        loop {
            let count = decoded.read(&mut buffer).map_err(io_failed)?;
            if count == 0 {
                break;
            }
            if rgb.get(offset..offset + count) != Some(&buffer[..count]) {
                return Err(JpegFailure::new(
                    ImagingFailureCode::VerificationFailed,
                    "o raster PDF não foi conservado",
                ));
            }
            offset += count;
        }
        if offset != rgb.len() {
            return Err(JpegFailure::new(
                ImagingFailureCode::VerificationFailed,
                "o raster PDF está incompleto",
            ));
        }
        let id = 4 + self.pages.len() * 3;
        let width = pdf_points(width_um);
        let height = pdf_points(height_um);
        self.object(id, format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /CropBox [0 0 {width} {height}] /Resources << /XObject << /Im {} 0 R >> >> /Contents {} 0 R >>", id + 1, id + 2).as_bytes())?;
        self.stream(id + 1, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /BitsPerComponent 8 /ColorSpace [/ICCBased 3 0 R] /Filter /FlateDecode", image.width(), image.height()), &compressed)?;
        self.stream(
            id + 2,
            "",
            format!("q\n{width} 0 0 {height} 0 0 cm\n/Im Do\nQ\n").as_bytes(),
        )?;
        self.pages.push(id);
        Ok(())
    }
    pub(crate) fn finish(mut self, path: &Path) -> Result<VerifiedJpeg, JpegFailure> {
        let kids = self
            .pages
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        self.object(
            2,
            format!(
                "<< /Type /Pages /Count {} /Kids [{kids}] >>",
                self.pages.len()
            )
            .as_bytes(),
        )?;
        let xref = self.writer.stream_position().map_err(io_failed)?;
        writeln!(
            self.writer,
            "xref\n0 {}\n0000000000 65535 f ",
            self.offsets.len()
        )
        .map_err(io_failed)?;
        for offset in &self.offsets[1..] {
            if *offset > 9_999_999_999 {
                return Err(failed("o PDF excedeu o tamanho suportado"));
            }
            writeln!(self.writer, "{offset:010} 00000 n ").map_err(io_failed)?;
        }
        writeln!(
            self.writer,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
            self.offsets.len()
        )
        .map_err(io_failed)?;
        self.writer.flush().map_err(io_failed)?;
        self.writer.get_ref().sync_all().map_err(io_failed)?;
        drop(self);
        receipt(path)
    }
}

fn pdf_points(um: i64) -> String {
    let scaled = (i128::from(um) * 72 * 1_000_000 + 12_700) / 25_400;
    let text = format!("{}.{:06}", scaled / 1_000_000, scaled % 1_000_000);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
