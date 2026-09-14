//! Owns discovery, preflight and serial generation; ProjectCore owns copied state and publication.
use crate::ipc_contract::{
    GenerationDecision, GenerationItemStatus, GenerationItemView, GenerationOptions,
    GenerationPhase, GenerationProgress, GenerationView,
};
use crate::media_runtime::{MediaBinding, MediaObservation, MediaResolver};
use myalbuns_core::{
    CreateAuthorization, ImportPhoto, ProjectCore, ProjectLocation, ProjectTemplate,
};
use myalbuns_paths::{
    ExpectedObject, MirroredDestination, OperationPathContext, PhysicalFileIdentity, ResolveError,
    RootBindingPlan,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

struct GenerationItem {
    id: String,
    name: String,
    destination: PathBuf,
    parent: PathBuf,
    photos: Vec<ImportPhoto>,
    observations: Vec<MediaObservation>,
    problems: Vec<String>,
    conflict: Option<PhysicalFileIdentity>,
    exists: bool,
    decision: Option<GenerationDecision>,
    status: GenerationItemStatus,
}

pub(crate) struct GenerationRunner {
    id: String,
    options: GenerationOptions,
    roots: RootBindingPlan,
    core: ProjectCore,
    template: ProjectTemplate,
    items: Vec<GenerationItem>,
    phase: GenerationPhase,
}

impl GenerationRunner {
    pub(crate) fn count_folders(source: &Path) -> Result<usize, String> {
        let mut paths = OperationPathContext::new();
        paths.capture(source).map_err(|error| error.to_string())?;
        Ok(discover_folders(source, &mut paths, &AtomicBool::new(false))?.len())
    }

    pub(crate) fn decide(
        &mut self,
        id: Option<&str>,
        decision: GenerationDecision,
    ) -> Result<(), String> {
        if self.phase != GenerationPhase::Prepared {
            return Err("Verifique a geração novamente.".into());
        }
        if id.is_some_and(|id| !self.items.iter().any(|item| item.id == id)) {
            return Err("Projeto não encontrado nesta geração.".into());
        }
        for item in &mut self.items {
            if id.is_some_and(|id| item.id != id) {
                continue;
            }
            if id.is_none() && !item.exists && item.problems.is_empty() {
                continue;
            }
            match decision {
                GenerationDecision::Ignore => {
                    item.decision = Some(decision);
                    item.status = GenerationItemStatus::Ignored;
                }
                GenerationDecision::Replace
                    if item.conflict.is_some() && item.problems.is_empty() =>
                {
                    item.decision = Some(decision);
                    item.status = GenerationItemStatus::Pending;
                }
                GenerationDecision::Replace => {}
            }
        }
        Ok(())
    }

    pub(crate) fn recheck(&mut self, cancellation: &AtomicBool) -> Result<(), String> {
        if self.phase != GenerationPhase::Prepared {
            return Err("Verifique a geração novamente.".into());
        }
        // New evidence always needs new overwrite consent, even when the path is unchanged.
        *self = Self::prepare(
            self.options.clone(),
            self.template.clone(),
            self.core.clone(),
            cancellation,
        )?;
        Ok(())
    }
    pub(crate) fn prepare(
        options: GenerationOptions,
        template: ProjectTemplate,
        core: ProjectCore,
        cancellation: &AtomicBool,
    ) -> Result<Self, String> {
        ensure_running(cancellation)?;
        let source = Path::new(&options.source_folder);
        let destination = Path::new(&options.destination_folder);
        let mut paths = OperationPathContext::new();
        paths.capture(source).map_err(|error| error.to_string())?;
        paths
            .capture(destination)
            .map_err(|error| error.to_string())?;
        let guard = MirroredDestination::open(&paths.current_plan(), source, destination)
            .map_err(|error| error.to_string())?;
        let folders = discover_folders(source, &mut paths, cancellation)?;
        if folders.is_empty() {
            return Err("Nenhuma pasta com fotos encontrada na origem.".into());
        }
        let roots = paths.freeze();
        let bindings = template
            .media()
            .iter()
            .map(|media| MediaBinding {
                media_id: media.id().to_string(),
                kind: media.kind(),
                logical_path: media.path().to_path_buf(),
            })
            .collect::<Vec<_>>();
        let mut items = Vec::new();
        let mut destinations = HashSet::new();
        for (folder, photos) in folders {
            ensure_running(cancellation)?;
            let name = folder
                .file_name()
                .ok_or("A pasta de origem precisa ter um nome.")?
                .to_string_lossy()
                .into_owned();
            let relative = folder
                .strip_prefix(source)
                .map_err(|_| "Pasta fora da origem.")?;
            let parent = relative
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            let output = destination.join(&parent).join(format!("{name}.myalbuns"));
            if !destinations.insert(output.to_string_lossy().to_lowercase()) {
                return Err(format!(
                    "Mais de uma pasta produziria {}. Escolha outra origem ou ajuste os nomes das pastas.",
                    output.display()
                ));
            }
            let proposal = MediaResolver.propose_media_imports_in_plan(
                myalbuns_core::MediaKind::Photo,
                photos,
                &bindings,
                &roots,
                |_| {},
            );
            let mut problems = proposal
                .problems
                .into_iter()
                .map(|problem| format!("{}: {}", problem.file_name, problem.reason))
                .collect::<Vec<_>>();
            if let Err(error) = guard.inspect_parent(&parent) {
                problems.push(error.to_string());
            }
            let mut exists = false;
            let conflict = match roots.resolve_existing(&output, ExpectedObject::RegularFile) {
                Ok(_) => {
                    exists = true;
                    match core.inspect_creation_destination(&ProjectLocation::new(
                        output.clone(),
                        roots.clone(),
                    )) {
                        Ok(identity) => identity,
                        Err(error) => {
                            problems.push(creation_error(error));
                            None
                        }
                    }
                }
                Err(ResolveError::NotFound) => None,
                Err(error) => {
                    problems.push(format!("Destino indisponível: {error}"));
                    None
                }
            };
            items.push(GenerationItem {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                destination: output,
                parent,
                photos: proposal.commands,
                observations: proposal
                    .inspections
                    .into_iter()
                    .map(|inspection| inspection.observation)
                    .collect(),
                problems,
                conflict,
                exists,
                decision: None,
                status: GenerationItemStatus::Pending,
            });
        }
        ensure_running(cancellation)?;
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            options,
            roots,
            core,
            template,
            items,
            phase: GenerationPhase::Prepared,
        })
    }

    pub(crate) fn view(&self) -> GenerationView {
        GenerationView {
            id: self.id.clone(),
            options: self.options.clone(),
            phase: self.phase,
            can_continue: self.phase == GenerationPhase::Prepared
                && self.items.iter().all(|item| {
                    item.status == GenerationItemStatus::Ignored
                        || item.problems.is_empty()
                            && (item.conflict.is_none()
                                || item.decision == Some(GenerationDecision::Replace))
                }),
            items: self
                .items
                .iter()
                .map(|item| GenerationItemView {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    destination: item.destination.to_string_lossy().into(),
                    status: item.status,
                    problems: if item.status == GenerationItemStatus::Ignored
                        && item.exists
                        && item.problems.is_empty()
                    {
                        vec!["Já existe um Projeto no destino.".into()]
                    } else {
                        item.problems.clone()
                    },
                    conflict: item.exists,
                    can_replace: item.conflict.is_some() && item.problems.is_empty(),
                    decision: item.decision,
                })
                .collect(),
        }
    }

    pub(crate) fn run(&mut self, cancellation: &AtomicBool, progress: &dyn Fn(GenerationProgress)) {
        if !self.view().can_continue {
            return;
        }
        self.phase = GenerationPhase::Running;
        let total = self.items.len() as u32;
        progress(GenerationProgress {
            completed: 0,
            total: Some(total),
        });
        for (index, item) in self.items.iter_mut().enumerate() {
            if cancellation.load(Ordering::Acquire) {
                self.phase = GenerationPhase::Cancelled;
                return;
            }
            if item.status == GenerationItemStatus::Pending {
                match generate_item(&self.core, &self.template, &self.roots, &self.options, item) {
                    Ok(()) => item.status = GenerationItemStatus::Completed,
                    Err(error) => {
                        item.status = GenerationItemStatus::Failed;
                        item.problems.push(error);
                    }
                }
            }
            progress(GenerationProgress {
                completed: index as u32 + 1,
                total: Some(total),
            });
        }
        self.phase = GenerationPhase::Finished;
    }
}

