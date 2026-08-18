use std::{
    fmt, fs,
    io::{self, Write},
    os::windows::ffi::OsStrExt,
    path::PathBuf,
};

use myalbuns_core::ProjectIdentityAuthority;
use myalbuns_paths::{AppPaths, CacheNamespaceUsage, CachePathPlan, publish_new_file};
use sha2::{Digest, Sha256};

use crate::{
    cache_engine::AuthorizedCacheNamespace,
    ipc_contract::{
        CacheClearAllOutcome, CacheFreeResult, CacheServiceCommandError,
        CacheServiceCommandErrorCode, CacheServiceStatus,
    },
    named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant},
    operation_gate::{OperationGate, OperationGateError},
};

const CLEAR_SCHEDULE_CONTENT: &[u8] = b"MyAlbuns Cache clear schedule v1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheScheduledCleanupOutcome {
    NotScheduled,
    Cleared,
    Deferred,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CacheServiceError {
    Busy,
    Storage(String),
    Reservation(String),
}

#[derive(Debug)]
pub(crate) struct CacheService {
    app_paths: AppPaths,
    maintenance: NamedMutex,
}

#[derive(Debug)]
pub(crate) struct CacheNamespaceOwner {
    namespace: AuthorizedCacheNamespace,
    _reservation: NamedMutexGrant,
}

impl CacheNamespaceOwner {
    pub(crate) fn namespace(&self) -> &AuthorizedCacheNamespace {
        &self.namespace
    }
}

impl CacheService {
    pub(crate) fn new(app_paths: AppPaths) -> Self {
        let maintenance = NamedMutex::new(
            mutex_name(&app_paths, "CacheMaintenance", "global"),
            "myalbuns-cache-maintenance",
        );
        Self {
            app_paths,
            maintenance,
        }
    }

    pub(crate) fn reserve_namespace(
        &self,
        authority: &ProjectIdentityAuthority,
    ) -> Result<CacheNamespaceOwner, CacheServiceError> {
        let _maintenance = self.try_maintenance()?;
        let namespace = AuthorizedCacheNamespace::mount(&self.app_paths, authority)
            .map_err(|error| CacheServiceError::Storage(error.message))?;
        let reservation = namespace_mutex(&self.app_paths, namespace.paths())
            .try_acquire()
            .map_err(map_reservation_error)?;
        self.app_paths
            .prepare_cache_storage(namespace.paths())
            .map_err(|error| CacheServiceError::Storage(error.to_string()))?;
        Ok(CacheNamespaceOwner {
            namespace,
            _reservation: reservation,
        })
    }

    pub(crate) fn measure(&self) -> Result<CacheServiceStatus, CacheServiceError> {
        let usages = self.inspect()?;
        let occupied_bytes = sum_usage(&usages)?;
        let mut releasable_bytes = 0_u64;
        let mut releasable_namespace_count = 0_usize;
        for usage in &usages {
            match namespace_mutex(&self.app_paths, usage.paths()).try_acquire() {
                Ok(_reservation) => {
                    releasable_bytes =
                        releasable_bytes.checked_add(usage.bytes()).ok_or_else(|| {
                            CacheServiceError::Storage("volume de Cache excedeu u64".into())
                        })?;
                    releasable_namespace_count += 1;
                }
                Err(NamedMutexError::Conflict) => {}
                Err(NamedMutexError::Unavailable(reason)) => {
                    return Err(CacheServiceError::Reservation(reason));
                }
            }
        }
        Ok(CacheServiceStatus {
            occupied_bytes,
            releasable_bytes,
            namespace_count: usages.len(),
            releasable_namespace_count,
            clear_all_scheduled: self.clear_is_scheduled()?,
        })
    }

    pub(crate) fn free_closed_projects(&self) -> Result<CacheFreeResult, CacheServiceError> {
        let usages = self.inspect()?;
        let mut reserved = Vec::new();
        let mut skipped_active_namespace_count = 0_usize;
        for usage in usages {
            match namespace_mutex(&self.app_paths, usage.paths()).try_acquire() {
                Ok(reservation) => reserved.push((usage, reservation)),
                Err(NamedMutexError::Conflict) => skipped_active_namespace_count += 1,
                Err(NamedMutexError::Unavailable(reason)) => {
                    return Err(CacheServiceError::Reservation(reason));
                }
            }
        }
        let measured_releasable_bytes = reserved.iter().try_fold(0_u64, |total, (usage, _)| {
            total
                .checked_add(usage.bytes())
                .ok_or_else(|| CacheServiceError::Storage("volume de Cache excedeu u64".into()))
        })?;
        let mut freed_bytes = 0_u64;
        let mut removed_namespace_count = 0_usize;
        for (usage, _reservation) in reserved {
            if self
                .app_paths
                .clear_project_cache(usage.paths())
                .map_err(|error| CacheServiceError::Storage(error.to_string()))?
            {
                freed_bytes = freed_bytes.checked_add(usage.bytes()).ok_or_else(|| {
                    CacheServiceError::Storage("volume de Cache excedeu u64".into())
                })?;
                removed_namespace_count += 1;
            }
        }
        Ok(CacheFreeResult {
            measured_releasable_bytes,
            freed_bytes,
            removed_namespace_count,
            skipped_active_namespace_count,
        })
    }

