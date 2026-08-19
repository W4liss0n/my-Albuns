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
    cache_engine::{AuthorizedCacheNamespace, CacheEngine},
    ipc_contract::{
        CacheClearAllOutcome, CacheFreeResult, CacheServiceCommandError,
        CacheServiceCommandErrorCode, CacheServiceStatus,
    },
    named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant},
    operation_gate::{OperationGate, OperationGateError},
    processor_lifetime::await_cache_writer_quiescence,
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

#[derive(Clone, Debug)]
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
        synchronize_cache_writer(&self.app_paths, namespace.paths())?;
        let recovery = CacheEngine::recover_reserved_namespace(&self.app_paths, &namespace)
            .map_err(|error| CacheServiceError::Storage(error.message))?;
        if recovery.removed_temporary_count > 0
            || recovery.removed_generation_count > 0
            || recovery.discarded_index
        {
            tracing::info!(
                target: "myalbuns.desktop",
                removed_temporary_count = recovery.removed_temporary_count,
                removed_generation_count = recovery.removed_generation_count,
                discarded_index = recovery.discarded_index,
                event = "cache_namespace_recovered",
            );
        }
        Ok(CacheNamespaceOwner {
            namespace,
            _reservation: reservation,
        })
    }

    pub(crate) fn measure(&self) -> Result<CacheServiceStatus, CacheServiceError> {
        let namespaces = self.list_namespaces()?;
        let mut occupied_bytes = 0_u64;
        let mut releasable_bytes = 0_u64;
        let mut namespace_count = 0_usize;
        let mut releasable_namespace_count = 0_usize;
        for paths in namespaces {
            match namespace_mutex(&self.app_paths, &paths).try_acquire() {
                Ok(_reservation) => {
                    synchronize_cache_writer(&self.app_paths, &paths)?;
                    let Some(usage) = self.inspect_namespace(&paths)? else {
                        continue;
                    };
                    occupied_bytes = checked_add_usage(occupied_bytes, usage.bytes())?;
                    releasable_bytes = checked_add_usage(releasable_bytes, usage.bytes())?;
                    namespace_count += 1;
                    releasable_namespace_count += 1;
                }
                Err(NamedMutexError::Conflict) => {
                    // An active namespace cannot be quiesced without blocking its editor. Take a
                    // guarded snapshot, then retry the reservation: if the owner closed during
                    // inspection, discard that snapshot and measure again after quiescence.
                    let active_snapshot = self.snapshot_active_namespace(&paths)?;
                    match namespace_mutex(&self.app_paths, &paths).try_acquire() {
                        Ok(_reservation) => {
                            synchronize_cache_writer(&self.app_paths, &paths)?;
                            let Some(usage) = self.inspect_namespace(&paths)? else {
                                continue;
                            };
                            occupied_bytes = checked_add_usage(occupied_bytes, usage.bytes())?;
                            releasable_bytes = checked_add_usage(releasable_bytes, usage.bytes())?;
                            namespace_count += 1;
                            releasable_namespace_count += 1;
                        }
                        Err(NamedMutexError::Conflict) => {
                            if let Some(usage) = active_snapshot {
                                occupied_bytes = checked_add_usage(occupied_bytes, usage.bytes())?;
                                namespace_count += 1;
                            }
                        }
                        Err(NamedMutexError::Unavailable(reason)) => {
                            return Err(CacheServiceError::Reservation(reason));
                        }
                    }
                }
                Err(NamedMutexError::Unavailable(reason)) => {
                    return Err(CacheServiceError::Reservation(reason));
                }
            }
        }
        Ok(CacheServiceStatus {
            occupied_bytes,
            releasable_bytes,
            namespace_count,
            releasable_namespace_count,
            clear_all_scheduled: self.clear_is_scheduled()?,
        })
    }

    pub(crate) fn free_closed_projects(&self) -> Result<CacheFreeResult, CacheServiceError> {
        let namespaces = self.list_namespaces()?;
        let mut reserved = Vec::new();
        let mut skipped_active_namespace_count = 0_usize;
        for paths in namespaces {
            match namespace_mutex(&self.app_paths, &paths).try_acquire() {
                Ok(reservation) => {
                    synchronize_cache_writer(&self.app_paths, &paths)?;
                    if let Some(usage) = self.inspect_namespace(&paths)? {
                        reserved.push((usage, reservation));
                    }
                }
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
        let namespaces = self.list_namespaces()?;
        let mut reserved = Vec::with_capacity(namespaces.len());
        for paths in namespaces {
            match namespace_mutex(&self.app_paths, &paths).try_acquire() {
                Ok(reservation) => {
                    synchronize_cache_writer(&self.app_paths, &paths)?;
                    if let Some(usage) = self.inspect_namespace(&paths)? {
                        reserved.push((usage, reservation));
                    }
                }
                Err(NamedMutexError::Conflict) => return Ok(None),
                Err(NamedMutexError::Unavailable(reason)) => {
                    return Err(CacheServiceError::Reservation(reason));
                }
            }
        }
        let measured_releasable_bytes = reserved.iter().try_fold(0_u64, |total, (usage, _)| {
            checked_add_usage(total, usage.bytes())
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
        self.clear_schedule_marker()?;
        Ok(Some(CacheFreeResult {
            measured_releasable_bytes,
            freed_bytes,
            removed_namespace_count,
            skipped_active_namespace_count: 0,
        }))
    }

    fn list_namespaces(&self) -> Result<Vec<CachePathPlan>, CacheServiceError> {
        self.app_paths
            .list_cache_namespaces()
            .map_err(|error| CacheServiceError::Storage(error.to_string()))
    }

    fn inspect_namespace(
        &self,
        paths: &CachePathPlan,
    ) -> Result<Option<CacheNamespaceUsage>, CacheServiceError> {
        self.app_paths
            .inspect_cache_namespace(paths)
            .map_err(|error| CacheServiceError::Storage(error.to_string()))
    }

    fn snapshot_active_namespace(
        &self,
        paths: &CachePathPlan,
    ) -> Result<Option<CacheNamespaceUsage>, CacheServiceError> {
        self.app_paths
            .snapshot_active_cache_namespace(paths)
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

fn synchronize_cache_writer(
    app_paths: &AppPaths,
    paths: &CachePathPlan,
) -> Result<(), CacheServiceError> {
    await_cache_writer_quiescence(app_paths, paths)
        .map_err(|error| CacheServiceError::Reservation(error.to_string()))
}

fn mutex_name(app_paths: &AppPaths, kind: &str, scope: &str) -> String {
    let mut digest = Sha256::new();
    for unit in app_paths.local_root().as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    digest.update([0]);
    // Cache namespace components are ASCII, while the Windows filesystem
    // normally compares their pathnames without regard to casing. The mutex
    // key must preserve that same equivalence or an enumerated casing alias
    // could bypass the active Host's reservation.
    for byte in scope.bytes() {
        digest.update([byte.to_ascii_lowercase()]);
    }
    let digest = digest.finalize();
    let suffix = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(r"Local\MyAlbuns.{kind}.v1.{suffix}")
}

fn checked_add_usage(total: u64, bytes: u64) -> Result<u64, CacheServiceError> {
    total
        .checked_add(bytes)
        .ok_or_else(|| CacheServiceError::Storage("volume de Cache excedeu u64".into()))
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

async fn run_cache_service_operation<T, F>(operation: F) -> Result<T, CacheServiceError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CacheServiceError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| {
            CacheServiceError::Storage(format!(
                "a tarefa de manutenção do Cache não pôde ser concluída: {error}"
            ))
        })?
}

#[tauri::command]
pub(crate) async fn cache_service_status(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheServiceStatus, CacheServiceCommandError> {
    let service = service.inner().clone();
    run_cache_service_operation(move || service.measure())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn free_closed_project_cache(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheFreeResult, CacheServiceCommandError> {
    let service = service.inner().clone();
    run_cache_service_operation(move || service.free_closed_projects())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) async fn clear_all_cache(
    service: tauri::State<'_, CacheService>,
) -> Result<CacheClearAllOutcome, CacheServiceCommandError> {
    let service = service.inner().clone();
    run_cache_service_operation(move || service.clear_all_or_schedule())
        .await
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::c_void,
        io::{BufRead, BufReader, Write},
        os::windows::fs::OpenOptionsExt,
        os::windows::io::AsRawHandle,
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
    use myalbuns_imaging_protocol::{CACHE_REPRESENTATION_VERSION, CacheRepresentationPolicy};
    use myalbuns_paths::{
        AppPaths, CacheArtifactFormat, CachePathPlan, OperationPathContext, ProcessInstanceId,
        project_data_namespace,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };

    use crate::ipc_contract::CacheClearAllOutcome;
    use crate::operation_gate::OperationGate;
    use crate::processor_lifetime::ProcessorChildLifetime;

    use super::{
        CacheScheduledCleanupOutcome, CacheService, CacheServiceError, namespace_mutex,
        run_cache_service_operation,
    };

    const NAMESPACE_OWNER_ROOT_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_ROOT";
    const NAMESPACE_OWNER_NAME_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_NAME";
    const NAMESPACE_OWNER_READY_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_READY";
    const NAMESPACE_OWNER_CHURN_ENV: &str = "MYALBUNS_CACHE_NAMESPACE_OWNER_CHURN";
    const HOST_DEATH_ROOT_ENV: &str = "MYALBUNS_CACHE_HOST_DEATH_ROOT";
    const HOST_DEATH_PROJECT_ENV: &str = "MYALBUNS_CACHE_HOST_DEATH_PROJECT";
    const HOST_DEATH_READY_ENV: &str = "MYALBUNS_CACHE_HOST_DEATH_READY";
    const HOST_DEATH_NAMESPACE_ENV: &str = "MYALBUNS_CACHE_HOST_DEATH_NAMESPACE";
    const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
    const ABANDONED_CACHE_PAYLOAD: &[u8] = b"partial Cache payload";

    fn app_paths(root: &Path) -> AppPaths {
        let roaming = root.join("roaming");
        let local = root.join("local");
        std::fs::create_dir_all(&roaming).expect("the roaming root exists");
        std::fs::create_dir_all(&local).expect("the local root exists");
        AppPaths::from_roots(&roaming, &local)
    }

    #[test]
    fn cache_service_operations_leave_the_tauri_caller_thread() {
        tauri::async_runtime::block_on(async {
            let caller = thread::current().id();
            let worker = run_cache_service_operation(move || {
                Ok::<_, CacheServiceError>(thread::current().id())
            })
            .await
            .expect("the blocking Cache operation completes");

            assert_ne!(
                worker, caller,
                "filesystem traversal must not run on the Tauri caller thread"
            );
        });
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

    fn closed_cache_with_stale_writer_claim(
        root: &Path,
        namespace: &str,
        payload: &[u8],
    ) -> (AppPaths, CachePathPlan) {
        let paths = app_paths(root);
        let cache = paths
            .project_cache(namespace)
            .expect("the closed Cache namespace is valid");
        drop(
            paths
                .prepare_cache_storage(&cache)
                .expect("the closed Cache storage is prepared"),
        );
        std::fs::write(cache.media_directory().join("payload.bin"), payload)
            .expect("the closed Cache payload is writable");
        let current = ProcessInstanceId::current().expect("the test process has an identity");
        let stale_creation_time = current
            .creation_time_wire()
            .checked_add(1)
            .expect("the process FILETIME is not maximal");
        let claim = serde_json::json!({
            "schemaVersion": 1,
            "process": {
                "processId": current.process_id(),
                "creationTime": stale_creation_time,
            },
        });
        std::fs::write(
            cache.root().join(".processor-writer.v1.json"),
            serde_json::to_vec(&claim).expect("the stale writer claim serializes"),
        )
        .expect("the stale writer claim is writable");
        (paths, cache)
    }

    struct DeadHostCacheFixture {
        _root: tempfile::TempDir,
        paths: AppPaths,
        core: ProjectCore,
        project_path: std::path::PathBuf,
        cache: CachePathPlan,
        abandoned: std::path::PathBuf,
        writer_claim: std::path::PathBuf,
        processor: HANDLE,
    }

    impl DeadHostCacheFixture {
        fn assert_processor_signaled(&self) {
            // SAFETY: the fixture keeps its exact Processor handle open until Drop.
            assert_eq!(
                unsafe { WaitForSingleObject(self.processor, 0) },
                WAIT_OBJECT_0
            );
        }
    }

    impl Drop for DeadHostCacheFixture {
        fn drop(&mut self) {
            // SAFETY: the fixture owns this non-null handle and closes it exactly once.
            unsafe { CloseHandle(self.processor) };
        }
    }

    fn dead_host_cache_fixture() -> DeadHostCacheFixture {
        let root = tempfile::tempdir().expect("temporary Host-death Cache fixture");
        let ready = root.path().join("processor.ready");
        let paths = app_paths(root.path());
        let project_path = root.path().join("Projeto.myalbuns");
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let project = create_project(&core, project_path.clone());
        let namespace_name = project_data_namespace(&project.project_id().hyphenated().to_string());
        drop(project);
        let mut host = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("cache_service::tests::cache_host_process_with_active_processor")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(HOST_DEATH_ROOT_ENV, root.path())
            .env(HOST_DEATH_PROJECT_ENV, &project_path)
            .env(HOST_DEATH_READY_ENV, &ready)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the independent Project Host starts");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !ready.is_file() {
            assert!(
                host.try_wait()
                    .expect("the Host state is readable")
                    .is_none(),
                "the Host exited before its contained Processor became active"
            );
            assert!(
                Instant::now() < deadline,
                "the Processor readiness timed out"
            );
            thread::sleep(Duration::from_millis(20));
        }
        let processor_id = std::fs::read_to_string(&ready)
            .expect("the Processor readiness is readable")
            .trim()
            .parse::<u32>()
            .expect("the Processor reports its PID");
        let cache = paths
            .project_cache(&namespace_name)
            .expect("the authorized namespace plan is valid");
        let abandoned = cache
            .preview_temporary_file(
                "host-death-media",
                "abandoned-generation",
                CacheArtifactFormat::Jpeg,
                processor_id,
            )
            .expect("the Processor temporary path is valid");
        assert!(abandoned.is_file());
        let writer_claim = cache.root().join(".processor-writer.v1.json");
        assert!(
            writer_claim.is_file(),
            "the Host publishes the exact contained writer before dispatch"
        );

        // SAFETY: the PID was published by the live child; the returned fixture
        // owns and closes this exact synchronization handle.
        let processor = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, processor_id) };
        assert!(!processor.is_null(), "the active Processor can be observed");
        // SAFETY: processor is a live synchronization handle.
        assert_eq!(unsafe { WaitForSingleObject(processor, 0) }, WAIT_TIMEOUT);

        host.kill()
            .expect("the Project Host is terminated abruptly");
        host.wait().expect("the terminated Host is reaped");

        DeadHostCacheFixture {
            _root: root,
            paths,
            core,
            project_path,
            cache,
            abandoned,
            writer_claim,
            processor,
        }
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
    fn active_namespace_with_different_windows_casing_is_never_releasable() {
        let root = tempfile::tempdir().expect("temporary Cache casing fixture");
        let paths = app_paths(root.path());
        let core = ProjectCore::new().with_identity_storage_roots(
            root.path().join("leases"),
            root.path().join("identities"),
        );
        let project = create_project(&core, root.path().join("project.myalbuns"));
        let canonical_namespace =
            project_data_namespace(&project.project_id().hyphenated().to_string());
        let stored_namespace = canonical_namespace.to_ascii_uppercase();
        let stored_paths = paths
            .project_cache(&stored_namespace)
            .expect("the differently-cased namespace remains structurally valid");
        drop(
            paths
                .prepare_cache_storage(&stored_paths)
                .expect("Windows creates the stored casing before reservation"),
        );

        let service = CacheService::new(paths);
        let owner = service
            .reserve_namespace(project.identity_authority())
            .expect("the canonical identity reserves the same physical namespace");
        std::fs::write(
            stored_paths.media_directory().join("active.bin"),
            b"active Cache",
        )
        .expect("the active Cache payload is writable through the stored casing");

        let measured = service.measure().expect("active Cache usage is measurable");
        assert_eq!(measured.occupied_bytes, 12);
        assert_eq!(measured.releasable_bytes, 0);
        assert_eq!(measured.namespace_count, 1);
        assert_eq!(measured.releasable_namespace_count, 0);

        let released = service
            .free_closed_projects()
            .expect("free space skips the differently-cased active namespace");
        assert_eq!(released.measured_releasable_bytes, 0);
        assert_eq!(released.freed_bytes, 0);
        assert_eq!(released.removed_namespace_count, 0);
        assert_eq!(released.skipped_active_namespace_count, 1);
        assert!(stored_paths.root().is_dir());

        drop(owner);
        assert_eq!(
            service
                .free_closed_projects()
                .expect("the same namespace becomes releasable after its owner closes")
                .freed_bytes,
            12
        );
        assert!(!stored_paths.root().exists());
    }

    #[test]
    fn free_closed_projects_quiesces_writers_before_measuring_removed_bytes() {
        let root = tempfile::tempdir().expect("temporary free-after-writer fixture");
        let payload = b"closed Cache payload";
        let (paths, cache) =
            closed_cache_with_stale_writer_claim(root.path(), "closed-free", payload);

        let freed = CacheService::new(paths)
            .free_closed_projects()
            .expect("the stale writer is quiesced before freeing its namespace");

        assert_eq!(freed.measured_releasable_bytes, payload.len() as u64);
        assert_eq!(freed.freed_bytes, payload.len() as u64);
        assert_eq!(freed.removed_namespace_count, 1);
        assert!(!cache.root().exists());
    }

    #[test]
    fn clear_all_quiesces_writers_before_measuring_removed_bytes() {
        let root = tempfile::tempdir().expect("temporary clear-after-writer fixture");
        let payload = b"clearable Cache payload";
        let (paths, cache) =
            closed_cache_with_stale_writer_claim(root.path(), "closed-clear", payload);

        let cleared = CacheService::new(paths)
            .clear_all_or_schedule()
            .expect("the stale writer is quiesced before total cleanup");

        let CacheClearAllOutcome::Cleared { result } = cleared else {
            panic!("a closed namespace can be cleared immediately");
        };
        assert_eq!(result.measured_releasable_bytes, payload.len() as u64);
        assert_eq!(result.freed_bytes, payload.len() as u64);
        assert_eq!(result.removed_namespace_count, 1);
        assert!(!cache.root().exists());
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
        let valid_cache_metadata = serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 5,
            "representationVersion": CACHE_REPRESENTATION_VERSION,
            "projectId": original_id.hyphenated().to_string(),
            "lastUsedUnixMs": 1,
            "policy": CacheRepresentationPolicy::measured_v1(),
            "entries": [],
        }))
        .expect("the current empty Cache index serializes");
        std::fs::write(original_cache.metadata_file(), &valid_cache_metadata)
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
            valid_cache_metadata
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
    fn project_open_during_free_space_is_serialized_by_namespace_reservation() {
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
    fn active_namespace_measurement_tolerates_real_writer_promotion_and_exclusive_files() {
        let root = tempfile::tempdir().expect("temporary active-writer fixture");
        let ready = root.path().join("active-writer.ready");
        let paths = app_paths(root.path());
        let namespace_name = "project-active-writer";
        let cache = paths.project_cache(namespace_name).unwrap();
        drop(paths.prepare_cache_storage(&cache).unwrap());
        let mut owner = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("cache_service::tests::cache_namespace_owner_process")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(NAMESPACE_OWNER_ROOT_ENV, root.path())
            .env(NAMESPACE_OWNER_NAME_ENV, namespace_name)
            .env(NAMESPACE_OWNER_READY_ENV, &ready)
            .env(NAMESPACE_OWNER_CHURN_ENV, "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the independent active writer starts");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.is_file() {
            assert!(owner.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "the active writer timed out");
            thread::sleep(Duration::from_millis(20));
        }

        let service = CacheService::new(paths);
        for iteration in 0..40 {
            let measured = service
                .measure()
                .expect("active promotion/removal cannot make measurement unavailable");
            assert_eq!(measured.namespace_count, 1);
            assert_eq!(measured.releasable_namespace_count, 0);
            assert_eq!(measured.releasable_bytes, 0);
            assert!(measured.occupied_bytes >= b"exclusive active Cache".len() as u64);
            if iteration % 10 == 0 {
                let freed = service
                    .free_closed_projects()
                    .expect("free-space skips the concurrently active namespace");
                assert_eq!(freed.removed_namespace_count, 0);
                assert_eq!(freed.skipped_active_namespace_count, 1);
                assert_eq!(
                    service
                        .clear_all_or_schedule()
                        .expect("total cleanup is scheduled while the writer is active"),
                    CacheClearAllOutcome::Scheduled
                );
                assert!(cache.root().is_dir());
            }
        }

        owner.kill().expect("the active writer is stopped");
        owner.wait().expect("the active writer is reaped");
    }

    #[test]
    fn reopening_after_host_death_recovers_the_contained_processors_temporary() {
        let fixture = dead_host_cache_fixture();
        let measured = CacheService::new(fixture.paths.clone())
            .measure()
            .expect("measurement quiesces the dead Host's contained writer");
        assert_eq!(
            measured.occupied_bytes,
            ABANDONED_CACHE_PAYLOAD.len() as u64,
            "the consumed writer claim is not reported as occupied Cache"
        );
        assert_eq!(measured.releasable_bytes, measured.occupied_bytes);
        fixture.assert_processor_signaled();

        let reopened = fixture
            .core
            .open_editable(OpenProjectRequest::new(project_location(
                &fixture.project_path,
            )))
            .expect("the Project reopens after the dead Host releases its authority");
        let owner = CacheService::new(fixture.paths.clone())
            .reserve_namespace(reopened.identity_authority())
            .expect("the new Host reserves and recovers the namespace");
        assert_eq!(owner.namespace().paths(), &fixture.cache);
        fixture.assert_processor_signaled();
        assert!(
            !fixture.abandoned.exists(),
            "exclusive startup recovery removes the proved-abandoned temporary"
        );
        assert!(
            !fixture.writer_claim.exists(),
            "recovery removes the consumed exact writer claim"
        );
    }

    #[test]
    fn free_closed_projects_after_host_death_waits_before_measuring_and_removing() {
        let fixture = dead_host_cache_fixture();

        let freed = CacheService::new(fixture.paths.clone())
            .free_closed_projects()
            .expect("freeing waits for the dead Host's contained writer");

        assert_eq!(
            freed.measured_releasable_bytes,
            ABANDONED_CACHE_PAYLOAD.len() as u64
        );
        assert_eq!(freed.freed_bytes, freed.measured_releasable_bytes);
        assert_eq!(freed.removed_namespace_count, 1);
        assert_eq!(freed.skipped_active_namespace_count, 0);
        fixture.assert_processor_signaled();
        assert!(!fixture.cache.root().exists());
    }

    #[test]
    fn clear_all_after_host_death_waits_before_measuring_and_removing() {
        let fixture = dead_host_cache_fixture();

        let outcome = CacheService::new(fixture.paths.clone())
            .clear_all_or_schedule()
            .expect("total cleanup waits for the dead Host's contained writer");

        let CacheClearAllOutcome::Cleared { result } = outcome else {
            panic!("the abandoned Host no longer blocks immediate total cleanup");
        };
        assert_eq!(
            result.measured_releasable_bytes,
            ABANDONED_CACHE_PAYLOAD.len() as u64
        );
        assert_eq!(result.freed_bytes, result.measured_releasable_bytes);
        assert_eq!(result.removed_namespace_count, 1);
        fixture.assert_processor_signaled();
        assert!(!fixture.cache.root().exists());
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
        let churn = env::var_os(NAMESPACE_OWNER_CHURN_ENV).is_some();
        let _exclusive = churn.then(|| {
            let path = cache.media_directory().join("exclusive.bin");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .share_mode(0)
                .open(path)
                .expect("the active writer owns an exclusive Cache file");
            file.write_all(b"exclusive active Cache")
                .expect("the exclusive Cache payload is written");
            file.sync_all()
                .expect("the exclusive Cache payload is synchronized");
            file
        });
        std::fs::write(ready, b"owned").expect("the child signals reservation ownership");
        if churn {
            let temporary = cache.media_directory().join("promotion.tmp");
            let published = cache.media_directory().join("promotion.bin");
            let deadline = Instant::now() + Duration::from_secs(120);
            while Instant::now() < deadline {
                std::fs::write(&temporary, b"candidate").expect("the candidate is written");
                std::fs::rename(&temporary, &published).expect("the candidate is promoted");
                std::fs::remove_file(&published).expect("the promoted generation becomes obsolete");
                thread::yield_now();
            }
        } else {
            thread::sleep(Duration::from_secs(120));
        }
    }

    #[test]
    #[ignore = "spawned by the real Host-death Cache recovery test"]
    fn cache_host_process_with_active_processor() {
        let root = std::path::PathBuf::from(
            env::var_os(HOST_DEATH_ROOT_ENV).expect("the Host-death root is configured"),
        );
        let project_path = std::path::PathBuf::from(
            env::var_os(HOST_DEATH_PROJECT_ENV).expect("the Project path is configured"),
        );
        let ready =
            env::var_os(HOST_DEATH_READY_ENV).expect("the Processor ready path is configured");
        let paths = app_paths(&root);
        let core = ProjectCore::new()
            .with_identity_storage_roots(root.join("leases"), root.join("identities"));
        let project = core
            .open_editable(OpenProjectRequest::new(project_location(&project_path)))
            .expect("the child Host opens the authorized Project");
        let service = CacheService::new(paths);
        let owner = service
            .reserve_namespace(project.identity_authority())
            .expect("the child Host reserves its Cache namespace");
        let namespace = owner
            .namespace()
            .paths()
            .root()
            .file_name()
            .expect("the namespace has a filename")
            .to_owned();
        let mut worker = Command::new(env::current_exe().expect("the test executable is known"))
            .arg("cache_service::tests::cache_abandoned_temporary_processor")
            .args(["--ignored", "--exact", "--nocapture"])
            .env(HOST_DEATH_ROOT_ENV, &root)
            .env(HOST_DEATH_NAMESPACE_ENV, namespace)
            .env(HOST_DEATH_READY_ENV, &ready)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the contained Processor starts");
        let worker_identity = ProcessInstanceId::from_process_handle(
            worker.id(),
            worker.as_raw_handle().cast::<c_void>(),
        )
        .expect("the exact contained Processor identity is observable");
        let mut lifetime = ProcessorChildLifetime::attach(worker_identity)
            .expect("the Host contains the Processor before dispatch");
        lifetime
            .publish_cache_writer_claim(&service.app_paths, owner.namespace().paths())
            .expect("the Host publishes the contained Processor claim before dispatch");
        worker
            .stdin
            .as_mut()
            .expect("the Processor stdin is available")
            .write_all(b"dispatch\n")
            .expect("the Host dispatches the Cache write");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !std::path::Path::new(&ready).is_file() {
            assert!(worker.try_wait().unwrap().is_none());
            assert!(
                Instant::now() < deadline,
                "the Processor did not write in time"
            );
            thread::sleep(Duration::from_millis(20));
        }
        thread::sleep(Duration::from_secs(120));
        worker
            .wait()
            .expect("the contained Processor is reaped if the Host is not terminated");
    }

    #[test]
    #[ignore = "spawned by the real Host-death Cache recovery test"]
    fn cache_abandoned_temporary_processor() {
        let root = std::path::PathBuf::from(
            env::var_os(HOST_DEATH_ROOT_ENV).expect("the Host-death root is configured"),
        );
        let namespace =
            env::var(HOST_DEATH_NAMESPACE_ENV).expect("the Cache namespace is configured");
        let ready =
            env::var_os(HOST_DEATH_READY_ENV).expect("the Processor ready path is configured");
        let mut dispatch = String::new();
        BufReader::new(std::io::stdin())
            .read_line(&mut dispatch)
            .expect("the Processor receives dispatch");
        assert_eq!(dispatch, "dispatch\n");
        let paths = app_paths(&root);
        let cache = paths
            .project_cache(&namespace)
            .expect("the Cache plan is valid");
        let storage = paths
            .prepare_cache_storage(&cache)
            .expect("the held namespace is prepared");
        let temporary = cache
            .preview_temporary_file(
                "host-death-media",
                "abandoned-generation",
                CacheArtifactFormat::Jpeg,
                std::process::id(),
            )
            .expect("the temporary path is valid");
        let final_path = cache
            .preview_file(
                "host-death-media",
                "abandoned-generation",
                CacheArtifactFormat::Jpeg,
            )
            .expect("the candidate path is valid");
        let mut publication = storage
            .begin_file_publication(&temporary, &final_path)
            .expect("the Processor owns an in-flight publication");
        publication
            .write_all(ABANDONED_CACHE_PAYLOAD)
            .expect("the Processor writes a partial payload");
        publication.flush().expect("the partial payload is visible");
        std::fs::write(ready, std::process::id().to_string())
            .expect("the Processor publishes readiness");
        thread::sleep(Duration::from_secs(120));
    }
}
