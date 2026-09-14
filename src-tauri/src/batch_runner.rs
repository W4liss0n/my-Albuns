//! Owns persisted-project discovery, preflight, serial execution and recovery.
//! Rendering and publication belong to the existing ExportPipeline.
mod checkpoint;
mod execution;
mod relink;
pub(crate) use execution::BatchCancellation;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use myalbuns_core::{
    ExportFormat, ExportMode, LoadProjectError, LoadProjectRequest, LoadedProjectRevision, MediaId,
    ProjectCore, ProjectLocation, project_name_from_path,
};
use myalbuns_imaging_protocol::RenderSource;
use myalbuns_paths::{
    ExpectedObject, ExportWriteAuthorization, OperationPathContext, RootBindingPlan,
};

use crate::{
    export_pipeline::{self, AlbumExportOptions, AlbumExportPlan},
    ipc_contract::{
        BatchExportOptions, BatchExportView, BatchItemStatus, BatchItemView, BatchPhase,
        BatchProblem, BatchProblemKind, ExportConflictPolicy,
    },
    media_runtime::{MediaAvailability, MediaBinding, MediaResolver},
};

#[derive(Clone, Debug)]
pub(crate) struct BatchConfiguration {
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub format: ExportFormat,
    pub mode: ExportMode,
}

#[derive(Clone, Debug)]
struct TemporaryRelink {
    original: PathBuf,
    replacement: PathBuf,
}

#[derive(Default, Debug)]
struct ItemRelinks {
    individual: HashMap<String, TemporaryRelink>,
    global: HashMap<String, TemporaryRelink>,
}

#[derive(Debug)]
struct BatchItem {
    id: String,
    path: PathBuf,
    name: String,
    destination: PathBuf,
    status: BatchItemStatus,
    problems: Vec<BatchProblem>,
    revision: Option<u64>,
    digest: Option<String>,
    relinks: ItemRelinks,
    has_conflicts: bool,
    preparation: Option<checkpoint::InterruptedPreparation>,
}

pub(crate) struct BatchRunner {
    id: String,
    configuration: BatchConfiguration,
    core: ProjectCore,
    checkpoint_root: PathBuf,
    paths: OperationPathContext,
    items: Vec<BatchItem>,
    phase: BatchPhase,
    partial_publication: bool,
    storage_volume: Option<myalbuns_paths::StorageVolume>,
    current: Option<String>,
}

impl BatchRunner {
    pub(crate) fn storage_volume(&self) -> Option<myalbuns_paths::StorageVolume> {
        self.storage_volume.clone()
    }
    pub(crate) fn count_projects(source: &Path) -> Result<usize, String> {
        let mut paths = OperationPathContext::new();
        paths.capture(source).map_err(|error| error.to_string())?;
        Ok(discover_projects(source, &mut paths)?.len())
    }

