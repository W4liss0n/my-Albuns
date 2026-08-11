mod cache;
mod progressive_jpeg;

pub(crate) use cache::{fingerprint_source, verify_source_fingerprint};
pub(crate) use progressive_jpeg::{JPEG_WORKER_MODE, run_jpeg_worker};

use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
};

use image::{
    ColorType, ImageDecoder, ImageError, Limits, RgbaImage,
    codecs::{jpeg::JpegDecoder, png::PngDecoder, tiff::TiffDecoder},
    metadata::Orientation,
};
use myalbuns_imaging_protocol::{
    CACHE_MAX_DECODER_ALLOC_BYTES, CacheBasicColorProfile, ImagingFailureCode, ImagingPathCode,
};
use myalbuns_paths::ResolvedObject;

pub(crate) const MAX_DECODED_SOURCE_PIXELS_TOTAL: u64 = 134_217_728;
const MAX_ALLOWED_ICC_PROFILE_BYTES: usize = 60_988;
const MAX_PNG_ICCP_CHUNK_BYTES: usize = 1024 * 1024;
const PNG_SRGB_GAMMA: u32 = 45_455;
const PNG_SRGB_CHROMATICITIES: [u32; 8] = [
    31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
];
const PNG_SRGB_CICP: [u8; 4] = [0x01, 0x0d, 0x00, 0x01];
const SRGB_2014: &[u8] = include_bytes!("../assets/sRGB2014.icc");
const SRGB_V4_PREFERENCE: &[u8] = include_bytes!("../assets/sRGB_v4_ICC_preference.icc");
const SRGB_V4_PREFERENCE_DISPLAY: &[u8] =
    include_bytes!("../assets/sRGB_v4_ICC_preference_displayclass.icc");
const ALLOWED_SRGB_PROFILES: &[&[u8]] =
    &[SRGB_2014, SRGB_V4_PREFERENCE, SRGB_V4_PREFERENCE_DISPLAY];

#[derive(Debug)]
pub(crate) struct SourceFailure {
    pub(crate) code: ImagingFailureCode,
    pub(crate) path_code: Option<ImagingPathCode>,
    pub(crate) message: String,
}

impl SourceFailure {
    fn new(code: ImagingFailureCode, message: impl Into<String>) -> Self {
        Self {
            code,
            path_code: None,
            message: message.into(),
        }
    }

    fn path(path_code: ImagingPathCode, message: impl Into<String>) -> Self {
        Self {
            code: ImagingFailureCode::SourceUnavailable,
            path_code: Some(path_code),
            message: message.into(),
        }
    }
}

pub(crate) struct OpenRenderSource {
    reader: BufReader<File>,
    preflight: SourcePreflight,
    source_bytes: u64,
}

enum SourcePreflight {
    Jpeg(JpegPreflight),
    Png(PngPreflight),
    Tiff(TiffPreflight),
}

struct JpegPreflight {
    width: u32,
    height: u32,
    orientation: Orientation,
    compressed_bytes: usize,
    is_progressive: bool,
    color_model: JpegColorModel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JpegColorModel {
    Grayscale,
    YCbCr,
    Rgb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JpegComponentLayout {
    Grayscale,
    Numeric123,
    RgbLetters,
}

struct PngPreflight {
    width: u32,
    height: u32,
    has_icc_profile: bool,
}

struct TiffPreflight {
    width: u32,
    height: u32,
    orientation: Orientation,
    has_icc_profile: bool,
}

impl OpenRenderSource {
    pub(crate) fn byte_count(&self) -> u64 {
        self.source_bytes
    }

    pub(crate) fn pixel_count(&self) -> Result<u64, SourceFailure> {
        let (width, height) = match &self.preflight {
            SourcePreflight::Jpeg(preflight) => (preflight.width, preflight.height),
            SourcePreflight::Png(preflight) => (preflight.width, preflight.height),
            SourcePreflight::Tiff(preflight) => (preflight.width, preflight.height),
        };
        u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                SourceFailure::new(
                    ImagingFailureCode::ResourceLimitExceeded,
                    "as dimensões da fonte excederam o intervalo seguro",
                )
            })
    }

    pub(crate) fn decode(self) -> Result<RgbaImage, SourceFailure> {
        match self.preflight {
            SourcePreflight::Jpeg(preflight) => decode_render_jpeg(self.reader, preflight),
            SourcePreflight::Png(preflight) => decode_render_png(self.reader, preflight),
            SourcePreflight::Tiff(preflight) => decode_render_tiff(self.reader, preflight),
        }
    }

    pub(crate) fn exif_orientation(&self) -> Option<u8> {
        match &self.preflight {
            SourcePreflight::Jpeg(preflight) => Some(preflight.orientation.to_exif()),
            SourcePreflight::Png(_) => None,
            SourcePreflight::Tiff(preflight) => Some(preflight.orientation.to_exif()),
        }
    }

    pub(crate) fn source_page_count(&self) -> Option<u32> {
        matches!(self.preflight, SourcePreflight::Tiff(_)).then_some(1)
    }

    pub(crate) const fn basic_color_profile(&self) -> CacheBasicColorProfile {
        CacheBasicColorProfile::Srgb
    }
}

pub(crate) fn open_render_source(
    resolved: &ResolvedObject,
) -> Result<OpenRenderSource, SourceFailure> {
    open_source(resolved, false)
}

pub(crate) fn open_cache_source(
    resolved: &ResolvedObject,
) -> Result<OpenRenderSource, SourceFailure> {
    open_source(resolved, true)
}

fn open_source(
    resolved: &ResolvedObject,
    allow_single_page_tiff: bool,
) -> Result<OpenRenderSource, SourceFailure> {
    let file = resolved.reopen_for_read().map_err(|error| {
        SourceFailure::path(
            ImagingPathCode::from_io_error(&error),
            format!("não foi possível abrir a fonte original para leitura: {error}"),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        SourceFailure::path(
            ImagingPathCode::from_io_error(&error),
            format!("não foi possível inspecionar a fonte original aberta: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(SourceFailure::path(
            ImagingPathCode::UnexpectedObjectType,
            "a fonte original não é um arquivo regular",
        ));
    }
    let source_bytes = metadata.len();
    let mut reader = BufReader::new(file);
    let mut signature = [0_u8; 8];
    let signature_length = read_signature(&mut reader, &mut signature)?;
    rewind(&mut reader)?;
    let preflight = if signature_length >= 2 && signature.starts_with(&[0xff, 0xd8]) {
        SourcePreflight::Jpeg(preflight_jpeg(&mut reader, source_bytes)?)
    } else if signature_length == signature.len() && signature == [137, 80, 78, 71, 13, 10, 26, 10]
    {
        SourcePreflight::Png(preflight_png(&mut reader, source_bytes)?)
    } else if allow_single_page_tiff
        && signature_length >= 4
        && matches!(
            &signature[..4],
            b"II\x2a\0" | b"MM\0\x2a" | b"II\x2b\0" | b"MM\0\x2b"
        )
    {
        SourcePreflight::Tiff(preflight_tiff(&mut reader)?)
    } else {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceFormat,
            "a fonte não contém JPEG ou PNG",
        ));
    };
    rewind(&mut reader)?;
    Ok(OpenRenderSource {
        reader,
        preflight,
        source_bytes,
    })
}

fn read_signature(reader: &mut impl Read, signature: &mut [u8]) -> Result<usize, SourceFailure> {
    let mut filled = 0;
    while filled < signature.len() {
        match reader.read(&mut signature[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(SourceFailure::path(
                    ImagingPathCode::from_io_error(&error),
                    format!("não foi possível identificar o formato da fonte original: {error}"),
                ));
            }
        }
    }
    Ok(filled)
}

fn rewind(reader: &mut impl Seek) -> Result<(), SourceFailure> {
    reader
        .seek(SeekFrom::Start(0))
        .map(|_| ())
        .map_err(|error| {
            SourceFailure::path(
                ImagingPathCode::from_io_error(&error),
                format!("não foi possível reposicionar a fonte original aberta: {error}"),
            )
        })
}

