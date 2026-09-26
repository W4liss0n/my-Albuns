use std::{
    fs::File,
    io::{BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) use crate::ipc_contract::{PreparedEyeCorrection as PreparedEyes, ViewerFace as Face};
use image::{
    ColorType, DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader, Limits, Rgba, RgbaImage, imageops::FilterType, metadata::Orientation,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
struct V2 {
    x: f32,
    y: f32,
}

impl V2 {
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
    fn mul(self, factor: f32) -> Self {
        Self {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
    fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

struct Eye {
    center: V2,
    along: V2,
    width: f32,
    openness: f32,
}

fn point(face: &Face, index: usize, width: u32, height: u32) -> Result<V2, String> {
    let point = face
        .0
        .get(index)
        .ok_or("Não foi possível identificar os olhos deste rosto. Selecione outro rosto.")?;
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !point.z.is_finite()
        || !(-0.1..=1.1).contains(&point.x)
        || !(-0.1..=1.1).contains(&point.y)
    {
        return Err(
            "Não foi possível identificar os olhos deste rosto. Selecione outro rosto.".into(),
        );
    }
    Ok(V2 {
        x: point.x * width as f32,
        y: point.y * height as f32,
    })
}

fn eye(
    face: &Face,
    size: (u32, u32),
    corners: (usize, usize),
    lids: (usize, usize),
) -> Result<Eye, String> {
    let a = point(face, corners.0, size.0, size.1)?;
    let b = point(face, corners.1, size.0, size.1)?;
    let top = point(face, lids.0, size.0, size.1)?;
    let bottom = point(face, lids.1, size.0, size.1)?;
    let vector = b.sub(a);
    let width = vector.length();
    if width < 8.0 {
        return Err("O rosto é pequeno demais para corrigir os olhos.".into());
    }
    Ok(Eye {
        center: a.add(b).mul(0.5),
        along: vector.mul(1.0 / width),
        width,
        openness: bottom.sub(top).length() / width,
    })
}

fn validate_pair(target: &[Eye; 2], reference: &[Eye; 2]) -> Result<(), String> {
    for (dst, src) in target.iter().zip(reference.iter()) {
        if src.openness < 0.12 {
            return Err("Os olhos da referência precisam estar abertos.".into());
        }
        if !(0.35..=3.0).contains(&(dst.width / src.width)) {
            return Err(
                "Os rostos têm tamanhos muito diferentes. Escolha outra referência.".into(),
            );
        }
    }
    let target_ratio = target[0].width / target[1].width;
    let reference_ratio = reference[0].width / reference[1].width;
    if !(0.6..=1.65).contains(&(target_ratio / reference_ratio)) {
        return Err(
            "Os rostos estão em posições muito diferentes. Escolha outra referência.".into(),
        );
    }
    Ok(())
}

fn bilinear(image: &RgbaImage, pos: V2) -> [f32; 3] {
    let x = pos.x.clamp(0.0, (image.width() - 1) as f32);
    let y = pos.y.clamp(0.0, (image.height() - 1) as f32);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width() - 1);
    let y1 = (y0 + 1).min(image.height() - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let pixels = [
        image.get_pixel(x0, y0),
        image.get_pixel(x1, y0),
        image.get_pixel(x0, y1),
        image.get_pixel(x1, y1),
    ];
    std::array::from_fn(|channel| {
        let top = pixels[0][channel] as f32 * (1.0 - fx) + pixels[1][channel] as f32 * fx;
        let bottom = pixels[2][channel] as f32 * (1.0 - fx) + pixels[3][channel] as f32 * fx;
        top * (1.0 - fy) + bottom * fy
    })
}

fn map_eye(dst: &Eye, src: &Eye, pos: V2) -> V2 {
    let diff = pos.sub(dst.center);
    let dst_perp = V2 {
        x: -dst.along.y,
        y: dst.along.x,
    };
    let src_perp = V2 {
        x: -src.along.y,
        y: src.along.x,
    };
    let scale = src.width / dst.width;
    src.center
        .add(
            src.along
                .mul((diff.x * dst.along.x + diff.y * dst.along.y) * scale),
        )
        .add(src_perp.mul((diff.x * dst_perp.x + diff.y * dst_perp.y) * scale))
}

fn transplant_eye(
    target: &mut RgbaImage,
    reference: &RgbaImage,
    dst: &Eye,
    src: &Eye,
) -> Result<(), String> {
    let rx = dst.width * 0.72;
    let ry = dst.width * 0.37;
    let bound = dst.width * 0.85;
    let x0 = (dst.center.x - bound).floor().max(0.0) as u32;
    let y0 = (dst.center.y - bound).floor().max(0.0) as u32;
    let x1 = (dst.center.x + bound)
        .ceil()
        .min(target.width() as f32 - 1.0) as u32;
    let y1 = (dst.center.y + bound)
        .ceil()
        .min(target.height() as f32 - 1.0) as u32;
    let perpendicular = V2 {
        x: -dst.along.y,
        y: dst.along.x,
    };
    let mut correction = [0.0_f32; 3];
    let mut samples = 0.0_f32;
    // Match skin immediately above and below the eyelid, away from the iris.
    for x in [-0.46, -0.23, 0.0, 0.23, 0.46] {
        for y in [-0.91, -0.78, 0.78, 0.91] {
            let position = dst
                .center
                .add(dst.along.mul(x * rx))
                .add(perpendicular.mul(y * ry));
            let source_position = map_eye(dst, src, position);
            if source_position.x < 0.0
                || source_position.y < 0.0
                || source_position.x >= reference.width() as f32
                || source_position.y >= reference.height() as f32
            {
                continue;
            }
            let from = bilinear(reference, source_position);
            let to = bilinear(target, position);
            for channel in 0..3 {
                correction[channel] += to[channel] - from[channel];
            }
            samples += 1.0;
        }
    }
    if samples < 8.0 {
        return Err("O olho está próximo demais da borda da foto.".into());
    }
    for channel in &mut correction {
        *channel = (*channel / samples).clamp(-38.0, 38.0);
    }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let position = V2 {
                x: x as f32 + 0.5,
                y: y as f32 + 0.5,
            };
            let diff = position.sub(dst.center);
            let u = (diff.x * dst.along.x + diff.y * dst.along.y) / rx;
            let v = (diff.x * perpendicular.x + diff.y * perpendicular.y) / ry;
            let radius = (u * u + v * v).sqrt();
            if radius >= 1.0 {
                continue;
            }
            let source_position = map_eye(dst, src, position);
            if source_position.x < 0.0
                || source_position.y < 0.0
                || source_position.x >= reference.width() as f32
                || source_position.y >= reference.height() as f32
            {
                continue;
            }
            let source = bilinear(reference, source_position);
            let alpha = ((1.0 - radius) / 0.3).clamp(0.0, 1.0);
            let original = target.get_pixel(x, y);
            let blended: [u8; 3] = std::array::from_fn(|channel| {
                ((source[channel] + correction[channel]).clamp(0.0, 255.0) * alpha
                    + original[channel] as f32 * (1.0 - alpha))
                    .round() as u8
            });
            target.put_pixel(
                x,
                y,
                Rgba([blended[0], blended[1], blended[2], original[3]]),
            );
        }
    }
    Ok(())
}

const SRGB_PROFILES: [&[u8]; 3] = [
    include_bytes!("../../crates/myalbuns-imaging/assets/sRGB2014.icc"),
    include_bytes!("../../crates/myalbuns-imaging/assets/sRGB_v4_ICC_preference.icc"),
    include_bytes!("../../crates/myalbuns-imaging/assets/sRGB_v4_ICC_preference_displayclass.icc"),
];
const LEGACY_SRGB_SHA256: [u8; 32] = [
    0x2b, 0x3a, 0xa1, 0x64, 0x57, 0x79, 0xa9, 0xe6, 0x34, 0x74, 0x4f, 0xaf, 0x9b, 0x01, 0xe9, 0x10,
    0x2b, 0x0c, 0x9b, 0x88, 0xfd, 0x6d, 0xec, 0xed, 0x79, 0x34, 0xdf, 0x86, 0xb9, 0x49, 0xaf, 0x7e,
];

fn validate_profile(profile: &[u8]) -> Result<(), String> {
    if SRGB_PROFILES.contains(&profile)
        || (profile.len() == 3_144 && Sha256::digest(profile)[..] == LEGACY_SRGB_SHA256)
    {
        Ok(())
    } else {
        Err(
            "O perfil de cor desta foto não é compatível com a correção. Use uma foto em sRGB."
                .into(),
        )
    }
}

fn validate_depth(color: ColorType) -> Result<(), String> {
    if matches!(
        color,
        ColorType::L16
            | ColorType::La16
            | ColorType::Rgb16
            | ColorType::Rgba16
            | ColorType::Rgb32F
            | ColorType::Rgba32F
    ) {
        Err("Esta correção aceita apenas fotos com até 8 bits por canal.".into())
    } else {
        Ok(())
    }
}

fn open_upright(path: &Path) -> Result<RgbaImage, String> {
    let reader = ImageReader::open(path)
        .map_err(|_| "Não foi possível abrir uma das fotos.")?
        .with_guessed_format()
        .map_err(|_| "Não foi possível identificar o formato de uma das fotos.")?;
    let format = reader.format();
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "Não foi possível ler uma das fotos.")?;
    validate_depth(decoder.color_type())?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > 10_000
        || height > 10_000
        || u64::from(width) * u64::from(height) > 36_000_000
    {
        return Err(
            "Esta correção aceita fotos de até 36 MP, com largura e altura de até 10.000 pixels."
                .into(),
        );
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(10_000);
    limits.max_image_height = Some(10_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    decoder
        .set_limits(limits)
        .map_err(|_| "A foto é grande demais para esta correção.".to_string())?;
    if let Some(profile) = decoder
        .icc_profile()
        .map_err(|_| "Não foi possível ler o perfil de cor da foto.")?
    {
        validate_profile(&profile)?;
    }
    let orientation = decoder
        .orientation()
        .map_err(|_| "Não foi possível ler a orientação da foto.")?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|_| "Não foi possível ler uma das fotos.")?;
    if matches!(format, Some(ImageFormat::Jpeg | ImageFormat::Tiff)) {
        image.apply_orientation(orientation);
    }
    Ok(image.to_rgba8())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderStage {
    Decode,
    Composite,
    Encode,
}

/// Everything needed to rebuild the corrected photo at confirmation: the
/// preparation keeps only its preview, so saving composes again from these
/// originals after checking that neither changed.
#[derive(Clone)]
pub(crate) struct CorrectionSource {
    pub(crate) target: PathBuf,
    pub(crate) reference: PathBuf,
    pub(crate) target_face: Face,
    pub(crate) reference_face: Face,
    pub(crate) target_digest: [u8; 32],
    pub(crate) reference_digest: [u8; 32],
}

/// Decodes both photos upright and transplants the reference eyes onto the target.
fn compose(
    target_path: &Path,
    reference_path: &Path,
    target_face: &Face,
    reference_face: &Face,
    checkpoint: impl Fn(RenderStage) -> Result<(), String>,
) -> Result<RgbaImage, String> {
    checkpoint(RenderStage::Decode)?;
    let (mut target, reference) = std::thread::scope(|scope| {
        let reference = scope.spawn(|| open_upright(reference_path));
        let target = open_upright(target_path)?;
        let reference = reference
            .join()
            .map_err(|_| "A leitura da referência foi interrompida.")??;
        Ok::<_, String>((target, reference))
    })?;
    checkpoint(RenderStage::Composite)?;
    let target_size = target.dimensions();
    let reference_size = reference.dimensions();
    let dst = [
        eye(target_face, target_size, (33, 133), (159, 145))?,
        eye(target_face, target_size, (362, 263), (386, 374))?,
    ];
    let src = [
        eye(reference_face, reference_size, (33, 133), (159, 145))?,
        eye(reference_face, reference_size, (362, 263), (386, 374))?,
    ];
    validate_pair(&dst, &src)?;
    for (dst, src) in dst.iter().zip(src.iter()) {
        transplant_eye(&mut target, &reference, dst, src)?;
    }
    Ok(target)
}

/// Prepares only the PNG preview; nothing is written to disk.
pub(crate) fn render_preview(
    target_path: &Path,
    reference_path: &Path,
    target_face: &Face,
    reference_face: &Face,
    checkpoint: impl Fn(RenderStage) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let target = compose(
        target_path,
        reference_path,
        target_face,
        reference_face,
        &checkpoint,
    )?;
    checkpoint(RenderStage::Encode)?;
    let target = DynamicImage::ImageRgba8(target);
    let preview_scale = (1600.0 / target.width().max(target.height()) as f32).min(1.0);
    let preview_width = ((target.width() as f32 * preview_scale).round() as u32).max(1);
    let preview_height = ((target.height() as f32 * preview_scale).round() as u32).max(1);
    let preview = target.resize_exact(preview_width, preview_height, FilterType::Lanczos3);
    let mut bytes = Cursor::new(Vec::new());
    preview
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|_| "Não foi possível preparar a prévia.")?;
    Ok(bytes.into_inner())
}

pub(crate) fn source_digest(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|_| "Não foi possível conferir o arquivo original.")?;
    let mut digest = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|_| "Não foi possível conferir o arquivo original.")?;
        if count == 0 {
            break;
        }
        digest.update(&chunk[..count]);
    }
    Ok(digest.finalize().into())
}

