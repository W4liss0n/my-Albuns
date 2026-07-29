use std::future::Future;

use myalbuns_imaging_protocol::IMAGING_PROTOCOL_VERSION;
use myalbuns_logging::ProcessRole;
use myalbuns_paths::{AppPaths, CachePathPlan};
use tauri::AppHandle;

use crate::{
    imaging_processor::{ImagingOperation, InvocationContext, InvocationFailure, invoke_once},
    logging::LoggingState,
};

pub(crate) async fn execute(
    app: &AppHandle,
    logging: &LoggingState,
    app_paths: &AppPaths,
    cache_paths: &CachePathPlan,
    payload: &[u8],
    context: InvocationContext<'_>,
) -> Result<Vec<u8>, InvocationFailure> {
    execute_with(
        |attempt| {
            invoke_once(
                app,
                logging,
                payload,
                context,
                ImagingOperation::Cache,
                attempt,
            )
        },
        |process_id| {
            app_paths
                .discard_project_cache_temporaries(cache_paths, process_id)
                .map_err(|error| error.to_string())
        },
        context,
    )
    .await
}

async fn execute_with<Invoke, Invocation, Cleanup>(
    mut invoke: Invoke,
    mut cleanup: Cleanup,
    context: InvocationContext<'_>,
) -> Result<Vec<u8>, InvocationFailure>
where
    Invoke: FnMut(u8) -> Invocation,
    Invocation: Future<Output = Result<Vec<u8>, InvocationFailure>>,
    Cleanup: FnMut(u32) -> Result<usize, String>,
{
    let mut attempt = 1_u8;
    loop {
        match invoke(attempt).await {
            Ok(stdout) => {
                if attempt > 1 {
                    tracing::info!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id,
                        project_id = context.project_id,
                        attempts = attempt,
                        event = "imaging_processor_restart_completed",
                    );
                }
                return Ok(stdout);
            }
            Err(failure) if failure.is_unexpected_termination() => {
                let Some(failed_process_id) = failure.process_id else {
                    return Err(failure);
                };
                let removed_temporary_count = cleanup(failed_process_id).map_err(|error| {
                    InvocationFailure::cache_recovery_cleanup(
                        &failure,
                        format!("Não foi possível descartar o item incompleto do Cache: {error}"),
                    )
                })?;
                if attempt == 1 {
                    tracing::warn!(
                        target: "myalbuns.desktop",
                        process_role = ProcessRole::DesktopHost.as_str(),
                        protocol_version = IMAGING_PROTOCOL_VERSION,
                        operation_id = context.operation_id,
                        project_id = context.project_id,
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
                        operation_id = context.operation_id,
                        project_id = context.project_id,
                        attempts = attempt,
                        exit_code = failure.exit_code,
                        event = "imaging_processor_restart_exhausted",
                    );
                    return Err(failure);
                }
            }
            Err(failure) => {
                return Err(failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, future::ready};

    use myalbuns_imaging_protocol::ImagingFailureStage;

    use super::execute_with;
    use crate::imaging_processor::{InvocationContext, InvocationFailure};

    const CONTEXT: InvocationContext<'static> = InvocationContext {
        operation_id: "cache-test",
        project_id: Some("project-test"),
    };

    #[test]
    fn an_unexpected_cache_termination_discards_only_its_pid_and_restarts_once() {
        tauri::async_runtime::block_on(async {
            let mut results = VecDeque::from([
                Err(InvocationFailure::unexpected_termination(4242)),
                Ok(b"completed".to_vec()),
            ]);
            let mut attempts = Vec::new();
            let mut cleaned_processes = Vec::new();

            let output = execute_with(
                |attempt| {
                    attempts.push(attempt);
                    ready(results.pop_front().expect("one result per attempt"))
                },
                |process_id| {
                    cleaned_processes.push(process_id);
                    Ok(1)
                },
                CONTEXT,
            )
            .await
            .expect("the relevant Cache request completes after one restart");

            assert_eq!(output, b"completed");
            assert_eq!(attempts, [1, 2]);
            assert_eq!(cleaned_processes, [4242]);
        });
    }

    #[test]
    fn a_deterministic_cache_failure_is_not_retried() {
        tauri::async_runtime::block_on(async {
            let mut attempts = Vec::new();
            let mut cleaned_processes = Vec::new();

            let failure = execute_with(
                |attempt| {
                    attempts.push(attempt);
                    ready(Err(InvocationFailure::deterministic(
                        ImagingFailureStage::CacheProcessing,
                        4242,
                    )))
                },
                |process_id| {
                    cleaned_processes.push(process_id);
                    Ok(0)
                },
                CONTEXT,
            )
            .await
            .expect_err("deterministic failures remain visible");

            assert!(!failure.is_unexpected_termination());
            assert_eq!(attempts, [1]);
            assert!(cleaned_processes.is_empty());
        });
    }

    #[test]
    fn a_second_unexpected_termination_is_not_retried_again() {
        tauri::async_runtime::block_on(async {
            let mut results = VecDeque::from([
                Err(InvocationFailure::unexpected_termination(4242)),
                Err(InvocationFailure::unexpected_termination(4343)),
            ]);
            let mut attempts = Vec::new();
            let mut cleaned_processes = Vec::new();

            execute_with(
                |attempt| {
                    attempts.push(attempt);
                    ready(results.pop_front().expect("one result per attempt"))
                },
                |process_id| {
                    cleaned_processes.push(process_id);
                    Ok(1)
                },
                CONTEXT,
            )
            .await
            .expect_err("the second crash exhausts the one-restart policy");

            assert_eq!(attempts, [1, 2]);
            assert_eq!(cleaned_processes, [4242, 4343]);
        });
    }
}