fn read_exact_source(reader: &mut impl Read, buffer: &mut [u8]) -> Result<(), SourceFailure> {
    reader.read_exact(buffer).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "a fonte terminou antes de completar sua estrutura",
            )
        } else {
            SourceFailure::path(
                ImagingPathCode::from_io_error(&error),
                format!("não foi possível ler a fonte original aberta: {error}"),
            )
        }
    })
}

fn read_segment(reader: &mut impl Read, length: usize) -> Result<Vec<u8>, SourceFailure> {
    let mut segment = Vec::new();
    segment.try_reserve_exact(length).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para inspecionar a fonte",
        )
    })?;
    segment.resize(length, 0);
    read_exact_source(reader, &mut segment)?;
    Ok(segment)
}

fn preflight_jpeg(
    reader: &mut (impl Read + Seek),
    source_bytes: u64,
) -> Result<JpegPreflight, SourceFailure> {
    let mut soi = [0_u8; 2];
    read_exact_source(reader, &mut soi)?;
    if soi != [0xff, 0xd8] {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceFormat,
            "a assinatura JPEG é inválida",
        ));
    }

    let mut dimensions = None;
    let mut component_layout = None;
    let mut is_progressive = None;
    let mut orientation = Orientation::NoTransforms;
    let mut adobe_transform = None;
    let mut icc_segments: Option<Vec<Option<Vec<u8>>>> = None;

    loop {
        let marker = read_jpeg_marker(reader)?;
        if marker == 0xda {
            skip_jpeg_segment(reader)?;
            break;
        }
        if marker == 0xd9 {
            return Err(SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o JPEG terminou antes dos dados de imagem",
            ));
        }
        if marker == 0xd8 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }

        let payload_length = jpeg_segment_payload_length(reader)?;
        if is_sof_marker(marker) {
            if !matches!(marker, 0xc0 | 0xc2) {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedSourceVariant,
                    "a variante JPEG não é baseline nem progressiva de 8 bits",
                ));
            }
            let payload = read_segment(reader, payload_length)?;
            if payload.len() < 6 {
                return Err(SourceFailure::new(
                    ImagingFailureCode::DecodeFailed,
                    "o cabeçalho de dimensões JPEG está truncado",
                ));
            }
            if payload[0] != 8 {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedSourceVariant,
                    "a profundidade JPEG não é 8 bits",
                ));
            }
            let height = u32::from(u16::from_be_bytes([payload[1], payload[2]]));
            let width = u32::from(u16::from_be_bytes([payload[3], payload[4]]));
            let components = payload[5];
            let expected = 6_usize
                .checked_add(usize::from(components).checked_mul(3).ok_or_else(|| {
                    SourceFailure::new(
                        ImagingFailureCode::DecodeFailed,
                        "o número de componentes JPEG excedeu o intervalo",
                    )
                })?)
                .ok_or_else(|| {
                    SourceFailure::new(
                        ImagingFailureCode::DecodeFailed,
                        "o cabeçalho JPEG excedeu o intervalo",
                    )
                })?;
            if width == 0 || height == 0 || payload.len() != expected {
                return Err(SourceFailure::new(
                    ImagingFailureCode::DecodeFailed,
                    "as dimensões ou componentes JPEG são inválidos",
                ));
            }
            let detected_layout = match components {
                1 => JpegComponentLayout::Grayscale,
                3 => match [payload[6], payload[9], payload[12]] {
                    [1, 2, 3] => JpegComponentLayout::Numeric123,
                    [b'R', b'G', b'B'] => JpegComponentLayout::RgbLetters,
                    _ => {
                        return Err(SourceFailure::new(
                            ImagingFailureCode::UnsupportedColorModel,
                            "os identificadores de componentes JPEG não descrevem RGB nem YCbCr",
                        ));
                    }
                },
                _ => {
                    return Err(SourceFailure::new(
                        ImagingFailureCode::UnsupportedColorModel,
                        "o modelo de componentes JPEG não é aceito",
                    ));
                }
            };
            if dimensions.replace((width, height)).is_some() {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedSourceVariant,
                    "o JPEG contém mais de um frame de imagem",
                ));
            }
            let progressive = marker == 0xc2;
            if progressive {
                progressive_jpeg::validate_working_budget(
                    width,
                    height,
                    components,
                    &payload,
                    source_bytes,
                )?;
            }
            component_layout = Some(detected_layout);
            is_progressive = Some(progressive);
        } else if marker == 0xe1 {
            let payload = read_segment(reader, payload_length)?;
            if let Some(exif) = payload.strip_prefix(b"Exif\0\0") {
                orientation =
                    Orientation::from_exif_chunk(exif).unwrap_or(Orientation::NoTransforms);
            }
        } else if marker == 0xe2 {
            let payload = read_segment(reader, payload_length)?;
            if payload.starts_with(b"ICC_PROFILE\0") {
                collect_jpeg_icc_segment(&mut icc_segments, &payload)?;
            }
        } else if marker == 0xee {
            let payload = read_segment(reader, payload_length)?;
            if payload.starts_with(b"Adobe")
                && (payload.len() != 12 || adobe_transform.replace(payload[11]).is_some())
            {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedColorModel,
                    "o marcador Adobe APP14 é duplicado ou malformado",
                ));
            }
        } else {
            reader
                .seek(SeekFrom::Current(i64::try_from(payload_length).map_err(
                    |_| {
                        SourceFailure::new(
                            ImagingFailureCode::ResourceLimitExceeded,
                            "o segmento JPEG excedeu o intervalo seguro",
                        )
                    },
                )?))
                .map_err(|error| {
                    SourceFailure::path(
                        ImagingPathCode::from_io_error(&error),
                        format!("não foi possível percorrer o cabeçalho JPEG: {error}"),
                    )
                })?;
        }
    }

    let component_layout = component_layout.ok_or_else(|| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o JPEG não contém um modelo de cor reconhecido",
        )
    })?;
    let color_model = classify_jpeg_color_model(component_layout, adobe_transform)?;
    validate_collected_jpeg_profile(icc_segments)?;
    let (width, height) = dimensions.ok_or_else(|| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o JPEG não contém um frame reconhecido",
        )
    })?;
    Ok(JpegPreflight {
        width,
        height,
        orientation,
        compressed_bytes: usize::try_from(source_bytes).map_err(|_| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "a fonte JPEG excede o intervalo de memória da plataforma",
            )
        })?,
        is_progressive: is_progressive.expect("recognized JPEG dimensions record the variant"),
        color_model,
    })
}

fn classify_jpeg_color_model(
    layout: JpegComponentLayout,
    adobe_transform: Option<u8>,
) -> Result<JpegColorModel, SourceFailure> {
    // APP14 is authoritative when present: transform 0 means RGB and transform 1
    // means YCbCr, regardless of whether the component IDs are numeric or letters.
    match (adobe_transform, layout) {
        (None, JpegComponentLayout::Grayscale) => Ok(JpegColorModel::Grayscale),
        (None, JpegComponentLayout::Numeric123) => Ok(JpegColorModel::YCbCr),
        (None, JpegComponentLayout::RgbLetters) => Ok(JpegColorModel::Rgb),
        (Some(0), JpegComponentLayout::Numeric123 | JpegComponentLayout::RgbLetters) => {
            Ok(JpegColorModel::Rgb)
        }
        (Some(1), JpegComponentLayout::Numeric123 | JpegComponentLayout::RgbLetters) => {
            Ok(JpegColorModel::YCbCr)
        }
        _ => Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedColorModel,
            "o transform Adobe não corresponde a um modelo JPEG aceito",
        )),
    }
}

fn read_jpeg_marker(reader: &mut impl Read) -> Result<u8, SourceFailure> {
    let mut byte = [0_u8; 1];
    loop {
        read_exact_source(reader, &mut byte)?;
        if byte[0] == 0xff {
            break;
        }
    }
    loop {
        read_exact_source(reader, &mut byte)?;
        match byte[0] {
            0xff => continue,
            0x00 => {
                return Err(SourceFailure::new(
                    ImagingFailureCode::DecodeFailed,
                    "o cabeçalho JPEG contém escape fora dos dados comprimidos",
                ));
            }
            marker => return Ok(marker),
        }
    }
}

