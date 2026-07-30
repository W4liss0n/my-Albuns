use std::io::Write;

use myalbuns_imaging_protocol::{
    CacheArtifact, CacheCompletion, CacheJob, CacheRequest, IMAGING_PROTOCOL_VERSION,
    ImagingCommand, ImagingResponse, MediaSource,
};
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{AppPaths, CachePathPlan, PreparedCacheStorage};

use crate::imaging_processor::{
    ImagingOperation, ImagingTransport, InvocationContext, InvocationFailure,
    InvocationFailureStage, OperationFailure,
};

const CACHE_REPRESENTATION_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub(crate) struct CacheWork {
    pub(crate) request_id: String,
    pub(crate) project_id: String,
    pub(crate) cache_paths: CachePathPlan,
    pub(crate) sources: Vec<MediaSource>,
    pub(crate) max_edge_px: u32,
}

impl CacheWork {
    pub(crate) fn new(
        request_id: impl Into<String>,
        project_id: impl Into<String>,
        cache_paths: CachePathPlan,
        sources: Vec<MediaSource>,
        max_edge_px: u32,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            project_id: project_id.into(),
            cache_paths,
            sources,
            max_edge_px,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheFailureStage {
    Plan,
    Processor(InvocationFailureStage),
    ValidateResponse,
    VerifyArtifacts,
    PublishIndex,
}

impl CacheFailureStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan_request",
            Self::Processor(stage) => stage.as_str(),
            Self::ValidateResponse => "validate_response",
            Self::VerifyArtifacts => "verify_artifacts",
            Self::PublishIndex => "publish_index",
        }
    }
}

pub(crate) type CacheFailure = OperationFailure<CacheFailureStage>;

#[derive(Debug)]
pub(crate) struct CacheExecution {
    pub(crate) completion: CacheCompletion,
    pub(crate) recovery: Option<CacheRecovery>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheRecovery {
    pub(crate) failed_process_id: u32,
    pub(crate) removed_temporary_count: usize,
}

pub(crate) async fn execute<T: ImagingTransport>(
    transport: &mut T,
    app_paths: &AppPaths,
    work: CacheWork,
    context: &InvocationContext,
) -> Result<CacheExecution, CacheFailure> {
    let request = plan_request(&work)?;
    let command = ImagingCommand::build_cache(request.clone());
    let (response, recovery) =
        invoke_with_recovery(transport, app_paths, &work.cache_paths, &command, context)
            .await
            .map_err(|failure| {
                CacheFailure::from_invocation(failure, CacheFailureStage::Processor)
            })?;
    let completion = response
        .cache_completed_for(&work.request_id)
        .cloned()
        .ok_or_else(|| {
            CacheFailure::new(
                CacheFailureStage::ValidateResponse,
                "O Processador devolveu uma resposta de Cache inesperada.",
            )
        })?;
    let storage = app_paths
        .prepare_cache_storage(&work.cache_paths)
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar o Cache: {error}"),
            )
        })?;
    verify_completion(&storage, &request, &completion)?;
    write_cache_metadata(&storage, &request, &completion.artifacts)?;
    Ok(CacheExecution {
        completion,
        recovery,
    })
}

fn plan_request(work: &CacheWork) -> Result<CacheRequest, CacheFailure> {
    let jobs = work
        .sources
        .iter()
        .cloned()
        .map(|source| {
            let generation_id = format!(
                "{}-v{}-{}",
                source.source_sha256()[..16].to_ascii_lowercase(),
                CACHE_REPRESENTATION_VERSION,
                work.max_edge_px
            );
            CacheJob::new(source, generation_id)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::Plan,
                format!("Não foi possível planejar o Cache: {error}"),
            )
        })?;
    CacheRequest::new(
        work.request_id.clone(),
        work.project_id.clone(),
        work.cache_paths.clone(),
        jobs,
        work.max_edge_px,
    )
    .map_err(|error| {
        CacheFailure::new(
            CacheFailureStage::Plan,
            format!("Não foi possível planejar o Cache: {error}"),
        )
    })
}

