use std::path::{Path, PathBuf};

use myalbuns_core::{LoadProjectRequest, MediaId, ProjectCore, ProjectLocation, ProjectTemplate};
use myalbuns_logging::{ProcessRole, init_local_logging};
use myalbuns_paths::{AppPaths, OperationPathContext, RootBindingPlan};
use serde::Deserialize;

use super::{
    CANCEL, ConflictChoice, Event, ItemReport, ItemStatus, Outcome, ProblemReport, Question,
    REPLACE, RETRY, SKIP, Session, Stop, project_core,
};
use crate::{
    generation_runner::{GenerationPreparationError, GenerationRunner},
    ipc_contract::{
        GenerationDecision, GenerationItemStatus, GenerationItemView, GenerationOptions,
        GenerationPhase, GenerationView,
    },
    media_by_structure::find_media_by_structure,
    media_runtime::MediaBinding,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GenerationJob {
    #[allow(dead_code)]
    version: u32,
    /// Saved project whose pages, layouts and settings every new project copies.
    model: PathBuf,
    source_folder: String,
    destination_folder: String,
    #[serde(default)]
    on_existing: ConflictChoice,
}

pub(super) fn run(job: GenerationJob, paths: &AppPaths, session: &Session, dry_run: bool) -> i32 {
    let _logging = init_local_logging(&paths.logs_dir(), ProcessRole::Global).ok();
    run_with_core(job, project_core(paths), session, dry_run)
}

pub(super) fn run_with_core(
    job: GenerationJob,
    core: ProjectCore,
    session: &Session,
    dry_run: bool,
) -> i32 {
    let template = match load_template(&core, &job.model) {
        Ok(template) => template,
        Err(message) => return session.fail("model-unavailable", message),
    };
    let options = GenerationOptions {
        source_folder: job.source_folder,
        destination_folder: job.destination_folder,
    };
    let cancellation = session.cancellation().flag();
    let mut runner = match GenerationRunner::prepare(options, template, core, cancellation) {
        Ok(runner) => runner,
        Err(GenerationPreparationError::Cancelled) => {
            return session.finish(Outcome::Cancelled, Vec::new(), None);
        }
        Err(GenerationPreparationError::Failed(message)) => {
            return session.fail("preparation-failed", message);
        }
    };
    session.emit(&checked(&runner.view()));
    if dry_run {
        return session.finish(Outcome::Checked, reports(&runner.view()), None);
    }
    if let Err(stop) = resolve(&mut runner, job.on_existing, session) {
        return session.stop(stop, reports(&runner.view()));
    }
    if !runner.view().can_continue {
        return session.fail(
            "blocked",
            "Resolva ou ignore os projetos com problemas antes de gerar.",
        );
    }
    runner.run(cancellation, &|progress| {
        session.emit(&Event::Progress {
            completed: progress.completed,
            total: progress.total,
            percent: None,
            current: None,
        });
    });
    let view = runner.view();
    let outcome = if view.phase == GenerationPhase::Cancelled {
        Outcome::Cancelled
    } else if view
        .items
        .iter()
        .all(|item| item.status == GenerationItemStatus::Completed)
    {
        Outcome::Completed
    } else {
        Outcome::Partial
    };
    session.finish(outcome, reports(&view), None)
}

/// The model is read as saved; changes still open in an editor are not included.
fn load_template(core: &ProjectCore, model: &Path) -> Result<ProjectTemplate, String> {
    const UNAVAILABLE: &str = "Não foi possível abrir o projeto modelo. Confira se o arquivo existe e é um projeto do myAlbuns.";
    let mut paths = OperationPathContext::new();
    paths.capture(model).map_err(|error| {
        tracing::warn!(target: "myalbuns.desktop", %error, event = "automation_model_unavailable");
        UNAVAILABLE
    })?;
    let paths = paths.freeze();
    let template = core
        .load_persisted_revision(LoadProjectRequest::new(ProjectLocation::new(
            model.to_path_buf(),
            paths.clone(),
        )))
        .map(|loaded| loaded.freeze_template())
        .map_err(|error| {
            tracing::warn!(target: "myalbuns.desktop", ?error, event = "automation_model_unavailable");
            UNAVAILABLE
        })?;
    Ok(rebind_template_by_structure(template, model, &paths))
}

/// The generated projects link the model's images where the model's folder
/// structure finds them (ADR 0016); the model file itself is not changed.
fn rebind_template_by_structure(
    template: ProjectTemplate,
    model: &Path,
    paths: &RootBindingPlan,
) -> ProjectTemplate {
    let bindings = template
        .media()
        .iter()
        .map(|media| MediaBinding {
            media_id: media.id().to_string(),
            kind: media.kind(),
            logical_path: media.path().to_path_buf(),
        })
        .collect::<Vec<_>>();
    let changes = find_media_by_structure(paths, model, &bindings)
        .into_iter()
        .filter_map(|found| Some((found.media_id.parse::<MediaId>().ok()?, found.found)))
        .collect::<Vec<_>>();
    if changes.is_empty() {
        return template;
    }
    template.with_rebound_media(&changes).unwrap_or(template)
}

fn awaits_replacement(item: &GenerationItemView) -> bool {
    item.status == GenerationItemStatus::Pending
        && item.can_replace
        && item.decision != Some(GenerationDecision::Replace)
}

fn needs_decision(item: &GenerationItemView) -> bool {
    item.status == GenerationItemStatus::Pending
        && (!item.problems.is_empty()
            || item.conflict && item.decision != Some(GenerationDecision::Replace))
}

fn resolve(
    runner: &mut GenerationRunner,
    mut existing: ConflictChoice,
    session: &Session,
) -> Result<(), Stop> {
    let mut skip_blocked = !session.interactive();
    if !session.interactive() && existing == ConflictChoice::Ask {
        existing = ConflictChoice::Skip;
    }
    loop {
        let view = runner.view();
        let waiting = view
            .items
            .iter()
            .filter(|item| needs_decision(item))
            .collect::<Vec<_>>();
        let Some(item) = waiting.first() else {
            return Ok(());
        };
        let remaining = waiting
            .iter()
            .filter(|other| other.can_replace == item.can_replace)
            .count()
            - 1;
        if item.can_replace {
            let (replace, all) = match existing {
                ConflictChoice::Replace => (true, false),
                ConflictChoice::Skip => (false, false),
                ConflictChoice::Ask => {
                    let answer = session.ask(
                        Question::new(
                            "destination-exists",
                            format!(
                                "Já existe um projeto em {}. Se você substituir, o projeto existente será perdido.",
                                item.destination
                            ),
                            &[REPLACE, SKIP, CANCEL],
                        )
                        .about(&item.id, &item.name, remaining),
                    )?;
                    match answer.option.as_str() {
                        "replace" => (true, answer.apply_to_all),
                        "skip" => (false, answer.apply_to_all),
                        _ => return Err(Stop::Cancelled),
                    }
                }
            };
            let decision = if replace {
                GenerationDecision::Replace
            } else {
                GenerationDecision::Ignore
            };
            runner.decide(Some(&item.id), decision)?;
            if all {
                existing = if replace {
                    ConflictChoice::Replace
                } else {
                    ConflictChoice::Skip
                };
            }
            continue;
        }
        if skip_blocked {
            runner.decide(Some(&item.id), GenerationDecision::Ignore)?;
            continue;
        }
        let message = if item.problems.is_empty() {
            "Já existe um projeto no destino.".to_owned()
        } else {
            item.problems.join(" ")
        };
        let answer = session.ask(
            Question::new("project-blocked", message, &[RETRY, SKIP, CANCEL])
                .about(&item.id, &item.name, remaining),
        )?;
        match answer.option.as_str() {
            "retry" => {
                // A new verification replaces every item and asks for consent again.
                runner
                    .recheck(session.cancellation().flag())
                    .map_err(|error| match error {
                        GenerationPreparationError::Cancelled => Stop::Cancelled,
                        GenerationPreparationError::Failed(message) => Stop::Failed(message),
                    })?;
                session.emit(&checked(&runner.view()));
            }
            "skip" => {
                runner.decide(Some(&item.id), GenerationDecision::Ignore)?;
                skip_blocked = answer.apply_to_all;
            }
            _ => return Err(Stop::Cancelled),
        }
    }
}

fn checked(view: &GenerationView) -> Event {
    Event::Checked {
        items: reports(view),
        can_continue: view.can_continue,
        has_conflicts: view.items.iter().any(awaits_replacement),
    }
}

fn reports(view: &GenerationView) -> Vec<ItemReport> {
    view.items
        .iter()
        .map(|item| {
            let code = if item.status == GenerationItemStatus::Failed {
                "failed"
            } else if item.can_replace {
                "destination-exists"
            } else {
                "blocked"
            };
            ItemReport {
                id: item.id.clone(),
                name: item.name.clone(),
                project_path: None,
                destination: item.destination.clone(),
                status: match item.status {
                    GenerationItemStatus::Pending => ItemStatus::Pending,
                    GenerationItemStatus::Completed => ItemStatus::Completed,
                    GenerationItemStatus::Ignored => ItemStatus::Skipped,
                    GenerationItemStatus::Failed => ItemStatus::Failed,
                },
                // The view names an existing destination only after it is ignored.
                problems: awaits_replacement(item)
                    .then(|| "Já existe um projeto no destino.".to_owned())
                    .iter()
                    .chain(&item.problems)
                    .map(|message| ProblemReport {
                        code,
                        message: message.clone(),
                        file_name: None,
                    })
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::load_template;

    #[test]
    fn a_copied_model_links_the_images_found_beside_it_without_changing_the_model() {
        let root = tempfile::tempdir().unwrap();
        let job = root.path().join("Copia").join("Job");
        let model = job.join("Modelo").join("Modelo.myalbuns");
        let original = root
            .path()
            .join("Servidor")
            .join("Job")
            .join("Arte")
            .join("001.jpg");
        let copy = job.join("Arte").join("001.jpg");
        let (core, project) =
            crate::batch_runner::tests::background_project(root.path(), &model, &original);
        drop(project);
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::rename(&original, &copy).unwrap();
        let bytes = std::fs::read(&model).unwrap();

        let template = load_template(&core, &model).unwrap();

        assert_eq!(template.media()[0].path(), copy);
        assert_eq!(std::fs::read(&model).unwrap(), bytes);
    }
}
