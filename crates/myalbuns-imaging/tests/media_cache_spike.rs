use std::{
    io::Cursor,
    time::{Duration, Instant},
};

use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, ImageFormat,
    ImageReader, Limits, Rgb, RgbImage, Rgba, RgbaImage,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::{FilterType, crop_imm, resize},
};
use serde_json::json;
use sha2::{Digest, Sha256};

mod png_exif_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/png_exif.rs"
    ));
}
use tiff::{
    decoder::Decoder as TiffDecoder,
    encoder::{TiffEncoder, colortype},
};

const MAX_EDGE_PX: u32 = 1_600;
const MAX_DECODED_PIXELS: u64 = 134_217_728;
const MAX_DECODER_ALLOC_BYTES: u64 = 512 * 1024 * 1024;
const OPAQUE_JPEG_QUALITY: u8 = 84;
const JPEG_QUALITY_CANDIDATES: [u8; 6] = [72, 76, 80, 84, 88, 92];
const SRGB_PROFILE: &[u8] = include_bytes!("../assets/sRGB2014.icc");
const PHOTOGRAPHIC_REFERENCE: &[u8] =
    include_bytes!("../../../docs/assets/referencia-layout-editor.png");

struct PhotographicFixture {
    name: &'static str,
    crop: [u32; 4],
    image: RgbImage,
}

struct JpegQualityMeasurement {
    quality: u8,
    total_bytes: usize,
    mean_bytes: usize,
    psnr_db: f64,
    mean_absolute_error: f64,
}