    pub(crate) fn project_path(&self, id: &str) -> Result<PathBuf, String> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.path.clone())
            .ok_or_else(|| "O Projeto não pertence ao lote.".into())
    }
    pub(crate) fn discover(
        mut configuration: BatchConfiguration,
        core: ProjectCore,
        checkpoint_root: PathBuf,
    ) -> Result<Self, String> {
        if matches!(configuration.format, ExportFormat::Jpeg { .. }) {
            configuration.format = ExportFormat::Jpeg { quality: 100 };
        }
        configuration.format.validate()?;
        let mut paths = OperationPathContext::new();
        paths
            .capture(&configuration.source)
            .map_err(|error| error.to_string())?;
        if let Some(destination) = &configuration.destination {
            paths
                .capture(destination)
                .map_err(|error| error.to_string())?;
        }
        let files = discover_projects(&configuration.source, &mut paths)?;
        if files.is_empty() {
            return Err("Nenhum Projeto encontrado nessa pasta.".into());
        }
        let items = files
            .into_iter()
            .map(|path| {
                let name = project_name_from_path(&path);
                let parent = path.parent().expect("discovered files have a parent");
                let destination = configuration
                    .destination
                    .as_ref()
                    .map_or_else(
                        || parent.to_path_buf(),
                        |root| {
                            root.join(
                                parent
                                    .strip_prefix(&configuration.source)
                                    .expect("discovery preserves logical source ancestry"),
                            )
                        },
                    )
                    .join(&name);
                BatchItem {
                    id: uuid::Uuid::new_v4().to_string(),
                    path,
                    name,
                    destination,
                    status: BatchItemStatus::Pending,
                    problems: vec![],
                    revision: None,
                    digest: None,
                    relinks: ItemRelinks::default(),
                    has_conflicts: false,
                    preparation: None,
                }
            })
            .collect();
        let mut batch = Self {
            id: uuid::Uuid::new_v4().to_string(),
            configuration,
            core,
            checkpoint_root,
            paths,
            items,
            phase: BatchPhase::Prepared,
            partial_publication: false,
            storage_volume: None,
            current: None,
        };
        batch.recheck();
        Ok(batch)
    }

    pub(crate) fn view(&self) -> BatchExportView {
        BatchExportView {
            id: self.id.clone(),
            options: BatchExportOptions {
                source_folder: self.configuration.source.to_string_lossy().into_owned(),
                destination_folder: self
                    .configuration
                    .destination
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                format: self.configuration.format.clone(),
                mode: self.configuration.mode,
            },
            phase: self.phase,
            partial_publication: self.partial_publication,
            has_conflicts: self
                .items
                .iter()
                .any(|item| item.status == BatchItemStatus::Pending && item.has_conflicts),
            can_continue: self.phase == BatchPhase::Prepared
                && !self.items.is_empty()
                && self.items.iter().all(|item| {
                    item.status != BatchItemStatus::Pending || item.problems.is_empty()
                }),
            items: self
                .items
                .iter()
                .map(|item| BatchItemView {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    project_path: item.path.to_string_lossy().into_owned(),
                    destination: item.destination.to_string_lossy().into_owned(),
                    status: item.status,
                    problems: item.problems.clone(),
                })
                .collect(),
        }
    }

    pub(crate) fn retry_preflight(&mut self) {
        self.current = None;
        self.partial_publication = false;
        self.storage_volume = None;
        self.paths = OperationPathContext::new();
        self.recheck();
    }

    pub(crate) fn recheck(&mut self) {
        self.phase = BatchPhase::Prepared;
        for item in &mut self.items {
            if let Some(preparation) = &item.preparation {
                let _ = self.paths.capture(preparation.path.as_path());
            }
            if matches!(
                item.status,
                BatchItemStatus::Completed | BatchItemStatus::Ignored
            ) {
                continue;
            }
            item.status = BatchItemStatus::Pending;
            item.problems.clear();
            item.has_conflicts = false;
            match load_item(&self.core, &mut self.paths, item) {
                Ok(loaded) => {
                    item.digest = Some(loaded.content_sha256().into());
                    item.revision = Some(loaded.revision());
                    let mut roots = loaded
                        .project()
                        .media()
                        .iter()
                        .map(|media| media.path().to_path_buf())
                        .collect::<Vec<_>>();
                    roots.push(item.destination.clone());
                    if let Some(preparation) = &item.preparation {
                        roots.push(preparation.path.clone().into_path_buf());
                    }
                    roots.extend(
                        item.relinks
                            .individual
                            .values()
                            .chain(item.relinks.global.values())
                            .map(|relink| relink.replacement.clone()),
                    );
                    for path in roots {
                        if let Err(error) = self.paths.capture(&path) {
                            item.problems
                                .push(problem(BatchProblemKind::Unavailable, error.to_string()));
                        }
                    }
                    if !item.problems.is_empty() {
                        continue;
                    }
                    match inspect_and_plan(
                        &loaded,
                        item,
                        &self.configuration,
                        &self.paths.current_plan(),
                        ExportConflictPolicy::Ask,
                        "batch-preflight",
                    ) {
                        Ok(plan) => match plan.conflicts() {
                            Ok(conflicts) => item.has_conflicts = !conflicts.is_empty(),
                            Err(error) => item
                                .problems
                                .push(problem(BatchProblemKind::Unavailable, error.message)),
                        },
                        Err(problems) => item.problems = problems,
                    }
                }
                Err(error) => item.problems.push(error),
            }
        }
    }
}

