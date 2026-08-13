#[cfg(debug_assertions)]
use std::fs;
#[cfg(not(test))]
use std::fs::File;
use std::{
    ffi::OsString,
    io::{BufReader, Cursor, Read, Write},
    process::ExitCode,
};

use image::RgbaImage;
#[cfg(not(test))]
use image::metadata::Orientation;
use myalbuns_imaging_protocol::ImagingFailureCode;

#[cfg(not(test))]
use super::{JpegPreflight, read_segment};
use super::{SourceFailure, decode_render_jpeg_in_process, preflight_jpeg, rewind};

const MAX_WORKING_BYTES: u64 = 512 * 1024 * 1024;
const AUXILIARY_I16S_PER_STRIDE: u64 = 320;
pub(crate) const JPEG_WORKER_MODE: &str = "--decode-progressive-jpeg";
const JPEG_WORKER_MAGIC: [u8; 4] = *b"MAJ1";
const JPEG_WORKER_HEADER_BYTES: usize = 21;
const JPEG_WORKER_COMPLETED: u8 = 0;
const JPEG_WORKER_RESOURCE_LIMIT: u8 = 1;
const JPEG_WORKER_DECODE_FAILED: u8 = 2;

pub(super) fn validate_working_budget(
    width: u32,
    height: u32,
    component_count: u8,
    sof_payload: &[u8],
    compressed_bytes: u64,
) -> Result<(), SourceFailure> {
    let working_bytes = working_bytes(
        width,
        height,
        component_count,
        sof_payload,
        compressed_bytes,
    )?;
    if working_bytes > MAX_WORKING_BYTES {
        return Err(SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            format!(
                "o JPEG progressivo exigiria {working_bytes} bytes de trabalho e excede o limite de {MAX_WORKING_BYTES}"
            ),
        ));
    }
    Ok(())
}

fn working_bytes(
    width: u32,
    height: u32,
    component_count: u8,
    sof_payload: &[u8],
    compressed_bytes: u64,
) -> Result<u64, SourceFailure> {
    let mut factors = [(0_u64, 0_u64); 4];
    let mut horizontal_max = 0_u64;
    let mut vertical_max = 0_u64;
    for (index, factor) in factors
        .iter_mut()
        .take(usize::from(component_count))
        .enumerate()
    {
        let sampling = sof_payload[7 + index * 3];
        let horizontal = u64::from(sampling >> 4);
        let vertical = u64::from(sampling & 0x0f);
        if horizontal == 0 || vertical == 0 || horizontal > 4 || vertical > 4 {
            return Err(SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "os fatores de amostragem JPEG são inválidos",
            ));
        }
        *factor = (horizontal, vertical);
        horizontal_max = horizontal_max.max(horizontal);
        vertical_max = vertical_max.max(vertical);
    }

    let mcu_width = u64::from(width)
        .checked_add(horizontal_max * 8 - 1)
        .and_then(|value| value.checked_div(horizontal_max * 8))
        .ok_or_else(working_range_failure)?;
    let mcu_height = u64::from(height)
        .checked_add(vertical_max * 8 - 1)
        .and_then(|value| value.checked_div(vertical_max * 8))
        .ok_or_else(working_range_failure)?;
    let mcu_count = mcu_width
        .checked_mul(mcu_height)
        .ok_or_else(working_range_failure)?;
    let coefficient_elements = factors
        .iter()
        .take(usize::from(component_count))
        .try_fold(0_u64, |total, (horizontal, vertical)| {
            horizontal
                .checked_mul(*vertical)
                .and_then(|sampling| sampling.checked_mul(64))
                .and_then(|per_mcu| per_mcu.checked_mul(mcu_count))
                .and_then(|component| total.checked_add(component))
        })
        .ok_or_else(working_range_failure)?;
    let coefficient_bytes = coefficient_elements
        .checked_mul(2)
        .ok_or_else(working_range_failure)?;

    // Locked implementation contract: image 0.25.10 uses zune-jpeg 0.5.15.
    // That decoder allocates progressive coefficient planes plus raw, row,
    // upsampling and scratch buffers with infallible vec!/resize operations.
    // Sampling factors are capped at four, so 320 i16 values per padded stride
    // conservatively covers the 0.5.15 auxiliary row allocations, including
    // the generic 4x4 upsampler and its doubled compatibility destination.
    let auxiliary_elements = factors
        .iter()
        .take(usize::from(component_count))
        .try_fold(0_u64, |total, (horizontal, vertical)| {
            mcu_width
                .checked_mul(*horizontal)
                .and_then(|stride_blocks| stride_blocks.checked_mul(8))
                .and_then(|stride| stride.checked_mul(*vertical))
                .and_then(|rows| rows.checked_mul(AUXILIARY_I16S_PER_STRIDE))
                .and_then(|component| total.checked_add(component))
        })
        .ok_or_else(working_range_failure)?;
    let auxiliary_bytes = auxiliary_elements
        .checked_mul(2)
        .ok_or_else(working_range_failure)?;
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(working_range_failure)?;
    let decoded_components = if component_count == 1 { 1 } else { 3 };
    let raw_bytes = pixels
        .checked_mul(decoded_components)
        .ok_or_else(working_range_failure)?;
    let normalized_bytes = pixels.checked_mul(4).ok_or_else(working_range_failure)?;
    let decoder_peak = compressed_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(raw_bytes))
        .and_then(|value| value.checked_add(coefficient_bytes))
        .and_then(|value| value.checked_add(auxiliary_bytes))
        .ok_or_else(working_range_failure)?;
    let normalization_peak = raw_bytes
        .checked_add(normalized_bytes)
        .ok_or_else(working_range_failure)?;
    let transport_peak = normalized_bytes
        .checked_mul(2)
        .ok_or_else(working_range_failure)?;
    let input_transfer_peak = compressed_bytes
        .checked_mul(2)
        .ok_or_else(working_range_failure)?;
    Ok(decoder_peak
        .max(normalization_peak)
        .max(transport_peak)
        .max(input_transfer_peak))
}

