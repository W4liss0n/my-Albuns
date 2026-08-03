#![cfg(windows)]

use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use myalbuns_paths::{
    AppPaths, CacheArtifactFormat, ExpectedObject, ExportPathPlan, OperationPathContext,
    PathRootKind, PhysicalIdentityEvidence, ProjectFileLock, ProjectFileLockError, ResolveError,
    RootBindingPlan,
};
use sha2::{Digest, Sha256};

const LOCAL_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_LOCAL_ROOT";
const UNC_ROOT_ENV: &str = "MYALBUNS_PATH_GATE_UNC_ROOT";
const DRIVE_ENV: &str = "MYALBUNS_PATH_GATE_DRIVE";
const EVIDENCE_ENV: &str = "MYALBUNS_PATH_GATE_EVIDENCE";
const LOCK_PROBE_PATH_ENV: &str = "MYALBUNS_PATH_GATE_LOCK_PROBE_PATH";
const LOCK_PROBE_EXPECT_ENV: &str = "MYALBUNS_PATH_GATE_LOCK_PROBE_EXPECT";

#[test]
#[ignore = "executed by scripts/Test-WindowsPathGate.ps1 with a writable UNC fixture"]
fn real_windows_paths_freeze_mapped_bindings_and_keep_unc_export_recoverable() {
    let local_root = required_path(LOCAL_ROOT_ENV);
    let unc_root = required_path(UNC_ROOT_ENV);
    let drive = env::var(DRIVE_ENV).expect("the mapped drive letter is configured");
    assert!(
        drive.len() == 2 && drive.ends_with(':'),
        "the mapped drive must use the form R:"
    );
    let _mapping = DriveMapping::reserve(&drive);

    let local_a = local_root.join("binding-a");
    let local_b = local_root.join("binding-b");
    let unc_a = unc_root.join("binding-a");
    let unc_b = unc_root.join("binding-b");
    std::fs::create_dir_all(&local_a).expect("binding A is materialized");
    std::fs::create_dir_all(local_b.join("exports")).expect("binding B is materialized");
    let local_source_a = local_a.join("source.bin");
    let local_source_b = local_b.join("source.bin");
    std::fs::write(&local_source_a, b"binding-a").expect("source A is writable");
    std::fs::write(&local_source_b, b"binding-b").expect("source B is writable");
    let logical_source = PathBuf::from(format!(r"{drive}\source.bin"));

    _mapping.map_to(&unc_a);
    let mut attempt_a = OperationPathContext::new();
    attempt_a
        .capture(&logical_source)
        .expect("attempt A captures the mapped root once");
    _mapping.map_to(&unc_b);
    attempt_a
        .capture(&PathBuf::from(format!(r"{drive}\another-source.bin")))
        .expect("the same attempt reuses its first root binding after a remap");
    let stable_attempt_a = attempt_a.freeze();
    assert_eq!(
        stable_attempt_a
            .resolve(&logical_source)
            .expect("the attempt remains bound to A"),
        unc_a.join("source.bin")
    );
    _mapping.map_to(&unc_a);
    let plan_a = operation_plan(&[
        logical_source.as_path(),
        local_source_a.as_path(),
        unc_a.join("source.bin").as_path(),
    ]);
    let resolved_a = plan_a
        .resolve(&logical_source)
        .expect("the mapped source resolves through binding A");
    assert_eq!(resolved_a, unc_a.join("source.bin"));
    assert_eq!(
        plan_a.compare_existing(
            &logical_source,
            &local_source_a,
            ExpectedObject::RegularFile,
        ),
        PhysicalIdentityEvidence::Same
    );
    let verbatim_logical_source = PathBuf::from(format!(r"\\?\{drive}\source.bin"));
    let verbatim_plan = operation_plan(&[verbatim_logical_source.as_path()]);
    assert_eq!(
        verbatim_plan
            .resolve(&verbatim_logical_source)
            .expect("the verbatim mapped root resolves to UNC"),
        unc_a.join("source.bin")
    );
    assert_eq!(
        plan_a.compare_existing(
            &logical_source,
            &unc_a.join("source.bin"),
            ExpectedObject::RegularFile,
        ),
        PhysicalIdentityEvidence::Same
    );
    let plan_a_wire = serde_json::to_vec(&plan_a).expect("plan A has a reversible wire form");
    let plan_a_round_trip: RootBindingPlan =
        serde_json::from_slice(&plan_a_wire).expect("plan A decodes");
    assert_eq!(plan_a_round_trip, plan_a);

    _mapping.map_to(&unc_b);
    assert_eq!(
        std::fs::read(
            plan_a
                .resolve(&logical_source)
                .expect("the old plan remains bound to A"),
        )
        .expect("binding A remains readable without the drive mapping"),
        b"binding-a"
    );
    let logical_output = PathBuf::from(format!(r"{drive}\exports\Album.png"));
    let plan_b = operation_plan(&[
        logical_source.as_path(),
        logical_output.as_path(),
        local_source_b.as_path(),
        unc_b.join("source.bin").as_path(),
    ]);
    assert_eq!(
        plan_b.resolve(&logical_source).expect("new attempt uses B"),
        unc_b.join("source.bin")
    );
    assert_eq!(
        plan_b.compare_existing(
            &logical_source,
            &local_source_b,
            ExpectedObject::RegularFile,
        ),
        PhysicalIdentityEvidence::Same
    );
    let plan_b_wire = serde_json::to_vec(&plan_b).expect("plan B serializes");
    assert_ne!(
        plan_hash(&plan_a_wire),
        plan_hash(&plan_b_wire),
        "an explicit retry captures a new binding plan"
    );

    let operational_output = plan_b
        .resolve(&logical_output)
        .expect("the UNC Export destination is bound");
    let export = ExportPathPlan::new(operational_output.clone(), "windows-path-gate")
        .expect("the UNC Export is planned");
    assert_eq!(
        export.preparation_directory().parent(),
        operational_output.parent(),
        "staging stays inside the UNC destination"
    );
    let prepared = export.prepare().expect("the UNC staging is reserved");
    std::fs::write(export.prepared_output_path(), b"unc export")
        .expect("the UNC staging file is writable");
    prepared.publish().expect("the UNC Export is published");
    assert_eq!(
        std::fs::read(&operational_output).expect("the UNC output is readable"),
        b"unc export"
    );
    assert!(!export.preparation_directory().exists());

    let app_data = local_root.join("application-data");
    let roaming_data = app_data.join("roaming");
    let local_data = app_data.join("local");
    std::fs::create_dir_all(&roaming_data).expect("the roaming Known Folder exists");
    std::fs::create_dir_all(&local_data).expect("the local Known Folder exists");
    let app_paths = AppPaths::from_known_folders(&roaming_data, &local_data);
    let cache = app_paths
        .project_cache("windows-path-gate")
        .expect("the local Cache is planned");
    let cache_storage = app_paths
        .prepare_cache_storage(&cache)
        .expect("the local Cache is prepared");
    let temporary = cache
        .preview_temporary_file("media", "generation", CacheArtifactFormat::Jpeg, 42)
        .expect("the Cache temporary path is safe");
    let published = cache
        .preview_file("media", "generation", CacheArtifactFormat::Jpeg)
        .expect("the Cache publication path is safe");
    let mut publication = cache_storage
        .begin_file_publication(&temporary, &published)
        .expect("the local Cache publication begins");
    publication
        .write_all(b"cached remote source")
        .expect("the local Cache is writable");
    publication
        .sync()
        .expect("the local Cache is synchronized")
        .publish()
        .expect("the local Cache is published");
    assert!(published.starts_with(app_paths.local_root()));
    assert!(!published.starts_with(&unc_root));
    drop(cache_storage);

    _mapping.unmap();
    let offline = local_root.join("binding-b-offline");
    std::fs::rename(&local_b, &offline).expect("binding B becomes unavailable");
    let offline_result = plan_b.resolve_existing(&logical_source, ExpectedObject::RegularFile);
    let offline_identity = plan_b.compare_existing(
        &logical_source,
        &local_source_b,
        ExpectedObject::RegularFile,
    );
    std::fs::rename(&offline, &local_b).expect("binding B is restored");
    assert_eq!(offline_result.unwrap_err(), ResolveError::Unavailable);
    assert_eq!(
        offline_identity,
        PhysicalIdentityEvidence::Indeterminate,
        "unavailable evidence must fail closed"
    );

    _mapping.map_to(&unc_a);
    let retry_plan = operation_plan(&[logical_source.as_path()]);
    assert_eq!(
        retry_plan
            .resolve(&logical_source)
            .expect("the explicit retry recaptures A"),
        unc_a.join("source.bin")
    );

    let local_project = local_a.join("Projeto.myalbum");
    let logical_project = PathBuf::from(format!(r"{drive}\Projeto.myalbum"));
    let unc_project = unc_a.join("Projeto.myalbum");
    std::fs::write(&local_project, b"project").expect("the Project file is writable");
    let identity_plan = operation_plan(&[
        logical_project.as_path(),
        unc_project.as_path(),
        local_project.as_path(),
    ]);
    assert_eq!(
        identity_plan
            .compare_existing(&logical_project, &unc_project, ExpectedObject::RegularFile,),
        PhysicalIdentityEvidence::Same,
        "mapped and UNC aliases provide the same physical identity evidence"
    );
    let editable_lock = ProjectFileLock::try_acquire(&logical_project)
        .expect("the first editable session acquires a real file lock");
    assert!(
        run_project_lock_probe(&unc_project, "conflict").success(),
        "another process cannot lock the physical alias"
    );
    assert!(
        run_project_lock_probe(&unc_project, "read").success(),
        "another process can still read the persisted revision"
    );
    drop(editable_lock);
    assert!(
        run_project_lock_probe(&unc_project, "acquired").success(),
        "dropping the owner releases the final file lock for another process"
    );

    app_paths
        .clear_project_cache(&cache)
        .expect("the isolated gate Cache is removed");
    write_evidence(serde_json::json!({
        "mappedRootKind": PathRootKind::Disk,
        "operationalRootKind": PathRootKind::Unc,
        "planRoundTripLossless": true,
        "firstPlanSha256": plan_hash(&plan_a_wire),
        "retryPlanSha256": plan_hash(&plan_b_wire),
        "plansDistinct": plan_a_wire != plan_b_wire,
        "mappedAndUncIdentity": "same",
        "mappedAndLocalIdentity": "same",
        "identityFailure": "indeterminate",
        "physicalAliasLockConflict": true,
        "readOnlyBatchAllowedWhileLocked": true,
        "lockReleasedAfterOwnerDrop": true,
        "bindingReusedAfterRemapWithinAttempt": true,
        "verbatimMappedBindingFrozenAsUnc": true,
        "uncExportPublished": true,
        "stagingInsideDestination": true,
        "cacheUnderLocalAppData": true,
        "unavailableBinding": "unavailable",
        "explicitRetryRecaptured": true,
    }));
}