fn jpeg_segment_payload_length(reader: &mut impl Read) -> Result<usize, SourceFailure> {
    let mut length = [0_u8; 2];
    read_exact_source(reader, &mut length)?;
    usize::from(u16::from_be_bytes(length))
        .checked_sub(2)
        .ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o segmento JPEG declara tamanho inválido",
            )
        })
}

fn skip_jpeg_segment(reader: &mut (impl Read + Seek)) -> Result<(), SourceFailure> {
    let length = jpeg_segment_payload_length(reader)?;
    reader
        .seek(SeekFrom::Current(i64::try_from(length).map_err(|_| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o segmento JPEG excedeu o intervalo seguro",
            )
        })?))
        .map(|_| ())
        .map_err(|error| {
            SourceFailure::path(
                ImagingPathCode::from_io_error(&error),
                format!("não foi possível percorrer o segmento JPEG: {error}"),
            )
        })
}

fn is_sof_marker(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

fn collect_jpeg_icc_segment(
    collected: &mut Option<Vec<Option<Vec<u8>>>>,
    payload: &[u8],
) -> Result<(), SourceFailure> {
    if payload.len() < 14 {
        return Err(unsupported_profile("o segmento ICC JPEG está truncado"));
    }
    let sequence = usize::from(payload[12]);
    let total = usize::from(payload[13]);
    if sequence == 0 || total == 0 || sequence > total {
        return Err(unsupported_profile("a sequência ICC JPEG é inválida"));
    }
    let segments = collected.get_or_insert_with(Vec::new);
    if segments.is_empty() {
        segments.try_reserve_exact(total).map_err(|_| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "não há memória suficiente para inspecionar o perfil ICC",
            )
        })?;
        segments.resize_with(total, || None);
    } else if segments.len() != total {
        return Err(unsupported_profile(
            "os segmentos ICC JPEG discordam sobre sua quantidade",
        ));
    }
    if segments[sequence - 1].is_some() {
        return Err(unsupported_profile("o JPEG repete um segmento ICC"));
    }
    let profile_bytes = payload.len() - 14;
    let accumulated = segments
        .iter()
        .flatten()
        .try_fold(profile_bytes, |total, segment| {
            total.checked_add(segment.len())
        })
        .ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o perfil ICC excedeu o intervalo seguro",
            )
        })?;
    if accumulated > MAX_ALLOWED_ICC_PROFILE_BYTES {
        return Err(unsupported_profile("o perfil ICC JPEG excede a allowlist"));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(profile_bytes).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para inspecionar o perfil ICC",
        )
    })?;
    bytes.extend_from_slice(&payload[14..]);
    segments[sequence - 1] = Some(bytes);
    Ok(())
}

fn validate_collected_jpeg_profile(
    segments: Option<Vec<Option<Vec<u8>>>>,
) -> Result<(), SourceFailure> {
    let Some(segments) = segments else {
        return Ok(());
    };
    let total = segments.iter().try_fold(0_usize, |total, segment| {
        let segment = segment
            .as_ref()
            .ok_or_else(|| unsupported_profile("faltam segmentos do perfil ICC JPEG"))?;
        total.checked_add(segment.len()).ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o perfil ICC excedeu o intervalo seguro",
            )
        })
    })?;
    if total > MAX_ALLOWED_ICC_PROFILE_BYTES {
        return Err(unsupported_profile("o perfil ICC JPEG excede a allowlist"));
    }
    let mut profile = Vec::new();
    profile.try_reserve_exact(total).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para inspecionar o perfil ICC",
        )
    })?;
    for segment in segments {
        profile.extend_from_slice(
            segment
                .as_deref()
                .expect("the complete profile was checked above"),
        );
    }
    validate_icc_profile(&profile)
}

fn preflight_png(
    reader: &mut BufReader<File>,
    source_bytes: u64,
) -> Result<PngPreflight, SourceFailure> {
    let mut signature = [0_u8; 8];
    read_exact_source(reader, &mut signature)?;
    if signature != [137, 80, 78, 71, 13, 10, 26, 10] {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceFormat,
            "a assinatura PNG é inválida",
        ));
    }

    let mut dimensions = None;
    let mut has_icc_profile = false;
    let mut has_srgb = false;
    let mut gamma = None;
    let mut chromaticities = None;
    let mut cicp = None;
    let mut found_iend = false;
    loop {
        let mut header = [0_u8; 8];
        read_exact_source(reader, &mut header)?;
        let length = usize::try_from(u32::from_be_bytes(
            header[..4].try_into().expect("four bytes"),
        ))
        .expect("u32 fits usize on supported targets");
        let kind: [u8; 4] = header[4..8].try_into().expect("four bytes");
        let position = reader.stream_position().map_err(|error| {
            SourceFailure::path(
                ImagingPathCode::from_io_error(&error),
                format!("não foi possível inspecionar a posição da fonte PNG: {error}"),
            )
        })?;
        let end = position
            .checked_add(u64::try_from(length).expect("usize fits u64"))
            .and_then(|value| value.checked_add(4))
            .ok_or_else(|| {
                SourceFailure::new(
                    ImagingFailureCode::ResourceLimitExceeded,
                    "o chunk PNG excedeu o intervalo seguro",
                )
            })?;
        if end > source_bytes {
            return Err(SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o chunk PNG ultrapassa o fim da fonte",
            ));
        }

        match &kind {
            b"IHDR" => {
                if dimensions.is_some() || length != 13 {
                    return Err(SourceFailure::new(
                        ImagingFailureCode::DecodeFailed,
                        "o cabeçalho IHDR do PNG é inválido",
                    ));
                }
                let payload = read_segment(reader, length)?;
                let width = u32::from_be_bytes(payload[0..4].try_into().expect("four bytes"));
                let height = u32::from_be_bytes(payload[4..8].try_into().expect("four bytes"));
                validate_png_color(payload[8], payload[9])?;
                if width == 0 || height == 0 {
                    return Err(SourceFailure::new(
                        ImagingFailureCode::DecodeFailed,
                        "as dimensões PNG precisam ser positivas",
                    ));
                }
                dimensions = Some((width, height));
            }
            b"acTL" | b"fcTL" | b"fdAT" => {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedSourceVariant,
                    "APNG não é aceito neste fluxo",
                ));
            }
            b"CgBI" => {
                return Err(SourceFailure::new(
                    ImagingFailureCode::UnsupportedSourceVariant,
                    "a variante CgBI de PNG não é aceita",
                ));
            }
            b"iCCP" => {
                if has_icc_profile || length > MAX_PNG_ICCP_CHUNK_BYTES {
                    return Err(unsupported_profile("o chunk iCCP do PNG é inválido"));
                }
                let payload = read_segment(reader, length)?;
                validate_png_iccp_chunk(&payload)?;
                has_icc_profile = true;
            }
            b"sRGB" => {
                if has_srgb || length != 1 {
                    return Err(unsupported_profile("a declaração sRGB do PNG é inválida"));
                }
                let payload = read_segment(reader, length)?;
                if payload[0] > 3 {
                    return Err(unsupported_profile("o rendering intent sRGB é inválido"));
                }
                has_srgb = true;
            }
            b"gAMA" => {
                if gamma.is_some() || length != 4 {
                    return Err(unsupported_profile("a declaração gAMA do PNG é inválida"));
                }
                let payload = read_segment(reader, length)?;
                gamma = Some(u32::from_be_bytes(
                    payload.as_slice().try_into().expect("quatro bytes"),
                ));
            }
            b"cHRM" => {
                if chromaticities.is_some() || length != 32 {
                    return Err(unsupported_profile("a declaração cHRM do PNG é inválida"));
                }
                let payload = read_segment(reader, length)?;
                chromaticities = Some(std::array::from_fn(|index| {
                    let start = index * 4;
                    u32::from_be_bytes(payload[start..start + 4].try_into().expect("quatro bytes"))
                }));
            }
            b"cICP" => {
                if cicp.is_some() || length != PNG_SRGB_CICP.len() {
                    return Err(unsupported_profile("a declaração cICP do PNG é inválida"));
                }
                let payload = read_segment(reader, length)?;
                let declaration: [u8; 4] = payload
                    .try_into()
                    .expect("o comprimento cICP foi validado acima");
                if declaration != PNG_SRGB_CICP {
                    return Err(unsupported_profile(
                        "a declaração cICP do PNG não descreve sRGB full-range",
                    ));
                }
                cicp = Some(declaration);
            }
            b"IEND" => {
                if length != 0 {
                    return Err(SourceFailure::new(
                        ImagingFailureCode::DecodeFailed,
                        "o término IEND do PNG é inválido",
                    ));
                }
                found_iend = true;
            }
            _ => {
                reader
                    .seek(SeekFrom::Current(i64::try_from(length).map_err(|_| {
                        SourceFailure::new(
                            ImagingFailureCode::ResourceLimitExceeded,
                            "o chunk PNG excedeu o intervalo seguro",
                        )
                    })?))
                    .map_err(|error| {
                        SourceFailure::path(
                            ImagingPathCode::from_io_error(&error),
                            format!("não foi possível percorrer a fonte PNG: {error}"),
                        )
                    })?;
            }
        }
        let mut crc = [0_u8; 4];
        read_exact_source(reader, &mut crc)?;
        if found_iend {
            break;
        }
    }
    if has_icc_profile && has_srgb {
        return Err(unsupported_profile(
            "o PNG combina declarações iCCP e sRGB contraditórias",
        ));
    }
    if gamma.is_some_and(|value| value != PNG_SRGB_GAMMA)
        || chromaticities.is_some_and(|value| value != PNG_SRGB_CHROMATICITIES)
    {
        return Err(unsupported_profile(
            "as declarações gAMA/cHRM do PNG contradizem sRGB",
        ));
    }
    let (width, height) = dimensions.ok_or_else(|| {
        SourceFailure::new(ImagingFailureCode::DecodeFailed, "o PNG não contém IHDR")
    })?;
    Ok(PngPreflight {
        width,
        height,
        has_icc_profile,
    })
}

