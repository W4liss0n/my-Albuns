#![cfg(windows)]

use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, InitialProject, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectIntent, ProjectLocation, SaveCopyAsError,
    SaveCopyAsRequest,
};
use myalbuns_paths::{OperationPathContext, ProcessInstanceId, project_data_namespace};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_OBJECT_0,
    },
    Storage::FileSystem::{
        DELETE, FILE_FLAG_OVERLAPPED, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FileRenameInfo, SetFileInformationByHandle,
    },
    System::{
        IO::{DeviceIoControl, OVERLAPPED},
        Ioctl::FSCTL_REQUEST_OPLOCK_LEVEL_1,
        Threading::{CreateEventW, WaitForSingleObject},
    },
};

fn project_location(path: &Path) -> ProjectLocation {
    let mut paths = OperationPathContext::new();
    paths
        .capture(path)
        .expect("the public path seam captures the Project root");
    ProjectLocation::new(path.to_path_buf(), paths.freeze())
}

fn project_core(storage_root: &Path) -> ProjectCore {
    ProjectCore::new()
        .with_identity_storage_roots(storage_root.join("leases"), storage_root.join("identities"))
}

#[allow(
    clippy::permissions_set_readonly_false,
    reason = "this integration suite runs only on Windows, where clearing FILE_ATTRIBUTE_READONLY is the intended operation"
)]
fn make_writable(path: &Path) {
    let mut permissions = std::fs::metadata(path)
        .expect("the read-only fixture still has metadata")
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).expect("the Windows fixture becomes writable");
}

struct BreakingOplock {
    file: Option<File>,
    event: HANDLE,
    _overlapped: Box<OVERLAPPED>,
}

impl BreakingOplock {
    fn request(path: &Path) -> Self {
        let file = OpenOptions::new()
            .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OVERLAPPED)
            .open(path)
            .expect("the original Project accepts an oplock fixture");
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        assert!(!event.is_null(), "the oplock break has a native event");
        let mut overlapped = Box::new(OVERLAPPED {
            hEvent: event,
            ..OVERLAPPED::default()
        });
        let requested = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as HANDLE,
                FSCTL_REQUEST_OPLOCK_LEVEL_1,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut *overlapped,
            )
        };
        assert_eq!(requested, 0, "the oplock remains pending until a real read");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(ERROR_IO_PENDING as i32),
            "the asynchronous oplock is active"
        );
        Self {
            file: Some(file),
            event,
            _overlapped: overlapped,
        }
    }

    fn wait_for_break(&self) {
        assert_eq!(
            unsafe { WaitForSingleObject(self.event, 10_000) },
            WAIT_OBJECT_0,
            "the public opening must causally request read access to A"
        );
    }

    fn rename_retained_file(&self, destination: &Path) {
        let name = destination.as_os_str().encode_wide().collect::<Vec<_>>();
        let byte_length = name.len() * size_of::<u16>();
        let buffer_length = std::mem::offset_of!(FILE_RENAME_INFO, FileName) + byte_length;
        let word_count = buffer_length.div_ceil(size_of::<usize>());
        let mut buffer = vec![0_usize; word_count];
        let rename = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        unsafe {
            (*rename).Anonymous.ReplaceIfExists = false;
            (*rename).RootDirectory = std::ptr::null_mut();
            (*rename).FileNameLength =
                u32::try_from(byte_length).expect("the fixture path fits the Win32 contract");
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                std::ptr::addr_of_mut!((*rename).FileName).cast::<u16>(),
                name.len(),
            );
        }
        assert_ne!(
            unsafe {
                SetFileInformationByHandle(
                    self.file
                        .as_ref()
                        .expect("the oplock still retains A")
                        .as_raw_handle() as HANDLE,
                    FileRenameInfo,
                    rename.cast(),
                    u32::try_from(buffer_length)
                        .expect("the rename fixture fits the Win32 contract"),
                )
            },
            0,
            "the retained handle renames A without releasing the pending read: {}",
            std::io::Error::last_os_error()
        );
    }

    fn release(mut self) {
        drop(self.file.take());
    }
}

impl Drop for BreakingOplock {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.event.is_null() {
            unsafe {
                CloseHandle(self.event);
            }
            self.event = std::ptr::null_mut();
        }
    }
}

struct ReadOnlyIsoMount {
    root: PathBuf,
    child: Child,
    stdin: Option<ChildStdin>,
}

