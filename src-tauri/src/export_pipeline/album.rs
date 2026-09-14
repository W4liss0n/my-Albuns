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

pub(crate) async fn execute_album<T: ImagingTransport>(
    transport: &mut T,
    plan: AlbumExportPlan,
    roots: &RootBindingPlan,
    control: &ExportExecutionControl,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
    context: &InvocationContext,
) -> Result<PublishedExport, ExportFailure> {
    ensure_not_cancelled(control)?;
    let mut obsolete_outputs = Vec::new();
    for path in &plan.obsolete_outputs {
        obsolete_outputs.push(
            roots.resolve(path).map_err(|error| {
                ExportFailure::new(ExportFailureStage::Prepare, error.to_string())
            })?,
        );
    }
    // A destination must never name an Original, even through another path alias.
    for path in plan
        .outputs
        .iter()
        .map(|(path, _)| path.output_path())
        .chain(plan.obsolete_outputs.iter().map(PathBuf::as_path))
    {
        let destination = roots
            .prepare_file_destination(path)
            .map_err(|error| ExportFailure::new(ExportFailureStage::Prepare, error.to_string()))?;
        if let Some(target) = destination
            .resolve_existing()
            .map_err(|error| ExportFailure::new(ExportFailureStage::Prepare, error.to_string()))?
        {
            for source in &plan.protected_originals {
                let original = match roots
                    .resolve_existing(source, myalbuns_paths::ExpectedObject::RegularFile)
                {
                    Ok(original) => original,
                    Err(myalbuns_paths::ResolveError::NotFound) => continue,
                    Err(error) => {
                        return Err(ExportFailure::new(
                            ExportFailureStage::Prepare,
                            format!(
                                "Não foi possível distinguir o Destino de um Original do Projeto: {error}. Escolha uma pasta nova ou restabeleça o acesso aos Originais."
                            ),
                        ));
                    }
                };
                if target.compare_physical(&original)
                    != myalbuns_paths::PhysicalIdentityEvidence::Different
                {
                    return Err(ExportFailure::new(
                        ExportFailureStage::Prepare,
                        "O Destino coincide com um Original ou sua identidade não pôde ser distinguida. Escolha outra pasta.",
                    ));
                }
            }
        }
    }
    let mut preparations: Vec<ExportPreparationGuard> = Vec::new();
    let mut operational = Vec::new();
    for (path, _) in &plan.outputs {
        let bound = bind_execution_paths(path, roots, &plan.request_id)?;
        let storage = match preparations.first() {
            Some(first) => first
                .storage
                .as_ref()
                .expect("owned preparation")
                .prepare_sibling(bound.clone()),
            None => bound
                .prepare()
                .map(PreparedExportStorage::into_shared_preparation),
        }
        .map_err(|error| {
            ExportFailure::from_path_error(ExportFailureStage::Prepare, error, error.to_string())
        })?;
        preparations.push(ExportPreparationGuard::new(storage, context));
        operational.push(bound);
    }
    let request = AlbumRenderRequest {
        protocol_version: IMAGING_PROTOCOL_VERSION,
        request_id: plan.request_id.clone(),
        snapshot: plan.snapshot,
        format: plan.format,
        sources: plan.sources,
        root_bindings: roots.clone(),
        outputs: plan
            .outputs
            .into_iter()
            .map(|(path, units)| AlbumRenderOutput {
                prepared_path: NativePathDto::from(path.prepared_output_path()),
                units,
            })
            .collect(),
    };
    request
        .validate()
        .map_err(|message| ExportFailure::new(ExportFailureStage::Plan, message))?;
    let report = |event: ImagingProgress| {
        progress(ExportProgress::measured(
            match event.stage {
                ImagingProgressStage::LoadingSources => ExportProgressStage::LoadingSources,
                ImagingProgressStage::Composing => ExportProgressStage::Composing,
                _ => ExportProgressStage::EncodingOutput,
            },
            event.completed_units,
            event.total_units,
            true,
        ))
    };
    let response = match transport
        .invoke(
            &ImagingCommand::RenderAlbum(request),
            context,
            ImagingOperation::Export,
            1,
            InvocationControl::controlled(control.cancellation_flag(), &report),
        )
        .await
    {
        Ok(response) => response,
        Err(failure) if failure.is_cancelled() => return Err(cancelled_failure()),
        Err(failure) => {
            if failure.is_termination_unconfirmed() {
                for preparation in preparations {
                    preparation.preserve();
                }
            }
            return Err(ExportFailure::from_invocation(
                failure,
                ExportFailureStage::Processor,
            ));
        }
    };
    ensure_not_cancelled(control)?;
    if let Some(failure) = response.failure_for(&plan.request_id) {
        return Err(ExportFailure::from_processor(
            ExportFailureStage::Processor(InvocationFailureStage::Processor(failure.code.stage())),
            failure.clone(),
            processor_failure_message(failure.code),
        ));
    }
    let ImagingResponse::AlbumCompleted {
        request_id,
        completion,
    } = response
    else {
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O Processador não confirmou o conjunto exportado.",
        ));
    };
    if request_id != plan.request_id || completion.outputs.len() != operational.len() {
        return Err(ExportFailure::new(
            ExportFailureStage::ValidateResponse,
            "O conjunto preparado não corresponde à seleção.",
        ));
    }
    progress(ExportProgress::unmeasured(
        ExportProgressStage::Verifying,
        true,
    ));
    for (path, receipt) in operational.iter().zip(&completion.outputs) {
        verify_preparation(path, receipt).map_err(|message| {
            ExportFailure::new(ExportFailureStage::VerifyPreparation, message)
        })?;
    }
    if !control.begin_publishing() {
        return Err(cancelled_failure());
    }
    let total = preparations.len() as u32;
    for (index, preparation) in preparations.into_iter().enumerate() {
        progress(ExportProgress::measured(
            ExportProgressStage::Publishing,
            index as u32,
            total,
            false,
        ));
        preparation.publish().map_err(|error| ExportFailure::from_path_error(ExportFailureStage::Publish { promoted_outputs: index as u32, total_outputs: total }, error,
            format!("Não foi possível concluir a publicação ({index} de {total} arquivos confirmados): {error}. O Destino pode conter saídas anteriores e novas. Faça uma nova Exportação integral.")))?;
    }
    if plan.cleanup_confirmed && !obsolete_outputs.is_empty() {
        PreparedExportStorage::remove_obsolete_outputs(
            operational[0].output_path().parent().expect("planned destination"), &obsolete_outputs,
        ).map_err(|error| ExportFailure::from_path_error(ExportFailureStage::Publish { promoted_outputs: total, total_outputs: total }, error,
            format!("As novas saídas foram publicadas, mas a limpeza das saídas antigas falhou: {error}. Faça uma nova Exportação integral.")))?;
    }
    progress(ExportProgress::measured(
        ExportProgressStage::Completed,
        total,
        total,
        false,
    ));
    Ok(PublishedExport {
        completion: completion
            .outputs
            .into_iter()
            .next()
            .expect("nonempty verified output set"),
    })
}