    pub(crate) fn clear_all_or_schedule(&self) -> Result<CacheClearAllOutcome, CacheServiceError> {
        match self.try_clear_all()? {
            Some(result) => Ok(CacheClearAllOutcome::Cleared { result }),
            None => {
                self.schedule_clear()?;
                Ok(CacheClearAllOutcome::Scheduled)
            }
        }
    }

    pub(crate) fn run_scheduled_cleanup(
        &self,
    ) -> Result<CacheScheduledCleanupOutcome, CacheServiceError> {
        if !self.clear_is_scheduled()? {
            return Ok(CacheScheduledCleanupOutcome::NotScheduled);
        }
        match self.try_clear_all()? {
            Some(_) => Ok(CacheScheduledCleanupOutcome::Cleared),
            None => Ok(CacheScheduledCleanupOutcome::Deferred),
        }
    }

    fn try_clear_all(&self) -> Result<Option<CacheFreeResult>, CacheServiceError> {
        let _operation = match OperationGate::new(&self.app_paths).try_acquire() {
            Ok(grant) => grant,
            Err(OperationGateError::Conflict) => return Ok(None),
            Err(OperationGateError::Unavailable { reason }) => {
                return Err(CacheServiceError::Reservation(reason));
            }
        };
        let _maintenance = match self.maintenance.try_acquire() {
            Ok(grant) => grant,
            Err(NamedMutexError::Conflict) => return Ok(None),
            Err(NamedMutexError::Unavailable(reason)) => {
                return Err(CacheServiceError::Reservation(reason));
            }
        };
        let usages = self.inspect()?;
        let measured_releasable_bytes = sum_usage(&usages)?;
        let mut reservations = Vec::with_capacity(usages.len());
        for usage in &usages {
            match namespace_mutex(&self.app_paths, usage.paths()).try_acquire() {
                Ok(reservation) => reservations.push(reservation),
                Err(NamedMutexError::Conflict) => return Ok(None),
                Err(NamedMutexError::Unavailable(reason)) => {
                    return Err(CacheServiceError::Reservation(reason));
                }
            }
        }
        let mut freed_bytes = 0_u64;
        let mut removed_namespace_count = 0_usize;
        for usage in &usages {
            if self
                .app_paths
                .clear_project_cache(usage.paths())
                .map_err(|error| CacheServiceError::Storage(error.to_string()))?
            {
                freed_bytes = freed_bytes.checked_add(usage.bytes()).ok_or_else(|| {
                    CacheServiceError::Storage("volume de Cache excedeu u64".into())
                })?;
                removed_namespace_count += 1;
            }
        }
        drop(reservations);
        self.clear_schedule_marker()?;
        Ok(Some(CacheFreeResult {
            measured_releasable_bytes,
            freed_bytes,
            removed_namespace_count,
            skipped_active_namespace_count: 0,
        }))
    }

    fn inspect(&self) -> Result<Vec<CacheNamespaceUsage>, CacheServiceError> {
        self.app_paths
            .inspect_cache_namespaces()
            .map_err(|error| CacheServiceError::Storage(error.to_string()))
    }

    fn try_maintenance(&self) -> Result<NamedMutexGrant, CacheServiceError> {
        self.maintenance
            .try_acquire()
            .map_err(map_reservation_error)
    }

    fn schedule_file(&self) -> PathBuf {
        self.app_paths.state_dir().join("clear-cache-on-startup.v1")
    }

