use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

use image::{
    ExtendedColorType, ImageEncoder, Rgba, RgbaImage,
    codecs::jpeg::{JpegEncoder, PixelDensity},
};
use myalbuns_imaging_protocol::ImagingFailureCode;
use sha2::{Digest, Sha256};

const MICROMETERS_PER_INCH: i128 = 25_400;
const ROUNDING_OFFSET: i128 = MICROMETERS_PER_INCH / 2;
const MAX_JPEG_AXIS: u32 = 65_535;
pub(crate) const MAX_OUTPUT_PIXELS: u64 = 134_217_728;
const MAX_JPEG_HEADER_BYTES: usize = 1024 * 1024;
const SRGB_2014: &[u8] = include_bytes!("../assets/sRGB2014.icc");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RasterPlan {
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pixel_count: u64,
    dpi: u32,
}

impl RasterPlan {
    pub(crate) fn new(width_um: i64, height_um: i64, dpi: u32) -> Result<Self, JpegFailure> {
        let width_px = raster_axis(width_um, dpi)?;
        let height_px = raster_axis(height_um, dpi)?;
        let pixel_count = u64::from(width_px)
            .checked_mul(u64::from(height_px))
            .ok_or_else(|| resource_failure("as dimensões raster excederam o intervalo seguro"))?;
        if pixel_count > MAX_OUTPUT_PIXELS {
            return Err(resource_failure(format!(
                "a saída teria {pixel_count} pixels e excede o limite de {MAX_OUTPUT_PIXELS}"
            )));
        }
        Ok(Self {
            width_px,
            height_px,
            pixel_count,
            dpi,
        })
    }

    pub(crate) fn edge_px(self, micrometers: i64) -> Result<i64, JpegFailure> {
        let scaled = i128::from(micrometers)
            .checked_mul(i128::from(self.dpi))
            .ok_or_else(|| resource_failure("a conversão de uma borda excedeu o intervalo"))?;
        let rounded = scaled
            .checked_add(ROUNDING_OFFSET)
            .map(|value| value.div_euclid(MICROMETERS_PER_INCH))
            .ok_or_else(|| resource_failure("a conversão de uma borda excedeu o intervalo"))?;
        i64::try_from(rounded).map_err(|_| resource_failure("uma borda raster excedeu o intervalo"))
    }