async fn invoke_with_recovery<T: ImagingTransport>(
    transport: &mut T,
    app_paths: &AppPaths,
    cache_paths: &CachePathPlan,
    command: &ImagingCommand,
    context: &InvocationContext,
) -> Result<(ImagingResponse, Option<CacheRecovery>), InvocationFailure> {
    let mut attempt = 1_u8;
    let mut recovery = None;
    loop {
        match transport
            .invoke(command, context, ImagingOperation::Cache, attempt)
            .await
        {
            Ok(response) => {
                if attempt > 1 {
                    tracing::info!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id.as_str(),
                        project_id = context.project_id.as_deref(),
                        attempts = attempt,
                        event = "imaging_processor_restart_completed",
                    );
                }
                return Ok((response, recovery));
            }
            Err(failure) if failure.is_unexpected_termination() => {
                let Some(failed_process_id) = failure.process_id else {
                    return Err(failure);
                };
                let removed_temporary_count = app_paths
                    .discard_project_cache_temporaries(cache_paths, failed_process_id)
                    .map_err(|error| {
                        InvocationFailure::cache_recovery_cleanup(
                            &failure,
                            format!(
                                "Não foi possível descartar o item incompleto do Cache: {error}"
                            ),
                        )
                    })?;
                if attempt == 1 {
                    recovery = Some(CacheRecovery {
                        failed_process_id,
                        removed_temporary_count,
                    });
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id.as_str(),
                        project_id = context.project_id.as_deref(),
                        failed_attempt = attempt,
                        failed_process_id,
                        exit_code = failure.exit_code,
                        removed_temporary_count,
                        event = "imaging_processor_restart_started",
                    );
                    attempt += 1;
                } else {
                    tracing::error!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id.as_str(),
                        project_id = context.project_id.as_deref(),
                        attempts = attempt,
                        exit_code = failure.exit_code,
                        event = "imaging_processor_restart_exhausted",
                    );
                    return Err(failure);
                }
            }
            Err(failure) => return Err(failure),
        }
    }
}

fn verify_completion(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    completion: &CacheCompletion,
) -> Result<(), CacheFailure> {
    if completion.artifacts.len() != request.jobs.len()
        || completion.generated_count + completion.reused_count != request.jobs.len()
        || completion.source_bytes
            != request
                .jobs
                .iter()
                .map(|job| job.source.source_bytes())
                .sum::<u64>()
        || completion.preview_bytes
            != completion
                .artifacts
                .iter()
                .map(|artifact| artifact.preview_bytes)
                .sum::<u64>()
    {
        return Err(CacheFailure::new(
            CacheFailureStage::ValidateResponse,
            "A conclusão do Cache não corresponde ao trabalho solicitado.",
        ));
    }

    for (job, artifact) in request.jobs.iter().zip(&completion.artifacts) {
        if artifact.media_id != job.source.media_id()
            || artifact.generation_id != job.generation_id
            || artifact.width_px == 0
            || artifact.height_px == 0
            || artifact.preview_bytes == 0
        {
            return Err(CacheFailure::new(
                CacheFailureStage::ValidateResponse,
                "A conclusão contém um artefato de Cache inesperado.",
            ));
        }
        let preview_path = request
            .cache_paths
            .preview_file(&artifact.media_id, &artifact.generation_id)
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::VerifyArtifacts,
                    format!("O caminho do artefato de Cache é inválido: {error}"),
                )
            })?;
        let file = storage
            .open_existing_file(&preview_path)
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::VerifyArtifacts,
                    format!("Não foi possível verificar a prévia do Cache: {error}"),
                )
            })?
            .ok_or_else(|| {
                CacheFailure::new(
                    CacheFailureStage::VerifyArtifacts,
                    "A prévia concluída não foi encontrada.",
                )
            })?;
        let actual_bytes = file.metadata().map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                format!("Não foi possível verificar a prévia do Cache: {error}"),
            )
        })?;
        if actual_bytes.len() != artifact.preview_bytes {
            return Err(CacheFailure::new(
                CacheFailureStage::VerifyArtifacts,
                "A prévia concluída não corresponde à resposta recebida.",
            ));
        }
    }
    Ok(())
}