    fn clear_is_scheduled(&self) -> Result<bool, CacheServiceError> {
        let path = self.schedule_file();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(CacheServiceError::Storage(error.to_string())),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CacheServiceError::Storage(
                "o agendamento de limpeza não é um arquivo regular".into(),
            ));
        }
        let bytes =
            fs::read(&path).map_err(|error| CacheServiceError::Storage(error.to_string()))?;
        if bytes != CLEAR_SCHEDULE_CONTENT {
            return Err(CacheServiceError::Storage(
                "o agendamento de limpeza é incompatível".into(),
            ));
        }
        Ok(true)
    }

    fn schedule_clear(&self) -> Result<(), CacheServiceError> {
        if self.clear_is_scheduled()? {
            return Ok(());
        }
        let path = self.schedule_file();
        let parent = path.parent().ok_or_else(|| {
            CacheServiceError::Storage("o agendamento não possui diretório pai".into())
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| CacheServiceError::Storage(error.to_string()))?;
        let temporary = path.with_file_name(format!(
            ".clear-cache-on-startup.{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| CacheServiceError::Storage(error.to_string()))?;
        let prepared = file
            .write_all(CLEAR_SCHEDULE_CONTENT)
            .and_then(|()| file.sync_all())
            .map_err(|error| CacheServiceError::Storage(error.to_string()));
        drop(file);
        if let Err(error) = prepared {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        match publish_new_file(&temporary, &path) {
            Ok(()) => Ok(()),
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    || matches!(error.raw_os_error(), Some(80 | 183)) =>
            {
                let _ = fs::remove_file(&temporary);
                self.clear_is_scheduled().and_then(|scheduled| {
                    scheduled.then_some(()).ok_or_else(|| {
                        CacheServiceError::Storage(
                            "a publicação concorrente do agendamento não ficou visível".into(),
                        )
                    })
                })
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(CacheServiceError::Storage(error.to_string()))
            }
        }
    }

    fn clear_schedule_marker(&self) -> Result<(), CacheServiceError> {
        let path = self.schedule_file();
        if !self.clear_is_scheduled()? {
            return Ok(());
        }
        fs::remove_file(path).map_err(|error| CacheServiceError::Storage(error.to_string()))
    }
}

fn namespace_mutex(app_paths: &AppPaths, paths: &CachePathPlan) -> NamedMutex {
    let namespace = paths
        .root()
        .file_name()
        .and_then(|value| value.to_str())
        .expect("a validated Cache namespace is UTF-8 and non-empty");
    NamedMutex::new(
        mutex_name(app_paths, "CacheNamespace", namespace),
        "myalbuns-cache-namespace",
    )
}

fn mutex_name(app_paths: &AppPaths, kind: &str, scope: &str) -> String {
    let mut digest = Sha256::new();
    for unit in app_paths.local_root().as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    digest.update([0]);
    digest.update(scope.as_bytes());
    let digest = digest.finalize();
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(r"Local\MyAlbuns.{kind}.v1.{suffix}")
}

fn sum_usage(usages: &[CacheNamespaceUsage]) -> Result<u64, CacheServiceError> {
    usages.iter().try_fold(0_u64, |total, usage| {
        total
            .checked_add(usage.bytes())
            .ok_or_else(|| CacheServiceError::Storage("volume de Cache excedeu u64".into()))
    })
}

fn map_reservation_error(error: NamedMutexError) -> CacheServiceError {
    match error {
        NamedMutexError::Conflict => CacheServiceError::Busy,
        NamedMutexError::Unavailable(reason) => CacheServiceError::Reservation(reason),
    }
}

impl fmt::Display for CacheServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => write!(
                formatter,
                "o namespace de Cache já possui proprietário ativo"
            ),
            Self::Storage(reason) => write!(formatter, "falha no armazenamento de Cache: {reason}"),
            Self::Reservation(reason) => write!(formatter, "falha na reserva de Cache: {reason}"),
        }
    }
}

impl std::error::Error for CacheServiceError {}

