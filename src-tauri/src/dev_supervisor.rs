use std::{
    collections::HashSet,
    env,
    ffi::{OsStr, OsString, c_void},
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    os::windows::io::AsRawHandle,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{INFINITE, OpenProcess, WaitForSingleObject},
    },
};

#[path = "dev_supervisor_protocol.rs"]
mod protocol;

use protocol::{
    AUTHORIZE_HOST_LEASE_REQUEST, HOST_LEASE_AUTHORITY_ENV, HOST_LEASE_AUTHORIZED_RESPONSE,
    HOST_LEASE_ENDPOINT_ENV, HOST_LEASE_REGISTERED_RESPONSE, REGISTER_HOST_LEASE_REQUEST,
};

const WORKSPACE_ROOT_ENV: &str = "MYALBUNS_DEV_WORKSPACE_ROOT";
const PROJECT_PATH_ENV: &str = "MYALBUNS_DEV_PROJECT_PATH";
const NODE_EXECUTABLE_ENV: &str = "MYALBUNS_NODE_EXECUTABLE";
const WORKER_ROLE_ENV: &str = "MYALBUNS_DEV_WORKER_ROLE";
const WORKER_GATE_ENDPOINT_ENV: &str = "MYALBUNS_DEV_WORKER_GATE_ENDPOINT";
const WORKER_GATE_TOKEN_ENV: &str = "MYALBUNS_DEV_WORKER_GATE_TOKEN";
const FRONTEND_HOST: &str = "localhost";
const FRONTEND_PORT: u16 = 1437;
const FRONTEND_REQUEST: &[u8] =
    b"GET /global.html HTTP/1.1\r\nHost: localhost:1437\r\nConnection: close\r\n\r\n";
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(50);
const WORKER_GATE_TIMEOUT: Duration = Duration::from_secs(10);
const FRONTEND_START_TIMEOUT: Duration = Duration::from_secs(120);
const HOST_HANDOFF_TIMEOUT: Duration = Duration::from_secs(10);
const JOB_TERMINATION_EXIT_CODE: u32 = 1;
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const HOST_LEASE_REJECTED_RESPONSE: &str = "REJECTED\n";

pub(crate) fn run() -> io::Result<i32> {
    if let Some(role) = env::var_os(WORKER_ROLE_ENV) {
        return run_worker(&role);
    }
    run_supervisor()
}

fn run_supervisor() -> io::Result<i32> {
    let workspace = workspace_root()?;
    let node = node_executable();
    validate_development_inputs(&workspace, &node)?;
    let leases = HostLeaseServer::start()?;
    let mut processes = DevelopmentProcesses::new()?;

    ensure_frontend_port_is_free()?;
    processes.vite = Some(spawn_assigned_worker(
        &processes.job,
        WorkerRole::Vite,
        &workspace,
        &[],
        &[],
    )?);
    wait_for_frontend(
        processes
            .vite
            .as_mut()
            .expect("the Vite worker was just installed"),
    )?;
    eprintln!(r#"{{"event":"dev_frontend_ready","port":1437}}"#);

    let tauri_arguments = tauri_cli_arguments();
    let tauri_environment = [
        (
            HOST_LEASE_ENDPOINT_ENV,
            OsString::from(leases.endpoint().to_string()),
        ),
        (HOST_LEASE_AUTHORITY_ENV, OsString::from(leases.authority())),
    ];
    processes.tauri = Some(spawn_assigned_worker(
        &processes.job,
        WorkerRole::Tauri,
        &workspace,
        &tauri_arguments,
        &tauri_environment,
    )?);

    let outcome = supervise_development(&mut processes, &leases);
    let cleanup = processes.shutdown();
    if cleanup.is_ok() {
        eprintln!(r#"{{"event":"dev_environment_cleanup_completed"}}"#);
    }
    match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(0),
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
    }
}

fn workspace_root() -> io::Result<PathBuf> {
    if let Some(root) = env::var_os(WORKSPACE_ROOT_ENV) {
        return Ok(PathBuf::from(root));
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::other("o workspace de desenvolvimento não foi localizado"))
}

fn node_executable() -> OsString {
    env::var_os(NODE_EXECUTABLE_ENV).unwrap_or_else(|| OsString::from("node.exe"))
}

fn validate_development_inputs(workspace: &Path, node: &OsStr) -> io::Result<()> {
    for (label, path) in [
        (
            "Vite",
            workspace
                .join("node_modules")
                .join("vite")
                .join("bin")
                .join("vite.js"),
        ),
        (
            "Tauri CLI",
            workspace
                .join("node_modules")
                .join("@tauri-apps")
                .join("cli")
                .join("tauri.js"),
        ),
    ] {
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{label} não foi encontrado em {}", path.display()),
            ));
        }
    }
    if node.is_empty() {
        return Err(io::Error::other("o executável Node.js não foi informado"));
    }
    Ok(())
}