#[test]
#[ignore = "spawned by the real Windows path gate as a second process"]
fn project_lock_probe_process() {
    let path = required_path(LOCK_PROBE_PATH_ENV);
    let expectation = env::var(LOCK_PROBE_EXPECT_ENV).expect("the lock expectation is configured");
    if expectation == "read" {
        assert_eq!(
            std::fs::read(path).expect("the persisted revision remains readable"),
            b"project"
        );
        return;
    }
    match (ProjectFileLock::try_acquire(&path), expectation.as_str()) {
        (Err(ProjectFileLockError::Conflict), "conflict") => {}
        (Ok(_lock), "acquired") => {}
        (Ok(_lock), "conflict") => panic!("the second process unexpectedly acquired the lock"),
        (Err(error), "acquired") => panic!("the released lock remained unavailable: {error}"),
        (Err(error), "conflict") => panic!("the lock conflict lost its typed form: {error}"),
        (_, other) => panic!("unsupported lock expectation: {other}"),
    }
}

fn required_path(name: &str) -> PathBuf {
    PathBuf::from(env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

fn operation_plan(paths: &[&Path]) -> RootBindingPlan {
    let mut owner = OperationPathContext::new();
    for path in paths {
        owner
            .capture(path)
            .unwrap_or_else(|error| panic!("{} could not be captured: {error}", path.display()));
    }
    owner.freeze()
}

fn plan_hash(wire: &[u8]) -> String {
    format!("{:x}", Sha256::digest(wire))
}

fn write_evidence(value: serde_json::Value) {
    let path = required_path(EVIDENCE_ENV);
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&value).expect("the path evidence serializes"),
    )
    .expect("the path evidence is writable");
}