fn validate_png_color(bit_depth: u8, color_type: u8) -> Result<(), SourceFailure> {
    let accepted = match color_type {
        0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
        2 => matches!(bit_depth, 8 | 16),
        3 => matches!(bit_depth, 1 | 2 | 4 | 8),
        4 | 6 => matches!(bit_depth, 8 | 16),
        _ => {
            return Err(SourceFailure::new(
                ImagingFailureCode::UnsupportedColorModel,
                "o modelo de cor PNG não é aceito",
            ));
        }
    };
    if accepted {
        Ok(())
    } else {
        Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceVariant,
            "a profundidade não é válida para o modelo PNG",
        ))
    }
}

fn validate_png_iccp_chunk(payload: &[u8]) -> Result<(), SourceFailure> {
    let Some(separator) = payload.iter().position(|byte| *byte == 0) else {
        return Err(unsupported_profile("o nome do perfil iCCP não termina"));
    };
    if !(1..=79).contains(&separator)
        || payload.get(separator + 1) != Some(&0)
        || payload.len() <= separator + 2
    {
        return Err(unsupported_profile("o chunk iCCP é malformado"));
    }
    Ok(())
}

fn validate_icc_profile(profile: &[u8]) -> Result<(), SourceFailure> {
    if ALLOWED_SRGB_PROFILES.contains(&profile) {
        Ok(())
    } else {
        Err(unsupported_profile(
            "o perfil ICC não pertence à allowlist sRGB",
        ))
    }
}

fn unsupported_profile(message: impl Into<String>) -> SourceFailure {
    SourceFailure::new(ImagingFailureCode::UnsupportedColorProfile, message)
}

struct FallibleJpegReader<R> {
    inner: R,
    total_bytes: usize,
}

impl<R> FallibleJpegReader<R> {
    fn new(inner: R, total_bytes: usize) -> Self {
        Self { inner, total_bytes }
    }
}

impl<R: Read + Seek> Read for FallibleJpegReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }

    fn read_to_end(&mut self, output: &mut Vec<u8>) -> std::io::Result<usize> {
        let position = usize::try_from(self.inner.stream_position()?)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
        let remaining = self.total_bytes.checked_sub(position).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a posição de leitura excede o tamanho verificado da fonte",
            )
        })?;
        let original_length = output.len();
        let final_length = original_length
            .checked_add(remaining)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
        output
            .try_reserve_exact(remaining)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
        output.resize(final_length, 0);
        if let Err(error) = self.inner.read_exact(&mut output[original_length..]) {
            output.truncate(original_length);
            return Err(error);
        }
        Ok(remaining)
    }
}

impl<R: BufRead + Seek> BufRead for FallibleJpegReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}

impl<R: Read + Seek> Seek for FallibleJpegReader<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

fn image_failure(
    error: ImageError,
    fallback_code: ImagingFailureCode,
    message: &'static str,
) -> SourceFailure {
    match error {
        ImageError::IoError(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData
            ) =>
        {
            SourceFailure::new(fallback_code, format!("{message}: {error}"))
        }
        ImageError::IoError(error) if error.kind() == std::io::ErrorKind::OutOfMemory => {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                format!("{message}: {error}"),
            )
        }
        ImageError::IoError(error) => SourceFailure::path(
            ImagingPathCode::from_io_error(&error),
            format!("{message}: {error}"),
        ),
        ImageError::Limits(error) => SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            format!("{message}: {error}"),
        ),
        error => SourceFailure::new(fallback_code, format!("{message}: {error}")),
    }
}

fn preflight_tiff(reader: &mut (impl Read + Seek)) -> Result<TiffPreflight, SourceFailure> {
    let mut decoder = tiff::decoder::Decoder::new(reader).map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível preparar o decoder TIFF: {error}"),
        )
    })?;
    let (width, height) = decoder.dimensions().map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível ler as dimensões TIFF: {error}"),
        )
    })?;
    if width == 0 || height == 0 {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "as dimensões TIFF são inválidas",
        ));
    }
    if decoder.more_images() {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceVariant,
            "TIFF com mais de uma página não é aceito neste fluxo",
        ));
    }
    let color_type = decoder.colortype().map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível identificar o modelo de cor TIFF: {error}"),
        )
    })?;
    if !matches!(
        color_type,
        tiff::ColorType::Gray(8 | 16)
            | tiff::ColorType::GrayA(8 | 16)
            | tiff::ColorType::RGB(8 | 16)
            | tiff::ColorType::RGBA(8 | 16)
    ) {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedColorModel,
            "o modelo de cor TIFF não é RGB, RGBA ou tons de cinza de 8/16 bits",
        ));
    }
    let orientation = decoder
        .find_tag_unsigned::<u16>(tiff::tags::Tag::Orientation)
        .map_err(|error| {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                format!("não foi possível ler a orientação TIFF: {error}"),
            )
        })?
        .and_then(|value| Orientation::from_exif(value.min(255) as u8))
        .unwrap_or(Orientation::NoTransforms);
    let profile = decoder
        .find_tag(tiff::tags::Tag::IccProfile)
        .map_err(|error| {
            SourceFailure::new(
                ImagingFailureCode::UnsupportedColorProfile,
                format!("não foi possível ler o perfil ICC do TIFF: {error}"),
            )
        })?
        .map(|value| {
            value.into_u8_vec().map_err(|error| {
                SourceFailure::new(
                    ImagingFailureCode::UnsupportedColorProfile,
                    format!("o perfil ICC do TIFF é inválido: {error}"),
                )
            })
        })
        .transpose()?;
    if let Some(profile) = &profile {
        validate_icc_profile(profile)?;
    }
    Ok(TiffPreflight {
        width,
        height,
        orientation,
        has_icc_profile: profile.is_some(),
    })
}