fn tauri_cli_arguments() -> Vec<OsString> {
    compose_tauri_cli_arguments(env::args_os().skip(1), env::var_os(PROJECT_PATH_ENV))
}

fn compose_tauri_cli_arguments(
    launcher_arguments: impl IntoIterator<Item = OsString>,
    project_path: Option<OsString>,
) -> Vec<OsString> {
    let launcher_arguments = launcher_arguments.into_iter().collect::<Vec<_>>();
    let mut arguments = vec![OsString::from("dev")];
    arguments.extend(launcher_arguments);
    if let Some(project_path) = project_path {
        arguments.push(OsString::from("--"));
        arguments.push(OsString::from("--"));
        arguments.push(project_path);
    }
    arguments
}

fn run_worker(role: &OsStr) -> io::Result<i32> {
    await_job_assignment()?;
    let workspace = workspace_root()?;
    let node = node_executable();
    let mut command = Command::new(node);
    command.current_dir(&workspace);
    match WorkerRole::parse(role)? {
        WorkerRole::Vite => {
            command.arg(
                workspace
                    .join("node_modules")
                    .join("vite")
                    .join("bin")
                    .join("vite.js"),
            );
        }
        WorkerRole::Tauri => {
            command
                .arg(
                    workspace
                        .join("node_modules")
                        .join("@tauri-apps")
                        .join("cli")
                        .join("tauri.js"),
                )
                .args(env::args_os().skip(1));
        }
    }
    let status = command.status()?;
    Ok(exit_code(status))
}

fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[derive(Clone, Copy)]
enum WorkerRole {
    Vite,
    Tauri,
}

impl WorkerRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Vite => "vite",
            Self::Tauri => "tauri",
        }
    }

    fn parse(value: &OsStr) -> io::Result<Self> {
        match value.to_str() {
            Some("vite") => Ok(Self::Vite),
            Some("tauri") => Ok(Self::Tauri),
            _ => Err(io::Error::other(
                "o papel interno do supervisor de desenvolvimento é inválido",
            )),
        }
    }
}