impl ReadOnlyIsoMount {
    fn mount(source_directory: &Path, image_path: &Path) -> Self {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/Mount-ReadOnlyIsoFixture.ps1");
        let mut child = Command::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-SourceDirectory")
            .arg(source_directory)
            .arg("-ImagePath")
            .arg(image_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Windows PowerShell starts the read-only ISO fixture");
        let mut reader = BufReader::new(
            child
                .stdout
                .take()
                .expect("the ISO fixture exposes its mounted root"),
        );
        let mut root = String::new();
        reader
            .read_line(&mut root)
            .expect("the ISO fixture reports its mounted root");
        if root.trim().is_empty() {
            let output = child
                .wait_with_output()
                .expect("the failed ISO fixture returns diagnostics");
            panic!(
                "the ISO fixture did not mount: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let stdin = child.stdin.take().expect("the ISO fixture accepts release");
        Self {
            root: PathBuf::from(root.trim()),
            child,
            stdin: Some(stdin),
        }
    }

    fn project_path(&self, file_name: &str) -> PathBuf {
        self.root.join(file_name)
    }

    fn release(mut self) {
        let mut stdin = self.stdin.take().expect("the ISO fixture is still mounted");
        stdin
            .write_all(b"release\n")
            .expect("the ISO fixture receives its causal release");
        drop(stdin);
        assert!(
            self.child
                .wait()
                .expect("the ISO fixture exits after dismount")
                .success(),
            "the ISO fixture dismounts cleanly"
        );
    }
}

impl Drop for ReadOnlyIsoMount {
    fn drop(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"release\n");
        }
        let _ = self.child.wait();
    }
}

#[test]
fn moving_a_closed_project_preserves_its_identity_and_namespace() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("original.myalbuns");
    let moved_path = fixture.path().join("movido.myalbuns");
    let core = project_core(fixture.path());

    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created through ProjectCore");
    let original_id = original.project_id();
    let original_namespace = project_data_namespace(&original_id.hyphenated().to_string());
    drop(original);

    std::fs::rename(&original_path, &moved_path)
        .expect("the operating system moves the closed Project");

    let moved = core
        .open_editable(OpenProjectRequest::new(project_location(&moved_path)))
        .expect("a confirmed movement opens without creating another Project");

    assert_eq!(moved.project_id(), original_id);
    assert_eq!(moved.identity_authority().project_id(), original_id);
    assert_eq!(moved.project_path(), moved_path);
    assert_eq!(
        project_data_namespace(&moved.project_id().hyphenated().to_string()),
        original_namespace,
        "movement keeps the Cache/Recovery/WebView namespace"
    );
}

#[test]
fn a_successful_open_never_authorizes_a_path_replacement_with_the_same_identity() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let project_path = fixture.path().join("Projeto A.myalbuns");
    let retired_path = fixture.path().join("Projeto A aposentado.myalbuns");
    let replacement_path = fixture.path().join("Projeto B.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&project_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("A establishes durable Identidade evidence");
    drop(original);
    std::fs::copy(&project_path, &replacement_path)
        .expect("B is a distinct physical copy with the same persisted Identidade");
    let original_bytes = std::fs::read(&project_path).expect("A remains readable");
    let replacement_bytes =
        std::fs::read(&replacement_path).expect("B remains readable before the race");
    let oplock = BreakingOplock::request(&project_path);
    let opening_core = core.clone();
    let opening_path = project_path.clone();
    let opening = std::thread::spawn(move || {
        opening_core.open_editable(OpenProjectRequest::new(project_location(&opening_path)))
    });

    oplock.wait_for_break();
    oplock.rename_retained_file(&retired_path);
    std::fs::rename(&replacement_path, &project_path)
        .expect("B takes the pathname before the public opening continues");
    oplock.release();

    assert_eq!(
        opening
            .join()
            .expect("the public opening thread does not panic")
            .expect_err("Different must never become an editable Sessão"),
        OpenProjectError::IdentityIndeterminate
    );
    assert_eq!(
        std::fs::read(&retired_path).expect("A remains readable"),
        original_bytes
    );
    assert_eq!(
        std::fs::read(&project_path).expect("B remains readable"),
        replacement_bytes,
        "the rejected B keeps its persisted Identidade and bytes"
    );
}

#[test]
fn a_second_physical_alias_focuses_the_existing_session() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let alias_path = fixture.path().join("Alias.myalbuns");
    let core = project_core(fixture.path());
    let opened = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let project_id = opened.project_id();
    std::fs::hard_link(&original_path, &alias_path)
        .expect("the alias names the same physical Project file");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&alias_path)))
            .expect_err("an alias must not create a second editable Sessão"),
        OpenProjectError::FocusExisting {
            project_id,
            owner_process: ProcessInstanceId::current()
                .expect("the owning process instance is captured"),
        }
    );
}