fn decode_render_jpeg(
    reader: BufReader<File>,
    preflight: JpegPreflight,
) -> Result<RgbaImage, SourceFailure> {
    if preflight.is_progressive {
        #[cfg(not(test))]
        return progressive_jpeg::decode_in_worker(reader, preflight);
    }
    decode_render_jpeg_in_process(reader, preflight)
}

fn decode_render_tiff(
    reader: BufReader<File>,
    preflight: TiffPreflight,
) -> Result<RgbaImage, SourceFailure> {
    let mut decoder = TiffDecoder::new(reader).map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "não foi possível preparar o decoder TIFF",
        )
    })?;
    if decoder.dimensions() != (preflight.width, preflight.height) {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "as dimensões TIFF mudaram entre preflight e decoder",
        ));
    }
    decoder
        .set_limits(decode_limits(&decoder)?)
        .map_err(|error| {
            image_failure(
                error,
                ImagingFailureCode::ResourceLimitExceeded,
                "o decoder TIFF recusou os limites da fonte",
            )
        })?;
    if let Some(profile) = decoder.icc_profile().map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::UnsupportedColorProfile,
            "o perfil ICC do TIFF não pôde ser lido",
        )
    })? {
        validate_icc_profile(&profile)?;
    } else if preflight.has_icc_profile {
        return Err(unsupported_profile(
            "o perfil declarado pelo TIFF não pôde ser recuperado",
        ));
    }
    let orientation = decoder.orientation().map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "não foi possível validar a orientação TIFF",
        )
    })?;
    if orientation != preflight.orientation {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "a orientação TIFF mudou entre preflight e decoder",
        ));
    }
    let color_type = decoder.color_type();
    let raw = decode_raw(decoder)?;
    let image = normalize_rgba(preflight.width, preflight.height, color_type, raw)?;
    apply_orientation(image, orientation)
}

fn decode_render_jpeg_in_process<R: BufRead + Seek>(
    reader: R,
    preflight: JpegPreflight,
) -> Result<RgbaImage, SourceFailure> {
    let reader = FallibleJpegReader::new(reader, preflight.compressed_bytes);
    let mut decoder = JpegDecoder::new(reader).map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "não foi possível preparar o decoder JPEG",
        )
    })?;
    if decoder.dimensions() != (preflight.width, preflight.height) {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "as dimensões JPEG mudaram entre preflight e decoder",
        ));
    }
    let color_type = decoder.color_type();
    let expected_color_type = match preflight.color_model {
        JpegColorModel::Grayscale => ColorType::L8,
        JpegColorModel::YCbCr | JpegColorModel::Rgb => ColorType::Rgb8,
    };
    if color_type != expected_color_type {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedColorModel,
            "o decoder JPEG produziu um modelo diferente do preflight",
        ));
    }
    decoder
        .set_limits(decode_limits(&decoder)?)
        .map_err(|error| {
            image_failure(
                error,
                ImagingFailureCode::ResourceLimitExceeded,
                "o decoder JPEG recusou os limites da fonte",
            )
        })?;
    let raw = decode_raw(decoder)?;
    let image = normalize_rgba(preflight.width, preflight.height, color_type, raw)?;
    apply_orientation(image, preflight.orientation)
}

fn decode_render_png(
    reader: BufReader<File>,
    preflight: PngPreflight,
) -> Result<RgbaImage, SourceFailure> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(preflight.width);
    limits.max_image_height = Some(preflight.height);
    let planned_bytes = u64::from(preflight.width)
        .checked_mul(u64::from(preflight.height))
        .and_then(|pixels| pixels.checked_mul(8))
        .ok_or_else(|| decoder_allocation_failure("o buffer PNG excedeu o intervalo seguro"))?;
    limits.max_alloc = Some(checked_decoder_allocation(
        planned_bytes,
        "o buffer PNG excede o limite medido do decoder",
    )?);
    let mut decoder = PngDecoder::with_limits(reader, limits).map_err(|error| {
        let fallback = if preflight.has_icc_profile {
            ImagingFailureCode::UnsupportedColorProfile
        } else {
            ImagingFailureCode::DecodeFailed
        };
        image_failure(error, fallback, "não foi possível preparar o decoder PNG")
    })?;
    if decoder.dimensions() != (preflight.width, preflight.height) {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "as dimensões PNG mudaram entre preflight e decoder",
        ));
    }
    if decoder.is_apng().map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "não foi possível validar a variante PNG",
        )
    })? {
        return Err(SourceFailure::new(
            ImagingFailureCode::UnsupportedSourceVariant,
            "APNG não é aceito neste fluxo",
        ));
    }
    if let Some(profile) = decoder.icc_profile().map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::UnsupportedColorProfile,
            "o perfil ICC do PNG não pôde ser lido",
        )
    })? {
        validate_icc_profile(&profile)?;
    } else if preflight.has_icc_profile {
        return Err(unsupported_profile(
            "o perfil declarado pelo PNG não pôde ser recuperado",
        ));
    }
    let orientation = decoder.orientation().map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "não foi possível validar a orientação PNG",
        )
    })?;
    let color_type = decoder.color_type();
    let raw = decode_raw(decoder)?;
    let image = normalize_rgba(preflight.width, preflight.height, color_type, raw)?;
    apply_orientation(image, orientation)
}

fn decode_limits(decoder: &impl ImageDecoder) -> Result<Limits, SourceFailure> {
    let (width, height) = decoder.dimensions();
    let raw_bytes = decoder.total_bytes();
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(checked_decoder_allocation(
        raw_bytes,
        "o buffer excede o limite medido do decoder",
    )?);
    Ok(limits)
}

fn checked_decoder_allocation(
    decoded_bytes: u64,
    message: &'static str,
) -> Result<u64, SourceFailure> {
    let planned_bytes = decoded_bytes.checked_add(1024 * 1024).ok_or_else(|| {
        decoder_allocation_failure("o buffer do decoder excedeu o intervalo seguro")
    })?;
    if planned_bytes > CACHE_MAX_DECODER_ALLOC_BYTES {
        return Err(decoder_allocation_failure(message));
    }
    Ok(planned_bytes)
}

fn decoder_allocation_failure(message: impl Into<String>) -> SourceFailure {
    SourceFailure::new(ImagingFailureCode::ResourceLimitExceeded, message)
}

fn decode_raw(decoder: impl ImageDecoder) -> Result<Vec<u8>, SourceFailure> {
    let length = usize::try_from(decoder.total_bytes()).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "o buffer decodificado excedeu o intervalo da plataforma",
        )
    })?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(length).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para decodificar a fonte",
        )
    })?;
    raw.resize(length, 0);
    decoder.read_image(&mut raw).map_err(|error| {
        image_failure(
            error,
            ImagingFailureCode::DecodeFailed,
            "a fonte permitida não pôde ser decodificada",
        )
    })?;
    Ok(raw)
}