fn problem(kind: BatchProblemKind, message: impl Into<String>) -> BatchProblem {
    BatchProblem {
        kind,
        message: message.into(),
        media_id: None,
    }
}

fn load_item(
    core: &ProjectCore,
    paths: &mut OperationPathContext,
    item: &BatchItem,
) -> Result<LoadedProjectRevision, BatchProblem> {
    paths.capture(&item.path).map_err(|error| {
        problem(
            BatchProblemKind::Unavailable,
            format!("Projeto indisponível: {error}"),
        )
    })?;
    load_in_plan(core, &paths.current_plan(), item)
}

fn load_in_plan(
    core: &ProjectCore,
    paths: &RootBindingPlan,
    item: &BatchItem,
) -> Result<LoadedProjectRevision, BatchProblem> {
    core.load_persisted_revision(LoadProjectRequest::new(ProjectLocation::new(
        item.path.clone(),
        paths.clone(),
    )))
    .map_err(|error| match error {
        LoadProjectError::Path(_) => problem(
            BatchProblemKind::Unavailable,
            "Não foi possível acessar o Projeto. Verifique a pasta e tente novamente.",
        ),
        LoadProjectError::ExternalCopyRequiresInteractiveResolution
        | LoadProjectError::IdentityIndeterminate => problem(
            BatchProblemKind::InvalidProject,
            "Abra o Projeto para resolver sua identificação e salve antes de verificar novamente.",
        ),
        LoadProjectError::Document(_) => problem(
            BatchProblemKind::InvalidProject,
            "O arquivo não é um Projeto válido ou usa uma versão incompatível.",
        ),
    })
}

