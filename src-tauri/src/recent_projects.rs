use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use myalbuns_paths::{AppPaths, NativePathDto};
use serde::{Deserialize, Serialize};

const RECENT_PROJECTS_SCHEMA_VERSION: u16 = 1;
const MAX_RECENT_PROJECTS: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecentProjectSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug)]
pub enum RecentProjectsError {
    Io(io::Error),
    InvalidState,
}

impl std::fmt::Display for RecentProjectsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("o estado de Projetos recentes está indisponível"),
            Self::InvalidState => formatter.write_str("o estado de Projetos recentes é inválido"),
        }
    }
}

impl std::error::Error for RecentProjectsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidState => None,
        }
    }
}

impl From<io::Error> for RecentProjectsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentProjectRecord {
    project_id: String,
    path: NativePathDto,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentProjectsEnvelope {
    schema_version: u16,
    projects: Vec<RecentProjectRecord>,
}

#[derive(Clone, Debug)]
pub struct RecentProjectsStore {
    file: PathBuf,
}

impl RecentProjectsStore {
    pub fn new(app_paths: &AppPaths) -> Self {
        Self {
            file: app_paths.recent_projects_file(),
        }
    }

    pub fn list(&self) -> Result<Vec<RecentProjectSummary>, RecentProjectsError> {
        Ok(self
            .load_records()?
            .into_iter()
            .map(|project| RecentProjectSummary {
                id: project.project_id,
                name: display_name(project.path.as_path()),
            })
            .collect())
    }

    pub fn path_for(&self, project_id: &str) -> Result<Option<NativePathDto>, RecentProjectsError> {
        Ok(self
            .load_records()?
            .into_iter()
            .find(|project| project.project_id == project_id)
            .map(|project| project.path))
    }

    pub fn promote(
        &self,
        project_id: &str,
        path: NativePathDto,
    ) -> Result<(), RecentProjectsError> {
        if project_id.is_empty() {
            return Err(RecentProjectsError::InvalidState);
        }
        let mut projects = self.load_records()?;
        projects.retain(|project| project.project_id != project_id);
        projects.insert(
            0,
            RecentProjectRecord {
                project_id: project_id.to_owned(),
                path,
            },
        );
        projects.truncate(MAX_RECENT_PROJECTS);
        self.publish(&RecentProjectsEnvelope {
            schema_version: RECENT_PROJECTS_SCHEMA_VERSION,
            projects,
        })
    }

    fn load_records(&self) -> Result<Vec<RecentProjectRecord>, RecentProjectsError> {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(RecentProjectsError::Io(error)),
        };
        let envelope: RecentProjectsEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| RecentProjectsError::InvalidState)?;
        if envelope.schema_version != RECENT_PROJECTS_SCHEMA_VERSION
            || envelope.projects.len() > MAX_RECENT_PROJECTS
            || envelope
                .projects
                .iter()
                .any(|project| project.project_id.is_empty())
        {
            return Err(RecentProjectsError::InvalidState);
        }
        let mut identities = std::collections::HashSet::new();
        if !envelope
            .projects
            .iter()
            .all(|project| identities.insert(project.project_id.as_str()))
        {
            return Err(RecentProjectsError::InvalidState);
        }
        Ok(envelope.projects)
    }

    fn publish(&self, envelope: &RecentProjectsEnvelope) -> Result<(), RecentProjectsError> {
        let parent = self
            .file
            .parent()
            .ok_or(RecentProjectsError::InvalidState)?;
        fs::create_dir_all(parent)?;
        let temporary = sibling_temporary(&self.file);
        let result = (|| {
            let bytes = serde_json::to_vec_pretty(envelope)
                .map_err(|_| RecentProjectsError::InvalidState)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            replace_file(&temporary, &self.file)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Projeto")
        .to_owned()
}

fn sibling_temporary(target: &Path) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("recent-projects.json");
    target.with_file_name(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ))
}

#[cfg(windows)]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let succeeded = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temporary, target)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use myalbuns_paths::{AppPaths, NativePathDto};

    use super::{RecentProjectSummary, RecentProjectsStore};

    #[test]
    fn an_absent_recent_projects_file_is_an_empty_list() {
        let roaming = tempfile::tempdir().expect("temporary roaming root");
        let local = tempfile::tempdir().expect("temporary local root");
        let temporary = tempfile::tempdir().expect("temporary data root");
        let paths = AppPaths::from_roots(roaming.path(), local.path(), temporary.path());

        let projects = RecentProjectsStore::new(&paths)
            .list()
            .expect("an absent State file is a valid empty list");

        assert!(projects.is_empty());
        assert!(!paths.recent_projects_file().exists());
    }

    #[test]
    fn promoting_a_project_moves_its_single_entry_to_the_top() {
        let roaming = tempfile::tempdir().expect("temporary roaming root");
        let local = tempfile::tempdir().expect("temporary local root");
        let temporary = tempfile::tempdir().expect("temporary data root");
        let paths = AppPaths::from_roots(roaming.path(), local.path(), temporary.path());
        let store = RecentProjectsStore::new(&paths);
        let horizon = NativePathDto::from(PathBuf::from(r"C:\Albuns\Horizonte.myalbuns"));
        let aurora = NativePathDto::from(PathBuf::from(r"C:\Albuns\Aurora.myalbuns"));

        store
            .promote("project-horizon", horizon)
            .expect("the first recent Project is persisted");
        store
            .promote("project-aurora", aurora)
            .expect("a later Project is promoted to the top");
        store
            .promote(
                "project-horizon",
                NativePathDto::from(PathBuf::from(r"D:\Clientes\Horizonte final.myalbuns")),
            )
            .expect("reopening updates and promotes the existing identity");

        assert_eq!(
            store.list().expect("the ordered list remains readable"),
            vec![
                RecentProjectSummary {
                    id: "project-horizon".into(),
                    name: "Horizonte final".into(),
                },
                RecentProjectSummary {
                    id: "project-aurora".into(),
                    name: "Aurora".into(),
                },
            ]
        );
    }
}