fn working_range_failure() -> SourceFailure {
    SourceFailure::new(
        ImagingFailureCode::ResourceLimitExceeded,
        "as estruturas internas do decoder JPEG excederam o intervalo seguro",
    )
}

#[cfg(not(test))]
pub(super) fn decode_in_worker(
    mut reader: BufReader<File>,
    preflight: JpegPreflight,
) -> Result<RgbaImage, SourceFailure> {
    let compressed = read_segment(&mut reader, preflight.compressed_bytes)?;
    let executable = std::env::current_exe().map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível localizar o worker JPEG: {error}"),
        )
    })?;
    let mut child = std::process::Command::new(executable)
        .arg(JPEG_WORKER_MODE)
        .arg(preflight.compressed_bytes.to_string())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                format!("não foi possível iniciar o worker JPEG: {error}"),
            )
        })?;
    let mut stdin = child
        .stdin
        .take()
        .expect("the JPEG worker stdin was configured as piped");
    let write_result = stdin.write_all(&compressed);
    drop(stdin);
    drop(compressed);

    let mut stdout = child
        .stdout
        .take()
        .expect("the JPEG worker stdout was configured as piped");
    let mut header = [0_u8; JPEG_WORKER_HEADER_BYTES];
    let header_result = stdout.read_exact(&mut header);
    if write_result.is_err() || header_result.is_err() {
        return Err(worker_transport_failure(child));
    }
    if header[..4] != JPEG_WORKER_MAGIC {
        let _ = child.kill();
        let _ = child.wait();
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o worker JPEG respondeu com um cabeçalho inválido",
        ));
    }
    match header[4] {
        JPEG_WORKER_RESOURCE_LIMIT => {
            let _ = child.wait();
            return Err(SourceFailure::new(
                ImagingFailureCode::ResourceLimitExceeded,
                "o worker JPEG não conseguiu reservar memória para a fonte progressiva",
            ));
        }
        JPEG_WORKER_DECODE_FAILED => {
            let _ = child.wait();
            return Err(SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o worker JPEG recusou a fonte progressiva",
            ));
        }
        JPEG_WORKER_COMPLETED => {}
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                "o worker JPEG respondeu com um estado inválido",
            ));
        }
    }

    let width = u32::from_be_bytes(header[5..9].try_into().expect("four bytes"));
    let height = u32::from_be_bytes(header[9..13].try_into().expect("four bytes"));
    let raw_length = u64::from_be_bytes(header[13..21].try_into().expect("eight bytes"));
    let expected_dimensions = oriented_dimensions(&preflight);
    let expected_length = u64::from(expected_dimensions.0)
        .checked_mul(u64::from(expected_dimensions.1))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(working_range_failure)?;
    if (width, height) != expected_dimensions || raw_length != expected_length {
        let _ = child.kill();
        let _ = child.wait();
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o worker JPEG produziu dimensões inesperadas",
        ));
    }
    let raw_length = match usize::try_from(raw_length) {
        Ok(raw_length) => raw_length,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(working_range_failure());
        }
    };
    let mut raw = Vec::new();
    if raw.try_reserve_exact(raw_length).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para receber o raster JPEG progressivo",
        ));
    }
    raw.resize(raw_length, 0);
    if stdout.read_exact(&mut raw).is_err() {
        return Err(worker_transport_failure(child));
    }
    let mut trailing = [0_u8; 1];
    if stdout.read(&mut trailing).unwrap_or(1) != 0 {
        let _ = child.kill();
        let _ = child.wait();
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o worker JPEG produziu bytes além do raster esperado",
        ));
    }
    let status = child.wait().map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível aguardar o worker JPEG: {error}"),
        )
    })?;
    if !status.success() {
        return Err(SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "o worker JPEG terminou durante a decodificação progressiva",
        ));
    }
    RgbaImage::from_raw(width, height, raw).ok_or_else(|| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o raster do worker JPEG não corresponde às dimensões declaradas",
        )
    })
}