#[test]
#[ignore = "reproducible release-mode measurement for issue #44"]
fn measured_media_cache_policy() {
    let total_started = Instant::now();

    let jpeg_fixture_started = Instant::now();
    let jpeg_source = RgbImage::from_fn(6_000, 4_000, |x, y| {
        let mixed = x
            .wrapping_mul(73_856_093)
            .wrapping_add(y.wrapping_mul(19_349_663));
        Rgb([
            mixed as u8,
            mixed.rotate_left(11) as u8,
            mixed.rotate_left(23) as u8,
        ])
    });
    let mut jpeg_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_bytes, 92)
        .encode_image(&jpeg_source)
        .expect("the representative JPEG fixture encodes");
    let jpeg_fixture_ms = elapsed_ms(jpeg_fixture_started.elapsed());

    let jpeg_hash_started = Instant::now();
    let jpeg_fingerprint = format!("{:x}", Sha256::digest(&jpeg_bytes));
    let jpeg_hash_ms = elapsed_ms(jpeg_hash_started.elapsed());
    assert_eq!(jpeg_fingerprint.len(), 64);
    assert_eq!(
        jpeg_fingerprint,
        format!("{:x}", Sha256::digest(&jpeg_bytes))
    );

    let jpeg_decode_started = Instant::now();
    let jpeg_decoded = decode_with_policy(&jpeg_bytes, ImageFormat::Jpeg);
    let jpeg_decode_ms = elapsed_ms(jpeg_decode_started.elapsed());
    assert_eq!(jpeg_decoded.width(), 6_000);
    assert_eq!(jpeg_decoded.height(), 4_000);

    let jpeg_reduce_started = Instant::now();
    let jpeg_reduced = jpeg_decoded.thumbnail(MAX_EDGE_PX, MAX_EDGE_PX).to_rgb8();
    let jpeg_reduce_ms = elapsed_ms(jpeg_reduce_started.elapsed());
    assert_eq!(jpeg_reduced.dimensions(), (1_600, 1_067));
    let jpeg_encode_started = Instant::now();
    let mut reduced_jpeg_bytes = Vec::new();
    let mut reduced_jpeg_encoder =
        JpegEncoder::new_with_quality(&mut reduced_jpeg_bytes, OPAQUE_JPEG_QUALITY);
    reduced_jpeg_encoder
        .set_icc_profile(SRGB_PROFILE.to_vec())
        .expect("the canonical sRGB profile fits the JPEG artifact");
    reduced_jpeg_encoder
        .encode_image(&jpeg_reduced)
        .expect("the opaque reduced representation encodes");
    let jpeg_encode_ms = elapsed_ms(jpeg_encode_started.elapsed());
    assert_srgb_profile(&reduced_jpeg_bytes, ImageFormat::Jpeg);

    let quality_sweep_started = Instant::now();
    let photographic_corpus = photographic_quality_corpus();
    let quality_sweep = measure_jpeg_quality_sweep(&photographic_corpus);
    let selected_quality = rate_distortion_knee(&quality_sweep);
    assert_eq!(
        selected_quality, OPAQUE_JPEG_QUALITY,
        "the measured rate-distortion knee must remain the published opaque-artifact quality"
    );
    let quality_sweep_ms = elapsed_ms(quality_sweep_started.elapsed());
    let photographic_corpus_evidence = photographic_corpus
        .iter()
        .map(|fixture| {
            json!({
                "name": fixture.name,
                "sourceAsset": "docs/assets/referencia-layout-editor.png",
                "crop": fixture.crop,
                "normalizedDimensions": [fixture.image.width(), fixture.image.height()],
                "normalizedRgbSha256": format!("{:x}", Sha256::digest(fixture.image.as_raw())),
            })
        })
        .collect::<Vec<_>>();
    let quality_sweep_evidence = quality_sweep
        .iter()
        .map(|measurement| {
            json!({
                "quality": measurement.quality,
                "totalBytes": measurement.total_bytes,
                "meanBytes": measurement.mean_bytes,
                "psnrDb": measurement.psnr_db,
                "meanAbsoluteError": measurement.mean_absolute_error,
            })
        })
        .collect::<Vec<_>>();

    let png_fixture_started = Instant::now();
    let png_source = RgbaImage::from_fn(2_400, 1_800, |x, y| {
        let alpha = if (x / 120 + y / 90) % 3 == 0 { 96 } else { 255 };
        Rgba([
            (x % 251) as u8,
            (y % 241) as u8,
            ((x + y) % 239) as u8,
            alpha,
        ])
    });
    let mut png_bytes = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(
            png_source.as_raw(),
            png_source.width(),
            png_source.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("the representative PNG fixture encodes");
    let png_fixture_ms = elapsed_ms(png_fixture_started.elapsed());

    let png_decode_started = Instant::now();
    let png_decoded = decode_with_policy(&png_bytes, ImageFormat::Png);
    let png_decode_ms = elapsed_ms(png_decode_started.elapsed());
    let png_reduced = png_decoded.thumbnail(MAX_EDGE_PX, MAX_EDGE_PX).to_rgba8();
    assert_eq!(png_reduced.dimensions(), (1_600, 1_200));
    assert!(png_reduced.pixels().any(|pixel| pixel[3] != u8::MAX));
    let png_encode_started = Instant::now();
    let mut reduced_png_bytes = Vec::new();
    let mut reduced_png_encoder = PngEncoder::new(&mut reduced_png_bytes);
    reduced_png_encoder
        .set_icc_profile(SRGB_PROFILE.to_vec())
        .expect("the canonical sRGB profile fits the PNG artifact");
    reduced_png_encoder
        .write_image(
            png_reduced.as_raw(),
            png_reduced.width(),
            png_reduced.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("the alpha reduced representation encodes");
    let png_encode_ms = elapsed_ms(png_encode_started.elapsed());
    assert_srgb_profile(&reduced_png_bytes, ImageFormat::Png);

    let tiff_fixture_started = Instant::now();
    let tiff_pixels = RgbImage::from_fn(4_096, 3_072, |x, y| {
        Rgb([(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 239) as u8])
    });
    let tiff_bytes = encode_tiff_pages(&[(
        tiff_pixels.width(),
        tiff_pixels.height(),
        tiff_pixels.as_raw(),
    )]);
    let tiff_fixture_ms = elapsed_ms(tiff_fixture_started.elapsed());
    assert_eq!(tiff_page_count(&tiff_bytes), 1);
    let tiff_decode_started = Instant::now();
    let tiff_decoded = decode_with_policy(&tiff_bytes, ImageFormat::Tiff);
    let tiff_decode_ms = elapsed_ms(tiff_decode_started.elapsed());
    assert_eq!(tiff_decoded.dimensions(), (4_096, 3_072));

    let second_page = vec![127_u8; 8 * 8 * 3];
    let multipage_tiff = encode_tiff_pages(&[
        (8, 8, second_page.as_slice()),
        (8, 8, second_page.as_slice()),
    ]);
    assert_eq!(tiff_page_count(&multipage_tiff), 2);

    let oriented_jpeg = oriented_jpeg_fixture();
    let mut reader = ImageReader::with_format(Cursor::new(&oriented_jpeg), ImageFormat::Jpeg);
    reader.limits(decode_limits());
    let mut decoder = reader
        .into_decoder()
        .expect("the EXIF JPEG decoder is prepared");
    let orientation = decoder
        .orientation()
        .expect("the EXIF orientation is readable");
    assert_eq!(orientation.to_exif(), 6);
    let mut oriented =
        DynamicImage::from_decoder(decoder).expect("the EXIF JPEG decodes under the policy");
    oriented.apply_orientation(orientation);
    assert_eq!(oriented.dimensions(), (4, 8));

    let oriented_png = png_exif_fixture::orientation_6_rgb_2x1();
    let mut png_reader = ImageReader::with_format(Cursor::new(&oriented_png), ImageFormat::Png);
    png_reader.limits(decode_limits());
    let mut png_decoder = png_reader
        .into_decoder()
        .expect("the PNG eXIf decoder is prepared");
    let png_orientation = png_decoder
        .orientation()
        .expect("the PNG eXIf metadata is readable");
    assert_eq!(png_orientation.to_exif(), 6);
    let png_metadata_only =
        DynamicImage::from_decoder(png_decoder).expect("the PNG eXIf fixture decodes");
    assert_eq!(
        png_metadata_only.dimensions(),
        (2, 1),
        "PNG eXIf is metadata-only and never rotates Cache pixels implicitly"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "policy": {
                "acceptedSources": ["jpeg", "png", "single-page-tiff"],
                "rejectedSources": ["multi-page-tiff"],
                "fingerprint": "sha256-full-file-v1",
                "maxDecodedPixels": MAX_DECODED_PIXELS,
                "maxDecoderAllocationBytes": MAX_DECODER_ALLOC_BYTES,
                "maxEdgePx": MAX_EDGE_PX,
                "opaqueArtifact": format!("jpeg-quality-{selected_quality}-with-srgb-icc"),
                "alphaArtifact": "png-rgba-with-srgb-icc",
                "orientation": {
                    "jpegExif": "apply-once",
                    "tiff": "apply-once",
                    "pngExif": "metadata-only-no-implicit-rotation"
                },
                "tiles": false
            },
            "corpus": {
                "jpeg": {
                    "sourceDimensions": [6_000, 4_000],
                    "sourceBytes": jpeg_bytes.len(),
                    "fingerprint": jpeg_fingerprint,
                    "reducedDimensions": [jpeg_reduced.width(), jpeg_reduced.height()],
                    "reducedBytes": reduced_jpeg_bytes.len()
                },
                "pngAlpha": {
                    "sourceDimensions": [2_400, 1_800],
                    "sourceBytes": png_bytes.len(),
                    "reducedDimensions": [png_reduced.width(), png_reduced.height()],
                    "reducedBytes": reduced_png_bytes.len()
                },
                "tiff": {
                    "sourceDimensions": [4_096, 3_072],
                    "sourceBytes": tiff_bytes.len(),
                    "pageCount": 1
                },
                "multiPageTiff": { "pageCount": 2 },
                "orientation": {
                    "jpegExif": 6,
                    "jpegDecodedDimensions": [oriented.width(), oriented.height()],
                    "pngExif": png_orientation.to_exif(),
                    "pngDecodedDimensions": [png_metadata_only.width(), png_metadata_only.height()]
                }
            },
            "jpegQualitySweep": {
                "method": "aggregate-psnr-mae-and-normalized-rate-distortion-knee-v1",
                "sourceAssetSha256": format!("{:x}", Sha256::digest(PHOTOGRAPHIC_REFERENCE)),
                "corpus": photographic_corpus_evidence,
                "candidates": quality_sweep_evidence,
                "selectedQuality": selected_quality
            },
            "elapsedMs": {
                "jpegFixture": jpeg_fixture_ms,
                "jpegFullSha256": jpeg_hash_ms,
                "jpegDecode": jpeg_decode_ms,
                "jpegReduce": jpeg_reduce_ms,
                "jpegArtifactEncode": jpeg_encode_ms,
                "jpegQualitySweep": quality_sweep_ms,
                "pngFixture": png_fixture_ms,
                "pngDecode": png_decode_ms,
                "pngArtifactEncode": png_encode_ms,
                "tiffFixture": tiff_fixture_ms,
                "tiffDecode": tiff_decode_ms,
                "total": elapsed_ms(total_started.elapsed())
            }
        }))
        .expect("the spike evidence serializes")
    );
}

