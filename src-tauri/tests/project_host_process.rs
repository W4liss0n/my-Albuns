#![cfg(windows)]

use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use myalbuns_core::{
    CreateAuthorization, CreateProjectRequest, EditableProject, InitialProject, OpenProjectError,
    OpenProjectRequest, ProjectCore, ProjectLocation,
};
use myalbuns_paths::{AppPaths, NativePathDto, OperationPathContext, RootBindingPlan};
use serde_json::{Value, json};
use tempfile::TempDir;
use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    },
};

const PROJECT_HOST_ARGUMENT: &str = "--myalbuns-project-host";
const PROCESS_GATE_ROOT_ENV: &str = "MYALBUNS_PROCESS_GATE_DATA_ROOT";
const PROCESS_GATE_HEADLESS_ENV: &str = "MYALBUNS_PROCESS_GATE_HEADLESS";
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const STILL_ACTIVE: u32 = 259;

#[test]
fn real_global_and_host_processes_preserve_ownership_and_lifetime_boundaries() {
    let direct = ProjectFixture::new("protocolo");
    prove_correlated_terminal_and_single_host_session(&direct);

    let associated = ProjectFixture::new("associacao");
    prove_host_outlives_the_global_parent(&associated);
}

fn prove_correlated_terminal_and_single_host_session(fixture: &ProjectFixture) {
    let attempt_id = format!("gate-attempt-{}", uuid::Uuid::new_v4().simple());
    let launch_nonce = uuid::Uuid::new_v4().simple().to_string();
    let request = fixture.bootstrap_request(&attempt_id, &launch_nonce);
    let mut host = ChildGuard::spawn(
        Command::new(desktop_binary())
            .arg(PROJECT_HOST_ARGUMENT)
            .env(PROCESS_GATE_ROOT_ENV, &fixture.process_data_root)
            .env(PROCESS_GATE_HEADLESS_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()),
    );
    let host_pid = host.id();

    let mut stdin = host
        .child_mut()
        .stdin
        .take()
        .expect("the real Host exposes its one-shot request input");
    serde_json::to_writer(&mut stdin, &request).expect("the bootstrap request serializes");
    stdin
        .write_all(b"\n")
        .expect("the bootstrap request is terminated");
    stdin.flush().expect("the bootstrap request is flushed");
    drop(stdin);

    let stdout = host
        .child_mut()
        .stdout
        .take()
        .expect("the real Host exposes its terminal output");
    let terminal = read_terminal_with_timeout(stdout, PROCESS_TIMEOUT);

    assert_eq!(
        terminal["state"], "ready",
        "the real Host rejected the request: {terminal}"
    );
    assert_eq!(terminal["attemptId"], attempt_id);
    assert_eq!(terminal["launchNonce"], launch_nonce);
    assert_eq!(terminal["hostPid"], host_pid);
    assert_eq!(terminal["projectId"], fixture.project_id);
    assert_eq!(terminal["revision"], 0);
    assert_ne!(host_pid, std::process::id());
    assert!(process_is_alive(host_pid));
    assert_eq!(
        fixture
            .try_open()
            .expect_err("the real Host owns the only editable Session"),
        OpenProjectError::ProjectInUse
    );

    host.terminate();
    fixture.wait_until_released(PROCESS_TIMEOUT);
}

