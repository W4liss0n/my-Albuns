use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use myalbuns_paths::{AppPaths, NativePathDto};
use serde::{Deserialize, Serialize};

const RECENT_PROJECTS_SCHEMA_VERSION: u16 = 1;
const MAX_RECENT_PROJECTS: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProjectSummary {
    pub id: String,
    pub name: String,
    pub last_opened_at_ms: Option<u64>,
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
    #[serde(default)]
    last_opened_at_ms: Option<u64>,
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
                last_opened_at_ms: project.last_opened_at_ms,
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
        projects.retain(|project| project.project_id != project_id && project.path != path);
        projects.insert(
            0,
            RecentProjectRecord {
                project_id: project_id.to_owned(),
                path,
                last_opened_at_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .and_then(|duration| u64::try_from(duration.as_millis()).ok()),
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
        self.file
            .parent()
            .ok_or(RecentProjectsError::InvalidState)?;
        let bytes =
            serde_json::to_vec_pretty(envelope).map_err(|_| RecentProjectsError::InvalidState)?;
        crate::local_store_io::write_atomically(&self.file, &bytes, "recent-projects.json")?;
        Ok(())
    }
}

fn display_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Projeto")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use myalbuns_paths::{AppPaths, NativePathDto};

    use super::RecentProjectsStore;

    #[test]
    fn a_failed_promotion_keeps_the_previous_list_and_can_be_retried() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);
        store
            .promote(
                "first",
                NativePathDto::from(root.path().join("Primeiro.myalbuns")),
            )
            .unwrap();
        let before = std::fs::read(paths.recent_projects_file()).unwrap();
        let first_opened_at = store.list().unwrap()[0].last_opened_at_ms;
        assert!(first_opened_at.is_some());
        let fault =
            myalbuns_paths::test_support::DiskFull::on_create(&paths.recent_projects_file());
        assert!(matches!(
            store.promote(
                "second",
                NativePathDto::from(root.path().join("Segundo.myalbuns"))
            ),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert_eq!(fault.failure_count(), 1);
        assert_eq!(std::fs::read(paths.recent_projects_file()).unwrap(), before);
        assert_eq!(store.list().unwrap()[0].id, "first");
        assert_eq!(store.list().unwrap()[0].last_opened_at_ms, first_opened_at);
        drop(fault);
        store
            .promote(
                "second",
                NativePathDto::from(root.path().join("Segundo.myalbuns")),
            )
            .unwrap();
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["second", "first"]
        );
    }

    #[test]
    fn an_absent_recent_projects_file_is_an_empty_list() {
        let roaming = tempfile::tempdir().expect("temporary roaming root");
        let local = tempfile::tempdir().expect("temporary local root");
        let paths = AppPaths::from_roots(roaming.path(), local.path());

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
        let paths = AppPaths::from_roots(roaming.path(), local.path());
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

        let recent = store.list().expect("the ordered list remains readable");
        assert_eq!(
            recent
                .iter()
                .map(|item| (item.id.as_str(), item.name.as_str()))
                .collect::<Vec<_>>(),
            [
                ("project-horizon", "Horizonte final"),
                ("project-aurora", "Aurora")
            ]
        );
        assert!(recent.iter().all(|item| item.last_opened_at_ms.is_some()));
    }

    #[test]
    fn replacing_a_project_at_the_same_native_path_removes_the_previous_identity() {
        let roaming = tempfile::tempdir().expect("temporary roaming root");
        let local = tempfile::tempdir().expect("temporary local root");
        let paths = AppPaths::from_roots(roaming.path(), local.path());
        let store = RecentProjectsStore::new(&paths);
        let path = NativePathDto::from(PathBuf::from(r"C:\Albuns\Horizonte.myalbuns"));

        store
            .promote("old-project", path.clone())
            .expect("the previous Project is persisted");
        store
            .promote("replacement-project", path)
            .expect("the replacement Project is promoted");

        let recent = store.list().expect("the replacement remains readable");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "replacement-project");
        assert_eq!(recent[0].name, "Horizonte");
        assert!(recent[0].last_opened_at_ms.is_some());
    }
    #[test]
    fn promoting_records_the_real_open_time_only_when_promoted() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        store
            .promote(
                "first",
                NativePathDto::from(root.path().join("First.myalbuns")),
            )
            .unwrap();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let recorded = store.list().unwrap()[0].last_opened_at_ms.unwrap();
        assert!(recorded >= before && recorded <= after);
        let saved = std::fs::read(paths.recent_projects_file()).unwrap();
        assert!(String::from_utf8_lossy(&saved).contains("lastOpenedAtMs"));
        assert_eq!(store.list().unwrap()[0].last_opened_at_ms, Some(recorded));
        assert_eq!(std::fs::read(paths.recent_projects_file()).unwrap(), saved);
    }

    #[test]
    fn a_legacy_record_has_no_invented_open_time_and_listing_does_not_rewrite_it() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let file = paths.recent_projects_file();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let path = NativePathDto::from(root.path().join("Legacy.myalbuns"));
        let legacy = serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "projects": [{ "projectId": "legacy", "path": path }],
        }))
        .unwrap();
        std::fs::write(&file, &legacy).unwrap();
        let listed = RecentProjectsStore::new(&paths).list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "legacy");
        assert_eq!(listed[0].name, "Legacy");
        assert_eq!(listed[0].last_opened_at_ms, None);
        assert_eq!(std::fs::read(file).unwrap(), legacy);
    }
}