fn photographic_quality_corpus() -> Vec<PhotographicFixture> {
    let reference = decode_with_policy(PHOTOGRAPHIC_REFERENCE, ImageFormat::Png).to_rgb8();
    let crops = [
        ("grupo-familiar", [0, 140, 325, 465]),
        ("retrato-triplo", [390, 140, 350, 465]),
        ("retrato-individual", [770, 140, 350, 465]),
    ];

    crops
        .into_iter()
        .map(|(name, crop)| {
            let cropped = crop_imm(&reference, crop[0], crop[1], crop[2], crop[3]).to_image();
            let longest_edge = crop[2].max(crop[3]);
            let width = crop[2] * MAX_EDGE_PX / longest_edge;
            let height = crop[3] * MAX_EDGE_PX / longest_edge;
            PhotographicFixture {
                name,
                crop,
                image: resize(&cropped, width, height, FilterType::Lanczos3),
            }
        })
        .collect()
}

fn measure_jpeg_quality_sweep(corpus: &[PhotographicFixture]) -> Vec<JpegQualityMeasurement> {
    JPEG_QUALITY_CANDIDATES
        .into_iter()
        .map(|quality| {
            let mut total_bytes = 0_usize;
            let mut total_squared_error = 0_u128;
            let mut total_absolute_error = 0_u128;
            let mut channel_count = 0_u64;

            for fixture in corpus {
                let encoded = encode_jpeg_with_srgb(&fixture.image, quality);
                let decoded = decode_with_policy(&encoded, ImageFormat::Jpeg).to_rgb8();
                assert_eq!(decoded.dimensions(), fixture.image.dimensions());
                let (squared_error, absolute_error, channels) =
                    error_totals(&fixture.image, &decoded);
                total_bytes += encoded.len();
                total_squared_error += squared_error;
                total_absolute_error += absolute_error;
                channel_count += channels;
            }

            let mean_squared_error = total_squared_error as f64 / channel_count as f64;
            let psnr_db = 10.0 * (65_025.0 / mean_squared_error).log10();
            JpegQualityMeasurement {
                quality,
                total_bytes,
                mean_bytes: total_bytes / corpus.len(),
                psnr_db: round_metric(psnr_db),
                mean_absolute_error: round_metric(
                    total_absolute_error as f64 / channel_count as f64,
                ),
            }
        })
        .collect()
}

