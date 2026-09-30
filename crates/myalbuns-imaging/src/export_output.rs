use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::jpeg_output::VerifiedJpeg;

#[cfg(not(test))]
pub(crate) type OutputFile = File;

/// Hashes exactly the bytes its inner writer accepted. The receipt then
/// describes what was handed to the prepared file without reading it back: on a
/// network Destination that readback was one more transfer of every output. The
/// Host still reads the file once and compares this digest, which proves the
/// disk holds these bytes.
pub(crate) struct HashingWriter<W> {
    inner: W,
    hasher: Sha256,
    bytes: u64,
}

impl<W> HashingWriter<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
        }
    }

    pub(crate) fn get_ref(&self) -> &W {
        &self.inner
    }

    pub(crate) fn into_receipt(self) -> (W, VerifiedJpeg) {
        (
            self.inner,
            VerifiedJpeg {
                output_bytes: self.bytes,
                output_sha256: format!("{:x}", self.hasher.finalize()),
            },
        )
    }
}

impl<W: io::Write> io::Write for HashingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..written]);
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Only reports the current position, which the PDF writer records for its
/// cross-reference table; a writer that hashes as it goes cannot move back.
impl<W> io::Seek for HashingWriter<W> {
    fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
        match position {
            io::SeekFrom::Current(0) => Ok(self.bytes),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a preparação da Exportação é gravada em sequência",
            )),
        }
    }
}

/// Each write to a network Destination is one request to the server: with the
/// default 8 KiB buffer a share accepted about 2 MiB/s, and 16 MiB/s with 1 MiB
/// blocks (`docs/research/2026-09-29-desempenho-em-rede.md`).
const OUTPUT_WRITE_BUFFER_BYTES: usize = 1024 * 1024;

/// Buffers the encoder's small writes so the prepared file receives large blocks.
pub(crate) fn output_writer<W: io::Write>(inner: W) -> io::BufWriter<W> {
    io::BufWriter::with_capacity(OUTPUT_WRITE_BUFFER_BYTES, inner)
}

pub(crate) fn create_output(path: &Path) -> io::Result<OutputFile> {
    #[cfg(test)]
    myalbuns_paths::test_support::create(path)?;
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    #[cfg(not(test))]
    return Ok(file);
    #[cfg(test)]
    Ok(OutputFile {
        file,
        path: path.to_owned(),
    })
}

// Faults stay at the actual file writer; production uses File directly.
#[cfg(test)]
pub(crate) struct OutputFile {
    file: File,
    path: std::path::PathBuf,
}

#[cfg(test)]
impl OutputFile {
    pub(crate) fn sync_all(&self) -> io::Result<()> {
        myalbuns_paths::test_support::sync(&self.path)?;
        self.file.sync_all()
    }
}

#[cfg(test)]
impl io::Write for OutputFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        myalbuns_paths::test_support::write(&self.path, buffer, |bytes| self.file.write(bytes))
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
impl io::Seek for OutputFile {
    fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};
    use myalbuns_paths::test_support::DiskFull;

    #[test]
    fn each_receipt_describes_exactly_the_prepared_file() {
        use sha2::{Digest, Sha256};
        let image = RgbaImage::from_fn(97, 61, |x, y| {
            Rgba([
                ((x * 61 + y * 97) % 256) as u8,
                ((x * 131 + y * 43) % 256) as u8,
                ((x * y * 17 + y) % 256) as u8,
                255,
            ])
        });
        for extension in ["jpg", "png", "pdf"] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join(format!("prepared.{extension}"));
            let receipt = match extension {
                "jpg" => crate::jpeg_output::write_verified_quality(&image, &path, 300, 100),
                "png" => crate::format_output::write_png(&image, &path, 300),
                _ => (|| {
                    let mut pdf = crate::format_output::PdfOutput::create(&path)?;
                    pdf.add_page(&image, 25400, 25400)?;
                    pdf.add_page(&image, 25400, 25400)?;
                    pdf.finish()
                })(),
            }
            .unwrap();
            let written = std::fs::read(&path).unwrap();
            assert_eq!(receipt.output_bytes, written.len() as u64, "{extension}");
            assert_eq!(
                receipt.output_sha256,
                format!("{:x}", Sha256::digest(&written)),
                "{extension}"
            );
        }
    }

    #[test]
    fn the_prepared_file_receives_the_encoders_small_writes_in_large_blocks() {
        use std::io::Write;
        struct Requests(Vec<usize>);
        impl Write for Requests {
            fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
                self.0.push(buffer.len());
                Ok(buffer.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = super::output_writer(Requests(Vec::new()));
        for _ in 0..(3 * 1024 + 512) {
            writer.write_all(&[7; 1024]).unwrap();
        }
        let requests = writer.into_inner().ok().unwrap().0;
        assert_eq!(
            requests,
            [1024 * 1024, 1024 * 1024, 1024 * 1024, 512 * 1024]
        );
    }

    #[test]
    fn disk_full_in_export_encoders_is_specific_at_create_write_and_sync() {
        for extension in ["jpg", "png", "pdf"] {
            for (edge, phase) in [(3, "create"), (3, "write"), (256, "write"), (3, "sync")] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join(format!("prepared.{extension}"));
                let image = RgbaImage::from_fn(edge, edge, |x, y| {
                    Rgba([
                        ((x * 61 + y * 97) % 256) as u8,
                        ((x * 131 + y * 43) % 256) as u8,
                        ((x * y * 17 + y) % 256) as u8,
                        255,
                    ])
                });
                let encode = |path: &std::path::Path| match extension {
                    "jpg" => crate::jpeg_output::write_verified_quality(&image, path, 300, 100),
                    "png" => crate::format_output::write_png(&image, path, 300),
                    _ => (|| {
                        let mut pdf = crate::format_output::PdfOutput::create(path)?;
                        pdf.add_page(&image, 25400, 25400)?;
                        pdf.finish()
                    })(),
                };
                let fault = match phase {
                    "create" => DiskFull::on_create(&path),
                    "sync" => DiskFull::on_sync(&path),
                    _ => DiskFull::after_bytes(&path, 32),
                };
                let error = encode(&path).unwrap_err();
                assert!(fault.failure_count() > 0);
                assert_eq!(
                    error.code.as_str(),
                    "output_storage_full",
                    "{extension}/{edge}/{phase}: {}",
                    error.message
                );
                if phase == "write" {
                    assert_eq!(fault.written_bytes(), 32);
                }
                drop(fault);
                let retry = root.path().join(format!("retry.{extension}"));
                assert!(encode(&retry).unwrap().output_bytes > 0);
                let prior = std::fs::read(&retry).unwrap();
                let conflict = encode(&retry).unwrap_err();
                assert_ne!(
                    conflict.code.as_str(),
                    "output_storage_full",
                    "an existing output is not a full disk"
                );
                assert_eq!(std::fs::read(&retry).unwrap(), prior);
            }
        }
    }
}