#[test]
fn a_writable_external_copy_gets_a_new_identity_without_pending_creative_changes() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let copy_path = fixture.path().join("Copia externa.myalbuns");
    let core = project_core(fixture.path());
    let mut original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    original
        .apply(ProjectIntent::SetDpi { dpi: 240 })
        .expect("the original has a pending creative change");
    assert_eq!(original.projection().state.document.dpi, 240);
    assert!(original.has_unsaved_changes());

    std::fs::copy(&original_path, &copy_path)
        .expect("Windows creates an external physical copy from persisted bytes");
    let copied = core
        .open_editable(OpenProjectRequest::new(project_location(&copy_path)))
        .expect("a writable external copy is promoted before opening");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(
        copied.identity_authority().project_id(),
        copied.project_id()
    );
    assert_eq!(copied.revision(), 0);
    assert_eq!(copied.saved_revision(), 0);
    assert_eq!(copied.projection().state.document.dpi, 300);
    assert!(!copied.has_unsaved_changes());
    assert!(!copied.can_undo());
    assert_ne!(
        project_data_namespace(&copied.project_id().hyphenated().to_string()),
        project_data_namespace(&original_id.hyphenated().to_string()),
        "the promoted copy authorizes a distinct local namespace"
    );

    let original_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&original_path).expect("the original remains readable"),
    )
    .expect("the original remains valid JSON");
    let copy_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&copy_path).expect("the promoted copy remains readable"),
    )
    .expect("the promoted copy remains valid JSON");
    assert_eq!(
        original_json["projectId"],
        serde_json::Value::String(original_id.hyphenated().to_string())
    );
    assert_eq!(copy_json["projectId"], copied.project_id().to_string());
    assert_eq!(copy_json["schemaVersion"], original_json["schemaVersion"]);
    assert_eq!(copy_json["revision"], original_json["revision"]);
    assert_eq!(copy_json["project"], original_json["project"]);
}

#[test]
fn a_read_only_external_copy_can_be_saved_as_a_new_editable_project() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let read_only_path = fixture.path().join("Copia somente leitura.myalbuns");
    let destination_path = fixture.path().join("Copia editavel.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    let original_revision = original.revision();
    drop(original);
    std::fs::copy(&original_path, &read_only_path)
        .expect("Windows creates the external physical copy");
    let mut permissions = std::fs::metadata(&read_only_path)
        .expect("the copied file has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&read_only_path, permissions)
        .expect("the external copy becomes read-only");
    let source_bytes = std::fs::read(&read_only_path).expect("the source is readable");

    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&read_only_path)))
        .expect_err("a read-only external copy cannot be corrected in place")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    let copied = core
        .save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&destination_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect("Salvar cópia como... publishes a new editable Project");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(copied.revision(), original_revision);
    assert_eq!(copied.saved_revision(), original_revision);
    assert!(!copied.has_unsaved_changes());
    assert!(!copied.can_undo());
    assert_eq!(
        std::fs::read(&read_only_path).expect("the source remains readable"),
        source_bytes,
        "the validated read-only source remains byte-for-byte intact"
    );
    let destination_json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&destination_path).expect("the destination is readable"),
    )
    .expect("the destination is valid JSON");
    assert_eq!(destination_json["schemaVersion"], 2);
    assert_eq!(destination_json["revision"], original_revision);
    assert_eq!(
        destination_json["projectId"],
        copied.project_id().hyphenated().to_string()
    );
}

#[test]
fn an_external_copy_on_write_protected_media_offers_save_copy_as() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let media_source = fixture.path().join("iso-source");
    let image_path = fixture.path().join("Copia somente leitura.iso");
    let file_name = "Copia externa.myalbuns";
    std::fs::create_dir(&media_source).expect("the ISO source exists");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    drop(original);
    std::fs::copy(&original_path, media_source.join(file_name))
        .expect("the external physical copy enters the ISO image");
    let mount = ReadOnlyIsoMount::mount(&media_source, &image_path);
    let read_only_path = mount.project_path(file_name);
    let source_bytes = std::fs::read(&read_only_path)
        .expect("the external copy is readable from write-protected media");

    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&read_only_path)))
        .expect_err("write-protected media cannot receive a new Identidade in place")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    drop(source);
    assert_eq!(
        std::fs::read(&read_only_path).expect("the read-only source remains readable"),
        source_bytes,
        "Salvar cópia como... preserves the validated source"
    );
    mount.release();
}

