//! Owns discovery, preflight and bounded generation; ProjectCore owns copied state and publication.
use crate::ipc_contract::{
    GenerationDecision, GenerationItemStatus, GenerationItemView, GenerationOptions,
    GenerationPhase, GenerationProgress, GenerationView,
};
use myalbuns_core::{CreateAuthorization, ProjectCore, ProjectLocation, ProjectTemplate};
use myalbuns_paths::{
    ExpectedObject, MirroredDestination, OperationPathContext, PhysicalFileIdentity, ResolveError,
    RootBindingPlan,
};
use std::{
    collections::HashSet,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const PROJECT_GENERATION_CONCURRENCY: usize = 4;

#[derive(Debug)]
pub(crate) enum GenerationPreparationError {
    Cancelled,
    Failed(String),
}
impl From<String> for GenerationPreparationError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}
impl From<&str> for GenerationPreparationError {
    fn from(message: &str) -> Self {
        Self::Failed(message.into())
    }
}
impl fmt::Display for GenerationPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("A geração foi cancelada."),
            Self::Failed(message) => formatter.write_str(message),
        }
    }
}

struct GenerationItem {
    id: String,
    name: String,
    destination: PathBuf,
    parent: PathBuf,
    photo_paths: Vec<PathBuf>,
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
        discover_folders(source, &mut paths, &AtomicBool::new(false))
            .map(|folders| folders.len())
            .map_err(|error| error.to_string())
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

    pub(crate) fn recheck(
        &mut self,
        cancellation: &AtomicBool,
    ) -> Result<(), GenerationPreparationError> {
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
    ) -> Result<Self, GenerationPreparationError> {
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
                ).into());
            }
            let mut problems = Vec::new();
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
                photo_paths: photos,
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
        self.phase = generate_items(
            &mut self.items,
            PROJECT_GENERATION_CONCURRENCY,
            cancellation,
            progress,
            |item| generate_item(&self.core, &self.template, &self.roots, &self.options, item),
        );
    }
}

/// Workers own separate items; only this coordinator emits ordered progress.
/// Scope completion joins every active publication before the result can be shown.
fn generate_items(
    items: &mut [GenerationItem],
    concurrency: usize,
    cancellation: &AtomicBool,
    progress: &dyn Fn(GenerationProgress),
    generate: impl Fn(&GenerationItem) -> Result<(), String> + Sync,
) -> GenerationPhase {
    let total = items.len() as u32;
    let pending = items
        .iter()
        .filter(|item| item.status == GenerationItemStatus::Pending)
        .count();
    let mut completed = total - pending as u32;
    let report = |completed| {
        progress(GenerationProgress {
            completed,
            total: Some(total),
        })
    };
    report(0);
    if completed > 0 {
        report(completed);
    }
    let candidates = Mutex::new(
        items
            .iter_mut()
            .filter(|item| item.status == GenerationItemStatus::Pending),
    );
    let process_next = || {
        let item = {
            let mut candidates = candidates.lock().expect("the generation queue is healthy");
            // Claiming an item admits it to publication. Cancellation stops new claims,
            // while already claimed items retain their destination guards until finished.
            if cancellation.load(Ordering::Acquire) {
                return false;
            }
            candidates.next()
        };
        let Some(item) = item else {
            return false;
        };
        match generate(item) {
            Ok(()) => item.status = GenerationItemStatus::Completed,
            Err(error) => {
                item.status = GenerationItemStatus::Failed;
                item.problems.push(error);
            }
        }
        true
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let mut workers = 0;
        for index in 0..pending.min(concurrency.clamp(1, PROJECT_GENERATION_CONCURRENCY)) {
            let sender = sender.clone();
            let process_next = &process_next;
            match std::thread::Builder::new()
                .name(format!("project-generation-{index}"))
                .spawn_scoped(scope, move || {
                    while process_next() {
                        if sender.send(()).is_err() {
                            break;
                        }
                    }
                }) {
                Ok(_) => workers += 1,
                Err(error) => {
                    tracing::warn!("Could not start a project generation worker: {error}");
                    break;
                }
            }
        }
        drop(sender);
        if workers == 0 {
            // The operation already runs off the UI thread; keep working serially
            // if the OS cannot create another thread, with the same cancellation rule.
            while process_next() {
                completed += 1;
                report(completed);
            }
        } else {
            for () in receiver {
                completed += 1;
                report(completed);
            }
        }
    });
    if completed < total {
        GenerationPhase::Cancelled
    } else {
        GenerationPhase::Finished
    }
}

fn generate_item(
    core: &ProjectCore,
    template: &ProjectTemplate,
    roots: &RootBindingPlan,
    options: &GenerationOptions,
    item: &GenerationItem,
) -> Result<(), String> {
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
        item.photo_paths.clone(),
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
) -> Result<Vec<(PathBuf, Vec<PathBuf>)>, GenerationPreparationError> {
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

fn ensure_running(cancellation: &AtomicBool) -> Result<(), GenerationPreparationError> {
    if cancellation.load(Ordering::Acquire) {
        Err(GenerationPreparationError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