fn write_cache_metadata(
    storage: &PreparedCacheStorage,
    request: &CacheRequest,
    artifacts: &[CacheArtifact],
) -> Result<(), CacheFailure> {
    let entries = artifacts
        .iter()
        .zip(&request.jobs)
        .map(
            |(artifact, job)| -> Result<serde_json::Value, CacheFailure> {
                let artifact_path = request
                    .cache_paths
                    .preview_file(&artifact.media_id, &artifact.generation_id)
                    .map_err(|error| {
                        CacheFailure::new(
                            CacheFailureStage::PublishIndex,
                            format!("O caminho do artefato de Cache é inválido: {error}"),
                        )
                    })?;
                let artifact_name = artifact_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| {
                        CacheFailure::new(
                            CacheFailureStage::PublishIndex,
                            "O nome do artefato de Cache é inválido.",
                        )
                    })?;
                Ok(serde_json::json!({
                    "mediaId": artifact.media_id,
                    "generationId": artifact.generation_id,
                    "artifactName": artifact_name,
                    "widthPx": artifact.width_px,
                    "heightPx": artifact.height_px,
                    "previewBytes": artifact.preview_bytes,
                    "sourceBytes": job.source.source_bytes(),
                    "sourceSha256": job.source.source_sha256(),
                }))
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = serde_json::json!({
        "schemaVersion": 1,
        "representationVersion": CACHE_REPRESENTATION_VERSION,
        "projectId": request.project_id,
        "maxEdgePx": request.max_edge_px,
        "format": "jpeg",
        "entries": entries,
    });
    let metadata_path = request.cache_paths.metadata_file();
    let temporary_path = request
        .cache_paths
        .metadata_temporary_file(std::process::id());
    let publish = (|| -> Result<(), CacheFailure> {
        let metadata_bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível serializar o índice: {error}"),
            )
        })?;
        let mut temporary = storage
            .create_temporary_file(&temporary_path)
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::PublishIndex,
                    format!("Não foi possível criar o índice temporário: {error}"),
                )
            })?;
        temporary.write_all(&metadata_bytes).map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível gravar o índice temporário: {error}"),
            )
        })?;
        temporary.sync_all().map_err(|error| {
            CacheFailure::new(
                CacheFailureStage::PublishIndex,
                format!("Não foi possível sincronizar o índice: {error}"),
            )
        })?;
        drop(temporary);
        storage
            .replace_file(&temporary_path, &metadata_path)
            .map_err(|error| {
                CacheFailure::new(
                    CacheFailureStage::PublishIndex,
                    format!("Não foi possível publicar o índice: {error}"),
                )
            })
    })();
    if publish.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    publish
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io::Write, path::PathBuf};

    use myalbuns_imaging_protocol::{
        CacheArtifact, CacheCompletion, ImagingCommand, ImagingFailureStage, ImagingResponse,
        MediaSource,
    };
    use myalbuns_paths::AppPaths;

    use super::{CacheFailureStage, CacheWork, execute, plan_request};
    use crate::imaging_processor::{
        ImagingOperation, ImagingTransport, InvocationContext, InvocationFailure, InvocationFuture,
    };

    struct ScriptedTransport {
        results: VecDeque<Result<ImagingResponse, InvocationFailure>>,
        attempts: Vec<u8>,
    }

    impl ImagingTransport for ScriptedTransport {
        fn invoke<'a>(
            &'a mut self,
            _command: &'a ImagingCommand,
            _context: &'a InvocationContext,
            operation: ImagingOperation,
            attempt: u8,
        ) -> InvocationFuture<'a> {
            assert_eq!(operation, ImagingOperation::Cache);
            self.attempts.push(attempt);
            let result = self.results.pop_front().expect("one result per attempt");
            Box::pin(async move { result })
        }
    }

    fn work() -> (tempfile::TempDir, AppPaths, CacheWork, InvocationContext) {
        let root = tempfile::tempdir().expect("temporary application roots");
        let roaming = root.path().join("roaming");
        let local = root.path().join("local");
        std::fs::create_dir_all(&roaming).expect("roaming root");
        std::fs::create_dir_all(&local).expect("local root");
        let app_paths = AppPaths::from_known_folders(&roaming, &local);
        let cache_paths = app_paths
            .project_cache("project-test")
            .expect("valid cache plan");
        let source = MediaSource::new(
            "media-test",
            PathBuf::from(r"C:\fixtures\photo.jpg"),
            1024,
            "a".repeat(64),
        )
        .expect("valid source");
        let work = CacheWork::new(
            "cache-test",
            "project-test",
            cache_paths,
            vec![source],
            1600,
        );
        let context = InvocationContext::new("cache-test", Some("project-test"));
        (root, app_paths, work, context)
    }

    fn completed(work: &CacheWork, app_paths: &AppPaths) -> ImagingResponse {
        let request = plan_request(work).expect("valid cache request");
        let job = &request.jobs[0];
        let preview_path = request
            .cache_paths
            .preview_file(job.source.media_id(), &job.generation_id)
            .expect("valid preview path");
        let temporary_path = request
            .cache_paths
            .preview_temporary_file(
                job.source.media_id(),
                &job.generation_id,
                std::process::id(),
            )
            .expect("valid temporary path");
        let storage = app_paths
            .prepare_cache_storage(&request.cache_paths)
            .expect("cache storage");
        let mut file = storage
            .create_temporary_file(&temporary_path)
            .expect("temporary preview");
        file.write_all(b"preview").expect("preview bytes");
        drop(file);
        storage
            .replace_file(&temporary_path, &preview_path)
            .expect("published preview");
        ImagingResponse::cache_completed(
            request.request_id,
            CacheCompletion {
                artifacts: vec![CacheArtifact {
                    media_id: job.source.media_id().to_owned(),
                    generation_id: job.generation_id.clone(),
                    width_px: 10,
                    height_px: 5,
                    preview_bytes: 7,
                }],
                generated_count: 1,
                reused_count: 0,
                source_bytes: job.source.source_bytes(),
                preview_bytes: 7,
            },
        )
    }

    #[test]
    fn an_unexpected_cache_termination_discards_only_its_pid_and_restarts_once() {
        tauri::async_runtime::block_on(async {
            let (_root, app_paths, work, context) = work();
            let response = completed(&work, &app_paths);
            let mut transport = ScriptedTransport {
                results: VecDeque::from([
                    Err(InvocationFailure::unexpected_termination(4242)),
                    Ok(response),
                ]),
                attempts: Vec::new(),
            };

            let execution = execute(&mut transport, &app_paths, work.clone(), &context)
                .await
                .expect("the Cache completes after one restart");

            assert_eq!(execution.completion.generated_count, 1);
            assert_eq!(
                execution.recovery,
                Some(super::CacheRecovery {
                    failed_process_id: 4242,
                    removed_temporary_count: 0,
                })
            );
            assert_eq!(transport.attempts, [1, 2]);
            assert!(work.cache_paths.metadata_file().is_file());
        });
    }

    #[test]
    fn a_deterministic_cache_failure_is_not_retried() {
        tauri::async_runtime::block_on(async {
            let (_root, app_paths, work, context) = work();
            let mut transport = ScriptedTransport {
                results: VecDeque::from([Err(InvocationFailure::deterministic(
                    ImagingFailureStage::CacheProcessing,
                    4242,
                ))]),
                attempts: Vec::new(),
            };

            let failure = execute(&mut transport, &app_paths, work, &context)
                .await
                .expect_err("deterministic failures remain visible");

            assert_eq!(
                failure.stage,
                CacheFailureStage::Processor(
                    crate::imaging_processor::InvocationFailureStage::Processor(
                        ImagingFailureStage::CacheProcessing
                    )
                )
            );
            assert_eq!(transport.attempts, [1]);
        });
    }

    #[test]
    fn a_second_unexpected_termination_is_not_retried_again() {
        tauri::async_runtime::block_on(async {
            let (_root, app_paths, work, context) = work();
            let mut transport = ScriptedTransport {
                results: VecDeque::from([
                    Err(InvocationFailure::unexpected_termination(4242)),
                    Err(InvocationFailure::unexpected_termination(4343)),
                ]),
                attempts: Vec::new(),
            };

            execute(&mut transport, &app_paths, work, &context)
                .await
                .expect_err("the second crash exhausts the one-restart policy");

            assert_eq!(transport.attempts, [1, 2]);
        });
    }
}
