use super::*;
use myalbuns_core::{ExportFormat, ExportMode};
use myalbuns_imaging_protocol::{AlbumRenderOutput, AlbumRenderRequest, ImagingResponse};

#[derive(Debug)]
pub(crate) struct AlbumExportPlan {
    paths: Option<RootBindingPlan>,
    protected_originals: Vec<PathBuf>,
    obsolete_outputs: Vec<PathBuf>,
    cleanup_confirmed: bool,
    snapshot: RenderSnapshot,
    request_id: String,
    outputs: Vec<(ExportPathPlan, Vec<myalbuns_core::SelectedExportUnit>)>,
    format: ExportFormat,
    sources: Vec<RenderSource>,
}

pub(crate) struct AlbumExportOptions {
    pub protected_originals: Vec<PathBuf>,
    pub sheet_ids: Vec<String>,
    pub whole_album: bool,
    pub mode: ExportMode,
    pub format: ExportFormat,
    pub destination: PathBuf,
    pub authorization: ExportWriteAuthorization,
    pub sources: Vec<RenderSource>,
    pub request_id: String,
}

pub(crate) fn plan_album(
    snapshot: RenderSnapshot,
    options: AlbumExportOptions,
) -> Result<AlbumExportPlan, ExportFailure> {
    plan_album_with_paths(snapshot, options, None)
}

/// Preflight and execution must observe the same bound destination, including
/// conflict checks and orphan discovery. Output plans retain their logical paths.
pub(crate) fn plan_album_in_paths(
    snapshot: RenderSnapshot,
    options: AlbumExportOptions,
    paths: &RootBindingPlan,
) -> Result<AlbumExportPlan, ExportFailure> {
    plan_album_with_paths(snapshot, options, Some(paths.clone()))
}

fn observed_path(
    paths: Option<&RootBindingPlan>,
    path: &std::path::Path,
) -> Result<PathBuf, ExportFailure> {
    paths
        .map_or_else(|| Ok(path.to_path_buf()), |paths| paths.resolve(path))
        .map_err(|error| ExportFailure::new(ExportFailureStage::Plan, error.to_string()))
}

fn plan_album_with_paths(
    snapshot: RenderSnapshot,
    options: AlbumExportOptions,
    paths: Option<RootBindingPlan>,
) -> Result<AlbumExportPlan, ExportFailure> {
    let AlbumExportOptions {
        protected_originals,
        sheet_ids,
        whole_album,
        mode,
        format,
        destination,
        authorization,
        sources,
        request_id,
    } = options;
    let invalid = |message| ExportFailure::new(ExportFailureStage::Plan, message);
    format.validate().map_err(invalid)?;
    let units = snapshot
        .export_units(&sheet_ids, mode)
        .map_err(|error| invalid(error.to_string()))?;
    if whole_album && sheet_ids.len() != snapshot.composition.sheets.len() {
        return Err(invalid(
            "Álbum inteiro precisa incluir todas as Lâminas.".into(),
        ));
    }
    let groups = if format == ExportFormat::Pdf {
        vec![units]
    } else {
        units.into_iter().map(|unit| vec![unit]).collect()
    };
    let name = crate::export_commands::export_name(&snapshot.project_name);
    let outputs = groups
        .into_iter()
        .map(|units| {
            let file = if format == ExportFormat::Pdf {
                format!("{name}.pdf")
            } else {
                format!("{name}_{:03}.{}", units[0].index, format.extension())
            };
            let path =
                ExportPathPlan::new_authorized(destination.join(file), &request_id, authorization)
                    .map_err(|error| invalid(error.to_string()))?;
            Ok((path, units))
        })
        .collect::<Result<Vec<_>, ExportFailure>>()?;
    let mut obsolete_outputs = Vec::new();
    let observed_destination = observed_path(paths.as_ref(), &destination)?;
    if whole_album && format != ExportFormat::Pdf && observed_destination.is_dir() {
        for entry in
            std::fs::read_dir(&observed_destination).map_err(|error| invalid(error.to_string()))?
        {
            let entry = entry.map_err(|error| invalid(error.to_string()))?;
            let file = entry.file_name();
            let Some(file) = file.to_str() else { continue };
            let Some(index) = file
                .strip_prefix(&format!("{name}_"))
                .and_then(|suffix| suffix.strip_suffix(&format!(".{}", format.extension())))
                .and_then(|digits| digits.parse::<usize>().ok())
            else {
                continue;
            };
            if index > 0
                && file == format!("{name}_{index:03}.{}", format.extension())
                && !outputs
                    .iter()
                    .any(|(path, _)| path.output_path() == destination.join(file))
            {
                obsolete_outputs.push(destination.join(file));
            }
        }
        obsolete_outputs.sort();
    }
    Ok(AlbumExportPlan {
        paths,
        protected_originals,
        obsolete_outputs,
        cleanup_confirmed: whole_album
            && authorization == ExportWriteAuthorization::ReplaceConfirmed,
        snapshot,
        outputs,
        format,
        sources,
        request_id,
    })
}