    pub(crate) fn allocate_rgba(self, color: Rgba<u8>) -> Result<RgbaImage, JpegFailure> {
        let byte_count = self
            .pixel_count
            .checked_mul(4)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| resource_failure("o raster RGBA excedeu o intervalo seguro"))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(byte_count)
            .map_err(|_| resource_failure("não há memória suficiente para compor a Exportação"))?;
        pixels.resize(byte_count, 0);
        let (pixels_rgba, remainder) = pixels.as_chunks_mut::<4>();
        debug_assert!(
            remainder.is_empty(),
            "the allocated RGBA buffer must contain complete pixels"
        );
        for pixel in pixels_rgba {
            pixel.copy_from_slice(&color.0);
        }
        RgbaImage::from_raw(self.width_px, self.height_px, pixels)
            .ok_or_else(|| resource_failure("não foi possível materializar o raster RGBA"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedJpeg {
    pub(crate) output_bytes: u64,
    pub(crate) output_sha256: String,
}

#[derive(Debug)]
pub(crate) struct JpegFailure {
    pub(crate) code: ImagingFailureCode,
    pub(crate) message: String,
}

impl JpegFailure {
    fn new(code: ImagingFailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub(crate) fn write_verified(
    image: &RgbaImage,
    prepared_output_path: &Path,
    dpi: u32,
) -> Result<VerifiedJpeg, JpegFailure> {
    let rgb = opaque_rgb_bytes(image)?;
    let icc_profile = fallible_copy(
        SRGB_2014,
        "não há memória suficiente para incorporar o perfil sRGB",
    )?;
    let density = u16::try_from(dpi).map_err(|_| {
        JpegFailure::new(
            ImagingFailureCode::EncodeFailed,
            "o DPI não pode ser representado pelo JFIF",
        )
    })?;
    let mut created = false;
    let write_result = (|| {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(prepared_output_path)
            .map_err(|error| {
                JpegFailure::new(
                    ImagingFailureCode::EncodeFailed,
                    format!("não foi possível criar a preparação da Exportação: {error}"),
                )
            })?;
        created = true;
        let mut writer = BufWriter::new(file);
        let mut encoder = JpegEncoder::new_with_quality(&mut writer, 100);
        encoder.set_pixel_density(PixelDensity::dpi(density));
        encoder.set_icc_profile(icc_profile).map_err(|error| {
            JpegFailure::new(
                ImagingFailureCode::EncodeFailed,
                format!("não foi possível incorporar o perfil sRGB: {error}"),
            )
        })?;
        encoder
            .encode(&rgb, image.width(), image.height(), ExtendedColorType::Rgb8)
            .map_err(|error| {
                JpegFailure::new(
                    ImagingFailureCode::EncodeFailed,
                    format!("não foi possível codificar a imagem exportada: {error}"),
                )
            })?;
        drop(encoder);
        writer.flush().map_err(|error| {
            JpegFailure::new(
                ImagingFailureCode::EncodeFailed,
                format!("não foi possível finalizar a imagem exportada: {error}"),
            )
        })?;
        let file = writer.into_inner().map_err(|error| {
            JpegFailure::new(
                ImagingFailureCode::EncodeFailed,
                format!("não foi possível finalizar a imagem exportada: {error}"),
            )
        })?;
        file.sync_all().map_err(|error| {
            JpegFailure::new(
                ImagingFailureCode::EncodeFailed,
                format!("não foi possível sincronizar a imagem exportada: {error}"),
            )
        })?;
        drop(file);
        verify_prepared_jpeg(prepared_output_path, image.width(), image.height(), density)
    })();
    if write_result.is_err() && created {
        let _ = fs::remove_file(prepared_output_path);
    }
    write_result
}

fn raster_axis(micrometers: i64, dpi: u32) -> Result<u32, JpegFailure> {
    if micrometers <= 0 {
        return Err(JpegFailure::new(
            ImagingFailureCode::CompositionFailed,
            "a dimensão física da Lâmina precisa ser positiva",
        ));
    }
    let pixels = i128::from(micrometers)
        .checked_mul(i128::from(dpi))
        .and_then(|value| value.checked_add(ROUNDING_OFFSET))
        .map(|value| value / MICROMETERS_PER_INCH)
        .ok_or_else(|| resource_failure("a conversão da dimensão raster excedeu o intervalo"))?;
    let pixels = u32::try_from(pixels)
        .map_err(|_| resource_failure("a dimensão raster excedeu o intervalo"))?;
    if !(1..=MAX_JPEG_AXIS).contains(&pixels) {
        return Err(resource_failure(format!(
            "a dimensão raster {pixels} está fora de 1..={MAX_JPEG_AXIS}"
        )));
    }
    Ok(pixels)
}

fn opaque_rgb_bytes(image: &RgbaImage) -> Result<Vec<u8>, JpegFailure> {
    let byte_count = u64::from(image.width())
        .checked_mul(u64::from(image.height()))
        .and_then(|value| value.checked_mul(3))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| resource_failure("o raster RGB excedeu o intervalo seguro"))?;
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(byte_count)
        .map_err(|_| resource_failure("não há memória suficiente para codificar o JPEG"))?;
    for pixel in image.pixels() {
        if pixel[3] != u8::MAX {
            return Err(JpegFailure::new(
                ImagingFailureCode::CompositionFailed,
                "a composição final ainda contém transparência",
            ));
        }
        rgb.extend_from_slice(&pixel.0[..3]);
    }
    Ok(rgb)
}

fn verify_prepared_jpeg(
    path: &Path,
    width: u32,
    height: u32,
    dpi: u16,
) -> Result<VerifiedJpeg, JpegFailure> {
    let metadata = fs::symlink_metadata(path).map_err(verify_io_failure)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(verify_failure("a preparação JPEG não é um arquivo regular"));
    }
    let file = File::open(path).map_err(verify_io_failure)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut total_bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    let mut header = Vec::new();
    let mut entropy: Option<EntropyScanner> = None;

    loop {
        let read = reader.read(&mut buffer).map_err(verify_io_failure)?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        hasher.update(chunk);
        total_bytes = total_bytes
            .checked_add(read as u64)
            .ok_or_else(|| verify_failure("o tamanho do JPEG excedeu o intervalo seguro"))?;

        if let Some(scanner) = entropy.as_mut() {
            scanner.feed(chunk)?;
            continue;
        }

        let header_length = header
            .len()
            .checked_add(read)
            .ok_or_else(|| verify_failure("o cabeçalho JPEG excedeu o intervalo seguro"))?;
        if header_length > MAX_JPEG_HEADER_BYTES {
            return Err(verify_failure("o cabeçalho JPEG excedeu o limite seguro"));
        }
        header
            .try_reserve_exact(read)
            .map_err(|_| resource_failure("não há memória suficiente para verificar o JPEG"))?;
        header.extend_from_slice(chunk);
        if let HeaderInspection::Complete { entropy_offset } =
            inspect_header(&header, width, height, dpi)?
        {
            let mut scanner = EntropyScanner::default();
            scanner.feed(&header[entropy_offset..])?;
            entropy = Some(scanner);
        }
    }

    let scanner = entropy.ok_or_else(|| verify_failure("o JPEG não contém scan de imagem"))?;
    scanner.finish()?;
    if total_bytes != metadata.len() {
        return Err(verify_failure(
            "o tamanho do JPEG mudou durante a verificação",
        ));
    }
    Ok(VerifiedJpeg {
        output_bytes: total_bytes,
        output_sha256: format!("{:x}", hasher.finalize()),
    })
}

enum HeaderInspection {
    Incomplete,
    Complete { entropy_offset: usize },
}

fn inspect_header(
    bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
    expected_dpi: u16,
) -> Result<HeaderInspection, JpegFailure> {
    if bytes.len() < 2 {
        return Ok(HeaderInspection::Incomplete);
    }
    if &bytes[..2] != b"\xFF\xD8" {
        return Err(verify_failure("a preparação não começa com JPEG SOI"));
    }

    let mut cursor = 2;
    let mut marker_index = 0_usize;
    let mut jfif_seen = false;
    let mut sof0_seen = false;
    let mut icc_chunks = Vec::new();
    loop {
        if cursor >= bytes.len() {
            return Ok(HeaderInspection::Incomplete);
        }
        if bytes[cursor] != 0xFF {
            return Err(verify_failure("o cabeçalho JPEG contém dados sem marcador"));
        }
        while cursor < bytes.len() && bytes[cursor] == 0xFF {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return Ok(HeaderInspection::Incomplete);
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker_index == 0 && marker != 0xE0 {
            return Err(verify_failure("o JFIF não aparece imediatamente após SOI"));
        }
        marker_index += 1;
        if marker == 0xD9 {
            return Err(verify_failure("o JPEG termina antes do scan de imagem"));
        }
        if cursor + 2 > bytes.len() {
            return Ok(HeaderInspection::Incomplete);
        }
        let segment_length = usize::from(u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]));
        if segment_length < 2 {
            return Err(verify_failure("o JPEG contém segmento inválido"));
        }
        let segment_end = cursor
            .checked_add(segment_length)
            .ok_or_else(|| verify_failure("o segmento JPEG excedeu o intervalo seguro"))?;
        if segment_end > bytes.len() {
            return Ok(HeaderInspection::Incomplete);
        }
        let payload = &bytes[cursor + 2..segment_end];
        cursor = segment_end;

        match marker {
            0xDA => {
                validate_header_contract(jfif_seen, sof0_seen, &mut icc_chunks)?;
                return Ok(HeaderInspection::Complete {
                    entropy_offset: cursor,
                });
            }
            0xE0 => {
                if jfif_seen
                    || payload.len() != 14
                    || !payload.starts_with(b"JFIF\0")
                    || payload[7] != 1
                    || u16::from_be_bytes([payload[8], payload[9]]) != expected_dpi
                    || u16::from_be_bytes([payload[10], payload[11]]) != expected_dpi
                    || payload[12] != 0
                    || payload[13] != 0
                {
                    return Err(verify_failure("o marcador JFIF/DPI é inválido"));
                }
                jfif_seen = true;
            }
            0xE1 | 0xFE => {
                return Err(verify_failure("o JPEG contém metadados proibidos"));
            }
            0xE2 => {
                if payload.len() < 14 || !payload.starts_with(b"ICC_PROFILE\0") {
                    return Err(verify_failure("o APP2 não contém o perfil ICC controlado"));
                }
                icc_chunks.try_reserve(1).map_err(|_| {
                    resource_failure("não há memória suficiente para verificar o perfil ICC")
                })?;
                icc_chunks.push((payload[12], payload[13], &payload[14..]));
            }
            0xE3..=0xEF => {
                return Err(verify_failure(
                    "o JPEG contém metadados APP não autorizados",
                ));
            }
            0xC0 => {
                let expected_width = u16::try_from(expected_width)
                    .map_err(|_| verify_failure("a largura JPEG excedeu o SOF0"))?;
                let expected_height = u16::try_from(expected_height)
                    .map_err(|_| verify_failure("a altura JPEG excedeu o SOF0"))?;
                if sof0_seen
                    || payload.len() != 15
                    || payload[0] != 8
                    || u16::from_be_bytes([payload[1], payload[2]]) != expected_height
                    || u16::from_be_bytes([payload[3], payload[4]]) != expected_width
                    || payload[5] != 3
                {
                    return Err(verify_failure("o marcador JPEG SOF0 é inválido"));
                }
                sof0_seen = true;
            }
            marker if is_other_start_of_frame(marker) => {
                return Err(verify_failure("o JPEG não usa somente baseline SOF0"));
            }
            _ => {}
        }
    }
}

fn validate_header_contract(
    jfif_seen: bool,
    sof0_seen: bool,
    icc_chunks: &mut Vec<(u8, u8, &[u8])>,
) -> Result<(), JpegFailure> {
    if !jfif_seen || !sof0_seen || icc_chunks.is_empty() {
        return Err(verify_failure(
            "o JPEG não contém todos os marcadores obrigatórios",
        ));
    }
    icc_chunks.sort_by_key(|(sequence, _, _)| *sequence);
    let total = icc_chunks[0].1;
    if total == 0
        || usize::from(total) != icc_chunks.len()
        || !icc_chunks
            .iter()
            .enumerate()
            .all(|(index, (sequence, count, _))| {
                usize::from(*sequence) == index + 1 && *count == total
            })
    {
        return Err(verify_failure("a sequência do perfil ICC é inválida"));
    }
    let mut profile = Vec::new();
    profile
        .try_reserve_exact(SRGB_2014.len())
        .map_err(|_| resource_failure("não há memória suficiente para verificar o perfil ICC"))?;
    for (_, _, chunk) in icc_chunks.iter() {
        if profile.len().saturating_add(chunk.len()) > SRGB_2014.len() {
            return Err(verify_failure("o perfil ICC excedeu o tamanho controlado"));
        }
        profile.extend_from_slice(chunk);
    }
    if profile != SRGB_2014 {
        return Err(verify_failure("o perfil ICC não é o sRGB2014 controlado"));
    }
    Ok(())
}

fn is_other_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
    )
}

