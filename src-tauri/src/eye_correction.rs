use std::{io::Cursor, path::{Path, PathBuf}};

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, Rgba, RgbaImage, imageops::FilterType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(crate) struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Face(pub Vec<Point>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedEyes {
    pub token: String,
    pub url: String,
}

#[derive(Clone, Copy)]
struct V2 { x: f32, y: f32 }

impl V2 {
    fn add(self, other: Self) -> Self { Self { x: self.x + other.x, y: self.y + other.y } }
    fn sub(self, other: Self) -> Self { Self { x: self.x - other.x, y: self.y - other.y } }
    fn mul(self, factor: f32) -> Self { Self { x: self.x * factor, y: self.y * factor } }
    fn length(self) -> f32 { self.x.hypot(self.y) }
}

struct Eye {
    center: V2,
    along: V2,
    width: f32,
    openness: f32,
}

fn point(face: &Face, index: usize, width: u32, height: u32) -> Result<V2, String> {
    let point = face.0.get(index).ok_or("Rosto incompleto. Escolha outra foto.")?;
    if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite()
        || !(-0.1..=1.1).contains(&point.x) || !(-0.1..=1.1).contains(&point.y)
    {
        return Err("Os pontos do rosto são inválidos. Escolha outra foto.".into());
    }
    Ok(V2 { x: point.x * width as f32, y: point.y * height as f32 })
}

fn eye(face: &Face, size: (u32, u32), corners: (usize, usize), lids: (usize, usize)) -> Result<Eye, String> {
    let a = point(face, corners.0, size.0, size.1)?;
    let b = point(face, corners.1, size.0, size.1)?;
    let top = point(face, lids.0, size.0, size.1)?;
    let bottom = point(face, lids.1, size.0, size.1)?;
    let vector = b.sub(a);
    let width = vector.length();
    if width < 8.0 { return Err("O rosto é pequeno demais para corrigir os olhos.".into()); }
    Ok(Eye { center: a.add(b).mul(0.5), along: vector.mul(1.0 / width), width, openness: bottom.sub(top).length() / width })
}

fn validate_pair(target: &[Eye; 2], reference: &[Eye; 2]) -> Result<(), String> {
    for (dst, src) in target.iter().zip(reference.iter()) {
        if src.openness < 0.12 {
            return Err("Os olhos da referência precisam estar abertos.".into());
        }
        if !(0.35..=3.0).contains(&(dst.width / src.width)) {
            return Err("Os rostos têm escalas muito diferentes para uma correção natural.".into());
        }
    }
    let target_ratio = target[0].width / target[1].width;
    let reference_ratio = reference[0].width / reference[1].width;
    if !(0.6..=1.65).contains(&(target_ratio / reference_ratio)) {
        return Err("A posição dos rostos é diferente demais. Escolha outra referência.".into());
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
    let pixels = [image.get_pixel(x0,y0), image.get_pixel(x1,y0), image.get_pixel(x0,y1), image.get_pixel(x1,y1)];
    std::array::from_fn(|channel| {
        let top = pixels[0][channel] as f32 * (1.0-fx) + pixels[1][channel] as f32 * fx;
        let bottom = pixels[2][channel] as f32 * (1.0-fx) + pixels[3][channel] as f32 * fx;
        top * (1.0-fy) + bottom * fy
    })
}

fn map_eye(dst: &Eye, src: &Eye, pos: V2) -> V2 {
    let diff = pos.sub(dst.center);
    let dst_perp = V2 { x: -dst.along.y, y: dst.along.x };
    let src_perp = V2 { x: -src.along.y, y: src.along.x };
    let scale = src.width / dst.width;
    src.center.add(src.along.mul((diff.x * dst.along.x + diff.y * dst.along.y) * scale))
        .add(src_perp.mul((diff.x * dst_perp.x + diff.y * dst_perp.y) * scale))
}

fn transplant_eye(target: &mut RgbaImage, reference: &RgbaImage, dst: &Eye, src: &Eye) -> Result<(), String> {
    let rx = dst.width * 0.72;
    let ry = dst.width * 0.37;
    let bound = dst.width * 0.85;
    let x0 = (dst.center.x - bound).floor().max(0.0) as u32;
    let y0 = (dst.center.y - bound).floor().max(0.0) as u32;
    let x1 = (dst.center.x + bound).ceil().min(target.width() as f32 - 1.0) as u32;
    let y1 = (dst.center.y + bound).ceil().min(target.height() as f32 - 1.0) as u32;
    let perpendicular = V2 { x: -dst.along.y, y: dst.along.x };
    let mut correction = [0.0_f32; 3];
    let mut samples = 0.0_f32;
    // Match skin immediately above and below the eyelid, away from the iris.
    for x in [-0.46, -0.23, 0.0, 0.23, 0.46] {
        for y in [-0.91, -0.78, 0.78, 0.91] {
            let position = dst.center.add(dst.along.mul(x * rx)).add(perpendicular.mul(y * ry));
            let source_position = map_eye(dst, src, position);
            if source_position.x < 0.0 || source_position.y < 0.0
                || source_position.x >= reference.width() as f32 || source_position.y >= reference.height() as f32 { continue; }
            let from = bilinear(reference, source_position);
            let to = bilinear(target, position);
            for channel in 0..3 { correction[channel] += to[channel] - from[channel]; }
            samples += 1.0;
        }
    }
    if samples < 8.0 { return Err("O olho está próximo demais da borda da foto.".into()); }
    for channel in &mut correction { *channel = (*channel / samples).clamp(-38.0, 38.0); }
    for y in y0..=y1 {
        for x in x0..=x1 {
            let position = V2 { x: x as f32 + 0.5, y: y as f32 + 0.5 };
            let diff = position.sub(dst.center);
            let u = (diff.x * dst.along.x + diff.y * dst.along.y) / rx;
            let v = (diff.x * perpendicular.x + diff.y * perpendicular.y) / ry;
            let radius = (u*u + v*v).sqrt();
            if radius >= 1.0 { continue; }
            let source_position = map_eye(dst, src, position);
            if source_position.x < 0.0 || source_position.y < 0.0
                || source_position.x >= reference.width() as f32 || source_position.y >= reference.height() as f32 { continue; }
            let source = bilinear(reference, source_position);
            let alpha = ((1.0 - radius) / 0.3).clamp(0.0, 1.0);
            let original = target.get_pixel(x,y);
            let blended: [u8; 3] = std::array::from_fn(|channel| {
                ((source[channel] + correction[channel]).clamp(0.0,255.0) * alpha
                    + original[channel] as f32 * (1.0-alpha)).round() as u8
            });
            target.put_pixel(x,y,Rgba([blended[0],blended[1],blended[2],original[3]]));
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
        || (profile.len() == 3_144 && Sha256::digest(profile)[..] == LEGACY_SRGB_SHA256) {
        Ok(())
    } else {
        Err("O perfil de cor da foto não é sRGB compatível com esta correção.".into())
    }
}

fn open_upright(path: &Path) -> Result<RgbaImage, String> {
    let reader = ImageReader::open(path).map_err(|_| "Não foi possível abrir um dos originais.")?
        .with_guessed_format().map_err(|_| "O formato do original é inválido.")?;
    let format = reader.format();
    let mut decoder = reader.into_decoder().map_err(|_| "Não foi possível decodificar um dos originais.")?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || width > 10_000 || height > 10_000
        || u64::from(width) * u64::from(height) > 36_000_000 {
        return Err("A foto excede o limite de 36 megapixels para esta correção.".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(10_000);
    limits.max_image_height = Some(10_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    decoder.set_limits(limits).map_err(|_| "A foto excede o limite de memória da correção.".to_string())?;
    if let Some(profile) = decoder.icc_profile().map_err(|_| "Não foi possível ler o perfil de cor do original.")? {
        validate_profile(&profile)?;
    }
    let orientation = decoder.orientation().map_err(|_| "Não foi possível ler a orientação do original.")?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| "Não foi possível decodificar um dos originais.")?;
    if matches!(format, Some(ImageFormat::Jpeg | ImageFormat::Tiff)) { image.apply_orientation(orientation); }
    Ok(image.to_rgba8())
}

pub(crate) fn render(target_path: &Path, reference_path: &Path, target_face: &Face, reference_face: &Face, output: &Path) -> Result<Vec<u8>, String> {
    let mut target = open_upright(target_path)?;
    let reference = open_upright(reference_path)?;
    let target_size = target.dimensions();
    let reference_size = reference.dimensions();
    let dst = [eye(target_face, target_size, (33,133), (159,145))?, eye(target_face, target_size, (362,263), (386,374))?];
    let src = [eye(reference_face, reference_size, (33,133), (159,145))?, eye(reference_face, reference_size, (362,263), (386,374))?];
    validate_pair(&dst, &src)?;
    for (dst, src) in dst.iter().zip(src.iter()) { transplant_eye(&mut target, &reference, dst, src)?; }
    let preview_scale = (1600.0 / target.width().max(target.height()) as f32).min(1.0);
    let preview_width = ((target.width() as f32 * preview_scale).round() as u32).max(1);
    let preview_height = ((target.height() as f32 * preview_scale).round() as u32).max(1);
    let preview = DynamicImage::ImageRgba8(image::imageops::resize(&target, preview_width, preview_height, FilterType::Lanczos3));
    let mut preview_bytes = Cursor::new(Vec::new());
    preview.write_to(&mut preview_bytes, ImageFormat::Png).map_err(|_| "Não foi possível preparar a prévia.")?;
    let temporary = output.with_extension("tmp");
    target.save_with_format(&temporary,ImageFormat::Png).map_err(|_| "Não foi possível gravar a imagem corrigida.")?;
    std::fs::rename(&temporary,output).map_err(|_| "Não foi possível concluir a imagem corrigida.")?;
    Ok(preview_bytes.into_inner())
}

pub(crate) fn corrected_path(project_folder: &Path, original: &Path) -> Result<PathBuf, String> {
    let folder = project_folder.join(".myalbuns-corrections");
    std::fs::create_dir_all(&folder).map_err(|_| "Não foi possível criar a pasta de correções do projeto.")?;
    let stem = original.file_stem().and_then(|name| name.to_str()).unwrap_or("foto");
    let stem: String = stem.chars().filter(|character| character.is_alphanumeric() || matches!(character, '-' | '_')).take(48).collect();
    Ok(folder.join(format!("{stem}-olhos-{}.png", uuid::Uuid::new_v4().simple())))
}

#[cfg(test)]
mod validation_tests {
    use super::*;

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
        assert!(validate_pair(&eyes([24.0, 24.0], [0.18, 0.19]), &eyes([24.0, 24.0], [0.18, 0.18])).is_ok());
    }

    #[test]
    fn accepts_reference_when_one_target_eye_is_already_open() {
        assert!(validate_pair(&eyes([24.0, 24.0], [0.04, 0.20]), &eyes([24.0, 24.0], [0.18, 0.18])).is_ok());
    }

    #[test]
    fn rejects_a_closed_reference_eye() {
        let result = validate_pair(&eyes([24.0, 24.0], [0.04, 0.04]), &eyes([24.0, 24.0], [0.11, 0.18]));
        assert_eq!(result.unwrap_err(), "Os olhos da referência precisam estar abertos.");
    }

    #[test]
    fn still_rejects_incompatible_scale_and_pose() {
        assert_eq!(
            validate_pair(&eyes([20.0, 20.0], [0.04, 0.04]), &eyes([80.0, 80.0], [0.18, 0.18])).unwrap_err(),
            "Os rostos têm escalas muito diferentes para uma correção natural."
        );
        assert_eq!(
            validate_pair(&eyes([20.0, 20.0], [0.04, 0.04]), &eyes([20.0, 40.0], [0.18, 0.18])).unwrap_err(),
            "A posição dos rostos é diferente demais. Escolha outra referência."
        );
    }
}





#[cfg(test)]
mod qa_tests {
    use super::*;

    #[test]
    #[ignore = "requires the ignored real-photo QA fixtures"]
    fn renders_real_pair_at_original_resolution_and_exif_orientation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.scratch/eye-correction/qa");
        let faces: Vec<Vec<Face>> = serde_json::from_slice(&std::fs::read(root.join("faces.json")).unwrap()).unwrap();
        let reference = &faces[0][0];
        let target = &faces[1][0];
        for (input, output, expected) in [
            ("nikki-closed.jpg", "nikki-corrected.png", (864,864)),
            ("nikki-closed-exif6.jpg", "nikki-corrected-exif6.png", (864,864)),
            ("nikki-closed-4x.jpg", "nikki-corrected-4x.png", (3456,3456)),
        ] {
            let reference_path = if input.ends_with("4x.jpg") { root.join("nikki-open-a-4x.png") } else { root.join("nikki-open-a.jpg") };
            let result = render(&root.join(input), &reference_path, target, reference, &root.join(output));
            match result { Ok(preview) => {
                assert!(!preview.is_empty());
                assert_eq!(image::image_dimensions(root.join(output)).unwrap(), expected);
            }, Err(error) => panic!("{input}: {error}") }
        }
    }
}