impl AlbumExportPlan {
    pub(crate) fn preparation_directory(&self) -> &std::path::Path {
        self.outputs[0].0.preparation_directory()
    }
    /// Keeps original numbering and create-only publication for the remaining files.
    /// A file appearing after this check must never be silently overwritten.
    pub(crate) fn skip_existing_outputs(&mut self) -> Result<bool, ExportFailure> {
        let existing = self.conflicts()?;
        self.outputs.retain(|(path, _)| {
            !existing
                .iter()
                .any(|name| path.output_path().file_name() == Some(std::ffi::OsStr::new(name)))
        });
        self.obsolete_outputs.clear();
        self.cleanup_confirmed = false;
        let sheets: std::collections::HashSet<_> = self
            .outputs
            .iter()
            .flat_map(|(_, units)| units.iter().map(|unit| &unit.sheet_id))
            .collect();
        let required: std::collections::HashSet<_> = self
            .snapshot
            .composition
            .sheets
            .iter()
            .filter(|sheet| sheets.contains(&sheet.sheet_id))
            .flat_map(|sheet| sheet.referenced_media_ids())
            .collect();
        self.sources
            .retain(|source| required.contains(&source.media_id()));
        Ok(!self.outputs.is_empty())
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }
    pub(crate) fn required_paths(&self) -> Vec<PathBuf> {
        let protects_existing = self.outputs.iter().any(|(path, _)| {
            observed_path(self.paths.as_ref(), path.output_path()).is_ok_and(|path| path.exists())
        }) || !self.obsolete_outputs.is_empty();
        self.outputs
            .iter()
            .map(|(path, _)| path.output_path().to_path_buf())
            .chain(
                self.sources
                    .iter()
                    .map(|source| source.source_path().to_path_buf()),
            )
            .chain(
                self.protected_originals
                    .iter()
                    .filter(|_| protects_existing)
                    .cloned(),
            )
            .collect()
    }
    pub(crate) fn conflicts(&self) -> Result<Vec<String>, ExportFailure> {
        let mut files = Vec::new();
        for path in self
            .outputs
            .iter()
            .map(|(path, _)| path.output_path())
            .chain(self.obsolete_outputs.iter().map(PathBuf::as_path))
        {
            match std::fs::symlink_metadata(observed_path(self.paths.as_ref(), path)?) {
                Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => files.push(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                ),
                Ok(_) => {
                    return Err(ExportFailure::new(
                        ExportFailureStage::Plan,
                        "Um nome de saída pertence a uma pasta ou vínculo. Escolha outro Destino.",
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(ExportFailure::new(
                        ExportFailureStage::Plan,
                        format!("Não foi possível verificar os conflitos no Destino: {error}"),
                    ));
                }
            }
        }
        Ok(files)
    }
}

mod recovery;
pub(crate) use recovery::{AlbumExportRecovery, execute_album, resume_album};