fn generate_item(
    core: &ProjectCore,
    template: &ProjectTemplate,
    roots: &RootBindingPlan,
    options: &GenerationOptions,
    item: &mut GenerationItem,
) -> Result<(), String> {
    for previous in &item.observations {
        let binding = MediaBinding {
            media_id: previous.media_id.clone(),
            kind: previous.kind,
            logical_path: previous.logical_path().to_path_buf(),
        };
        let current = MediaResolver.observe_in_plan(roots, &binding);
        if !previous.same_source(&current) {
            return Err(format!(
                "A foto {} mudou ou ficou indisponível. Verifique novamente.",
                previous.logical_path().display()
            ));
        }
    }
    let destination = MirroredDestination::open(
        roots,
        Path::new(&options.source_folder),
        Path::new(&options.destination_folder),
    )
    .map_err(|error| error.to_string())?;
    let _parent = destination
        .prepare_parent(&item.parent)
        .map_err(|error| error.to_string())?;
    let authorization = item.conflict.map_or(
        CreateAuthorization::CreateOnly,
        CreateAuthorization::ReplaceTargetConfirmed,
    );
    core.create_from_template(
        template,
        ProjectLocation::new(item.destination.clone(), roots.clone()),
        authorization,
        item.photos.clone(),
    )
    .map_err(creation_error)?;
    Ok(())
}