fn inspect_and_plan(
    loaded: &LoadedProjectRevision,
    item: &BatchItem,
    configuration: &BatchConfiguration,
    paths: &RootBindingPlan,
    policy: ExportConflictPolicy,
    request_id: &str,
) -> Result<AlbumExportPlan, Vec<BatchProblem>> {
    let frozen = loaded.freeze_rendering();
    let sheet_ids = frozen
        .render_snapshot()
        .composition
        .sheets
        .iter()
        .map(|sheet| sheet.sheet_id.clone())
        .collect::<Vec<_>>();
    let mut problems = vec![];
    match frozen.validate_export_sheets(&sheet_ids) {
        Ok(layout) if !layout.is_empty() => problems.push(problem(
            BatchProblemKind::Placeholder,
            "Preencha os Frames vazios e salve o Projeto.",
        )),
        Err(error) => problems.push(problem(BatchProblemKind::InvalidProject, error.to_string())),
        _ => {}
    }
    let referenced: HashSet<_> = frozen
        .render_snapshot()
        .composition
        .sheets
        .iter()
        .flat_map(|sheet| sheet.referenced_media_ids())
        .collect();
    let mut sources = vec![];
    let mut originals = vec![];
    for media in loaded.project().media() {
        originals.push(media.path().to_path_buf());
        if !paths.covers(media.path()) {
            problems.push(problem(
                BatchProblemKind::Unavailable,
                "Uma origem do Projeto não pertence às pastas verificadas. Verifique novamente.",
            ));
        }
        let media_id =
            MediaId::try_from(media.id()).expect("persisted media have valid identities");
        if !referenced.contains(&media_id) {
            continue;
        }
        let id = media_id.to_string();
        let replacement = item
            .relinks
            .individual
            .get(&id)
            .filter(|relink| relink.original == media.path())
            .or_else(|| {
                item.relinks
                    .global
                    .get(&id)
                    .filter(|relink| relink.original == media.path())
            });
        let path = replacement.map_or(media.path(), |relink| relink.replacement.as_path());
        if !paths.covers(path) {
            problems.push(BatchProblem {
                kind: BatchProblemKind::Unavailable,
                message: format!(
                    "Imagem indisponível: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
                media_id: Some(id),
            });
            continue;
        }
        let observation = MediaResolver.observe_in_plan(
            paths,
            &MediaBinding {
                media_id: id.clone(),
                kind: media.kind(),
                logical_path: path.to_path_buf(),
            },
        );
        let file_name = media
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        match observation.availability {
            MediaAvailability::Candidate => {
                originals.push(path.to_path_buf());
                sources.push(
                    RenderSource::new(media_id, path.to_path_buf()).map_err(|error| {
                        vec![problem(BatchProblemKind::Unavailable, error.to_string())]
                    })?,
                );
            }
            state => problems.push(BatchProblem {
                kind: if state == MediaAvailability::Absent {
                    BatchProblemKind::MissingMedia
                } else {
                    BatchProblemKind::Unavailable
                },
                message: if state == MediaAvailability::Absent {
                    format!("Imagem ausente: {file_name}")
                } else {
                    format!("Imagem indisponível: {file_name}")
                },
                media_id: Some(id),
            }),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let (snapshot, _) = frozen
        .into_export(&sheet_ids)
        .map_err(|error| vec![problem(BatchProblemKind::InvalidProject, error.to_string())])?;
    let plan = export_pipeline::plan_album_in_paths(
        snapshot,
        AlbumExportOptions {
            protected_originals: originals,
            sheet_ids,
            whole_album: true,
            mode: configuration.mode,
            format: configuration.format.clone(),
            destination: item.destination.clone(),
            authorization: if policy == ExportConflictPolicy::Replace {
                ExportWriteAuthorization::ReplaceConfirmed
            } else {
                ExportWriteAuthorization::CreateOnly
            },
            sources,
            request_id: request_id.into(),
        },
        paths,
    )
    .map_err(|error| vec![problem(BatchProblemKind::Unavailable, error.message)])?;
    for path in plan.required_paths() {
        if !paths.covers(&path) {
            return Err(vec![problem(
                BatchProblemKind::Changed,
                "O Projeto passou a usar outra pasta. Verifique novamente.",
            )]);
        }
    }
    Ok(plan)
}

fn discover_projects(
    root: &Path,
    paths: &mut OperationPathContext,
) -> Result<Vec<PathBuf>, String> {
    walk_files(root, paths, &|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("myalbuns"))
    })
}

fn walk_files(
    root: &Path,
    paths: &mut OperationPathContext,
    include: &dyn Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = HashSet::new();
    let mut files = HashSet::new();
    let mut projects = vec![];
    while let Some(directory) = pending.pop() {
        let resolved = paths
            .resolve_existing(&directory, ExpectedObject::Directory)
            .map_err(|error| {
                format!(
                    "Não foi possível verificar {}: {error}",
                    directory.display()
                )
            })?;
        if let Some(identity) = resolved.physical_identity()
            && !directories.insert(identity.to_local_token())
        {
            continue;
        }
        let mut entries = std::fs::read_dir(resolved.operational_path())
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let logical = directory.join(entry.file_name());
            let metadata = std::fs::metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                pending.push(logical);
                continue;
            }
            if !include(&logical) {
                continue;
            }
            // An inaccessible document remains an item for actionable preflight.
            if let Ok(document) = paths.resolve_existing(&logical, ExpectedObject::RegularFile)
                && let Some(identity) = document.physical_identity()
                && !files.insert(identity.to_local_token())
            {
                continue;
            }
            projects.push(logical);
        }
    }
    projects.sort();
    Ok(projects)
}

#[cfg(test)]
mod tests;