#[cfg(not(test))]
fn worker_transport_failure(mut child: std::process::Child) -> SourceFailure {
    let status = child.wait().ok();
    let code = if status.is_some_and(|status| status.success()) {
        ImagingFailureCode::DecodeFailed
    } else {
        ImagingFailureCode::ResourceLimitExceeded
    };
    SourceFailure::new(code, "o worker JPEG não concluiu sua resposta")
}

#[cfg(not(test))]
fn oriented_dimensions(preflight: &JpegPreflight) -> (u32, u32) {
    if matches!(
        preflight.orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    ) {
        (preflight.height, preflight.width)
    } else {
        (preflight.width, preflight.height)
    }
}

pub(crate) fn run_jpeg_worker(total_bytes: Option<OsString>) -> ExitCode {
    let Some(total_bytes) = total_bytes
        .and_then(|value| value.into_string().ok())
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return ExitCode::FAILURE;
    };
    let outcome = std::panic::catch_unwind(|| decode_worker_jpeg(total_bytes));
    let response = match outcome {
        Ok(Ok(image)) => write_worker_image(image),
        Ok(Err(failure)) => write_worker_header(worker_failure_status(Some(failure.code)), 0, 0, 0),
        Err(_) => write_worker_header(worker_failure_status(None), 0, 0, 0),
    };
    if response.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn worker_failure_status(code: Option<ImagingFailureCode>) -> u8 {
    match code {
        Some(ImagingFailureCode::ResourceLimitExceeded) | None => JPEG_WORKER_RESOURCE_LIMIT,
        Some(_) => JPEG_WORKER_DECODE_FAILED,
    }
}

fn decode_worker_jpeg(total_bytes: usize) -> Result<RgbaImage, SourceFailure> {
    let mut compressed = Vec::new();
    compressed.try_reserve_exact(total_bytes).map_err(|_| {
        SourceFailure::new(
            ImagingFailureCode::ResourceLimitExceeded,
            "não há memória suficiente para receber a fonte JPEG progressiva",
        )
    })?;
    compressed.resize(total_bytes, 0);
    std::io::stdin()
        .lock()
        .read_exact(&mut compressed)
        .map_err(|error| {
            SourceFailure::new(
                ImagingFailureCode::DecodeFailed,
                format!("não foi possível receber a fonte JPEG progressiva: {error}"),
            )
        })?;
    let mut reader = BufReader::new(Cursor::new(compressed));
    let preflight = preflight_jpeg(&mut reader, total_bytes as u64)?;
    if !preflight.is_progressive {
        return Err(SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            "o worker recebeu um JPEG que não é progressivo",
        ));
    }
    wait_at_decode_test_barrier()?;
    rewind(&mut reader)?;
    decode_render_jpeg_in_process(reader, preflight)
}

#[cfg(debug_assertions)]
fn wait_at_decode_test_barrier() -> Result<(), SourceFailure> {
    let Some(path) = std::env::var_os("MYALBUNS_TEST_PROGRESSIVE_DECODE_BARRIER") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    fs::write(&path, std::process::id().to_string()).map_err(|error| {
        SourceFailure::new(
            ImagingFailureCode::DecodeFailed,
            format!("não foi possível sinalizar o barrier de teste JPEG: {error}"),
        )
    })?;
    while path.exists() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn wait_at_decode_test_barrier() -> Result<(), SourceFailure> {
    Ok(())
}

fn write_worker_image(image: RgbaImage) -> std::io::Result<()> {
    let width = image.width();
    let height = image.height();
    let raw = image.into_raw();
    write_worker_header(JPEG_WORKER_COMPLETED, width, height, raw.len() as u64)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&raw)?;
    stdout.flush()
}

fn write_worker_header(
    status: u8,
    width: u32,
    height: u32,
    raw_length: u64,
) -> std::io::Result<()> {
    let mut header = [0_u8; JPEG_WORKER_HEADER_BYTES];
    header[..4].copy_from_slice(&JPEG_WORKER_MAGIC);
    header[4] = status;
    header[5..9].copy_from_slice(&width.to_be_bytes());
    header[9..13].copy_from_slice(&height.to_be_bytes());
    header[13..21].copy_from_slice(&raw_length.to_be_bytes());
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&header)?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::{JPEG_WORKER_RESOURCE_LIMIT, worker_failure_status};

    #[test]
    fn panic_is_classified_as_a_resource_limit_failure() {
        assert_eq!(worker_failure_status(None), JPEG_WORKER_RESOURCE_LIMIT);
    }
}