fn normalize_rgba(
    width: u32,
    height: u32,
    color_type: ColorType,
    raw: Vec<u8>,
) -> Result<RgbaImage, SourceFailure> {
    if color_type == ColorType::Rgba8 {
        return RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o raster RGBA8 não corresponde às dimensões da fonte",
            )
        });
    }
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "as dimensões normalizadas excederam o intervalo",
            )
        })?;
    let output_len = pixel_count
        .checked_mul(4)
        .and_then(|length| usize::try_from(length).ok())
        .ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o raster RGBA8 excedeu o intervalo da plataforma",
            )
        })?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(output_len).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para normalizar a fonte",
        )
    })?;

    match color_type {
        ColorType::L8 => {
            for value in raw {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        ColorType::La8 => {
            for value in raw.chunks_exact(2) {
                rgba.extend_from_slice(&[value[0], value[0], value[0], value[1]]);
            }
        }
        ColorType::Rgb8 => {
            for value in raw.chunks_exact(3) {
                rgba.extend_from_slice(&[value[0], value[1], value[2], 255]);
            }
        }
        ColorType::L16 => {
            for value in raw.chunks_exact(2) {
                let luminance = reduce_16(value);
                rgba.extend_from_slice(&[luminance, luminance, luminance, 255]);
            }
        }
        ColorType::La16 => {
            for value in raw.chunks_exact(4) {
                let luminance = reduce_16(&value[..2]);
                rgba.extend_from_slice(&[luminance, luminance, luminance, reduce_16(&value[2..4])]);
            }
        }
        ColorType::Rgb16 => {
            for value in raw.chunks_exact(6) {
                rgba.extend_from_slice(&[
                    reduce_16(&value[..2]),
                    reduce_16(&value[2..4]),
                    reduce_16(&value[4..6]),
                    255,
                ]);
            }
        }
        ColorType::Rgba16 => {
            for value in raw.chunks_exact(8) {
                rgba.extend_from_slice(&[
                    reduce_16(&value[..2]),
                    reduce_16(&value[2..4]),
                    reduce_16(&value[4..6]),
                    reduce_16(&value[6..8]),
                ]);
            }
        }
        _ => {
            return Err(SourceFailure::new(
                ImagingFailureCode::UnsupportedColorModel,
                "o decoder produziu um modelo de cor não aceito",
            ));
        }
    }
    if rgba.len() != output_len {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o tamanho decodificado não corresponde às dimensões da fonte",
        ));
    }
    RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "não foi possível materializar o raster RGBA8",
        )
    })
}

fn reduce_16(bytes: &[u8]) -> u8 {
    let value = u32::from(u16::from_ne_bytes([bytes[0], bytes[1]]));
    ((value + 128) / 257) as u8
}