fn original_format(path: &Path) -> Result<ImageFormat, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let expected = match extension.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "tif" | "tiff" => ImageFormat::Tiff,
        _ => {
            return Err(
                "O formato do original não permite substituir esta foto com segurança.".into(),
            );
        }
    };
    let actual = ImageReader::open(path)
        .map_err(|_| "Não foi possível abrir o original.")?
        .with_guessed_format()
        .map_err(|_| "Não foi possível identificar o formato do original.")?
        .format();
    if actual != Some(expected) {
        return Err("O formato do arquivo original não corresponde à sua extensão.".into());
    }
    Ok(expected)
}

/// Encode only at confirmation; the destination receives bytes matching its
/// original extension.
fn encode_replacement(corrected: RgbaImage, original: &Path, staging: &Path) -> Result<(), String> {
    let format = original_format(original)?;
    let mut decoder = ImageReader::open(original)
        .map_err(|_| "Não foi possível abrir o original.")?
        .with_guessed_format()
        .map_err(|_| "Não foi possível identificar o formato do original.")?
        .into_decoder()
        .map_err(|_| "Não foi possível ler os metadados do original.")?;
    validate_depth(decoder.color_type())?;
    let dimensions = decoder.dimensions();
    let icc = decoder
        .icc_profile()
        .map_err(|_| "Não foi possível ler o perfil de cor da foto.")?;
    if let Some(profile) = &icc {
        validate_profile(profile)?;
    }
    let mut exif = decoder
        .exif_metadata()
        .map_err(|_| "Não foi possível ler os metadados do original.")?;
    let orientation = decoder
        .orientation()
        .map_err(|_| "Não foi possível ler a orientação da foto.")?;
    if let Some(metadata) = &mut exif {
        let _ = Orientation::remove_from_exif_chunk(metadata);
    }
    let (width, height) = corrected.dimensions();
    let rotated = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    let expected = if rotated && matches!(format, ImageFormat::Jpeg | ImageFormat::Tiff) {
        (dimensions.1, dimensions.0)
    } else {
        dimensions
    };
    if (width, height) != expected {
        return Err("A correção preparada não corresponde ao tamanho original da foto.".into());
    }
    let file = File::create(staging)
        .map_err(|_| "Não foi possível preparar a substituição do original.")?;
    let mut writer = BufWriter::new(file);
    match format {
        ImageFormat::Jpeg => {
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 95);
            if let Some(profile) = icc {
                encoder
                    .set_icc_profile(profile)
                    .map_err(|_| "Não foi possível salvar a foto mantendo o perfil de cor.")?;
            }
            if let Some(metadata) = exif {
                encoder
                    .set_exif_metadata(metadata)
                    .map_err(|_| "Não foi possível salvar a foto mantendo os metadados.")?;
            }
            let rgb = DynamicImage::ImageRgba8(corrected).to_rgb8();
            encoder
                .write_image(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
                .map_err(|_| "Não foi possível salvar a foto corrigida em JPEG.")?;
        }
        ImageFormat::Png => {
            let mut encoder = image::codecs::png::PngEncoder::new(&mut writer);
            if let Some(profile) = icc {
                encoder
                    .set_icc_profile(profile)
                    .map_err(|_| "Não foi possível salvar a foto mantendo o perfil de cor.")?;
            }
            if let Some(metadata) = exif {
                encoder
                    .set_exif_metadata(metadata)
                    .map_err(|_| "Não foi possível salvar a foto mantendo os metadados.")?;
            }
            encoder
                .write_image(corrected.as_raw(), width, height, ExtendedColorType::Rgba8)
                .map_err(|_| "Não foi possível salvar a foto corrigida em PNG.")?;
        }
        ImageFormat::Tiff => {
            let mut encoder = image::codecs::tiff::TiffEncoder::new(&mut writer);
            if let Some(profile) = icc {
                encoder
                    .set_icc_profile(profile)
                    .map_err(|_| "Não foi possível salvar a foto mantendo o perfil de cor.")?;
            }
            encoder
                .write_image(corrected.as_raw(), width, height, ExtendedColorType::Rgba8)
                .map_err(|_| "Não foi possível salvar a foto corrigida em TIFF.")?;
        }
        _ => unreachable!(),
    }
    writer
        .flush()
        .map_err(|_| "Não foi possível gravar a foto corrigida.")?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| "Não foi possível concluir a gravação da foto corrigida.")?;
    Ok(())
}

