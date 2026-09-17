//! Real filesystem journeys through the same bootstrap, Host and ExportPipeline
//! used by the application. The Windows gate owns the isolated SMB fixture.
use super::*;
use crate::{
    export_pipeline::{self, AlbumExportOptions, ExportExecutionControl},
    imaging_processor::InvocationContext,
    imaging_recovery_integration::RealProcessTransport,
    media_runtime::MediaResolver,
    project_host::ProjectHost,
};
use image::{GenericImageView, ImageFormat, Rgb, RgbImage};
use myalbuns_core::{ExportFormat, ExportMode, PhotoPlacementMode, ProjectIntent};
use myalbuns_paths::{AppPathsError, ExportWriteAuthorization};
use std::{env, fs, os::windows::process::CommandExt, process::Command};

fn environment_path(name: &str) -> PathBuf {
    env::var_os(name).map(PathBuf::from).expect(name)
}

fn fixture_at(path: PathBuf) -> Fixture {
    let mut fixture = Fixture::new();
    fixture.project_path = path;
    fixture
}

fn create_host(fixture: &Fixture) -> ProjectHost {
    let mut request = fixture.create_request(CreateWriteAuthorization::CreateOnly);
    let BootstrapIntent::CreateNew { configuration, .. } = &mut request.intent else {
        unreachable!()
    };
    configuration.document.sheet_width_um = 101_600;
    configuration.document.sheet_height_um = 50_800;
    configuration.document.dpi = 72;
    let (created, worker) = bootstrap_host_project_with_thread(request, &fixture.paths).unwrap();
    assert_ne!(worker, thread::current().id());
    let HostBootstrap::Ready(created) = created else {
        panic!("creation must be ready")
    };
    ProjectHost::new(created.into_parts().1)
}

async fn export(
    host: &ProjectHost,
    destination: PathBuf,
    logs: &Path,
) -> Result<(), export_pipeline::ExportFailure> {
    let sheet = host.projection().unwrap().state.album.sheets[1].id.clone();
    let (snapshot, sources) = host.freeze_export(std::slice::from_ref(&sheet)).unwrap();
    let plan = export_pipeline::plan_album(
        snapshot,
        AlbumExportOptions {
            request_id: "windows-project-paths".into(),
            destination: destination.clone(),
            authorization: ExportWriteAuthorization::CreateOnly,
            sheet_ids: vec![sheet],
            whole_album: false,
            mode: ExportMode::Sheet,
            format: ExportFormat::Jpeg { quality: 90 },
            protected_originals: host
                .authorized_media_catalog()
                .unwrap()
                .bindings
                .into_iter()
                .map(|binding| binding.logical_path)
                .collect(),
            sources,
        },
    )
    .unwrap();
    let paths = crate::path_io::capture_root_bindings(plan.required_paths())
        .await
        .unwrap();
    fs::create_dir_all(logs).unwrap();
    let mut transport = RealProcessTransport::stable(
        environment_path("MYALBUNS_REAL_IMAGING_PROCESSOR"),
        logs.to_path_buf(),
    );
    let published = export_pipeline::execute_album(
        &mut transport,
        plan,
        &paths,
        &ExportExecutionControl::default(),
        &|_| {},
        &InvocationContext::new(
            "windows-project-paths",
            Some(host.projection().unwrap().state.project_id),
        ),
    )
    .await?;
    let outputs: Vec<_> = fs::read_dir(&destination)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(outputs.len(), 1, "only the requested output remains");
    assert_eq!(image::open(&outputs[0]).unwrap().dimensions(), (288, 144));
    assert_eq!(
        (
            published.completion.width_px,
            published.completion.height_px
        ),
        (288, 144)
    );
    Ok(())
}