fn apply_orientation(
    image: RgbaImage,
    orientation: Orientation,
) -> Result<RgbaImage, SourceFailure> {
    if orientation == Orientation::NoTransforms {
        return Ok(image);
    }
    let (source_width, source_height) = image.dimensions();
    let (width, height) = match orientation {
        Orientation::Rotate90
        | Orientation::Rotate270
        | Orientation::Rotate90FlipH
        | Orientation::Rotate270FlipH => (source_height, source_width),
        _ => (source_width, source_height),
    };
    let byte_count = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| {
            SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o raster orientado excedeu o intervalo seguro",
            )
        })?;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(byte_count).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para orientar a fonte",
        )
    })?;
    pixels.resize(byte_count, 0);
    let mut output = RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não foi possível materializar o raster orientado",
        )
    })?;
    for y in 0..height {
        for x in 0..width {
            let (source_x, source_y) = match orientation {
                Orientation::NoTransforms => unreachable!(),
                Orientation::FlipHorizontal => (source_width - 1 - x, y),
                Orientation::FlipVertical => (x, source_height - 1 - y),
                Orientation::Rotate180 => (source_width - 1 - x, source_height - 1 - y),
                Orientation::Rotate90 => (y, source_height - 1 - x),
                Orientation::Rotate270 => (source_width - 1 - y, x),
                Orientation::Rotate90FlipH => (y, x),
                Orientation::Rotate270FlipH => (source_width - 1 - y, source_height - 1 - x),
            };
            output.put_pixel(x, y, *image.get_pixel(source_x, source_y));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod render_source_tests {
    use std::io::Read as _;

    use image::{
        ExtendedColorType, ImageEncoder, ImageError, Rgb, RgbImage, codecs::jpeg::JpegEncoder,
    };
    use myalbuns_imaging_protocol::{ImagingFailureCode, ImagingPathCode};
    use myalbuns_paths::{ExpectedObject, OperationPathContext};
    use sha2::{Digest, Sha256};

    use super::{FallibleJpegReader, image_failure, open_render_source};

    #[test]
    fn decoder_io_errors_preserve_the_central_path_taxonomy() {
        for (kind, expected) in [
            (std::io::ErrorKind::NotFound, ImagingPathCode::NotFound),
            (
                std::io::ErrorKind::PermissionDenied,
                ImagingPathCode::AccessDenied,
            ),
            (std::io::ErrorKind::TimedOut, ImagingPathCode::Unavailable),
            (std::io::ErrorKind::Other, ImagingPathCode::IoFailure),
        ] {
            let failure = image_failure(
                ImageError::IoError(std::io::Error::from(kind)),
                ImagingFailureCode::DecodeFailed,
                "decoder fixture",
            );
            assert_eq!(failure.code, ImagingFailureCode::SourceUnavailable);
            assert_eq!(failure.path_code, Some(expected));
        }

        for kind in [
            std::io::ErrorKind::UnexpectedEof,
            std::io::ErrorKind::InvalidData,
        ] {
            let failure = image_failure(
                ImageError::IoError(std::io::Error::from(kind)),
                ImagingFailureCode::DecodeFailed,
                "decoder fixture",
            );
            assert_eq!(failure.code, ImagingFailureCode::DecodeFailed);
            assert_eq!(failure.path_code, None);
        }

        let root = tempfile::tempdir().expect("temporary allocation fixture");
        let empty_path = root.path().join("empty.jpg");
        std::fs::write(&empty_path, []).expect("the empty fixture is writable");
        let mut reader = FallibleJpegReader::new(
            std::io::BufReader::new(
                std::fs::File::open(empty_path).expect("the empty fixture opens"),
            ),
            usize::MAX,
        );
        let mut output = Vec::new();
        let allocation_error = reader
            .read_to_end(&mut output)
            .expect_err("an impossible JPEG input reservation is fallible");
        assert_eq!(allocation_error.kind(), std::io::ErrorKind::OutOfMemory);
        let failure = image_failure(
            ImageError::IoError(allocation_error),
            ImagingFailureCode::DecodeFailed,
            "decoder fixture",
        );
        assert_eq!(failure.code, ImagingFailureCode::ResourceLimitExceeded);
        assert_eq!(failure.path_code, None);
    }

    #[test]
    fn png_input_matrix_normalizes_supported_depths_and_alpha_deterministically() {
        let cases = [
            (png_fixture(8, 2, &[0, 10, 20, 30], &[]), [10, 20, 30, 255]),
            (
                png_fixture(8, 6, &[0, 10, 20, 30, 40], &[]),
                [10, 20, 30, 40],
            ),
            (
                png_fixture(
                    1,
                    3,
                    &[0, 0x80],
                    &[(b"PLTE", &[0, 0, 0, 100, 110, 120]), (b"tRNS", &[0, 128])],
                ),
                [100, 110, 120, 128],
            ),
            (png_fixture(1, 0, &[0, 0x80], &[]), [255, 255, 255, 255]),
            (png_fixture(8, 4, &[0, 64, 128], &[]), [64, 64, 64, 128]),
            (
                png_fixture(16, 0, &[0, 0x80, 0x00], &[]),
                [128, 128, 128, 255],
            ),
            (
                png_fixture(16, 6, &[0, 0, 0, 0, 129, 0x80, 0, 0xff, 0xff], &[]),
                [0, 1, 128, 255],
            ),
        ];

        for (bytes, expected) in cases {
            let decoded = decode_fixture(&bytes).expect("the supported PNG source decodes");
            assert_eq!(decoded.dimensions(), (1, 1));
            assert_eq!(decoded.get_pixel(0, 0).0, expected);
        }
    }

    #[test]
    fn allowed_png_and_jpeg_srgb_profiles_decode_but_unknown_profiles_are_refused() {
        let allowed_profiles: [&[u8]; 3] = [
            include_bytes!("../assets/sRGB2014.icc"),
            include_bytes!("../assets/sRGB_v4_ICC_preference.icc"),
            include_bytes!("../assets/sRGB_v4_ICC_preference_displayclass.icc"),
        ];
        let expected = [
            (
                3_024,
                "384b832de3412066743b52a75ee906b6fb9fb8d9e09e936fc2c43223815c6e0a",
            ),
            (
                60_960,
                "83174717332326ddc198d9df188a4daec27b8979ba152cebbfc470c793d0bb11",
            ),
            (
                60_988,
                "f54b145a18e4b12112750e672f1c79cac9347dc8403da3955e7f74a352816a21",
            ),
        ];

        for (index, allowed) in allowed_profiles.into_iter().enumerate() {
            assert_eq!(allowed.len(), expected[index].0);
            assert_eq!(format!("{:x}", Sha256::digest(allowed)), expected[index].1);

            let mut iccp = b"sRGB\0\0".to_vec();
            iccp.extend_from_slice(&zlib_stored(allowed));
            let allowed_png = png_fixture(8, 2, &[0, 10, 20, 30], &[(b"iCCP", &iccp)]);
            assert!(decode_fixture(&allowed_png).is_ok());

            let allowed_jpeg = jpeg_fixture(Some(allowed), ExtendedColorType::Rgb8, &[10, 20, 30]);
            assert!(decode_fixture(&allowed_jpeg).is_ok());
        }

        let unknown = [0_u8; 128];
        let unknown_jpeg = jpeg_fixture(Some(&unknown), ExtendedColorType::Rgb8, &[10, 20, 30]);
        assert_eq!(
            decode_fixture(&unknown_jpeg)
                .expect_err("an unknown ICC profile is rejected")
                .code,
            ImagingFailureCode::UnsupportedColorProfile
        );
    }

    #[test]
    fn png_color_declarations_accept_srgb_values_and_reject_contradictions() {
        let srgb_gamma = 45_455_u32.to_be_bytes();
        let srgb_chromaticities = png_chromaticities([
            31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
        ]);
        let srgb_intent = [0_u8];

        for chunks in [
            vec![
                (b"sRGB", srgb_intent.as_slice()),
                (b"gAMA", srgb_gamma.as_slice()),
                (b"cHRM", srgb_chromaticities.as_slice()),
            ],
            vec![
                (b"gAMA", srgb_gamma.as_slice()),
                (b"cHRM", srgb_chromaticities.as_slice()),
            ],
        ] {
            let png = png_fixture(8, 2, &[0, 10, 20, 30], &chunks);
            assert!(
                decode_fixture(&png).is_ok(),
                "canonical sRGB declarations are mutually consistent"
            );
        }

        let conflicting_gamma = 100_000_u32.to_be_bytes();
        let conflicting_chromaticities = png_chromaticities([
            34_567, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
        ]);
        for chunks in [
            vec![
                (b"sRGB", srgb_intent.as_slice()),
                (b"gAMA", conflicting_gamma.as_slice()),
            ],
            vec![
                (b"sRGB", srgb_intent.as_slice()),
                (b"cHRM", conflicting_chromaticities.as_slice()),
            ],
            vec![(b"gAMA", conflicting_gamma.as_slice())],
        ] {
            let png = png_fixture(8, 2, &[0, 10, 20, 30], &chunks);
            assert_eq!(
                decode_fixture(&png)
                    .expect_err("a contradictory PNG color declaration is rejected in preflight")
                    .code,
                ImagingFailureCode::UnsupportedColorProfile
            );
        }
    }

    #[test]
    fn png_cicp_accepts_only_full_range_srgb_and_rejects_conflicts() {
        let srgb_cicp = [0x01, 0x0d, 0x00, 0x01];
        let srgb_intent = [0_u8];
        let srgb_gamma = 45_455_u32.to_be_bytes();
        let srgb_chromaticities = png_chromaticities([
            31_270, 32_900, 64_000, 33_000, 30_000, 60_000, 15_000, 6_000,
        ]);

        for chunks in [
            vec![(b"cICP", srgb_cicp.as_slice())],
            vec![
                (b"cICP", srgb_cicp.as_slice()),
                (b"sRGB", srgb_intent.as_slice()),
                (b"gAMA", srgb_gamma.as_slice()),
                (b"cHRM", srgb_chromaticities.as_slice()),
            ],
        ] {
            let png = png_fixture(8, 2, &[0, 10, 20, 30], &chunks);
            assert!(
                decode_fixture(&png).is_ok(),
                "full-range sRGB cICP is compatible with canonical sRGB declarations"
            );
        }

        let display_p3 = [0x0c, 0x0d, 0x00, 0x01];
        let pq = [0x09, 0x10, 0x00, 0x01];
        let hlg = [0x09, 0x12, 0x00, 0x01];
        let malformed = [0x01, 0x0d, 0x00];
        let non_rgb_matrix = [0x01, 0x0d, 0x01, 0x01];
        let narrow_range = [0x01, 0x0d, 0x00, 0x00];
        for chunks in [
            vec![(b"cICP", display_p3.as_slice())],
            vec![(b"cICP", pq.as_slice())],
            vec![(b"cICP", hlg.as_slice())],
            vec![(b"cICP", malformed.as_slice())],
            vec![(b"cICP", non_rgb_matrix.as_slice())],
            vec![(b"cICP", narrow_range.as_slice())],
            vec![
                (b"cICP", srgb_cicp.as_slice()),
                (b"cICP", srgb_cicp.as_slice()),
            ],
            vec![
                (b"cICP", display_p3.as_slice()),
                (b"sRGB", srgb_intent.as_slice()),
            ],
        ] {
            let png = png_fixture(8, 2, &[0, 10, 20, 30], &chunks);
            assert_eq!(
                decode_fixture(&png)
                    .expect_err("a non-sRGB or contradictory cICP declaration is rejected")
                    .code,
                ImagingFailureCode::UnsupportedColorProfile
            );
        }
    }

    #[test]
    fn jpeg_rgb_grayscale_and_exif_orientation_are_normalized_once() {
        let rgb = jpeg_fixture(None, ExtendedColorType::Rgb8, &[10, 20, 30]);
        assert!(decode_fixture(&rgb).is_ok());
        let gray = jpeg_fixture(None, ExtendedColorType::L8, &[73]);
        let gray = decode_fixture(&gray).expect("the grayscale JPEG decodes");
        let gray_pixel = gray.get_pixel(0, 0);
        assert_eq!(gray_pixel[0], gray_pixel[1]);
        assert_eq!(gray_pixel[1], gray_pixel[2]);

        let mut source = RgbImage::new(8, 4);
        for (x, _, pixel) in source.enumerate_pixels_mut() {
            *pixel = if x < 4 {
                Rgb([240, 10, 10])
            } else {
                Rgb([10, 10, 240])
            };
        }
        let mut oriented = Vec::new();
        JpegEncoder::new_with_quality(&mut oriented, 100)
            .encode(
                source.as_raw(),
                source.width(),
                source.height(),
                ExtendedColorType::Rgb8,
            )
            .expect("the asymmetric JPEG is encoded");
        insert_exif_orientation(&mut oriented, 6);

        let oriented = decode_fixture(&oriented).expect("the oriented JPEG decodes");

        assert_eq!(oriented.dimensions(), (4, 8));
        let top = oriented.get_pixel(2, 1);
        let bottom = oriented.get_pixel(2, 6);
        assert!(top[0] > top[2] * 3, "the red half rotates to the top");
        assert!(
            bottom[2] > bottom[0] * 3,
            "the blue half rotates to the bottom exactly once"
        );
    }

    #[test]
    fn jpeg_preflight_accepts_only_explicit_supported_color_models_and_adobe_transforms() {
        let ycbcr = jpeg_fixture(None, ExtendedColorType::Rgb8, &[10, 20, 30]);
        assert!(decode_fixture(&ycbcr).is_ok());

        let mut ycbcr_with_transform = ycbcr.clone();
        insert_adobe_transform(&mut ycbcr_with_transform, 1);
        assert!(decode_fixture(&ycbcr_with_transform).is_ok());

        let mut rgb = ycbcr.clone();
        replace_jpeg_component_ids(&mut rgb, *b"RGB");
        insert_adobe_transform(&mut rgb, 0);
        assert!(decode_fixture(&rgb).is_ok());

        let mut rgb_with_numeric_component_ids = ycbcr.clone();
        insert_adobe_transform(&mut rgb_with_numeric_component_ids, 0);
        assert!(
            decode_fixture(&rgb_with_numeric_component_ids).is_ok(),
            "Adobe APP14 transform=0 takes precedence over numeric component identifiers"
        );

        let progressive = include_bytes!("../tests/fixtures/progressive-420-dri.jpg");
        assert!(decode_fixture(progressive).is_ok());

        for transform in [2, 3] {
            let mut incompatible = ycbcr.clone();
            insert_adobe_transform(&mut incompatible, transform);
            assert_eq!(
                decode_fixture(&incompatible)
                    .expect_err("an incompatible Adobe transform is rejected before decode")
                    .code,
                ImagingFailureCode::UnsupportedColorModel
            );
        }

        let mut unknown = ycbcr;
        replace_jpeg_component_ids(&mut unknown, [7, 8, 9]);
        assert_eq!(
            decode_fixture(&unknown)
                .expect_err("unknown component identifiers are rejected before decode")
                .code,
            ImagingFailureCode::UnsupportedColorModel
        );
    }

    #[test]
    fn jpeg_preflight_refuses_nonstandard_and_four_component_models_before_decode() {
        let unsupported_variant = [0xff, 0xd8, 0xff, 0xc1, 0, 2];
        assert_eq!(
            decode_fixture(&unsupported_variant)
                .expect_err("SOF1 is outside the accepted JPEG variants")
                .code,
            ImagingFailureCode::UnsupportedSourceVariant
        );

        let four_component = four_component_jpeg(false);
        assert_eq!(
            decode_fixture(&four_component)
                .expect_err("CMYK is rejected before conversion")
                .code,
            ImagingFailureCode::UnsupportedColorModel
        );
        let ycck = four_component_jpeg(true);
        assert_eq!(
            decode_fixture(&ycck)
                .expect_err("YCCK is rejected before conversion")
                .code,
            ImagingFailureCode::UnsupportedColorModel
        );
    }

    #[test]
    fn progressive_jpeg_that_exceeds_decoder_working_budget_is_rejected_in_preflight() {
        let oversized = progressive_jpeg_header(12_000, 10_000);

        let failure = match open_fixture(&oversized) {
            Ok(_) => {
                panic!("the hostile progressive allocation plan must be rejected in preflight")
            }
            Err(failure) => failure,
        };

        assert_eq!(failure.code, ImagingFailureCode::ResourceLimitExceeded);
        assert_eq!(failure.path_code, None);
    }

    fn decode_fixture(bytes: &[u8]) -> Result<image::RgbaImage, super::SourceFailure> {
        open_fixture(bytes)?.decode()
    }

    fn open_fixture(bytes: &[u8]) -> Result<super::OpenRenderSource, super::SourceFailure> {
        let root = tempfile::tempdir().expect("temporary render-source fixture");
        let path = root.path().join("original.bin");
        std::fs::write(&path, bytes).expect("the render-source fixture is writable");
        let mut context = OperationPathContext::new();
        context.capture(&path).expect("the source root is captured");
        let resolved = context
            .freeze()
            .resolve_existing(&path, ExpectedObject::RegularFile)
            .expect("the source is resolved once");
        open_render_source(&resolved)
    }

    fn jpeg_fixture(profile: Option<&[u8]>, color: ExtendedColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 100);
        if let Some(profile) = profile {
            encoder
                .set_icc_profile(profile.to_vec())
                .expect("the test ICC profile fits JPEG APP2");
        }
        encoder
            .encode(pixels, 1, 1, color)
            .expect("the JPEG fixture is encoded");
        bytes
    }

    fn insert_exif_orientation(jpeg: &mut Vec<u8>, orientation: u16) {
        let mut payload = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        payload.extend_from_slice(&orientation.to_le_bytes());
        payload.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        let mut segment = vec![0xff, 0xe1];
        segment.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(&payload);
        jpeg.splice(2..2, segment);
    }

    fn insert_adobe_transform(jpeg: &mut Vec<u8>, transform: u8) {
        let segment = [
            0xff, 0xee, 0, 14, b'A', b'd', b'o', b'b', b'e', 0, 100, 0, 0, 0, 0, transform,
        ];
        jpeg.splice(2..2, segment);
    }

    fn replace_jpeg_component_ids(jpeg: &mut [u8], component_ids: [u8; 3]) {
        let mut cursor = 2;
        while cursor + 4 <= jpeg.len() {
            assert_eq!(jpeg[cursor], 0xff, "the fixture marker is aligned");
            while cursor < jpeg.len() && jpeg[cursor] == 0xff {
                cursor += 1;
            }
            let marker = jpeg[cursor];
            cursor += 1;
            if marker == 0xd8 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
                continue;
            }
            let length = usize::from(u16::from_be_bytes([jpeg[cursor], jpeg[cursor + 1]]));
            let payload_start = cursor + 2;
            let payload_end = cursor + length;
            match marker {
                0xc0 | 0xc2 => {
                    assert_eq!(jpeg[payload_start + 5], 3);
                    for (index, component_id) in component_ids.into_iter().enumerate() {
                        jpeg[payload_start + 6 + index * 3] = component_id;
                    }
                }
                0xda => {
                    assert_eq!(jpeg[payload_start], 3);
                    for (index, component_id) in component_ids.into_iter().enumerate() {
                        jpeg[payload_start + 1 + index * 2] = component_id;
                    }
                    return;
                }
                _ => {}
            }
            cursor = payload_end;
        }
        panic!("the JPEG fixture did not contain a scan header");
    }

    fn four_component_jpeg(ycck: bool) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8];
        if ycck {
            bytes.extend_from_slice(&[
                0xff, 0xee, 0, 14, b'A', b'd', b'o', b'b', b'e', 0, 100, 0, 0, 0, 0, 2,
            ]);
        }
        bytes.extend_from_slice(&[
            0xff, 0xc0, 0, 20, 8, 0, 1, 0, 1, 4, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0, 4, 0x11, 0,
        ]);
        bytes
    }

    fn progressive_jpeg_header(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xc2, 0, 17, 8];
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 0, 3, 0x11, 0]);
        bytes.extend_from_slice(&[0xff, 0xda, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]);
        bytes
    }

    fn png_fixture(
        bit_depth: u8,
        color_type: u8,
        scanline: &[u8],
        chunks: &[(&[u8; 4], &[u8])],
    ) -> Vec<u8> {
        let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut ihdr = Vec::from(1_u32.to_be_bytes());
        ihdr.extend_from_slice(&1_u32.to_be_bytes());
        ihdr.extend_from_slice(&[bit_depth, color_type, 0, 0, 0]);
        push_png_chunk(&mut bytes, b"IHDR", &ihdr);
        for (kind, payload) in chunks {
            push_png_chunk(&mut bytes, kind, payload);
        }
        push_png_chunk(&mut bytes, b"IDAT", &zlib_stored(scanline));
        push_png_chunk(&mut bytes, b"IEND", &[]);
        bytes
    }

    fn png_chromaticities(values: [u32; 8]) -> Vec<u8> {
        values.into_iter().flat_map(u32::to_be_bytes).collect()
    }

    fn zlib_stored(bytes: &[u8]) -> Vec<u8> {
        assert!(bytes.len() <= u16::MAX as usize);
        let length = bytes.len() as u16;
        let mut encoded = vec![0x78, 0x01, 0x01];
        encoded.extend_from_slice(&length.to_le_bytes());
        encoded.extend_from_slice(&(!length).to_le_bytes());
        encoded.extend_from_slice(bytes);
        encoded.extend_from_slice(&adler32(bytes).to_be_bytes());
        encoded
    }

    fn adler32(bytes: &[u8]) -> u32 {
        let (mut a, mut b) = (1_u32, 0_u32);
        for byte in bytes {
            a = (a + u32::from(*byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        (b << 16) | a
    }

    fn push_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
        output.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(payload);
        let mut crc_input = Vec::from(*kind);
        crc_input.extend_from_slice(payload);
        output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }
}