impl From<CacheServiceError> for CacheServiceCommandError {
    fn from(error: CacheServiceError) -> Self {
        let code = match error {
            CacheServiceError::Busy => CacheServiceCommandErrorCode::Busy,
            CacheServiceError::Storage(_) => CacheServiceCommandErrorCode::StorageUnavailable,
            CacheServiceError::Reservation(_) => {
                CacheServiceCommandErrorCode::ReservationUnavailable
            }
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

#[tauri::command]
pub(crate) fn cache_service_status(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheServiceStatus, CacheServiceCommandError> {
    service.measure().map_err(Into::into)
}

#[tauri::command]
pub(crate) fn free_closed_project_cache(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheFreeResult, CacheServiceCommandError> {
    service.free_closed_projects().map_err(Into::into)
}

#[tauri::command]
pub(crate) fn clear_all_cache(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheClearAllOutcome, CacheServiceCommandError> {
    service.clear_all_or_schedule().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::Path,
        process::{Command, Stdio},
        sync::{Arc, Barrier},
        thread,
        time::{Duration, Instant},
    };

    use myalbuns_core::{
        CreateAuthorization, CreateProjectRequest, EditableProject, InitialProject,
        OpenProjectError, OpenProjectRequest, ProjectCore, ProjectLocation,
    };
    use myalbuns_paths::{AppPaths, OperationPathContext};

    use crate::ipc_contract::CacheClearAllOutcome;
    use crate::operation_gate::OperationGate;

    use super::{CacheScheduledCleanupOutcome, CacheService, namespace_mutex};

    const NAMESPACE_OWNER_ROOT_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_ROOT";
    const NAMESPACE_OWNER_NAME_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_NAME";
    const NAMESPACE_OWNER_READY_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_READY";

    fn app_paths(root: &Path) -> AppPaths {
        let roaming = root.join("roaming");
        let local = root.join("local");
        std::fs::create_dir_all(&roaming).expect("the roaming root exists");
        std::fs::create_dir_all(&local).expect("the local root exists");
        AppPaths::from_roots(&roaming, &local)
    }

    fn project_location(path: &Path) -> ProjectLocation {
        let mut paths = OperationPathContext::new();
        paths
            .capture(path)
            .expect("the Project path is captured authoritatively");
        ProjectLocation::new(path.to_path_buf(), paths.freeze())
    }

    fn create_project(core: &ProjectCore, path: std::path::PathBuf) -> EditableProject {
        core.create_editable(CreateProjectRequest::new(
            project_location(&path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the Project identity is authorized")
    }

    #[test]
    fn measures_and_frees_only_namespaces_without_an_active_owner() {
        let root = tempfile::tempdir().expect("temporary Cache service fixture");
        let app_paths =
            AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        std::fs::create_dir_all(root.path().join("roaming")).expect("the roaming root exists");
        std::fs::create_dir_all(root.path().join("local")).expect("the local root exists");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let first = create_project(&core, root.path().join("first.myalbuns"));
        let second = create_project(&core, root.path().join("second.myalbuns"));
        let service = CacheService::new(app_paths.clone());
        let first_owner = service
            .reserve_namespace(first.identity_authority())
            .expect("the first Host reserves its Cache");
        let second_owner = service
            .reserve_namespace(second.identity_authority())
            .expect("the second Host reserves its Cache");
        let first_paths = first_owner.namespace().paths().clone();
        let second_paths = second_owner.namespace().paths().clone();
        std::fs::write(
            first_paths.media_directory().join("first.bin"),
            vec![1_u8; 10],
        )
        .expect("the first Cache payload is writable");
        std::fs::write(
            second_paths.media_directory().join("second.bin"),
            vec![2_u8; 20],
        )
        .expect("the second Cache payload is writable");
        drop(second_owner);

        let measured = service.measure().expect("Cache usage is measurable");
        assert_eq!(measured.occupied_bytes, 30);
        assert_eq!(measured.releasable_bytes, 20);
        assert_eq!(measured.namespace_count, 2);
        assert_eq!(measured.releasable_namespace_count, 1);

        let released = service
            .free_closed_projects()
            .expect("closed Cache namespaces are removable");
        assert_eq!(released.measured_releasable_bytes, 20);
        assert_eq!(released.freed_bytes, 20);
        assert_eq!(released.removed_namespace_count, 1);
        assert_eq!(released.skipped_active_namespace_count, 1);
        assert!(first_paths.root().is_dir());
        assert!(!second_paths.root().exists());

        drop(first_owner);
        assert_eq!(
            service
                .free_closed_projects()
                .expect("the final closed namespace is removable")
                .freed_bytes,
            10
        );
    }

    #[test]
    fn schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup() {
        let root = tempfile::tempdir().expect("temporary scheduled-cleanup fixture");
        let app_paths =
            AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        std::fs::create_dir_all(root.path().join("roaming")).expect("the roaming root exists");
        std::fs::create_dir_all(root.path().join("local")).expect("the local root exists");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let project = create_project(&core, root.path().join("project.myalbuns"));
        let service = CacheService::new(app_paths);
        let owner = service
            .reserve_namespace(project.identity_authority())
            .expect("the active Project owns its Cache");
        std::fs::write(
            owner
                .namespace()
                .paths()
                .media_directory()
                .join("active.bin"),
            b"active Cache",
        )
        .expect("the active Cache payload is writable");

        assert_eq!(
            service
                .clear_all_or_schedule()
                .expect("the active cleanup is scheduled"),
            CacheClearAllOutcome::Scheduled
        );
        assert!(service.measure().unwrap().clear_all_scheduled);
        assert!(owner.namespace().paths().root().is_dir());

        let cache_root = owner.namespace().paths().root().to_path_buf();
        drop(owner);
        assert_eq!(
            service
                .run_scheduled_cleanup()
                .expect("startup owns the scheduled cleanup"),
            CacheScheduledCleanupOutcome::Cleared
        );
        assert!(!cache_root.exists());
        assert!(!service.measure().unwrap().clear_all_scheduled);
    }

    #[test]
    fn a_new_authorized_identity_reserves_an_independent_empty_namespace() {
        let root = tempfile::tempdir().expect("temporary fresh-identity fixture");
        let app_paths =
            AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        std::fs::create_dir_all(root.path().join("roaming")).expect("the roaming root exists");
        std::fs::create_dir_all(root.path().join("local")).expect("the local root exists");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let first = create_project(&core, root.path().join("first.myalbuns"));
        let fresh = create_project(&core, root.path().join("fresh.myalbuns"));
        let service = CacheService::new(app_paths);
        let first_owner = service
            .reserve_namespace(first.identity_authority())
            .unwrap();
        std::fs::write(
            first_owner.namespace().paths().metadata_file(),
            b"existing namespace",
        )
        .expect("the first namespace has observable state");
        let fresh_owner = service
            .reserve_namespace(fresh.identity_authority())
            .unwrap();

        assert_ne!(
            first_owner.namespace().paths(),
            fresh_owner.namespace().paths()
        );
        assert!(!fresh_owner.namespace().paths().metadata_file().exists());
        assert_eq!(
            std::fs::read_dir(fresh_owner.namespace().paths().media_directory())
                .expect("the fresh Media directory is readable")
                .count(),
            0
        );
    }

    #[test]
    fn cache_consumes_authoritative_identity_transitions_without_owning_them() {
        let root = tempfile::tempdir().expect("temporary identity-consumer fixture");
        let app_paths = app_paths(root.path());
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let original_path = root.path().join("Original.myalbuns");
        let moved_path = root.path().join("Movido.myalbuns");
        let alias_path = root.path().join("Alias.myalbuns");
        let external_path = root.path().join("Copia externa.myalbuns");
        let read_only_path = root.path().join("Copia somente leitura.myalbuns");
        let service = CacheService::new(app_paths.clone());

        // #10 is the producer. Cache only consumes the emitted authority.
        let original = create_project(&core, original_path.clone());
        let original_id = original.project_id();
        let original_owner = service
            .reserve_namespace(original.identity_authority())
            .expect("the original authority reserves Cache");
        let original_cache = original_owner.namespace().paths().clone();
        std::fs::write(original_cache.metadata_file(), b"identity-owned Cache")
            .expect("the original namespace has observable state");
        drop(original_owner);
        drop(original);

        std::fs::rename(&original_path, &moved_path)
            .expect("the operating system moves the closed Project");
        let moved = core
            .open_editable(OpenProjectRequest::new(project_location(&moved_path)))
            .expect("ProjectCore authorizes the confirmed movement");
        let moved_owner = service
            .reserve_namespace(moved.identity_authority())
            .expect("Cache consumes the movement authority");
        assert_eq!(moved.project_id(), original_id);
        assert_eq!(moved_owner.namespace().paths(), &original_cache);
        assert_eq!(
            std::fs::read(original_cache.metadata_file()).unwrap(),
            b"identity-owned Cache"
        );

        std::fs::hard_link(&moved_path, &alias_path)
            .expect("the second pathname aliases the same physical Project");
        assert!(matches!(
            core.open_editable(OpenProjectRequest::new(project_location(&alias_path))),
            Err(OpenProjectError::FocusExisting { project_id, .. }) if project_id == original_id
        ));
        assert_eq!(service.measure().unwrap().namespace_count, 1);

        std::fs::copy(&moved_path, &external_path)
            .expect("the external writable copy is a distinct physical file");
        let external = core
            .open_editable(OpenProjectRequest::new(project_location(&external_path)))
            .expect("ProjectCore promotes the writable external copy");
        let external_owner = service
            .reserve_namespace(external.identity_authority())
            .expect("Cache consumes only the promoted authority");
        assert_ne!(external.project_id(), original_id);
        assert_ne!(external_owner.namespace().paths(), &original_cache);
        assert_eq!(
            std::fs::read_dir(external_owner.namespace().paths().media_directory())
                .unwrap()
                .count(),
            0
        );

        std::fs::copy(&moved_path, &read_only_path)
            .expect("the external read-only copy is materialized");
        let mut permissions = std::fs::metadata(&read_only_path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&read_only_path, permissions)
            .expect("the external copy becomes read-only");
        let refused = core
            .open_editable(OpenProjectRequest::new(project_location(&read_only_path)))
            .expect_err("a read-only copy cannot emit editable identity authority");
        assert!(matches!(
            refused,
            OpenProjectError::ExternalCopyNotWritable(_)
        ));
        assert_eq!(
            service.measure().unwrap().namespace_count,
            2,
            "Cache mounts nothing for the refused transition"
        );
        make_writable(&read_only_path);

        assert!(
            original_cache.root().starts_with(app_paths.local_root()),
            "Project pathnames never relocate Cache outside LocalAppData"
        );
    }

    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "this Windows-only test restores its own disposable fixture"
    )]
    fn make_writable(path: &Path) {
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn concurrent_callers_schedule_one_idempotent_total_cleanup() {
        let root = tempfile::tempdir().expect("temporary scheduling race fixture");
        let paths = app_paths(root.path());
        let _active_operation = OperationGate::new(&paths)
            .try_acquire()
            .expect("an active Processor or Export owns the operation gate");
        let barrier = Arc::new(Barrier::new(9));
        let callers = (0..8)
            .map(|_| {
                let paths = paths.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    CacheService::new(paths).clear_all_or_schedule()
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();

        for caller in callers {
            assert_eq!(
                caller.join().expect("the scheduling caller does not panic"),
                Ok(CacheClearAllOutcome::Scheduled)
            );
        }
        assert!(
            CacheService::new(paths)
                .measure()
                .unwrap()
                .clear_all_scheduled
        );
    }

    #[test]
    fn namespace_reservation_survives_process_boundaries_and_owner_termination() {
        let root = tempfile::tempdir().expect("temporary namespace process fixture");
        let ready = root.path().join("namespace-owner.ready");
        let paths = app_paths(root.path());
        let namespace_name = "project-process-reservation";
        let cache = paths.project_cache(namespace_name).unwrap();
        let storage = paths.prepare_cache_storage(&cache).unwrap();
        std::fs::write(cache.media_directory().join("active.bin"), b"active Cache")
            .expect("the active Cache fixture is writable");
        drop(storage);
        let mut owner = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("cache_service::tests::cache_namespace_owner_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(NAMESPACE_OWNER_ROOT_ENV, root.path())
            .env(NAMESPACE_OWNER_NAME_ENV, namespace_name)
            .env(NAMESPACE_OWNER_READY_ENV, &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the independent Cache owner starts");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.is_file() {
            assert!(owner.try_wait().unwrap().is_none());
            assert!(
                Instant::now() < deadline,
                "the owner did not reserve in time"
            );
            thread::sleep(Duration::from_millis(20));
        }

        let service = CacheService::new(paths.clone());
        let protected = service.free_closed_projects().unwrap();
        assert_eq!(protected.removed_namespace_count, 0);
        assert_eq!(protected.skipped_active_namespace_count, 1);
        assert!(cache.root().is_dir());

        owner.kill().expect("the independent owner is terminated");
        owner.wait().expect("the terminated owner is reaped");
        let released = service.free_closed_projects().unwrap();
        assert_eq!(released.removed_namespace_count, 1);
        assert!(!cache.root().exists());
    }

    #[test]
    #[ignore = "spawned by the real Cache namespace process test"]
    fn cache_namespace_owner_process() {
        let root = std::path::PathBuf::from(
            env::var_os(NAMESPACE_OWNER_ROOT_ENV).expect("the owner root is configured"),
        );
        let name = env::var(NAMESPACE_OWNER_NAME_ENV).expect("the namespace is configured");
        let ready =
            env::var_os(NAMESPACE_OWNER_READY_ENV).expect("the owner ready path is configured");
        let paths = app_paths(&root);
        let cache = paths
            .project_cache(&name)
            .expect("the namespace plan is valid");
        let _reservation = namespace_mutex(&paths, &cache)
            .try_acquire()
            .expect("the child owns the namespace reservation");
        std::fs::write(ready, b"owned").expect("the child signals reservation ownership");
        thread::sleep(Duration::from_secs(120));
    }
}