#[test]
#[ignore = "executed by the isolated Windows path gate with SMB and the real Processor"]
fn create_reopen_and_export_across_native_windows_paths() {
    tauri::async_runtime::block_on(async {
        let local = environment_path("MYALBUNS_PATH_GATE_LOCAL_ROOT");
        let unc = environment_path("MYALBUNS_PATH_GATE_UNC_ROOT");
        let drive = env::var("MYALBUNS_PATH_GATE_DRIVE").unwrap();
        let _mapping = Mapping::new(&drive, &unc);
        let mut long_suffix = PathBuf::from("host-long");
        for _ in 0..7 {
            long_suffix.push("Fotografias e álbuns de formatura");
        }
        let cases = [
            ("local", local.join("host-local")),
            ("unc", unc.join("host-unc")),
            ("mapped", PathBuf::from(format!("{drive}\\host-mapped"))),
            (
                "verbatim-disk",
                PathBuf::from(format!(r"\\?\{}", local.join("host-verbatim").display())),
            ),
            (
                "verbatim-unc",
                PathBuf::from(format!(
                    r"\\?\UNC\{}",
                    unc.join("host-verbatim-unc")
                        .display()
                        .to_string()
                        .trim_start_matches('\\')
                )),
            ),
            ("long-local", local.join(&long_suffix)),
            ("long-unc", unc.join(&long_suffix).join("UNC")),
        ];
        let mut evidence = vec![];
        for (case, directory) in cases {
            fs::create_dir_all(&directory).unwrap();
            let project_path = directory.join("Álbum de família.myalbuns");
            let photo_path = directory.join("Foto de João.jpg");
            let output_path = directory.join("Exportação");
            fs::create_dir_all(&output_path).unwrap();
            RgbImage::from_pixel(300, 200, Rgb([30, 210, 70]))
                .save_with_format(&photo_path, ImageFormat::Jpeg)
                .unwrap();
            let original = fs::read(&photo_path).unwrap();
            let fixture = fixture_at(project_path.clone());
            let host = create_host(&fixture);
            let crate::ipc_contract::ImportMediaResult::Completed { media_ids, .. } = host
                .import_photos(vec![photo_path.clone()], |_| {})
                .unwrap()
            else {
                panic!("import completes")
            };
            let sheet = host.projection().unwrap().state.album.sheets[1].id.clone();
            let added = host
                .apply_with_outcome(ProjectIntent::AddPhoto {
                    sheet_id: sheet,
                    media_id: media_ids[0].parse().unwrap(),
                    mode: PhotoPlacementMode::Normal,
                })
                .unwrap()
                .projection;
            host.save(added.state.revision).unwrap();
            let before = host.projection().unwrap();
            drop(host);
            let (opened, worker) =
                bootstrap_host_project_with_thread(fixture.request(), &fixture.paths).unwrap();
            assert_ne!(worker, thread::current().id());
            let HostBootstrap::Ready(opened) = opened else {
                panic!("reopening must be ready")
            };
            let host = ProjectHost::new(opened.into_parts().1);
            for binding in host.authorized_media_catalog().unwrap().bindings {
                host.observe_photo_source(
                    &binding,
                    MediaResolver.inspect_photo_binding(&binding).unwrap(),
                )
                .unwrap();
            }
            assert_eq!(host.projection().unwrap().state.album, before.state.album);
            let saved = fs::read(&project_path).unwrap();
            export(
                &host,
                output_path.clone(),
                &local.join("host-processor-logs"),
            )
            .await
            .unwrap();
            assert_eq!(fs::read(&project_path).unwrap(), saved);
            assert_eq!(fs::read(&photo_path).unwrap(), original);
            assert!(
                !directory
                    .join(".myalbuns-export-windows-project-paths.tmp")
                    .exists()
            );
            if case.starts_with("long-") {
                assert!(project_path.as_os_str().len() > 260);
            }
            evidence.push(serde_json::json!({"case":case,"created":true,"reopened":true,"exported":true,"originalUnchanged":true,"projectUnchangedByExport":true,"stagingRemoved":true}));
        }
        fs::write(
            environment_path("MYALBUNS_PATH_GATE_PROJECT_EVIDENCE"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    });
}

#[test]
#[ignore = "uses real ACLs only inside the Windows gate fixture"]
fn denied_creation_opening_and_export_preserve_files_and_allow_explicit_retry() {
    tauri::async_runtime::block_on(async {
        let root = environment_path("MYALBUNS_PATH_GATE_LOCAL_ROOT")
            .join(format!("host-permissions-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let fixture = fixture_at(root.join("Permissões.myalbuns"));
        let denied = Denied::new(&root, "Write");
        let request = fixture.create_request(CreateWriteAuthorization::CreateOnly);
        let error = bootstrap_host_project_with_thread(request, &fixture.paths)
            .err()
            .expect("creation is denied");
        assert!(matches!(
            error,
            HostTerminal::Failed {
                code: FailureCode::AccessDenied,
                ..
            }
        ));
        assert!(!fixture.project_path.exists());
        drop(denied);
        let host = create_host(&fixture);
        drop(host);
        let original = fs::read(&fixture.project_path).unwrap();
        let denied = Denied::new(&fixture.project_path, "ReadData");
        let error = bootstrap_host_project_with_thread(fixture.request(), &fixture.paths)
            .err()
            .expect("opening is denied");
        assert!(matches!(
            error,
            HostTerminal::Failed {
                code: FailureCode::AccessDenied,
                ..
            }
        ));
        drop(denied);
        let (opened, _) =
            bootstrap_host_project_with_thread(fixture.request(), &fixture.paths).unwrap();
        let HostBootstrap::Ready(opened) = opened else {
            panic!("retry opens")
        };
        let host = ProjectHost::new(opened.into_parts().1);
        let destination = root.join("Destino");
        fs::create_dir(&destination).unwrap();
        let denied = Denied::new(&destination, "Write");
        let failure = export(&host, destination.clone(), &root.join("logs"))
            .await
            .unwrap_err();
        assert_eq!(
            failure.path_failure,
            Some(AppPathsError::OperationPathAccessDenied)
        );
        assert!(fs::read_dir(&destination).unwrap().next().is_none());
        assert_eq!(fs::read(&fixture.project_path).unwrap(), original);
        drop(denied);
        export(&host, destination, &root.join("logs"))
            .await
            .unwrap();
        assert_eq!(fs::read(&fixture.project_path).unwrap(), original);
        fs::write(
            environment_path("MYALBUNS_PATH_GATE_PERMISSION_EVIDENCE"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "creationDenied": true, "openingDenied": true, "exportDenied": true,
                "explicitRetryExported": true, "projectUnchanged": true,
            }))
            .unwrap(),
        )
        .unwrap();
    });
}

struct Mapping(String);
impl Mapping {
    fn new(drive: &str, unc: &Path) -> Self {
        assert!(
            !PathBuf::from(format!("{drive}\\")).exists(),
            "fixture drive must be unused"
        );
        assert!(
            Command::new("net.exe")
                .creation_flags(0x0800_0000)
                .args(["use", drive])
                .arg(unc)
                .arg("/persistent:no")
                .output()
                .unwrap()
                .status
                .success()
        );
        Self(drive.into())
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        let _ = Command::new("net.exe")
            .creation_flags(0x0800_0000)
            .args(["use", &self.0, "/delete", "/y"])
            .output();
    }
}

/// Changes only an exact, owned fixture's ACL; removal also runs on assertion failure.
struct Denied {
    path: PathBuf,
    rights: String,
    active: bool,
}
impl Denied {
    fn new(path: &Path, rights: &str) -> Self {
        let mut guard = Self {
            path: path.into(),
            rights: rights.into(),
            active: false,
        };
        guard.update("add").expect("fixture ACL is applied");
        guard.active = true;
        guard
    }
    fn update(&self, action: &str) -> Result<(), String> {
        let script = r#"
$ErrorActionPreference = 'Stop'
Import-Module "$PSHOME/Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1"
$target = [IO.Path]::GetFullPath($env:MYALBUNS_ACL_TARGET)
$root = [IO.Path]::GetFullPath($env:MYALBUNS_PATH_GATE_LOCAL_ROOT).TrimEnd('\') + '\'
if (-not $target.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)) { throw 'ACL fixture escaped its root' }
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
$rule = [Security.AccessControl.FileSystemAccessRule]::new($sid, [Security.AccessControl.FileSystemRights]$env:MYALBUNS_ACL_RIGHTS, [Security.AccessControl.AccessControlType]::Deny)
$acl = Get-Acl -LiteralPath $target
if ($env:MYALBUNS_ACL_ACTION -eq 'add') { $acl.AddAccessRule($rule) } else { $acl.RemoveAccessRuleSpecific($rule) }
Set-Acl -LiteralPath $target -AclObject $acl
"#;
        let result = Command::new("powershell.exe")
            .creation_flags(0x0800_0000)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("MYALBUNS_ACL_TARGET", &self.path)
            .env("MYALBUNS_ACL_RIGHTS", &self.rights)
            .env("MYALBUNS_ACL_ACTION", action)
            .output()
            .map_err(|error| error.to_string())?;
        if result.status.success() {
            Ok(())
        } else {
            Err(format!(
                "fixture ACL {action}: {}",
                String::from_utf8_lossy(&result.stderr)
            ))
        }
    }
}
impl Drop for Denied {
    fn drop(&mut self) {
        if self.active
            && let Err(error) = self.update("remove")
        {
            if thread::panicking() {
                eprintln!("{error}");
            } else {
                panic!("{error}");
            }
        }
    }
}