fn spawn_assigned_worker(
    job: &KillOnCloseJob,
    role: WorkerRole,
    workspace: &Path,
    arguments: &[OsString],
    environment: &[(&str, OsString)],
) -> io::Result<Child> {
    let gate = TcpListener::bind(("127.0.0.1", 0))?;
    gate.set_nonblocking(true)?;
    let gate_token = uuid::Uuid::new_v4().simple().to_string();
    let mut command = Command::new(env::current_exe()?);
    command
        .current_dir(workspace)
        .args(arguments)
        .env(WORKER_ROLE_ENV, role.as_str())
        .env(WORKSPACE_ROOT_ENV, workspace)
        .env(WORKER_GATE_ENDPOINT_ENV, gate.local_addr()?.to_string())
        .env(WORKER_GATE_TOKEN_ENV, &gate_token)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (name, value) in environment {
        command.env(name, value);
    }
    let mut child = command.spawn()?;

    let assignment = complete_worker_assignment(&gate, &gate_token, &mut child, job);
    if let Err(error) = assignment {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

fn complete_worker_assignment(
    gate: &TcpListener,
    expected_token: &str,
    child: &mut Child,
    job: &KillOnCloseJob,
) -> io::Result<()> {
    let deadline = Instant::now() + WORKER_GATE_TIMEOUT;
    loop {
        match gate.accept() {
            Ok((stream, _)) => {
                let mut reader = BufReader::new(stream);
                reader
                    .get_mut()
                    .set_read_timeout(Some(WORKER_GATE_TIMEOUT))?;
                let mut line = String::new();
                reader.read_line(&mut line)?;
                if line.trim_end() != expected_token {
                    continue;
                }
                job.assign(child)?;
                reader.get_mut().write_all(b"1")?;
                reader.get_mut().flush()?;
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait()? {
                    return Err(io::Error::other(format!(
                        "o worker encerrou antes da associação ao Job Object ({status})"
                    )));
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "o worker não confirmou a barreira de associação ao Job Object",
                    ));
                }
                thread::sleep(PROCESS_POLL_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
}

fn await_job_assignment() -> io::Result<()> {
    let endpoint = required_environment(WORKER_GATE_ENDPOINT_ENV)?
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::other("a barreira do Job Object possui endpoint inválido"))?;
    let token = required_environment(WORKER_GATE_TOKEN_ENV)?;
    let mut stream = TcpStream::connect_timeout(&endpoint, WORKER_GATE_TIMEOUT)?;
    stream.set_read_timeout(Some(WORKER_GATE_TIMEOUT))?;
    stream.write_all(token.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut release = [0_u8; 1];
    stream.read_exact(&mut release)?;
    if release != *b"1" {
        return Err(io::Error::other(
            "a barreira do Job Object retornou confirmação inválida",
        ));
    }
    Ok(())
}

fn required_environment(name: &str) -> io::Result<String> {
    env::var(name).map_err(|_| io::Error::other(format!("a variável {name} não foi definida")))
}

fn wait_for_frontend(vite: &mut Child) -> io::Result<()> {
    let deadline = Instant::now() + FRONTEND_START_TIMEOUT;
    loop {
        if let Some(status) = vite.try_wait()? {
            return Err(io::Error::other(format!(
                "o Vite encerrou antes de servir a UI ({status})"
            )));
        }
        if frontend_responds() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "o Vite não serviu a UI de desenvolvimento no prazo",
            ));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn ensure_frontend_port_is_free() -> io::Result<()> {
    if frontend_addresses()?
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok())
    {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "a porta 1437 já pertence a outro processo",
        ));
    }
    Ok(())
}

fn frontend_responds() -> bool {
    let Ok(addresses) = frontend_addresses() else {
        return false;
    };
    addresses.into_iter().any(|address| {
        let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(250))
        else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
        if stream.write_all(FRONTEND_REQUEST).is_err() {
            return false;
        }
        let mut response = [0_u8; 32];
        let Ok(length) = stream.read(&mut response) else {
            return false;
        };
        response[..length].starts_with(b"HTTP/1.1 200")
            || response[..length].starts_with(b"HTTP/1.1 304")
    })
}

fn frontend_addresses() -> io::Result<impl Iterator<Item = SocketAddr>> {
    (FRONTEND_HOST, FRONTEND_PORT).to_socket_addrs()
}