fn run_project_lock_probe(path: &Path, expectation: &str) -> std::process::ExitStatus {
    Command::new(env::current_exe().expect("the path-gate test executable is known"))
        .arg("project_lock_probe_process")
        .args(["--ignored", "--exact", "--nocapture"])
        .env(LOCK_PROBE_PATH_ENV, path)
        .env(LOCK_PROBE_EXPECT_ENV, expectation)
        .status()
        .expect("the second lock-probe process starts")
}

struct DriveMapping {
    drive: String,
}

impl DriveMapping {
    fn reserve(drive: &str) -> Self {
        let mapping = Self {
            drive: drive.to_owned(),
        };
        mapping.unmap();
        mapping
    }

    fn map_to(&self, remote: &Path) {
        self.unmap();
        let status = Command::new("net.exe")
            .arg("use")
            .arg(&self.drive)
            .arg(remote)
            .arg("/persistent:no")
            .status()
            .expect("net.exe starts");
        assert!(
            status.success(),
            "the temporary mapped drive {} could not target {}",
            self.drive,
            remote.display()
        );
    }

    fn unmap(&self) {
        let _ = Command::new("net.exe")
            .args(["use", &self.drive, "/delete", "/y"])
            .output();
    }
}

impl Drop for DriveMapping {
    fn drop(&mut self) {
        self.unmap();
    }
}