#[test]
fn save_copy_as_never_rewrites_a_source_that_became_writable() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let source_path = fixture.path().join("Fonte externa.myalbuns");
    let destination_path = fixture.path().join("Destino editavel.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    let original_id = original.project_id();
    drop(original);
    std::fs::copy(&original_path, &source_path).expect("the external source is copied");
    let source_bytes = std::fs::read(&source_path).expect("the source baseline is captured");
    let mut permissions = std::fs::metadata(&source_path)
        .expect("the source has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source_path, permissions)
        .expect("the source first refuses in-place Identidade correction");
    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("the opening returns the validated opaque source")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    make_writable(&source_path);
    let copied = core
        .save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&destination_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the external source is saved at a distinct destination");

    assert_ne!(copied.project_id(), original_id);
    assert_eq!(
        std::fs::read(&source_path).expect("the source remains readable"),
        source_bytes,
        "Salvar cópia como... never corrects Identidade in the source pathname"
    );
}

#[test]
fn cancelling_or_failing_save_copy_as_leaves_no_source_session_or_path_mutation() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let original_path = fixture.path().join("Original.myalbuns");
    let source_path = fixture.path().join("Fonte externa.myalbuns");
    let occupied_path = fixture.path().join("Destino ocupado.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&original_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project is created");
    drop(original);
    std::fs::copy(&original_path, &source_path).expect("the external source is copied");
    let mut permissions = std::fs::metadata(&source_path)
        .expect("the source has metadata")
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&source_path, permissions)
        .expect("the external source becomes read-only");
    std::fs::write(&occupied_path, b"occupied destination")
        .expect("the destination conflict is explicit");
    let source_bytes = std::fs::read(&source_path).expect("source bytes are captured");
    let occupied_bytes = std::fs::read(&occupied_path).expect("destination bytes are captured");

    let cancelled = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("the refused opening returns the cancellable source")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    drop(cancelled);
    assert_eq!(
        std::fs::read(&source_path).expect("the cancelled source remains readable"),
        source_bytes
    );

    let source = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("cancellation retained no editable ownership")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    assert_eq!(
        core.save_copy_as(SaveCopyAsRequest::new(
            source,
            project_location(&occupied_path),
            CreateAuthorization::CreateOnly,
        ))
        .expect_err("an occupied destination fails before a Sessão exists"),
        SaveCopyAsError::DestinationConflict
    );
    assert_eq!(
        std::fs::read(&source_path).expect("the failed source remains readable"),
        source_bytes
    );
    assert_eq!(
        std::fs::read(&occupied_path).expect("the occupied destination remains readable"),
        occupied_bytes
    );

    let retry = match core
        .open_editable(OpenProjectRequest::new(project_location(&source_path)))
        .expect_err("failure retained no editable ownership or duplicated source Sessão")
    {
        OpenProjectError::ExternalCopyNotWritable(source) => *source,
        other => panic!("unexpected open error: {other:?}"),
    };
    drop(retry);
    make_writable(&source_path);
}

#[test]
fn a_reused_previous_location_is_indeterminate_and_never_rewrites_the_candidate() {
    let fixture = tempfile::tempdir().expect("temporary Project fixture");
    let previous_path = fixture.path().join("Local anterior.myalbuns");
    let candidate_path = fixture.path().join("Candidato.myalbuns");
    let replacement_path = fixture.path().join("Outro projeto.myalbuns");
    let core = project_core(fixture.path());
    let original = core
        .create_editable(CreateProjectRequest::new(
            project_location(&previous_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("the original Project establishes durable Identidade evidence");
    std::fs::copy(&previous_path, &candidate_path)
        .expect("the candidate preserves the original persisted Identidade");
    drop(original);
    let replacement = core
        .create_editable(CreateProjectRequest::new(
            project_location(&replacement_path),
            InitialProject::neutral(),
            CreateAuthorization::CreateOnly,
        ))
        .expect("another Project is created");
    drop(replacement);
    std::fs::copy(&replacement_path, &previous_path)
        .expect("the previous pathname is reused by another valid Project");
    let candidate_bytes = std::fs::read(&candidate_path).expect("candidate bytes are captured");

    assert_eq!(
        core.open_editable(OpenProjectRequest::new(project_location(&candidate_path)))
            .expect_err("a reused pathname cannot prove movement or an external copy"),
        OpenProjectError::IdentityIndeterminate
    );
    assert_eq!(
        std::fs::read(&candidate_path).expect("the rejected candidate remains readable"),
        candidate_bytes,
        "Indeterminate fails closed without rewriting Identidade"
    );
}