fn encode_jpeg_with_srgb(image: &RgbImage, quality: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .set_icc_profile(SRGB_PROFILE.to_vec())
        .expect("the canonical sRGB profile fits the quality-sweep artifact");
    encoder
        .encode_image(image)
        .expect("the quality-sweep artifact encodes");
    assert_srgb_profile(&bytes, ImageFormat::Jpeg);
    bytes
}

fn error_totals(reference: &RgbImage, candidate: &RgbImage) -> (u128, u128, u64) {
    let mut squared_error = 0_u128;
    let mut absolute_error = 0_u128;
    for (reference_channel, candidate_channel) in reference.as_raw().iter().zip(candidate.as_raw())
    {
        let difference = i16::from(*reference_channel) - i16::from(*candidate_channel);
        let absolute = difference.unsigned_abs();
        absolute_error += u128::from(absolute);
        squared_error += u128::from(absolute) * u128::from(absolute);
    }
    (
        squared_error,
        absolute_error,
        reference.as_raw().len() as u64,
    )
}

fn rate_distortion_knee(measurements: &[JpegQualityMeasurement]) -> u8 {
    let first = measurements
        .first()
        .expect("the quality sweep has a lower endpoint");
    let last = measurements
        .last()
        .expect("the quality sweep has an upper endpoint");
    let byte_range = (last.total_bytes - first.total_bytes) as f64;
    let fidelity_range = last.psnr_db - first.psnr_db;

    measurements
        .iter()
        .max_by(|left, right| {
            let score = |measurement: &JpegQualityMeasurement| {
                let normalized_bytes =
                    (measurement.total_bytes - first.total_bytes) as f64 / byte_range;
                let normalized_fidelity = (measurement.psnr_db - first.psnr_db) / fidelity_range;
                normalized_fidelity - normalized_bytes
            };
            score(left).total_cmp(&score(right))
        })
        .expect("the quality sweep selects a rate-distortion knee")
        .quality
}

