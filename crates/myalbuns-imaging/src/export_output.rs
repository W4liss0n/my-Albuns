use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

#[cfg(not(test))]
pub(crate) type OutputFile = File;

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
                        pdf.finish(path)
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