fn prove_host_outlives_the_global_parent(fixture: &ProjectFixture) {
    let mut global = ChildGuard::spawn(
        Command::new(desktop_binary())
            .arg(&fixture.project_path)
            .env(PROCESS_GATE_ROOT_ENV, &fixture.process_data_root)
            .env(PROCESS_GATE_HEADLESS_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    );
    let global_pid = global.id();
    assert_ne!(global_pid, std::process::id());

    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let host_pid = loop {
        if let Some(host_pid) = project_host_child_of(global_pid) {
            break host_pid;
        }
        assert!(
            Instant::now() < deadline,
            "the real global process did not create its Project Host"
        );
        thread::sleep(Duration::from_millis(100));
    };
    assert_ne!(host_pid, global_pid);
    assert_ne!(host_pid, std::process::id());
    let mut host_cleanup = ProcessGuard::new(host_pid);

    let global_status = global.wait_for_exit(PROCESS_TIMEOUT);
    assert!(
        global_status.success(),
        "the global parent exits after Ready"
    );
    assert!(
        process_is_alive(host_pid),
        "the Ready Host must remain alive after its global parent exits"
    );
    assert_eq!(
        fixture.try_open().expect_err(
            "after the global exits, the independent Host must still own the only Session"
        ),
        OpenProjectError::ProjectInUse
    );

    host_cleanup.terminate();
    fixture.wait_until_released(PROCESS_TIMEOUT);
}

struct ProjectFixture {
    _directory: TempDir,
    project_path: PathBuf,
    root_bindings: RootBindingPlan,
    project_id: String,
    identity_lease_root: PathBuf,
    process_data_root: PathBuf,
}

impl ProjectFixture {
    fn new(label: &str) -> Self {
        let directory = tempfile::tempdir().expect("a temporary process-gate directory exists");
        let project_path = directory.path().join(format!("Álbum {label}.myalbuns"));
        let mut context = OperationPathContext::new();
        context
            .capture(&project_path)
            .expect("the temporary Windows root is captured once");
        let root_bindings = context.freeze();
        let process_data_root = directory.path().join("ProcessData");
        let app_paths = AppPaths::from_roots(
            &process_data_root.join("Roaming"),
            &process_data_root.join("Local"),
            &process_data_root.join("Temporary"),
        );
        let identity_lease_root = app_paths.project_identity_leases_dir();
        let project = ProjectCore::new()
            .with_identity_lease_root(identity_lease_root.clone())
            .create_editable(CreateProjectRequest::new(
                ProjectLocation::new(project_path.clone(), root_bindings.clone()),
                InitialProject::neutral(),
                CreateAuthorization::CreateOnly,
            ))
            .expect("a productive v1 project fixture is created");
        let project_id = project.project_id().hyphenated().to_string();
        drop(project);

        Self {
            _directory: directory,
            project_path,
            root_bindings,
            project_id,
            identity_lease_root,
            process_data_root,
        }
    }

    fn bootstrap_request(&self, attempt_id: &str, launch_nonce: &str) -> Value {
        json!({
            "protocolVersion": 1,
            "attemptId": attempt_id,
            "launchNonce": launch_nonce,
            "intent": "openExisting",
            "authority": {
                "logicalTarget": NativePathDto::from(self.project_path.clone()),
                "rootBindings": self.root_bindings,
            },
        })
    }

    fn try_open(&self) -> Result<EditableProject, OpenProjectError> {
        ProjectCore::new()
            .with_identity_lease_root(self.identity_lease_root.clone())
            .open_editable(OpenProjectRequest::new(ProjectLocation::new(
                self.project_path.clone(),
                self.root_bindings.clone(),
            )))
    }

    fn wait_until_released(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            match self.try_open() {
                Ok(project) => {
                    drop(project);
                    return;
                }
                Err(OpenProjectError::ProjectInUse) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(error) => panic!("the Host left an unexpected open error: {error:?}"),
            }
        }
    }
}

fn desktop_binary() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_myalbuns-desktop"))
}

fn read_terminal_with_timeout(
    stdout: impl std::io::Read + Send + 'static,
    timeout: Duration,
) -> Value {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = sender.send(result);
    });
    let line = receiver
        .recv_timeout(timeout)
        .expect("the real Host emits one terminal before the deadline")
        .expect("the real Host terminal is readable");
    serde_json::from_str(&line).expect("the real Host terminal is structured JSON")
}

fn project_host_child_of(parent_pid: u32) -> Option<u32> {
    let script = format!(
        "$p = Get-CimInstance Win32_Process -Filter \"ParentProcessId = {parent_pid}\" \
         -ErrorAction Stop | Where-Object {{ $_.CommandLine -like \
         '*{PROJECT_HOST_ARGUMENT}*' }} | Select-Object -First 1; \
         if ($null -ne $p) {{ [Console]::Out.Write($p.ProcessId) }}"
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("the Windows process query starts");
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()?.trim().parse().ok()
}

fn process_is_alive(process_id: u32) -> bool {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if handle.is_null() {
        return false;
    }
    let mut exit_code = 0;
    let alive =
        unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0 && exit_code == STILL_ACTIVE;
    unsafe { CloseHandle(handle) };
    alive
}

fn terminate_process(process_id: u32) {
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, process_id) };
    assert!(!handle.is_null(), "the exact Host process can be opened");
    assert_ne!(
        unsafe { TerminateProcess(handle, 1) },
        0,
        "the exact Host process can be terminated"
    );
    unsafe { CloseHandle(handle) };

    let deadline = Instant::now() + PROCESS_TIMEOUT;
    while process_is_alive(process_id) {
        assert!(
            Instant::now() < deadline,
            "the terminated Host process did not exit"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn spawn(command: &mut Command) -> Self {
        Self(Some(
            command.spawn().expect("the real MyAlbuns process starts"),
        ))
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("the child process is present")
    }

    fn id(&self) -> u32 {
        self.0.as_ref().expect("the child process is present").id()
    }

    fn terminate(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .child_mut()
                .try_wait()
                .expect("the global process status is readable")
            {
                self.0.take();
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "the global parent did not exit after the Host became Ready"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

struct ProcessGuard(Option<u32>);

impl ProcessGuard {
    fn new(process_id: u32) -> Self {
        Self(Some(process_id))
    }

    fn terminate(&mut self) {
        if let Some(process_id) = self.0.take()
            && process_is_alive(process_id)
        {
            terminate_process(process_id);
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}
