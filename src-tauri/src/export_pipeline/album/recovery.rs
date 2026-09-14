use super::*;
use myalbuns_imaging_protocol::RenderCompletion;

/// Live, failure-scoped recovery. Never serialized into the batch checkpoint.
#[derive(Debug)]
pub(crate) struct AlbumExportRecovery {
    plan: AlbumExportPlan,
    roots: RootBindingPlan,
    preparations: Vec<ExportPreparationGuard>,
    operational: Vec<ExportPathPlan>,
    receipts: Vec<RenderCompletion>,
    published: usize,
    sources: Vec<File>,
}

impl AlbumExportRecovery {
    pub(crate) fn roots(&self) -> &RootBindingPlan {
        &self.roots
    }
    pub(crate) fn request_id(&self) -> &str {
        &self.plan.request_id
    }
    pub(crate) fn project_id(&self) -> &str {
        &self.plan.snapshot.project_id
    }
    pub(crate) fn required_paths(&self) -> Vec<PathBuf> {
        self.plan.required_paths()
    }

    async fn advance<T: ImagingTransport>(
        &mut self,
        transport: &mut T,
        control: &ExportExecutionControl,
        progress: &(dyn Fn(ExportProgress) + Send + Sync),
        context: &InvocationContext,
    ) -> Result<PublishedExport, ExportFailure> {
        ensure_not_cancelled(control)?;
        let plan = &self.plan;
        let roots = &self.roots;
        let total_units = plan
            .outputs
            .iter()
            .map(|(_, units)| units.len() as u32)
            .sum();
        let prepared_units = plan
            .outputs
            .iter()
            .take(self.receipts.len())
            .map(|(_, units)| units.len() as u32)
            .sum();
        progress(ExportProgress::measured(
            ExportProgressStage::Preparing,
            prepared_units,
            total_units,
            true,
        ));
        // Freeze originals across pauses with the same read-only capture used by
        // the Processor. No decode or image-sized allocation is added here.
        if self.sources.is_empty() {
            self.sources = plan
                .sources
                .iter()
                .map(|source| {
                    roots
                        .resolve_existing(
                            source.source_path(),
                            myalbuns_paths::ExpectedObject::RegularFile,
                        )
                        .map_err(|error| {
                            ExportFailure::new(ExportFailureStage::Prepare, error.to_string())
                        })?
                        .capture_for_read()
                        .map_err(|error| {
                            ExportFailure::new(ExportFailureStage::Prepare, error.to_string())
                        })
                })
                .collect::<Result<_, _>>()?;
        }
        protect_originals(plan, roots)?;
        while self.preparations.len() < plan.outputs.len() {
            let (path, _) = &plan.outputs[self.preparations.len()];
            let bound = bind_execution_paths(path, roots, &plan.request_id)?;
            let storage = match self.preparations.first() {
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
                ExportFailure::from_path_error(
                    ExportFailureStage::Prepare,
                    error,
                    error.to_string(),
                )
            })?;
            self.preparations
                .push(ExportPreparationGuard::new(storage, context));
            self.operational.push(bound);
        }
        // Validate retained files before trusting them again, but never read
        // already-published paths as if they were still preparations.
        for index in self.published..self.receipts.len() {
            verify_preparation(&self.operational[index], &self.receipts[index]).map_err(
                |message| ExportFailure::new(ExportFailureStage::VerifyPreparation, message),
            )?;
        }
        if self.receipts.len() < plan.outputs.len() {
            let offset = self.receipts.len();
            for preparation in &self.preparations[offset..] {
                preparation
                    .storage
                    .as_ref()
                    .expect("unfinished output")
                    .discard_unfinished_output()
                    .map_err(|error| {
                        ExportFailure::from_path_error(
                            ExportFailureStage::Prepare,
                            error,
                            error.to_string(),
                        )
                    })?;
            }
            let outputs = plan.outputs[offset..]
                .iter()
                .map(|(path, units)| AlbumRenderOutput {
                    prepared_path: NativePathDto::from(path.prepared_output_path()),
                    units: units.clone(),
                })
                .collect::<Vec<_>>();
            let required: std::collections::HashSet<_> = outputs
                .iter()
                .flat_map(|output| output.units.iter())
                .flat_map(|unit| {
                    plan.snapshot
                        .composition
                        .sheets
                        .iter()
                        .filter(move |sheet| sheet.sheet_id == unit.sheet_id)
                })
                .flat_map(|sheet| sheet.referenced_media_ids())
                .collect();
            let request = AlbumRenderRequest {
                protocol_version: IMAGING_PROTOCOL_VERSION,
                request_id: plan.request_id.clone(),
                snapshot: plan.snapshot.clone(),
                format: plan.format.clone(),
                outputs,
                sources: plan
                    .sources
                    .iter()
                    .filter(|source| required.contains(&source.media_id()))
                    .cloned()
                    .collect(),
                root_bindings: roots.clone(),
            };
            request
                .validate()
                .map_err(|message| ExportFailure::new(ExportFailureStage::Plan, message))?;
            let report = |event: ImagingProgress| {
                let stage = match event.stage {
                    ImagingProgressStage::LoadingSources => ExportProgressStage::LoadingSources,
                    ImagingProgressStage::Composing => ExportProgressStage::Composing,
                    _ => ExportProgressStage::EncodingOutput,
                };
                let (completed, total) = if stage == ExportProgressStage::LoadingSources {
                    (event.completed_units, event.total_units)
                } else {
                    (
                        offset as u32 + event.completed_units,
                        offset as u32 + event.total_units,
                    )
                };
                progress(ExportProgress::measured(stage, completed, total, true));
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
                        for preparation in self.preparations.drain(..) {
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
            let (completion, failure) = match response {
                ImagingResponse::AlbumCompleted {
                    request_id,
                    completion,
                } if request_id == plan.request_id
                    && completion.outputs.len() == plan.outputs.len() - offset =>
                {
                    (completion, None)
                }
                ImagingResponse::AlbumStorageFull {
                    request_id,
                    completion,
                    failure,
                } if request_id == plan.request_id
                    && failure.code == ImagingFailureCode::OutputStorageFull
                    && completion.outputs.len() <= plan.outputs.len() - offset =>
                {
                    (completion, Some(failure))
                }
                response => {
                    if let Some(failure) = response.failure_for(&plan.request_id) {
                        return Err(ExportFailure::from_processor(
                            ExportFailureStage::Processor(InvocationFailureStage::Processor(
                                failure.code.stage(),
                            )),
                            failure.clone(),
                            processor_failure_message(failure.code),
                        ));
                    }
                    return Err(ExportFailure::new(
                        ExportFailureStage::ValidateResponse,
                        "O Processador não confirmou o conjunto exportado.",
                    ));
                }
            };
            for receipt in completion.outputs {
                verify_preparation(&self.operational[self.receipts.len()], &receipt).map_err(
                    |message| ExportFailure::new(ExportFailureStage::VerifyPreparation, message),
                )?;
                self.receipts.push(receipt);
            }
            if let Some(failure) = failure {
                return Err(ExportFailure::from_processor(
                    ExportFailureStage::Processor(InvocationFailureStage::Processor(
                        failure.code.stage(),
                    )),
                    failure.clone(),
                    processor_failure_message(failure.code),
                ));
            }
        }
        progress(ExportProgress::unmeasured(
            ExportProgressStage::Verifying,
            true,
        ));
        if !control.begin_publishing() {
            return Err(cancelled_failure());
        }
        let total = self.preparations.len() as u32;
        while self.published < self.preparations.len() {
            progress(ExportProgress::measured(
                ExportProgressStage::Publishing,
                self.published as u32,
                total,
                false,
            ));
            self.preparations[self.published]
                .publish_retaining()
                .map_err(|error| {
                    ExportFailure::from_path_error(
                        ExportFailureStage::Publish {
                            promoted_outputs: self.published as u32,
                            total_outputs: total,
                        },
                        error,
                        if error == AppPathsError::ExportStorageFull {
                            if self.published > 0 {
                                "O álbum foi publicado parcialmente. Libere espaço e retome para concluir. Os arquivos já exportados foram mantidos.".into()
                            } else {
                                "Libere espaço para continuar. Os arquivos já existentes foram mantidos.".into()
                            }
                        } else {
                            format!(
                                "Não foi possível concluir a publicação: {error}. {}",
                                if self.published > 0 {
                                    "O álbum foi publicado parcialmente. Tente exportar novamente para concluir."
                                } else {
                                    "Os arquivos já existentes foram mantidos."
                                }
                            )
                        },
                    )
                })?;
            self.published += 1;
        }
        if plan.cleanup_confirmed && !plan.obsolete_outputs.is_empty() {
            let obsolete = plan
                .obsolete_outputs
                .iter()
                .map(|path| {
                    roots.resolve(path).map_err(|error| {
                        ExportFailure::new(
                            ExportFailureStage::Publish {
                                promoted_outputs: total,
                                total_outputs: total,
                            },
                            error.to_string(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            PreparedExportStorage::remove_obsolete_outputs(
                self.operational[0]
                    .output_path()
                    .parent()
                    .expect("destination"),
                &obsolete,
            )
            .map_err(|error| {
                ExportFailure::from_path_error(
                    ExportFailureStage::Publish {
                        promoted_outputs: total,
                        total_outputs: total,
                    },
                    error,
                    error.to_string(),
                )
            })?;
        }
        progress(ExportProgress::measured(
            ExportProgressStage::Completed,
            total,
            total,
            false,
        ));
        Ok(PublishedExport {
            completion: self.receipts[0].clone(),
        })
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
    resume_album(
        transport,
        Box::new(AlbumExportRecovery {
            plan,
            roots: roots.clone(),
            preparations: Vec::new(),
            operational: Vec::new(),
            receipts: Vec::new(),
            published: 0,
            sources: Vec::new(),
        }),
        control,
        progress,
        context,
    )
    .await
}

pub(crate) async fn resume_album<T: ImagingTransport>(
    transport: &mut T,
    mut recovery: Box<AlbumExportRecovery>,
    control: &ExportExecutionControl,
    progress: &(dyn Fn(ExportProgress) + Send + Sync),
    context: &InvocationContext,
) -> Result<PublishedExport, ExportFailure> {
    match recovery
        .advance(transport, control, progress, context)
        .await
    {
        Ok(result) => Ok(result),
        Err(mut failure) => {
            if failure.is_storage_full() {
                failure.recovery = Some(recovery);
            }
            Err(failure)
        }
    }
}

fn protect_originals(plan: &AlbumExportPlan, roots: &RootBindingPlan) -> Result<(), ExportFailure> {
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
    Ok(())
}
