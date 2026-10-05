use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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
    pub favorite: bool,
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
    #[serde(default)]
    favorite: bool,
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
    mutation_lock: Arc<Mutex<()>>,
}

impl RecentProjectsStore {
    pub fn new(app_paths: &AppPaths) -> Self {
        Self {
            file: app_paths.recent_projects_file(),
            mutation_lock: Arc::new(Mutex::new(())),
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
                favorite: project.favorite,
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
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| RecentProjectsError::InvalidState)?;
        let mut projects = self.load_records()?;
        let favorite = projects
            .iter()
            .find(|project| project.project_id == project_id)
            .is_some_and(|project| project.favorite);
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
                favorite,
            },
        );
        retain_recent_and_favorites(&mut projects);
        self.publish(&RecentProjectsEnvelope {
            schema_version: RECENT_PROJECTS_SCHEMA_VERSION,
            projects,
        })
    }

    pub fn set_favorite(
        &self,
        project_id: &str,
        favorite: bool,
    ) -> Result<Vec<RecentProjectSummary>, RecentProjectsError> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| RecentProjectsError::InvalidState)?;
        let mut projects = self.load_records()?;
        let project = projects
            .iter_mut()
            .find(|project| project.project_id == project_id)
            .ok_or(RecentProjectsError::InvalidState)?;
        if project.favorite != favorite {
            project.favorite = favorite;
            retain_recent_and_favorites(&mut projects);
            self.publish(&RecentProjectsEnvelope {
                schema_version: RECENT_PROJECTS_SCHEMA_VERSION,
                projects,
            })?;
        }
        self.list()
    }

    fn load_records(&self) -> Result<Vec<RecentProjectRecord>, RecentProjectsError> {
        let bytes = match fs::read(&self.file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(RecentProjectsError::Io(error)),
        };
        // Content this version cannot read is an empty list, and the next write
        // replaces it: nobody has to delete this file by hand.
        let Some(mut projects) = serde_json::from_slice::<RecentProjectsEnvelope>(&bytes)
            .ok()
            .filter(|envelope| envelope.schema_version == RECENT_PROJECTS_SCHEMA_VERSION)
            .map(|envelope| envelope.projects)
        else {
            tracing::warn!(
                target: "myalbuns.desktop",
                event = "recent_projects_unreadable_content_discarded",
            );
            return Ok(Vec::new());
        };
        // A readable list keeps every record that is still valid, in stored
        // order: the first of each identity, a favorite if any of its records
        // was one, then the usual limit.
        let stored = projects.len();
        let favorites: std::collections::HashSet<String> = projects
            .iter()
            .filter(|project| project.favorite)
            .map(|project| project.project_id.clone())
            .collect();
        let mut identities = std::collections::HashSet::new();
        projects.retain_mut(|project| {
            project.favorite = favorites.contains(&project.project_id);
            !project.project_id.is_empty() && identities.insert(project.project_id.clone())
        });
        retain_recent_and_favorites(&mut projects);
        if projects.len() != stored {
            tracing::warn!(
                target: "myalbuns.desktop",
                dropped_records = stored - projects.len(),
                event = "recent_projects_invalid_records_dropped",
            );
        }
        Ok(projects)
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

fn retain_recent_and_favorites(projects: &mut Vec<RecentProjectRecord>) {
    let mut non_favorites = 0;
    projects.retain(|project| {
        if project.favorite {
            return true;
        }
        non_favorites += 1;
        non_favorites <= MAX_RECENT_PROJECTS
    });
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
    use std::path::{Path, PathBuf};

    use myalbuns_paths::{AppPaths, NativePathDto};

    use super::{RecentProjectSummary, RecentProjectsStore};

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
        let summary = serde_json::to_value(&store.list().unwrap()[0]).unwrap();
        assert_eq!(summary["lastOpenedAtMs"].as_u64(), Some(recorded));
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
        assert!(!listed[0].favorite);
        assert_eq!(std::fs::read(file).unwrap(), legacy);
    }

    #[test]
    fn favorites_persist_without_changing_open_time_or_order_and_survive_reopen() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);
        for id in ["first", "second", "third"] {
            store
                .promote(
                    id,
                    NativePathDto::from(root.path().join(format!("{id}.myalbuns"))),
                )
                .unwrap();
        }
        let before = store.list().unwrap();
        store.set_favorite("first", true).unwrap();
        assert_eq!(
            store
                .list()
                .unwrap()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["third", "second", "first"]
        );
        assert_eq!(
            store.list().unwrap()[2].last_opened_at_ms,
            before[2].last_opened_at_ms
        );
        assert!(RecentProjectsStore::new(&paths).list().unwrap()[2].favorite);
        store
            .promote(
                "first",
                NativePathDto::from(root.path().join("first.myalbuns")),
            )
            .unwrap();
        assert!(store.list().unwrap()[0].favorite);
        store.set_favorite("first", true).unwrap();
        assert!(store.list().unwrap()[0].favorite);
    }

    #[test]
    fn favorite_retention_and_unfavorite_apply_chronological_limit() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);
        store
            .promote(
                "keeper",
                NativePathDto::from(root.path().join("keeper.myalbuns")),
            )
            .unwrap();
        store.set_favorite("keeper", true).unwrap();
        for index in 0..25 {
            let id = format!("other-{index}");
            store
                .promote(
                    &id,
                    NativePathDto::from(root.path().join(format!("{id}.myalbuns"))),
                )
                .unwrap();
        }
        assert_eq!(store.list().unwrap().len(), 21);
        assert!(store.list().unwrap().last().unwrap().favorite);
        store.set_favorite("keeper", false).unwrap();
        assert_eq!(store.list().unwrap().len(), 20);
        assert!(!store.list().unwrap().iter().any(|item| item.id == "keeper"));
        assert!(matches!(
            store.set_favorite("missing", true),
            Err(super::RecentProjectsError::InvalidState)
        ));
    }

    #[test]
    fn replacement_identity_does_not_inherit_favorite_and_failed_write_keeps_state() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);
        let path = NativePathDto::from(root.path().join("shared.myalbuns"));
        store.promote("old", path.clone()).unwrap();
        let original = std::fs::read(paths.recent_projects_file()).unwrap();
        let fault =
            myalbuns_paths::test_support::DiskFull::on_create(&paths.recent_projects_file());
        assert!(matches!(
            store.set_favorite("old", true),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert_eq!(fault.failure_count(), 1);
        assert_eq!(
            std::fs::read(paths.recent_projects_file()).unwrap(),
            original
        );
        assert!(!store.list().unwrap()[0].favorite);
        drop(fault);
        store.set_favorite("old", true).unwrap();
        store.promote("new", path).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(!store.list().unwrap()[0].favorite);
    }

    // -----------------------------------------------------------------------
    // Content that cannot be read as stored heals on the next write

    fn stored_record(root: &Path, id: &str, favorite: bool) -> serde_json::Value {
        serde_json::json!({
            "projectId": id,
            "path": NativePathDto::from(root.join(format!("{id}.myalbuns"))),
            "lastOpenedAtMs": 1,
            "favorite": favorite,
        })
    }

    fn stored_file(projects: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec_pretty(&serde_json::json!({ "schemaVersion": 1, "projects": projects }))
            .unwrap()
    }

    fn store_with_content(content: &[u8]) -> (tempfile::TempDir, PathBuf, RecentProjectsStore) {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let file = paths.recent_projects_file();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, content).unwrap();
        let store = RecentProjectsStore::new(&paths);
        (root, file, store)
    }

    fn ids(list: &[RecentProjectSummary]) -> Vec<&str> {
        list.iter().map(|item| item.id.as_str()).collect()
    }

    /// Reads the file as any later version would: a current envelope with
    /// distinct, named identities within the limit.
    fn valid_stored_ids(file: &Path) -> Vec<String> {
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(file).unwrap()).expect("the file is JSON");
        assert_eq!(stored["schemaVersion"], 1);
        let projects = stored["projects"].as_array().expect("the list is stored");
        let ids: Vec<String> = projects
            .iter()
            .map(|project| project["projectId"].as_str().unwrap().to_owned())
            .collect();
        assert!(ids.iter().all(|id| !id.is_empty()));
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            ids.len()
        );
        assert!(
            projects
                .iter()
                .filter(|project| project["favorite"] != true)
                .count()
                <= super::MAX_RECENT_PROJECTS
        );
        ids
    }

    /// Lists the stored content, promotes one more Project and reopens the
    /// list, returning what was listed before and after the promotion.
    fn heal_by_promoting(content: &[u8]) -> (Vec<RecentProjectSummary>, Vec<RecentProjectSummary>) {
        let (root, file, store) = store_with_content(content);

        let healed = store.list().expect("unreadable content is not an error");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            content,
            "listing never rewrites the file"
        );

        store
            .promote(
                "promoted",
                NativePathDto::from(root.path().join("Promovido.myalbuns")),
            )
            .expect("the next promotion replaces the stored content");
        let stored = valid_stored_ids(&file);
        assert_eq!(stored[0], "promoted");
        let reopened = store.list().expect("the replaced content is readable");
        assert_eq!(ids(&reopened), stored);
        (healed, reopened)
    }

    #[test]
    fn a_truncated_file_is_an_empty_list_that_the_next_promotion_replaces() {
        let root = tempfile::tempdir().unwrap();
        let complete = stored_file(vec![stored_record(root.path(), "first", true)]);

        for content in [&complete[..complete.len() / 2], b"not JSON", b""] {
            let (healed, reopened) = heal_by_promoting(content);
            assert!(healed.is_empty());
            assert_eq!(ids(&reopened), ["promoted"]);
        }
    }

    #[test]
    fn another_schema_version_is_an_empty_list_that_the_next_promotion_replaces() {
        let root = tempfile::tempdir().unwrap();
        let other_version = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "projects": [stored_record(root.path(), "first", true)],
        }))
        .unwrap();

        let (healed, reopened) = heal_by_promoting(&other_version);

        assert!(healed.is_empty());
        assert_eq!(ids(&reopened), ["promoted"]);
    }

    #[test]
    fn a_list_over_the_limit_keeps_its_favorites_and_the_first_recent_projects() {
        let root = tempfile::tempdir().unwrap();
        let mut projects: Vec<_> = (0..25)
            .map(|index| stored_record(root.path(), &format!("recent-{index}"), false))
            .collect();
        projects.push(stored_record(root.path(), "keeper", true));

        let (healed, reopened) = heal_by_promoting(&stored_file(projects));

        let mut expected: Vec<String> = (0..20).map(|index| format!("recent-{index}")).collect();
        expected.push("keeper".into());
        assert_eq!(ids(&healed), expected);
        assert!(healed.last().unwrap().favorite);
        // The promoted Project takes the place of the last one kept.
        expected.remove(19);
        expected.insert(0, "promoted".into());
        assert_eq!(ids(&reopened), expected);
        assert!(reopened.last().unwrap().favorite);
    }

    #[test]
    fn a_record_without_identity_is_dropped_and_the_others_are_kept() {
        let root = tempfile::tempdir().unwrap();
        let content = stored_file(vec![
            stored_record(root.path(), "first", false),
            stored_record(root.path(), "", true),
            stored_record(root.path(), "second", true),
        ]);

        let (healed, reopened) = heal_by_promoting(&content);

        assert_eq!(ids(&healed), ["first", "second"]);
        assert_eq!(ids(&reopened), ["promoted", "first", "second"]);
        assert!(!reopened[1].favorite);
        assert!(reopened[2].favorite);
    }

    #[test]
    fn a_repeated_identity_keeps_its_first_record_and_its_favorite_mark() {
        let root = tempfile::tempdir().unwrap();
        let mut repeated = stored_record(root.path(), "first", true);
        repeated["path"] =
            serde_json::json!(NativePathDto::from(root.path().join("Outro.myalbuns")));
        let content = stored_file(vec![
            stored_record(root.path(), "first", false),
            stored_record(root.path(), "second", false),
            repeated,
        ]);

        let (healed, reopened) = heal_by_promoting(&content);

        assert_eq!(ids(&healed), ["first", "second"]);
        assert_eq!(healed[0].name, "first");
        assert!(healed[0].favorite, "a favorite mark is never dropped");
        assert!(!healed[1].favorite);
        assert_eq!(ids(&reopened), ["promoted", "first", "second"]);
        assert!(reopened[1].favorite);
    }

    #[test]
    fn changing_a_favorite_replaces_content_with_invalid_records() {
        let root = tempfile::tempdir().unwrap();
        let (_root, file, store) = store_with_content(&stored_file(vec![
            stored_record(root.path(), "first", false),
            stored_record(root.path(), "first", false),
            stored_record(root.path(), "", false),
        ]));

        let listed = store
            .set_favorite("first", true)
            .expect("the kept record becomes a favorite");

        assert_eq!(ids(&listed), ["first"]);
        assert!(listed[0].favorite);
        assert_eq!(valid_stored_ids(&file), ["first"]);
        assert_eq!(ids(&store.list().unwrap()), ["first"]);
    }

    #[test]
    fn a_favorite_cannot_be_changed_in_unreadable_content() {
        let (_root, file, store) = store_with_content(b"not JSON");

        // The Project is not in the empty list, as with a healthy file.
        assert!(matches!(
            store.set_favorite("first", true),
            Err(super::RecentProjectsError::InvalidState)
        ));
        assert_eq!(std::fs::read(&file).unwrap(), b"not JSON");
    }

    #[test]
    fn a_file_that_cannot_be_read_stays_an_error() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let file = paths.recent_projects_file();
        std::fs::create_dir_all(&file).expect("a directory occupies the State file path");
        let store = RecentProjectsStore::new(&paths);

        assert!(matches!(
            store.list(),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert!(matches!(
            store.path_for("first"),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert!(matches!(
            store.promote(
                "first",
                NativePathDto::from(root.path().join("first.myalbuns"))
            ),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert!(matches!(
            store.set_favorite("first", true),
            Err(super::RecentProjectsError::Io(_))
        ));
        assert!(file.is_dir());
    }

    #[test]
    fn promoting_without_an_identity_is_refused_and_writes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_roots(root.path(), root.path());
        let store = RecentProjectsStore::new(&paths);

        assert!(matches!(
            store.promote("", NativePathDto::from(root.path().join("Sem.myalbuns"))),
            Err(super::RecentProjectsError::InvalidState)
        ));
        assert!(!paths.recent_projects_file().exists());

        store
            .promote(
                "first",
                NativePathDto::from(root.path().join("first.myalbuns")),
            )
            .unwrap();
        let before = std::fs::read(paths.recent_projects_file()).unwrap();
        assert!(matches!(
            store.promote("", NativePathDto::from(root.path().join("Sem.myalbuns"))),
            Err(super::RecentProjectsError::InvalidState)
        ));
        assert_eq!(std::fs::read(paths.recent_projects_file()).unwrap(), before);
    }
}