fn creation_error(error: myalbuns_core::CreateProjectError) -> String {
    use myalbuns_core::CreateProjectError as E;
    match error {
        E::ProjectInUse => {
            "O Projeto está aberto. Feche-o e verifique novamente, ou ignore este item."
        }
        E::DestinationConflict => "O destino mudou após a verificação. Verifique novamente.",
        E::IdentityIndeterminate => {
            "Não foi possível confirmar se o Projeto está em uso. Verifique novamente."
        }
        _ => "Não foi possível gravar o Projeto. Verifique o acesso à pasta de destino.",
    }
    .into()
}

fn discover_folders(
    root: &Path,
    paths: &mut OperationPathContext,
    cancellation: &AtomicBool,
) -> Result<Vec<(PathBuf, Vec<PathBuf>)>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut seen = HashSet::new();
    let mut folders = Vec::new();
    while let Some(folder) = pending.pop() {
        ensure_running(cancellation)?;
        let directory = paths
            .resolve_existing(&folder, ExpectedObject::Directory)
            .map_err(|error| format!("Não foi possível ler {}: {error}", folder.display()))?;
        let identity = directory
            .physical_identity()
            .ok_or("Não foi possível confirmar a identidade da pasta de origem.")?;
        if !seen.insert(identity.to_local_token()) {
            continue;
        }
        let entries =
            std::fs::read_dir(directory.operational_path()).map_err(|error| error.to_string())?;
        let mut photos = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = folder.join(entry.file_name());
            let metadata = std::fs::metadata(entry.path()).map_err(|error| {
                format!("Não foi possível verificar {}: {error}", path.display())
            })?;
            if metadata.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    ["jpg", "jpeg", "png", "tif", "tiff"]
                        .iter()
                        .any(|accepted| extension.eq_ignore_ascii_case(accepted))
                })
            {
                paths.capture(&path).map_err(|error| error.to_string())?;
                photos.push(path);
            }
        }
        photos.sort();
        if !photos.is_empty() {
            folders.push((folder, photos));
        }
    }
    folders.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(folders)
}

fn ensure_running(cancellation: &AtomicBool) -> Result<(), String> {
    if cancellation.load(Ordering::Acquire) {
        Err("A geração foi cancelada.".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