#[cfg(windows)]
fn replace_file(original: &Path, staging: &Path, backup: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let (original_wide, staging_wide, backup_wide) = (wide(original), wide(staging), wide(backup));
    let result = unsafe {
        ReplaceFileW(
            original_wide.as_ptr(),
            staging_wide.as_ptr(),
            backup_wide.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result != 0 {
        return Ok(());
    }
    // ReplaceFileW can fail after moving the replaced file to its backup.
    if backup.exists() {
        let restored = if original.exists() {
            restore_original(original, backup)
        } else {
            std::fs::rename(backup, original).map_err(|_| "Não foi possível restaurar automaticamente o original; a cópia de segurança foi mantida.".into())
        };
        restored?;
    }
    Err("Não foi possível substituir o arquivo original.".into())
}

#[cfg(not(windows))]
fn replace_file(original: &Path, staging: &Path, backup: &Path) -> Result<(), String> {
    std::fs::copy(original, backup).map_err(|_| "Não foi possível proteger o original.")?;
    std::fs::rename(staging, original)
        .map_err(|_| "Não foi possível substituir o arquivo original.".into())
}

const TARGET_CHANGED: &str =
    "A foto original mudou desde a prévia. Feche a correção e use Abrir olhos novamente.";
const REFERENCE_CHANGED: &str =
    "A foto de referência mudou desde a prévia. Feche a correção e use Abrir olhos novamente.";

fn verify_sources(source: &CorrectionSource) -> Result<(), String> {
    if source_digest(&source.target)? != source.target_digest {
        return Err(TARGET_CHANGED.into());
    }
    if source_digest(&source.reference)? != source.reference_digest {
        return Err(REFERENCE_CHANGED.into());
    }
    Ok(())
}

/// Rebuilds the previewed correction from the unchanged originals and replaces
/// the target with it. Returns the backup of the replaced original.
pub(crate) fn replace_original(source: &CorrectionSource) -> Result<PathBuf, String> {
    verify_sources(source)?;
    let corrected = compose(
        &source.target,
        &source.reference,
        &source.target_face,
        &source.reference_face,
        |_| Ok(()),
    )?;
    verify_sources(source)?;
    commit_replacement(&source.target, corrected, source.target_digest)
}

fn commit_replacement(
    original: &Path,
    corrected: RgbaImage,
    expected_digest: [u8; 32],
) -> Result<PathBuf, String> {
    if source_digest(original)? != expected_digest {
        return Err(TARGET_CHANGED.into());
    }
    let folder = original
        .parent()
        .ok_or("A localização do original é inválida.")?;
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let staging = folder.join(format!(".myalbuns-eye-{nonce}.stage"));
    let backup = folder.join(format!(".myalbuns-eye-{nonce}.backup"));
    let result = (|| {
        encode_replacement(corrected, original, &staging)?;
        if source_digest(original)? != expected_digest {
            return Err(TARGET_CHANGED.into());
        }
        replace_file(original, &staging, &backup)?;
        Ok(backup.clone())
    })();
    if result.is_err() && original.exists() {
        let _ = std::fs::remove_file(staging);
    }
    result
}

pub(crate) fn restore_original(original: &Path, backup: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
        let wide = |path: &Path| {
            path.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let (original, backup) = (wide(original), wide(backup));
        let result = unsafe {
            ReplaceFileW(
                original.as_ptr(),
                backup.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if result == 0 {
            return Err("Não foi possível restaurar automaticamente o original; a cópia de segurança foi mantida.".into());
        }
    }
    #[cfg(not(windows))]
    std::fs::rename(backup, original).map_err(|_| "Não foi possível restaurar automaticamente o original; a cópia de segurança foi mantida.")?;
    Ok(())
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    /// Two textured photos whose synthetic faces pass the pair validation.
    fn synthetic_pair(folder: &Path) -> CorrectionSource {
        let target = folder.join("target.png");
        let reference = folder.join("reference.png");
        RgbaImage::from_fn(200, 200, |x, y| {
            Rgba([80 + (x % 16) as u8, 100, 120 + (y % 8) as u8, 255])
        })
        .save(&target)
        .unwrap();
        RgbaImage::from_fn(200, 200, |x, y| {
            Rgba([(x * 5 % 256) as u8, (y * 3 % 256) as u8, 60, 255])
        })
        .save(&reference)
        .unwrap();
        let face = |opening: f32| {
            let mut points = vec![
                crate::ipc_contract::ViewerFacePoint {
                    x: 0.5,
                    y: 0.5,
                    z: 0.0
                };
                468
            ];
            for (a, b, top, bottom, center) in [(33, 133, 159, 145, 0.3), (362, 263, 386, 374, 0.7)]
            {
                points[a].x = center - 0.05;
                points[b].x = center + 0.05;
                points[top].x = center;
                points[bottom].x = center;
                points[top].y = 0.5 - opening * 0.05;
                points[bottom].y = 0.5 + opening * 0.05;
            }
            Face(points)
        };
        CorrectionSource {
            target_digest: source_digest(&target).unwrap(),
            reference_digest: source_digest(&reference).unwrap(),
            target,
            reference,
            target_face: face(0.06),
            reference_face: face(0.2),
        }
    }

    #[test]
    fn superseded_preview_stops_before_encoding() {
        let folder = tempfile::tempdir().unwrap();
        let source = synthetic_pair(folder.path());
        let stages = std::sync::Mutex::new(Vec::new());
        let result = render_preview(
            &source.target,
            &source.reference,
            &source.target_face,
            &source.reference_face,
            |stage| {
                stages.lock().unwrap().push(stage);
                if stage == RenderStage::Encode {
                    Err("A correção foi cancelada.".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err(), "A correção foi cancelada.");
        assert_eq!(
            *stages.lock().unwrap(),
            [
                RenderStage::Decode,
                RenderStage::Composite,
                RenderStage::Encode
            ]
        );
    }

    #[test]
    fn preview_writes_nothing_to_disk() {
        let folder = tempfile::tempdir().unwrap();
        let source = synthetic_pair(folder.path());
        let preview = render_preview(
            &source.target,
            &source.reference,
            &source.target_face,
            &source.reference_face,
            |_| Ok(()),
        )
        .unwrap();
        let preview = image::load_from_memory(&preview).unwrap();
        assert_eq!((preview.width(), preview.height()), (200, 200));
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 2);
    }

    #[test]
    fn saving_rebuilds_the_same_bytes_as_the_former_prepared_png() {
        let folder = tempfile::tempdir().unwrap();
        let source = synthetic_pair(folder.path());
        let former = folder.path().join("former.png");
        std::fs::copy(&source.target, &former).unwrap();
        // The former pipeline saved the composition as PNG and read it back at confirmation.
        let prepared = folder.path().join("prepared.png");
        compose(
            &source.target,
            &source.reference,
            &source.target_face,
            &source.reference_face,
            |_| Ok(()),
        )
        .unwrap()
        .save_with_format(&prepared, ImageFormat::Png)
        .unwrap();
        let reread = image::open(&prepared).unwrap().to_rgba8();
        let former_backup =
            commit_replacement(&former, reread, source_digest(&former).unwrap()).unwrap();
        let backup = replace_original(&source).unwrap();
        assert_ne!(source_digest(&source.target).unwrap(), source.target_digest);
        assert_eq!(
            std::fs::read(&source.target).unwrap(),
            std::fs::read(&former).unwrap()
        );
        std::fs::remove_file(backup).unwrap();
        std::fs::remove_file(former_backup).unwrap();
    }

    #[test]
    fn changed_reference_refuses_to_save_and_keeps_the_original() {
        let folder = tempfile::tempdir().unwrap();
        let source = synthetic_pair(folder.path());
        let before = std::fs::read(&source.target).unwrap();
        RgbaImage::from_pixel(200, 200, Rgba([10, 20, 30, 255]))
            .save(&source.reference)
            .unwrap();
        assert_eq!(replace_original(&source).unwrap_err(), REFERENCE_CHANGED);
        assert_eq!(std::fs::read(&source.target).unwrap(), before);
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 2);
    }

    fn eyes(widths: [f32; 2], openings: [f32; 2]) -> [Eye; 2] {
        std::array::from_fn(|index| Eye {
            center: V2 { x: 0.0, y: 0.0 },
            along: V2 { x: 1.0, y: 0.0 },
            width: widths[index],
            openness: openings[index],
        })
    }

    #[test]
    fn accepts_open_reference_with_similar_target_opening() {
        assert!(
            validate_pair(
                &eyes([24.0, 24.0], [0.18, 0.19]),
                &eyes([24.0, 24.0], [0.18, 0.18])
            )
            .is_ok()
        );
    }

    #[test]
    fn accepts_reference_when_one_target_eye_is_already_open() {
        assert!(
            validate_pair(
                &eyes([24.0, 24.0], [0.04, 0.20]),
                &eyes([24.0, 24.0], [0.18, 0.18])
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_a_closed_reference_eye() {
        let result = validate_pair(
            &eyes([24.0, 24.0], [0.04, 0.04]),
            &eyes([24.0, 24.0], [0.11, 0.18]),
        );
        assert_eq!(
            result.unwrap_err(),
            "Os olhos da referência precisam estar abertos."
        );
    }

    #[test]
    fn still_rejects_incompatible_scale_and_pose() {
        assert_eq!(
            validate_pair(
                &eyes([20.0, 20.0], [0.04, 0.04]),
                &eyes([80.0, 80.0], [0.18, 0.18])
            )
            .unwrap_err(),
            "Os rostos têm tamanhos muito diferentes. Escolha outra referência."
        );
        assert_eq!(
            validate_pair(
                &eyes([20.0, 20.0], [0.04, 0.04]),
                &eyes([20.0, 40.0], [0.18, 0.18])
            )
            .unwrap_err(),
            "Os rostos estão em posições muito diferentes. Escolha outra referência."
        );
    }

    #[test]
    fn replacement_preserves_original_format_dimensions_and_rollback() {
        let folder = tempfile::tempdir().unwrap();
        let corrected = RgbaImage::from_pixel(24, 16, Rgba([30, 100, 180, 255]));
        for (name, format) in [
            ("photo.jpg", ImageFormat::Jpeg),
            ("photo.png", ImageFormat::Png),
            ("photo.tiff", ImageFormat::Tiff),
        ] {
            let original = folder.path().join(name);
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(24, 16, Rgba([190, 40, 20, 255])))
                .save_with_format(&original, format)
                .unwrap();
            let before = std::fs::read(&original).unwrap();
            let digest = source_digest(&original).unwrap();
            let backup = commit_replacement(&original, corrected.clone(), digest).unwrap();
            assert_eq!(
                image::ImageReader::open(&original)
                    .unwrap()
                    .with_guessed_format()
                    .unwrap()
                    .format(),
                Some(format)
            );
            assert_eq!(image::image_dimensions(&original).unwrap(), (24, 16));
            assert_ne!(std::fs::read(&original).unwrap(), before);
            restore_original(&original, &backup).unwrap();
            assert_eq!(std::fs::read(&original).unwrap(), before);
        }
    }

    #[test]
    fn stale_original_aborts_without_writing() {
        let folder = tempfile::tempdir().unwrap();
        let original = folder.path().join("photo.png");
        RgbaImage::from_pixel(24, 16, Rgba([20, 30, 40, 255]))
            .save(&original)
            .unwrap();
        let digest = source_digest(&original).unwrap();
        RgbaImage::from_pixel(24, 16, Rgba([50, 60, 70, 255]))
            .save(&original)
            .unwrap();
        let latest = std::fs::read(&original).unwrap();
        assert!(
            commit_replacement(
                &original,
                RgbaImage::from_pixel(24, 16, Rgba([80, 90, 100, 255])),
                digest
            )
            .unwrap_err()
            .contains("mudou desde a prévia")
        );
        assert_eq!(std::fs::read(&original).unwrap(), latest);
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn high_depth_png_and_tiff_leave_originals_unchanged() {
        let folder = tempfile::tempdir().unwrap();
        for (name, format) in [
            ("deep.png", ImageFormat::Png),
            ("deep.tiff", ImageFormat::Tiff),
        ] {
            let original = folder.path().join(name);
            image::ImageBuffer::<Rgba<u16>, Vec<u16>>::from_pixel(
                24,
                16,
                Rgba([1000, 2000, 3000, 65535]),
            )
            .save_with_format(&original, format)
            .unwrap();
            let digest = source_digest(&original).unwrap();
            assert!(
                open_upright(&original)
                    .unwrap_err()
                    .contains("até 8 bits por canal")
            );
            assert!(
                commit_replacement(
                    &original,
                    RgbaImage::from_pixel(24, 16, Rgba([20, 30, 40, 255])),
                    digest
                )
                .unwrap_err()
                .contains("até 8 bits por canal")
            );
            assert_eq!(source_digest(&original).unwrap(), digest);
        }
    }

    #[test]
    fn jpeg_replacement_normalizes_exif_orientation_and_keeps_profile() {
        let folder = tempfile::tempdir().unwrap();
        let original = folder.path().join("rotated.jpg");
        let profile = SRGB_PROFILES[0].to_vec();
        let exif = vec![
            b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut file = File::create(&original).unwrap();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 95);
        encoder.set_icc_profile(profile.clone()).unwrap();
        encoder.set_exif_metadata(exif).unwrap();
        encoder
            .write_image(&vec![128; 16 * 24 * 3], 24, 16, ExtendedColorType::Rgb8)
            .unwrap();
        drop(file);
        let backup = commit_replacement(
            &original,
            RgbaImage::from_pixel(16, 24, Rgba([30, 40, 50, 255])),
            source_digest(&original).unwrap(),
        )
        .unwrap();
        let mut decoder = ImageReader::open(&original)
            .unwrap()
            .into_decoder()
            .unwrap();
        assert_eq!(decoder.orientation().unwrap(), Orientation::NoTransforms);
        assert_eq!(decoder.icc_profile().unwrap(), Some(profile));
        assert_eq!(decoder.dimensions(), (16, 24));
        std::fs::remove_file(backup).unwrap();
    }
}

#[cfg(test)]
mod qa_tests {
    use super::*;

    #[cfg(windows)]
    fn process_cpu_time() -> std::time::Duration {
        #[repr(C)]
        #[derive(Default)]
        struct FileTime {
            low: u32,
            high: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessTimes(
                process: *mut std::ffi::c_void,
                created: *mut FileTime,
                exited: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
        }
        let (mut created, mut exited, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        let ok = unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        };
        assert_ne!(ok, 0);
        let ticks = |time: FileTime| (u64::from(time.high) << 32) | u64::from(time.low);
        std::time::Duration::from_nanos((ticks(kernel) + ticks(user)) * 100)
    }

    fn real_pair() -> (PathBuf, PathBuf, Face, Face) {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../.scratch/face-detection-debug-20260923");
        let results: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("render-pair-results.json")).unwrap())
                .unwrap();
        (
            root.join("inputs/failing.jpg"),
            root.join("inputs/reference.jpg"),
            serde_json::from_value(results[0]["faces"][0].clone()).unwrap(),
            serde_json::from_value(results[1]["faces"][0].clone()).unwrap(),
        )
    }

    #[test]
    #[ignore = "requires the ignored 24 MP real-photo QA fixtures"]
    #[cfg(windows)]
    fn benchmark_serial_native_render_with_supersession_checkpoints() {
        let (target_path, reference_path, target, reference) = real_pair();
        let source_hashes = (
            source_digest(&target_path).unwrap(),
            source_digest(&reference_path).unwrap(),
        );
        let preview = |checkpoint: &dyn Fn(RenderStage) -> Result<(), String>| {
            render_preview(
                &target_path,
                &reference_path,
                &target,
                &reference,
                checkpoint,
            )
        };
        let measure = |label: &str, run: &dyn Fn() -> Vec<u8>| {
            let wall = std::time::Instant::now();
            let cpu = process_cpu_time();
            let preview = run();
            let elapsed = wall.elapsed();
            let cpu = process_cpu_time() - cpu;
            let preview_hash = Sha256::digest(preview);
            println!(
                "{label} wall_ms={} cpu_ms={} preview_sha256={:x}",
                elapsed.as_millis(),
                cpu.as_millis(),
                preview_hash
            );
            preview_hash.to_vec()
        };
        let single = measure("single_pair", &|| preview(&|_| Ok(())).unwrap());
        // This controls native processing work, not concurrent Tauri command or window latency.
        let uncancelled = measure("three_requests_uncancelled_serial", &|| {
            let mut result = None;
            for _ in 0..3 {
                result = Some(preview(&|_| Ok(())).unwrap());
            }
            result.unwrap()
        });
        let checkpointed = measure("three_requests_checkpointed_serial", &|| {
            let stages = std::sync::Mutex::new(Vec::new());
            let superseded_at_encode = |stage| {
                stages.lock().unwrap().push(stage);
                if stage == RenderStage::Encode {
                    Err("superseded".into())
                } else {
                    Ok(())
                }
            };
            assert_eq!(preview(&superseded_at_encode).unwrap_err(), "superseded");
            // The next queued request is invalidated before decoding.
            let superseded_at_decode = |stage| {
                stages.lock().unwrap().push(stage);
                Err("superseded".into())
            };
            assert_eq!(preview(&superseded_at_decode).unwrap_err(), "superseded");
            let current = preview(&|stage| {
                stages.lock().unwrap().push(stage);
                Ok(())
            })
            .unwrap();
            let stages = stages.into_inner().unwrap();
            let count = |wanted| stages.iter().filter(|&&stage| stage == wanted).count();
            println!(
                "burst_after decode={} composite={} encode_reached={}",
                count(RenderStage::Decode),
                count(RenderStage::Composite),
                count(RenderStage::Encode)
            );
            current
        });
        assert_eq!(single, uncancelled);
        assert_eq!(single, checkpointed);
        assert_eq!(source_digest(&target_path).unwrap(), source_hashes.0);
        assert_eq!(source_digest(&reference_path).unwrap(), source_hashes.1);
    }

    #[test]
    #[ignore = "requires the ignored 24 MP real-photo QA fixtures"]
    fn profiles_real_pair_in_desktop_crate() {
        let (target_path, reference_path, target, reference) = real_pair();
        let start = std::time::Instant::now();
        let target_digest = source_digest(&target_path).unwrap();
        let reference_digest = source_digest(&reference_path).unwrap();
        let preview = render_preview(&target_path, &reference_path, &target, &reference, |_| {
            Ok(())
        })
        .unwrap();
        assert_eq!(source_digest(&target_path).unwrap(), target_digest);
        assert_eq!(source_digest(&reference_path).unwrap(), reference_digest);
        let elapsed = start.elapsed().as_millis();
        println!(
            "desktop_prepare_with_digests_ms={elapsed} preview_bytes={}",
            preview.len()
        );
        if let Ok(budget) = std::env::var("EYE_PREPARE_BUDGET_MS") {
            let budget: u128 = budget.parse().unwrap();
            assert!(
                elapsed < budget,
                "24 MP desktop preparation exceeded local {budget} ms budget"
            );
        }
    }

    #[test]
    #[ignore = "requires the ignored real-photo QA fixtures"]
    fn renders_real_pair_at_original_resolution_and_exif_orientation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.scratch/eye-correction/qa");
        let faces: Vec<Vec<Face>> =
            serde_json::from_slice(&std::fs::read(root.join("faces.json")).unwrap()).unwrap();
        let reference = &faces[0][0];
        let target = &faces[1][0];
        for (input, expected) in [
            ("nikki-closed.jpg", (864, 864)),
            ("nikki-closed-exif6.jpg", (864, 864)),
            ("nikki-closed-4x.jpg", (3456, 3456)),
        ] {
            let reference_path = if input.ends_with("4x.jpg") {
                root.join("nikki-open-a-4x.png")
            } else {
                root.join("nikki-open-a.jpg")
            };
            match compose(
                &root.join(input),
                &reference_path,
                target,
                reference,
                |_| Ok(()),
            ) {
                Ok(corrected) => assert_eq!(corrected.dimensions(), expected),
                Err(error) => panic!("{input}: {error}"),
            }
        }
    }
}
