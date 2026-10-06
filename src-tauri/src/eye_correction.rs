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
    pub(super) fn synthetic_pair(folder: &Path) -> CorrectionSource {
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

/// The branches that refuse a photo, a face or a replacement. The module
/// reports them as text, so each expected message comes from the simplest call
/// that reaches the same branch instead of from a copy of the wording.
#[cfg(test)]
mod refusal_tests {
    use super::*;
    use crate::ipc_contract::ViewerFacePoint;

    fn landmarks() -> Vec<ViewerFacePoint> {
        vec![
            ViewerFacePoint {
                x: 0.5,
                y: 0.5,
                z: 0.0
            };
            468
        ]
    }

    fn unusable_landmark() -> String {
        point(&Face(vec![]), 0, 100, 100)
            .err()
            .expect("a missing landmark is refused")
    }

    fn unsupported_profile() -> String {
        validate_profile(b"").expect_err("an empty profile is not sRGB")
    }

    fn png(folder: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let path = folder.join(name);
        RgbaImage::from_pixel(width, height, Rgba([190, 40, 20, 255]))
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();
        path
    }

    fn png_with_profile(folder: &Path, name: &str, profile: &[u8]) -> PathBuf {
        let path = folder.join(name);
        let mut encoder = image::codecs::png::PngEncoder::new(File::create(&path).unwrap());
        encoder.set_icc_profile(profile.to_vec()).unwrap();
        encoder
            .write_image(&[128; 24 * 16 * 4], 24, 16, ExtendedColorType::Rgba8)
            .unwrap();
        path
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    (crc >> 1) ^ 0xEDB8_8320
                };
            }
        }
        !crc
    }

    /// A PNG that declares its size and carries no pixels: enough for every
    /// check made before decoding, and unreadable after them.
    fn png_declaring(folder: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut header = Vec::new();
        header.extend(width.to_be_bytes());
        header.extend(height.to_be_bytes());
        header.extend([8, 0, 0, 0, 0]);
        for (kind, data) in [(b"IHDR", header.as_slice()), (b"IDAT", &[]), (b"IEND", &[])] {
            bytes.extend((data.len() as u32).to_be_bytes());
            let mut chunk = kind.to_vec();
            chunk.extend(data);
            bytes.extend(&chunk);
            bytes.extend(crc32(&chunk).to_be_bytes());
        }
        let path = folder.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn names(folder: &Path) -> Vec<String> {
        let mut names = std::fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    /// The test host supplies the legacy profile; the product recognizes it by
    /// digest and never reads it from the operating system.
    #[cfg(windows)]
    fn legacy_windows_profile() -> Vec<u8> {
        let system_root = std::env::var_os("SystemRoot").expect("Windows defines SystemRoot");
        std::fs::read(
            PathBuf::from(system_root)
                .join("System32/spool/drivers/color/sRGB Color Space Profile.icm"),
        )
        .expect("Windows supplies its standard sRGB profile")
    }

    #[test]
    fn landmarks_must_be_present_finite_and_close_to_the_photo() {
        let refused = unusable_landmark();
        let with = |change: fn(&mut ViewerFacePoint)| {
            let mut points = landmarks();
            change(&mut points[33]);
            point(&Face(points), 33, 200, 100)
        };
        for (name, change) in [
            (
                "x is not a number",
                (|point| point.x = f32::NAN) as fn(&mut ViewerFacePoint),
            ),
            ("y is not a number", |point| point.y = f32::NAN),
            ("z is not a number", |point| point.z = f32::NAN),
            ("x is infinite", |point| point.x = f32::INFINITY),
            ("y is infinite", |point| point.y = f32::NEG_INFINITY),
            ("x is left of the margin", |point| point.x = -0.11),
            ("x is right of the margin", |point| point.x = 1.11),
            ("y is above the margin", |point| point.y = -0.11),
            ("y is below the margin", |point| point.y = 1.11),
        ] {
            assert_eq!(with(change).err(), Some(refused.clone()), "{name}");
        }
        assert_eq!(
            point(&Face(landmarks()), 468, 200, 100).err(),
            Some(refused),
            "a landmark the detector did not send"
        );
        for (name, change, expected) in [
            (
                "inside the photo",
                (|point| (point.x, point.y) = (0.25, 0.75)) as fn(&mut ViewerFacePoint),
                (50.0, 75.0),
            ),
            (
                "on the near margin",
                |point| (point.x, point.y) = (-0.1, -0.1),
                (-20.0, -10.0),
            ),
            (
                "on the far margin",
                |point| (point.x, point.y) = (1.1, 1.1),
                (220.0, 110.0),
            ),
        ] {
            let scaled = with(change).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(
                (scaled.x - expected.0).abs() < 0.001 && (scaled.y - expected.1).abs() < 0.001,
                "{name}: ({}, {})",
                scaled.x,
                scaled.y
            );
        }
    }

    #[test]
    fn an_eye_narrower_than_eight_pixels_is_refused_as_a_face_too_small() {
        let eye_spanning = |fraction: f32| {
            let mut points = landmarks();
            points[33].x = 0.5;
            points[133].x = 0.5 + fraction;
            eye(&Face(points), (1_000, 1_000), (33, 133), (159, 145))
        };
        let too_small = eye_spanning(0.0079).err().expect("7.9 pixels are too few");
        assert_ne!(too_small, unusable_landmark());
        assert_eq!(eye_spanning(0.0).err(), Some(too_small));
        let accepted =
            eye_spanning(0.0081).unwrap_or_else(|error| panic!("8.1 pixels are enough: {error}"));
        assert!((accepted.width - 8.1).abs() < 0.01, "{}", accepted.width);
    }

    #[test]
    fn an_eye_whose_surroundings_leave_the_reference_is_refused_without_painting() {
        let eye_at = |x: f32, y: f32| Eye {
            center: V2 { x, y },
            along: V2 { x: 1.0, y: 0.0 },
            width: 40.0,
            openness: 0.2,
        };
        let reference = RgbaImage::from_fn(200, 200, |x, y| {
            Rgba([(x * 5 % 256) as u8, (y * 3 % 256) as u8, 60, 255])
        });
        let untouched = RgbaImage::from_pixel(200, 200, Rgba([80, 100, 120, 255]));
        let mut target = untouched.clone();

        // With the reference eye on the corner, most of the skin around it is
        // outside the reference photo.
        let near_border = transplant_eye(
            &mut target,
            &reference,
            &eye_at(100.0, 100.0),
            &eye_at(0.0, 0.0),
        )
        .expect_err("too little skin can be matched");
        assert_eq!(target, untouched, "a refused eye changes no pixel");

        transplant_eye(
            &mut target,
            &reference,
            &eye_at(100.0, 100.0),
            &eye_at(100.0, 100.0),
        )
        .expect("an eye inside the reference is transplanted");
        assert_ne!(target, untouched);
        assert_ne!(near_border, unusable_landmark());
    }

    #[test]
    fn only_the_known_srgb_profiles_are_accepted() {
        let refused = unsupported_profile();
        for profile in SRGB_PROFILES {
            validate_profile(profile).expect("a bundled sRGB profile is accepted");
            assert_eq!(
                validate_profile(&profile[1..]).err(),
                Some(refused.clone()),
                "a truncated profile is another profile"
            );
        }
        assert_eq!(
            validate_profile(&[0; 3_144]).err(),
            Some(refused),
            "the length of the legacy profile is not its identity"
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_legacy_windows_srgb_profile_is_accepted_only_by_its_exact_bytes() {
        let folder = tempfile::tempdir().unwrap();
        let legacy = legacy_windows_profile();
        validate_profile(&legacy).expect("the legacy sRGB profile is accepted by digest");
        open_upright(&png_with_profile(folder.path(), "legacy.png", &legacy))
            .expect("a photo carrying the legacy profile opens");

        let mut altered = legacy;
        altered[100] ^= 1;
        assert_eq!(
            validate_profile(&altered).err(),
            Some(unsupported_profile())
        );
    }

    #[test]
    fn a_photo_with_another_color_profile_is_neither_opened_nor_replaced() {
        let folder = tempfile::tempdir().unwrap();
        let original = png_with_profile(folder.path(), "wide-gamut.png", b"not an sRGB profile");
        let before = std::fs::read(&original).unwrap();

        assert_eq!(open_upright(&original).err(), Some(unsupported_profile()));
        assert_eq!(
            commit_replacement(
                &original,
                RgbaImage::from_pixel(24, 16, Rgba([20, 30, 40, 255])),
                source_digest(&original).unwrap(),
            )
            .err(),
            Some(unsupported_profile())
        );
        assert_eq!(std::fs::read(&original).unwrap(), before);
        assert_eq!(names(folder.path()), ["wide-gamut.png"]);
    }

    #[test]
    fn photos_over_ten_thousand_pixels_or_thirty_six_megapixels_are_refused_before_decoding() {
        let folder = tempfile::tempdir().unwrap();
        let too_large = open_upright(&png(folder.path(), "wide.png", 10_001, 1))
            .expect_err("10 001 pixels of width are over the limit");
        assert_ne!(too_large, unsupported_profile());
        assert_eq!(
            open_upright(&png(folder.path(), "tall.png", 1, 10_001)).err(),
            Some(too_large.clone())
        );
        assert_eq!(
            open_upright(&png(folder.path(), "widest.png", 10_000, 1))
                .expect("10 000 pixels of width are accepted")
                .dimensions(),
            (10_000, 1)
        );

        // 6001 x 6000 fits both edges and exceeds 36 MP. The file has no pixels,
        // so only a check made before decoding can name the size as the reason.
        assert_eq!(
            open_upright(&png_declaring(folder.path(), "area.png", 6_001, 6_000)).err(),
            Some(too_large.clone())
        );
        let unreadable = open_upright(&png_declaring(folder.path(), "limit.png", 6_000, 6_000))
            .expect_err("a file without pixels cannot be read");
        assert_ne!(
            unreadable, too_large,
            "exactly 36 MP passes the size check and fails only at decoding"
        );
    }

    #[test]
    fn the_original_must_be_a_supported_format_that_matches_its_extension() {
        let folder = tempfile::tempdir().unwrap();
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(24, 16, Rgba([1, 2, 3, 255])));
        let saved = |name: &str, format: ImageFormat| {
            let path = folder.path().join(name);
            image.save_with_format(&path, format).unwrap();
            path
        };
        for (name, format) in [
            ("photo.jpg", ImageFormat::Jpeg),
            ("PHOTO.JPEG", ImageFormat::Jpeg),
            ("photo.png", ImageFormat::Png),
            ("photo.tif", ImageFormat::Tiff),
            ("photo.TIFF", ImageFormat::Tiff),
        ] {
            assert_eq!(original_format(&saved(name, format)), Ok(format), "{name}");
        }

        // The extension alone decides; these bytes are a valid PNG.
        let unsupported = original_format(&saved("photo.bmp", ImageFormat::Png))
            .expect_err("only JPEG, PNG and TIFF originals are replaced");
        assert_eq!(
            original_format(&saved("photo", ImageFormat::Png)).err(),
            Some(unsupported.clone()),
            "a file without extension"
        );
        let mismatched = original_format(&saved("jpeg-inside.png", ImageFormat::Jpeg))
            .expect_err("the content is not what the extension says");
        assert_ne!(mismatched, unsupported);
        assert_eq!(
            original_format(&saved("png-inside.jpg", ImageFormat::Png)).err(),
            Some(mismatched.clone())
        );

        for (name, refusal) in [("photo.bmp", unsupported), ("jpeg-inside.png", mismatched)] {
            let original = folder.path().join(name);
            let before = std::fs::read(&original).unwrap();
            let entries = names(folder.path());
            assert_eq!(
                commit_replacement(
                    &original,
                    RgbaImage::from_pixel(24, 16, Rgba([20, 30, 40, 255])),
                    source_digest(&original).unwrap(),
                )
                .err(),
                Some(refusal),
                "{name}"
            );
            assert_eq!(std::fs::read(&original).unwrap(), before);
            assert_eq!(
                names(folder.path()),
                entries,
                "nothing is staged for {name}"
            );
        }
    }

    #[test]
    fn a_correction_of_another_size_is_refused_before_anything_is_staged() {
        let folder = tempfile::tempdir().unwrap();
        let original = png(folder.path(), "photo.png", 24, 16);
        let before = std::fs::read(&original).unwrap();
        let staging = folder.path().join("probe.stage");
        let rotated = RgbaImage::from_pixel(16, 24, Rgba([20, 30, 40, 255]));

        let wrong_size = encode_replacement(rotated.clone(), &original, &staging)
            .expect_err("a 16 x 24 correction does not fit a 24 x 16 photo");
        assert!(!staging.exists());
        assert_eq!(
            commit_replacement(&original, rotated, source_digest(&original).unwrap()).err(),
            Some(wrong_size)
        );
        assert_eq!(std::fs::read(&original).unwrap(), before);
        assert_eq!(names(folder.path()), ["photo.png"]);

        encode_replacement(
            RgbaImage::from_pixel(24, 16, Rgba([20, 30, 40, 255])),
            &original,
            &staging,
        )
        .expect("the same size is staged");
        assert!(staging.is_file());
    }

    #[test]
    fn a_changed_target_refuses_to_save_before_reading_it_and_keeps_the_newer_file() {
        let folder = tempfile::tempdir().unwrap();
        let source = super::validation_tests::synthetic_pair(folder.path());
        // Not even a photo any more: only the comparison made before composing
        // can still name the change as the reason.
        std::fs::write(&source.target, b"replaced by another program").unwrap();
        let newer = std::fs::read(&source.target).unwrap();
        let reference = std::fs::read(&source.reference).unwrap();

        assert_eq!(replace_original(&source).err(), Some(TARGET_CHANGED.into()));

        assert_eq!(std::fs::read(&source.target).unwrap(), newer);
        assert_eq!(std::fs::read(&source.reference).unwrap(), reference);
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 2);
    }

    #[cfg(windows)]
    #[test]
    fn a_failed_file_replacement_keeps_the_original_and_leaves_no_staging_or_backup() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

        let folder = tempfile::tempdir().unwrap();
        let original = png(folder.path(), "photo.png", 24, 16);
        let before = std::fs::read(&original).unwrap();
        let digest = source_digest(&original).unwrap();
        let corrected = RgbaImage::from_pixel(24, 16, Rgba([30, 100, 180, 255]));
        // Files of an earlier, interrupted correction belong to nobody now.
        let leftover = folder.path().join(".myalbuns-eye-earlier.stage");
        std::fs::write(&leftover, b"left by an earlier attempt").unwrap();

        // Another program keeps the photo open for reading only: it can still be
        // read and compared, and it cannot be replaced.
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&original)
            .expect("the photo is held open");
        let probe = folder.path().join("probe.stage");
        std::fs::write(&probe, b"probe").unwrap();
        let refused = replace_file(&original, &probe, &folder.path().join("probe.backup"))
            .expect_err("the held photo cannot be replaced");
        std::fs::remove_file(&probe).unwrap();

        assert_eq!(
            commit_replacement(&original, corrected.clone(), digest).err(),
            Some(refused),
            "the correction was staged and only its replacement failed"
        );
        assert_eq!(std::fs::read(&original).unwrap(), before);
        assert_eq!(
            names(folder.path()),
            [".myalbuns-eye-earlier.stage", "photo.png"],
            "neither the staged file nor a backup stays behind"
        );

        drop(held);
        let backup = commit_replacement(&original, corrected, digest)
            .expect("the replacement succeeds once the photo is released");
        assert_ne!(std::fs::read(&original).unwrap(), before);
        assert_eq!(std::fs::read(&backup).unwrap(), before);
        assert!(leftover.is_file());
        assert_eq!(names(folder.path()).len(), 3);
    }
}