fn supervise_development(
    processes: &mut DevelopmentProcesses,
    leases: &HostLeaseServer,
) -> io::Result<()> {
    let mut lifecycle = DevelopmentLifecycle::default();
    let mut tauri_exit = None;
    let mut no_host_deadline = None;

    loop {
        while let Ok(event) = leases.events().try_recv() {
            lifecycle.apply(event);
        }

        if let Some(status) = processes
            .vite
            .as_mut()
            .expect("Vite is supervised")
            .try_wait()?
        {
            return Err(io::Error::other(format!(
                "o Vite encerrou enquanto o ambiente ainda o possuía ({status})"
            )));
        }

        if tauri_exit.is_none() {
            tauri_exit = processes
                .tauri
                .as_mut()
                .expect("the Tauri CLI is supervised")
                .try_wait()?;
            if let Some(status) = tauri_exit {
                lifecycle.cli_exited(status.success());
                if status.success() && !lifecycle.host_seen() {
                    no_host_deadline = Some(Instant::now() + HOST_HANDOFF_TIMEOUT);
                }
            }
        }

        let handoff_expired = no_host_deadline.is_some_and(|deadline| Instant::now() >= deadline);
        match lifecycle.decision(handoff_expired) {
            LifecycleDecision::Wait => {}
            LifecycleDecision::Complete => {
                if lifecycle.host_seen() {
                    eprintln!(r#"{{"event":"dev_last_host_exited"}}"#);
                } else {
                    eprintln!(r#"{{"event":"dev_global_only_exited"}}"#);
                }
                return Ok(());
            }
            LifecycleDecision::Fail(reason) => return Err(io::Error::other(reason)),
        }

        match leases.events().recv_timeout(PROCESS_POLL_INTERVAL) {
            Ok(event) => lifecycle.apply(event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other(
                    "o monitor de Hosts de desenvolvimento foi encerrado",
                ));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct HostLeaseId(uuid::Uuid);

impl HostLeaseId {
    fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    #[cfg(test)]
    fn from_u128(value: u128) -> Self {
        Self(uuid::Uuid::from_u128(value))
    }
}

#[derive(Debug)]
enum HostLeaseEvent {
    Connected {
        lease_id: HostLeaseId,
        process_id: u32,
    },
    Disconnected {
        lease_id: HostLeaseId,
        process_id: u32,
    },
    TrackingFailed {
        lease_id: HostLeaseId,
        process_id: u32,
    },
}

#[derive(Default)]
struct DevelopmentLifecycle {
    active_hosts: HashSet<HostLeaseId>,
    host_seen: bool,
    cli_success: Option<bool>,
    tracking_failed: bool,
}

impl DevelopmentLifecycle {
    fn apply(&mut self, event: HostLeaseEvent) {
        match event {
            HostLeaseEvent::Connected {
                lease_id,
                process_id,
            } => {
                self.host_seen = true;
                self.active_hosts.insert(lease_id);
                eprintln!(r#"{{"event":"dev_host_connected","processId":{process_id}}}"#);
            }
            HostLeaseEvent::Disconnected {
                lease_id,
                process_id,
            } => {
                self.active_hosts.remove(&lease_id);
                eprintln!(r#"{{"event":"dev_host_disconnected","processId":{process_id}}}"#);
            }
            HostLeaseEvent::TrackingFailed {
                lease_id,
                process_id,
            } => {
                self.active_hosts.remove(&lease_id);
                self.tracking_failed = true;
                eprintln!(r#"{{"event":"dev_host_tracking_failed","processId":{process_id}}}"#);
            }
        }
    }

    fn cli_exited(&mut self, success: bool) {
        self.cli_success = Some(success);
    }

    fn host_seen(&self) -> bool {
        self.host_seen
    }

    fn decision(&self, handoff_expired: bool) -> LifecycleDecision {
        if self.tracking_failed {
            return LifecycleDecision::Fail("o handle do Host ficou indisponível");
        }
        match self.cli_success {
            Some(false) => LifecycleDecision::Fail("o Tauri CLI encerrou com falha"),
            Some(true) if !self.active_hosts.is_empty() => LifecycleDecision::Wait,
            Some(true) if self.host_seen => LifecycleDecision::Complete,
            Some(true) if handoff_expired => LifecycleDecision::Complete,
            Some(true) | None => LifecycleDecision::Wait,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleDecision {
    Wait,
    Complete,
    Fail(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct HostRegistrationCredential(uuid::Uuid);

impl HostRegistrationCredential {
    fn parse(value: &str) -> Option<Self> {
        uuid::Uuid::parse_str(value).ok().map(Self)
    }
}

#[derive(Default)]
struct HostRegistrationCredentials {
    authorized: HashSet<HostRegistrationCredential>,
    consumed: HashSet<HostRegistrationCredential>,
}

impl HostRegistrationCredentials {
    fn authorize(&mut self, credential: HostRegistrationCredential) -> bool {
        if self.authorized.contains(&credential) || self.consumed.contains(&credential) {
            return false;
        }
        self.authorized.insert(credential)
    }

    fn consume(&mut self, credential: HostRegistrationCredential) -> bool {
        if !self.authorized.remove(&credential) {
            return false;
        }
        self.consumed.insert(credential);
        true
    }
}

enum HostLeaseRequest {
    Authorize {
        authority: String,
        credential: HostRegistrationCredential,
    },
    Register {
        credential: HostRegistrationCredential,
        process_id: u32,
    },
}

struct HostLeaseServer {
    endpoint: SocketAddr,
    authority: String,
    receiver: Receiver<HostLeaseEvent>,
    stop: Arc<AtomicBool>,
    accept_thread: Option<thread::JoinHandle<()>>,
}

impl HostLeaseServer {
    fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = listener.local_addr()?;
        let authority = uuid::Uuid::new_v4().simple().to_string();
        let expected_authority = authority.clone();
        let credentials = Arc::new(Mutex::new(HostRegistrationCredentials::default()));
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let accept_thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((connection, _)) => {
                        let sender = sender.clone();
                        let expected_authority = expected_authority.clone();
                        let credentials = credentials.clone();
                        thread::spawn(move || {
                            handle_host_lease_connection(
                                connection,
                                &expected_authority,
                                &credentials,
                                sender,
                            )
                        });
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(PROCESS_POLL_INTERVAL);
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            endpoint,
            authority,
            receiver,
            stop,
            accept_thread: Some(accept_thread),
        })
    }

    fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }

    fn authority(&self) -> &str {
        &self.authority
    }

    fn events(&self) -> &Receiver<HostLeaseEvent> {
        &self.receiver
    }
}

impl Drop for HostLeaseServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_host_lease_connection(
    connection: TcpStream,
    expected_authority: &str,
    credentials: &Mutex<HostRegistrationCredentials>,
    sender: mpsc::Sender<HostLeaseEvent>,
) {
    let mut reader = BufReader::new(connection);
    if reader
        .get_mut()
        .set_read_timeout(Some(WORKER_GATE_TIMEOUT))
        .is_err()
    {
        return;
    }
    let _ = reader
        .get_mut()
        .set_write_timeout(Some(WORKER_GATE_TIMEOUT));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let Some(request) = parse_host_lease_request(&line) else {
        let _ = write_host_lease_response(reader.get_mut(), HOST_LEASE_REJECTED_RESPONSE);
        return;
    };
    match request {
        HostLeaseRequest::Authorize {
            authority,
            credential,
        } => {
            let accepted = authority == expected_authority
                && credentials
                    .lock()
                    .ok()
                    .is_some_and(|mut credentials| credentials.authorize(credential));
            let response = if accepted {
                HOST_LEASE_AUTHORIZED_RESPONSE
            } else {
                HOST_LEASE_REJECTED_RESPONSE
            };
            let _ = write_host_lease_response(reader.get_mut(), response);
        }
        HostLeaseRequest::Register {
            credential,
            process_id,
        } => {
            let accepted = credentials
                .lock()
                .ok()
                .is_some_and(|mut credentials| credentials.consume(credential));
            if !accepted {
                let _ = write_host_lease_response(reader.get_mut(), HOST_LEASE_REJECTED_RESPONSE);
                return;
            }
            track_host_lease(reader.into_inner(), process_id, sender);
        }
    }
}

fn track_host_lease(
    mut connection: TcpStream,
    process_id: u32,
    sender: mpsc::Sender<HostLeaseEvent>,
) {
    let lease_id = HostLeaseId::new();
    // SAFETY: OpenProcess receives a non-zero PID authenticated by the
    // per-launch token; this thread closes the returned handle exactly once.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, process_id) };
    if process.is_null() {
        let _ = sender.send(HostLeaseEvent::TrackingFailed {
            lease_id,
            process_id,
        });
        let _ = write_host_lease_response(&mut connection, HOST_LEASE_REJECTED_RESPONSE);
        return;
    }
    if sender
        .send(HostLeaseEvent::Connected {
            lease_id,
            process_id,
        })
        .is_err()
    {
        let _ = write_host_lease_response(&mut connection, HOST_LEASE_REJECTED_RESPONSE);
        unsafe { CloseHandle(process) };
        return;
    }
    let _ = write_host_lease_response(&mut connection, HOST_LEASE_REGISTERED_RESPONSE);
    drop(connection);
    // The authenticated process handle is the durable lease. TCP is only the
    // bootstrap channel; WebView2 may reset inherited sockets during handoff.
    let wait = unsafe { WaitForSingleObject(process, INFINITE) };
    unsafe { CloseHandle(process) };
    let event = if wait == WAIT_OBJECT_0 {
        HostLeaseEvent::Disconnected {
            lease_id,
            process_id,
        }
    } else {
        HostLeaseEvent::TrackingFailed {
            lease_id,
            process_id,
        }
    };
    let _ = sender.send(event);
}

fn write_host_lease_response(connection: &mut TcpStream, response: &str) -> io::Result<()> {
    connection.write_all(response.as_bytes())?;
    connection.flush()
}

fn parse_host_lease_request(line: &str) -> Option<HostLeaseRequest> {
    let mut fields = line.split_whitespace();
    let request = match fields.next()? {
        AUTHORIZE_HOST_LEASE_REQUEST => HostLeaseRequest::Authorize {
            authority: fields.next()?.to_owned(),
            credential: HostRegistrationCredential::parse(fields.next()?)?,
        },
        REGISTER_HOST_LEASE_REQUEST => HostLeaseRequest::Register {
            credential: HostRegistrationCredential::parse(fields.next()?)?,
            process_id: fields
                .next()?
                .parse::<u32>()
                .ok()
                .filter(|process_id| *process_id != 0)?,
        },
        _ => return None,
    };
    fields.next().is_none().then_some(request)
}

struct DevelopmentProcesses {
    job: KillOnCloseJob,
    vite: Option<Child>,
    tauri: Option<Child>,
}

impl DevelopmentProcesses {
    fn new() -> io::Result<Self> {
        Ok(Self {
            job: KillOnCloseJob::new()?,
            vite: None,
            tauri: None,
        })
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.job.terminate()?;
        for child in [&mut self.tauri, &mut self.vite] {
            if let Some(mut child) = child.take() {
                let _ = child.wait();
            }
        }
        Ok(())
    }
}

impl Drop for DevelopmentProcesses {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct KillOnCloseJob {
    handle: HANDLE,
}

impl KillOnCloseJob {
    fn new() -> io::Result<Self> {
        // SAFETY: the null pointers request an unnamed Job Object with default
        // security, and the initialized structure remains live for the call.
        unsafe {
            let handle = CreateJobObjectW(ptr::null(), ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                let error = io::Error::last_os_error();
                CloseHandle(handle);
                return Err(error);
            }
            Ok(Self { handle })
        }
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: Child owns a live process handle for the duration of this call.
        let assigned = unsafe {
            AssignProcessToJobObject(self.handle, child.as_raw_handle().cast::<c_void>())
        };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn terminate(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        // SAFETY: handle is the live Job Object owned exclusively by self.
        let terminated = unsafe { TerminateJobObject(self.handle, JOB_TERMINATION_EXIT_CODE) };
        let error = (terminated == 0).then(io::Error::last_os_error);
        // SAFETY: handle is closed once and replaced with null immediately.
        unsafe { CloseHandle(self.handle) };
        self.handle = ptr::null_mut();
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // Closing the sole non-inheritable handle is the fail-closed path:
            // Windows terminates every assigned worker and descendant.
            unsafe { CloseHandle(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DevelopmentLifecycle, HostLeaseEvent, HostLeaseId, HostLeaseRequest, HostLeaseServer,
        KillOnCloseJob, LifecycleDecision, compose_tauri_cli_arguments, parse_host_lease_request,
    };
    use std::ffi::OsString;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpStream,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    fn send_host_lease_request(endpoint: std::net::SocketAddr, request: &str) -> String {
        let mut connection = TcpStream::connect(endpoint).expect("lease server connection");
        connection
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("response timeout");
        connection
            .write_all(request.as_bytes())
            .expect("lease request");
        connection.flush().expect("lease request flush");
        let mut response = String::new();
        BufReader::new(connection)
            .read_line(&mut response)
            .expect("lease response");
        response
    }

    #[test]
    fn a_host_registration_credential_is_consumed_and_cannot_be_replayed() {
        let server = HostLeaseServer::start().expect("lease server");
        let credential = uuid::Uuid::from_u128(7).simple().to_string();
        assert_eq!(
            send_host_lease_request(
                server.endpoint(),
                &format!("AUTHORIZE {} {credential}\n", server.authority()),
            ),
            "AUTHORIZED\n"
        );

        let mut process = Command::new("cmd.exe")
            .args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("waitable child process");
        let registration = format!("REGISTER {credential} {}\n", process.id());
        assert_eq!(
            send_host_lease_request(server.endpoint(), &registration),
            "REGISTERED\n"
        );
        let (lease_id, process_id) = match server
            .events()
            .recv_timeout(Duration::from_secs(1))
            .expect("connected lease")
        {
            HostLeaseEvent::Connected {
                lease_id,
                process_id,
            } => (lease_id, process_id),
            event => panic!("unexpected lease event: {event:?}"),
        };
        assert_eq!(process_id, process.id());

        assert_eq!(
            send_host_lease_request(
                server.endpoint(),
                &format!("AUTHORIZE {} {credential}\n", server.authority()),
            ),
            "REJECTED\n"
        );
        assert_eq!(
            send_host_lease_request(server.endpoint(), &registration),
            "REJECTED\n"
        );
        assert!(
            server
                .events()
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "a replay must not create another lease"
        );

        process.kill().expect("child terminates");
        process.wait().expect("child is reaped");
        assert!(matches!(
            server.events().recv_timeout(Duration::from_secs(1)),
            Ok(HostLeaseEvent::Disconnected {
                lease_id: disconnected_lease,
                process_id: disconnected_process,
            }) if disconnected_lease == lease_id && disconnected_process == process_id
        ));
    }

    #[test]
    fn closing_the_job_handle_terminates_an_assigned_process() {
        let job = KillOnCloseJob::new().expect("kill-on-close Job Object");
        let mut process = Command::new("cmd.exe")
            .args(["/d", "/c", "ping -n 30 127.0.0.1 >nul"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("long-running child");
        job.assign(&process).expect("child assigned to job");

        drop(job);

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if process.try_wait().expect("child state").is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = process.kill();
                let _ = process.wait();
                panic!("closing the Job Object did not terminate its child");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn application_arguments_follow_the_tauri_runner_separator() {
        let project = OsString::from(r"C:\Projetos\Dev.myalbuns");
        assert_eq!(
            compose_tauri_cli_arguments([], Some(project.clone())),
            vec![
                OsString::from("dev"),
                OsString::from("--"),
                OsString::from("--"),
                project.clone(),
            ]
        );
        assert_eq!(
            compose_tauri_cli_arguments([OsString::from("--verbose")], Some(project.clone()),),
            vec![
                OsString::from("dev"),
                OsString::from("--verbose"),
                OsString::from("--"),
                OsString::from("--"),
                project,
            ]
        );
        assert_eq!(
            compose_tauri_cli_arguments([OsString::from("--verbose")], None),
            vec![OsString::from("dev"), OsString::from("--verbose")]
        );
    }

    #[test]
    fn host_lease_requests_are_command_and_value_typed() {
        let credential = uuid::Uuid::from_u128(7).simple().to_string();
        assert!(matches!(
            parse_host_lease_request(&format!("AUTHORIZE secret {credential}\n")),
            Some(HostLeaseRequest::Authorize { authority, .. }) if authority == "secret"
        ));
        assert!(matches!(
            parse_host_lease_request(&format!("REGISTER {credential} 42\n")),
            Some(HostLeaseRequest::Register { process_id: 42, .. })
        ));
        assert!(parse_host_lease_request(&format!("REGISTER {credential} 0\n")).is_none());
        assert!(parse_host_lease_request(&format!("REGISTER {credential} nope\n")).is_none());
        assert!(parse_host_lease_request("REGISTER not-a-uuid 42\n").is_none());
        assert!(
            parse_host_lease_request(&format!("REGISTER {credential} 42 trailing\n")).is_none()
        );
    }

    #[test]
    fn vite_remains_owned_after_cli_exit_while_a_host_is_alive() {
        let mut lifecycle = DevelopmentLifecycle::default();
        lifecycle.apply(HostLeaseEvent::Connected {
            lease_id: HostLeaseId::from_u128(1),
            process_id: 41,
        });
        lifecycle.cli_exited(true);

        assert_eq!(lifecycle.decision(false), LifecycleDecision::Wait);
    }

    #[test]
    fn the_last_host_disconnect_completes_the_development_session() {
        let mut lifecycle = DevelopmentLifecycle::default();
        let first_lease = HostLeaseId::from_u128(1);
        let second_lease = HostLeaseId::from_u128(2);
        lifecycle.apply(HostLeaseEvent::Connected {
            lease_id: first_lease,
            process_id: 41,
        });
        lifecycle.apply(HostLeaseEvent::Connected {
            lease_id: second_lease,
            process_id: 42,
        });
        lifecycle.cli_exited(true);
        lifecycle.apply(HostLeaseEvent::Disconnected {
            lease_id: first_lease,
            process_id: 41,
        });
        assert_eq!(lifecycle.decision(false), LifecycleDecision::Wait);

        lifecycle.apply(HostLeaseEvent::Disconnected {
            lease_id: second_lease,
            process_id: 42,
        });
        assert_eq!(lifecycle.decision(false), LifecycleDecision::Complete);
    }

    #[test]
    fn a_delayed_disconnect_cannot_remove_a_new_lease_that_reuses_the_pid() {
        let old_lease = HostLeaseId::from_u128(1);
        let new_lease = HostLeaseId::from_u128(2);
        let mut lifecycle = DevelopmentLifecycle::default();
        lifecycle.apply(HostLeaseEvent::Connected {
            lease_id: old_lease,
            process_id: 41,
        });
        lifecycle.apply(HostLeaseEvent::Connected {
            lease_id: new_lease,
            process_id: 41,
        });
        lifecycle.cli_exited(true);

        lifecycle.apply(HostLeaseEvent::Disconnected {
            lease_id: old_lease,
            process_id: 41,
        });
        assert_eq!(lifecycle.decision(false), LifecycleDecision::Wait);

        lifecycle.apply(HostLeaseEvent::Disconnected {
            lease_id: new_lease,
            process_id: 41,
        });
        assert_eq!(lifecycle.decision(false), LifecycleDecision::Complete);
    }

    #[test]
    fn bootstrap_failure_fails_closed() {
        let mut lifecycle = DevelopmentLifecycle::default();
        lifecycle.cli_exited(false);
        assert!(matches!(
            lifecycle.decision(false),
            LifecycleDecision::Fail(_)
        ));
    }

    #[test]
    fn a_successful_global_only_session_completes_after_the_handoff_window() {
        let mut no_host = DevelopmentLifecycle::default();
        no_host.cli_exited(true);
        assert_eq!(no_host.decision(true), LifecycleDecision::Complete);
    }
}