#[derive(Default)]
struct EntropyScanner {
    pending_marker_prefix: bool,
    eoi_seen: bool,
}

impl EntropyScanner {
    fn feed(&mut self, bytes: &[u8]) -> Result<(), JpegFailure> {
        for &byte in bytes {
            if self.eoi_seen {
                return Err(verify_failure("o JPEG contém dados depois de EOI"));
            }
            if !self.pending_marker_prefix {
                self.pending_marker_prefix = byte == 0xFF;
                continue;
            }
            match byte {
                0x00 => self.pending_marker_prefix = false,
                0xFF => {}
                0xD0..=0xD7 => self.pending_marker_prefix = false,
                0xD9 => {
                    self.pending_marker_prefix = false;
                    self.eoi_seen = true;
                }
                _ => {
                    return Err(verify_failure("o scan JPEG contém um marcador inesperado"));
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<(), JpegFailure> {
        if !self.eoi_seen {
            return Err(verify_failure("o JPEG não termina com EOI"));
        }
        Ok(())
    }
}

fn resource_failure(message: impl Into<String>) -> JpegFailure {
    JpegFailure::new(ImagingFailureCode::ResourceLimitExceeded, message)
}

fn fallible_copy(bytes: &[u8], message: &'static str) -> Result<Vec<u8>, JpegFailure> {
    let mut copied = Vec::new();
    copied
        .try_reserve_exact(bytes.len())
        .map_err(|_| resource_failure(message))?;
    copied.extend_from_slice(bytes);
    Ok(copied)
}

fn verify_failure(message: impl Into<String>) -> JpegFailure {
    JpegFailure::new(ImagingFailureCode::VerificationFailed, message)
}

fn verify_io_failure(error: std::io::Error) -> JpegFailure {
    verify_failure(format!(
        "não foi possível verificar a preparação JPEG: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{MAX_OUTPUT_PIXELS, RasterPlan};

    #[test]
    fn raster_dimensions_use_checked_integer_rounding() {
        let plan = RasterPlan::new(600_000, 300_000, 300).expect("the neutral Sheet fits");
        assert_eq!(plan.width_px, 7_087);
        assert_eq!(plan.height_px, 3_543);
    }

    #[test]
    fn resource_guard_rejects_output_before_allocation() {
        let failure = RasterPlan::new(600_000, 300_000, 1_200)
            .expect_err("the oversized output must be rejected");
        assert_eq!(failure.code.as_str(), "resource_limit_exceeded");
        assert!(failure.message.contains(&MAX_OUTPUT_PIXELS.to_string()));
    }

    #[test]
    fn every_composed_edge_uses_the_same_checked_integer_conversion() {
        let raster = RasterPlan::new(600_000, 300_000, 300).expect("the raster is valid");

        assert_eq!(raster.edge_px(0).expect("zero is exact"), 0);
        assert_eq!(raster.edge_px(300_000).expect("the center is valid"), 3_543);
        assert_eq!(
            raster.edge_px(600_000).expect("the far edge is valid"),
            7_087
        );
        assert_eq!(
            raster.edge_px(-300_000).expect("negative crops are valid"),
            -3_543
        );

        let exact_half = RasterPlan::new(25_400, 25_400, 100).expect("the raster is valid");
        assert_eq!(
            exact_half.edge_px(-127).expect("a negative half is valid"),
            0,
            "the normative formula rounds an exact negative half toward positive infinity"
        );
        assert_eq!(
            exact_half
                .edge_px(-128)
                .expect("a value beyond the negative half is valid"),
            -1
        );
    }
}