fn round_metric(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn decode_with_policy(bytes: &[u8], format: ImageFormat) -> DynamicImage {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(decode_limits());
    let dimensions = reader
        .into_dimensions()
        .expect("the representative source dimensions are readable");
    assert!(
        u64::from(dimensions.0) * u64::from(dimensions.1) <= MAX_DECODED_PIXELS,
        "the representative source must remain inside the shared decode budget"
    );
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(decode_limits());
    reader
        .decode()
        .expect("the representative source decodes under the policy")
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_DECODER_ALLOC_BYTES);
    limits
}

fn assert_srgb_profile(bytes: &[u8], format: ImageFormat) {
    let mut decoder = ImageReader::with_format(Cursor::new(bytes), format)
        .into_decoder()
        .expect("the reduced representation decoder is prepared");
    let profile = decoder
        .icc_profile()
        .expect("the reduced representation ICC metadata is readable")
        .expect("the reduced representation carries an explicit sRGB profile");
    assert_eq!(profile, SRGB_PROFILE);
}

fn encode_tiff_pages(pages: &[(u32, u32, &[u8])]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut output).expect("the TIFF encoder is prepared");
        for (width, height, pixels) in pages {
            encoder
                .write_image::<colortype::RGB8>(*width, *height, pixels)
                .expect("the TIFF page encodes");
        }
    }
    output.into_inner()
}

fn tiff_page_count(bytes: &[u8]) -> usize {
    let mut decoder = TiffDecoder::new(Cursor::new(bytes)).expect("the TIFF directory is readable");
    let mut count = 1;
    while decoder.more_images() {
        decoder
            .next_image()
            .expect("the next TIFF page is readable");
        count += 1;
    }
    count
}

fn oriented_jpeg_fixture() -> Vec<u8> {
    let mut source = RgbImage::new(8, 4);
    for (x, _, pixel) in source.enumerate_pixels_mut() {
        *pixel = if x < 4 {
            Rgb([240, 10, 10])
        } else {
            Rgb([10, 10, 240])
        };
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 100)
        .encode_image(&source)
        .expect("the oriented JPEG fixture encodes");
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&6_u16.to_le_bytes());
    exif.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    segment.extend_from_slice(&exif);
    jpeg.splice(2..2, segment);
    jpeg
}

fn elapsed_ms(duration: Duration) -> u128 {
    duration.as_millis()
}
