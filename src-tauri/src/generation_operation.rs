//! Coordinates a generation attempt and the lifetime of its progress presentation.
use crate::{
    generation_runner::GenerationRunner,
    ipc_contract::{GenerationOptions, GenerationProgress, GenerationView},
};
use myalbuns_core::{ProjectCore, ProjectTemplate};
use std::{
    future::Future,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::sync::Mutex;

pub(crate) enum GenerationRequest {
    Prepare {
        options: GenerationOptions,
        template: Box<ProjectTemplate>,
        core: ProjectCore,
    },
    Recheck,
    Run,
}

pub(crate) trait GenerationPresentation: Clone + Send + Sync + 'static {
    type Surface: Send;
    fn open(
        &self,
        progress: GenerationProgress,
    ) -> impl Future<Output = Result<Self::Surface, String>> + Send;
    fn progress(&self, progress: GenerationProgress);
    fn finish(&self, view: &GenerationView) -> impl Future<Output = Result<(), String>> + Send;
}

pub(crate) async fn execute_generation(
    request: GenerationRequest,
    runner: Arc<Mutex<Option<GenerationRunner>>>,
    cancellation: Arc<AtomicBool>,
    presentation: impl GenerationPresentation,
) -> Result<GenerationView, String> {
    let mut runner = runner.lock_owned().await;
    let running = matches!(request, GenerationRequest::Run);
    let initial_progress = if running {
        let initial = runner
            .as_ref()
            .ok_or("Verifique as pastas primeiro.")?
            .view();
        if !initial.can_continue {
            return Err("Resolva ou ignore os problemas antes de gerar.".into());
        }
        GenerationProgress {
            completed: 0,
            total: Some(initial.items.len() as u32),
        }
    } else {
        GenerationProgress {
            completed: 0,
            total: None,
        }
    };
    let _surface = presentation.open(initial_progress).await?;
    let generate_when_ready = matches!(request, GenerationRequest::Prepare { .. });
    let updates = presentation.clone();
    let view = tokio::task::spawn_blocking(move || {
        match request {
            GenerationRequest::Prepare {
                options,
                template,
                core,
            } => {
                *runner = Some(GenerationRunner::prepare(
                    options,
                    *template,
                    core,
                    &cancellation,
                )?);
            }
            GenerationRequest::Recheck => runner
                .as_mut()
                .ok_or("Verifique as pastas primeiro.")?
                .recheck(&cancellation)?,
            GenerationRequest::Run => runner
                .as_mut()
                .ok_or("Verifique as pastas primeiro.")?
                .run(&cancellation, &|progress| updates.progress(progress)),
        }
        if generate_when_ready {
            let prepared = runner.as_ref().ok_or("Geração indisponível.")?.view();
            if prepared.can_continue
                && prepared
                    .items
                    .iter()
                    .all(|item| !item.conflict && item.problems.is_empty())
            {
                runner
                    .as_mut()
                    .ok_or("Geração indisponível.")?
                    .run(&cancellation, &|progress| updates.progress(progress));
            }
        }
        Ok::<_, String>(runner.as_ref().ok_or("Geração indisponível.")?.view())
    })
    .await
    .map_err(|error| error.to_string())??;
    presentation.finish(&view).await?;
    Ok(view)
}

#[cfg(test)]
mod tests;
