use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use myalbuns_paths::{AppPaths, NativePathDto, ProcessInstanceHandle, ProcessInstanceId};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::{
    Foundation::{ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
        GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_WAIT, WaitNamedPipeW,
    },
    UI::WindowsAndMessaging::AllowSetForegroundWindow,
};

use crate::named_mutex::{NamedMutex, NamedMutexError, NamedMutexGrant, scoped_name};

const PROTOCOL_VERSION: u32 = 1;
const PIPE_BUFFER_SIZE: u32 = 64 * 1024;
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;
const MAX_PROJECTS_PER_ACTIVATION: usize = 256;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalActivationBatch {
    pub(crate) client: ProcessInstanceId,
    pub(crate) projects: Vec<PathBuf>,
}

pub(crate) struct PrimaryGlobalActivation {
    initial_projects: Vec<PathBuf>,
    receiver: Mutex<Receiver<GlobalActivationBatch>>,
    flow: Arc<ActivationFlow>,
    _server: ActivationServer,
    _grant: NamedMutexGrant,
}

impl PrimaryGlobalActivation {
    pub(crate) fn initial_projects(&self) -> &[PathBuf] {
        &self.initial_projects
    }

    pub(crate) fn receive_timeout(
        &self,
        timeout: Duration,
    ) -> Result<GlobalActivationBatch, mpsc::RecvTimeoutError> {
        self.receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recv_timeout(timeout)
    }

    /// Stops admitting new activations and reports whether every activation
    /// already acknowledged to a client has reached its terminal outcome.
    pub(crate) fn stop_accepting(&self) -> bool {
        self.flow.stop_accepting()
    }

    pub(crate) fn resume_accepting(&self) {
        self.flow.resume_accepting();
    }

    /// Completes exactly one previously acknowledged activation.
    pub(crate) fn complete_activation(&self) -> bool {
        self.flow.complete_activation()
    }
}

pub(crate) enum GlobalActivationEntry {
    Primary(PrimaryGlobalActivation),
    Forwarded,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationRequest {
    version: u32,
    client: ProcessInstanceId,
    projects: Vec<NativePathDto>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "status", deny_unknown_fields)]
enum ActivationResponse {
    Prepared { server: ProcessInstanceId },
    Accepted,
    Retry,
    Rejected { reason: String },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationCommit {
    version: u32,
    server: ProcessInstanceId,
}

struct VerifiedActivation {
    batch: GlobalActivationBatch,
    _client_guard: ProcessInstanceHandle,
}

#[derive(Default)]
struct ActivationFlow {
    state: Mutex<ActivationFlowState>,
}

struct ActivationFlowState {
    accepting: bool,
    pending: usize,
}

impl Default for ActivationFlowState {
    fn default() -> Self {
        Self {
            accepting: true,
            pending: 0,
        }
    }
}

impl ActivationFlow {
    fn enqueue(
        &self,
        batch: GlobalActivationBatch,
        sender: &mpsc::Sender<GlobalActivationBatch>,
    ) -> Result<bool, mpsc::SendError<GlobalActivationBatch>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.accepting {
            return Ok(false);
        }
        state.pending += 1;
        if let Err(error) = sender.send(batch) {
            state.pending -= 1;
            return Err(error);
        }
        Ok(true)
    }

    fn stop_accepting(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.accepting = false;
        state.pending == 0
    }

    fn resume_accepting(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .accepting = true;
    }

    fn complete_activation(&self) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(state.pending > 0, "activation completion must be balanced");
        state.pending = state.pending.saturating_sub(1);
        !state.accepting && state.pending == 0
    }
}

struct ActivationServer {
    stop: Arc<AtomicBool>,
    pipe_name: Vec<u16>,
    worker: Option<JoinHandle<()>>,
}

pub(crate) fn enter_global_activation(
    app_paths: &AppPaths,
    projects: Vec<PathBuf>,
) -> io::Result<GlobalActivationEntry> {
    validate_projects(&projects)?;
    let pipe_name = pipe_name(app_paths)?;
    let mutex = NamedMutex::scoped(
        app_paths,
        "GlobalActivation",
        "product",
        "myalbuns-global-activation-mutex",
    );

    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match mutex.try_acquire() {
            Ok(grant) => {
                let (sender, receiver) = mpsc::channel();
                let flow = Arc::new(ActivationFlow::default());
                let server = ActivationServer::start(pipe_name.clone(), sender, Arc::clone(&flow))?;
                return Ok(GlobalActivationEntry::Primary(PrimaryGlobalActivation {
                    initial_projects: projects,
                    receiver: Mutex::new(receiver),
                    flow,
                    _server: server,
                    _grant: grant,
                }));
            }
            Err(NamedMutexError::Conflict) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "a instância primária não concluiu a ativação no prazo",
                    ));
                }
                match forward_activation(&pipe_name, &projects, remaining) {
                    Ok(true) => return Ok(GlobalActivationEntry::Forwarded),
                    Ok(false) | Err(_) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(false) => {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "a instância primária encerrou sem aceitar a ativação",
                        ));
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(NamedMutexError::Unavailable(reason)) => {
                return Err(io::Error::other(format!(
                    "não foi possível arbitrar a ativação global: {reason}"
                )));
            }
        }
    }
}

impl ActivationServer {
    fn start(
        pipe_name: Vec<u16>,
        sender: mpsc::Sender<GlobalActivationBatch>,
        flow: Arc<ActivationFlow>,
    ) -> io::Result<Self> {
        // Materialize the first server instance synchronously. Once the mutex
        // is visible to another process, its pipe endpoint is therefore ready.
        let first_pipe = create_server_pipe(&pipe_name)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_name = pipe_name.clone();
        let worker = thread::Builder::new()
            .name("myalbuns-global-activation-pipe".into())
            .spawn(move || {
                let mut next_pipe = Some(first_pipe);
                while let Some(pipe) = next_pipe.take() {
                    if serve_connection(&pipe, &sender, &flow, &worker_stop).is_err()
                        && worker_stop.load(Ordering::Acquire)
                    {
                        break;
                    }
                    drop(pipe);
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    next_pipe = create_server_pipe(&worker_name).ok();
                }
            })?;
        Ok(Self {
            stop,
            pipe_name,
            worker: Some(worker),
        })
    }
}

impl Drop for ActivationServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Connect without writing so a worker blocked in ConnectNamedPipe can
        // observe the stop flag and terminate before the mutex is released.
        let _ = connect_pipe(&self.pipe_name, Duration::from_millis(250));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve_connection(
    pipe: &File,
    sender: &mpsc::Sender<GlobalActivationBatch>,
    flow: &ActivationFlow,
    stop: &AtomicBool,
) -> io::Result<()> {
    let connected = unsafe { ConnectNamedPipe(pipe.as_raw_handle().cast(), std::ptr::null_mut()) };
    if connected == 0 {
        let code = unsafe { GetLastError() };
        if code != ERROR_PIPE_CONNECTED {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
    }
    if stop.load(Ordering::Acquire) {
        unsafe {
            DisconnectNamedPipe(pipe.as_raw_handle().cast());
        }
        return Ok(());
    }

    let result = complete_verified_handshake(pipe, sender, flow);
    unsafe {
        DisconnectNamedPipe(pipe.as_raw_handle().cast());
    }
    result
}

fn complete_verified_handshake(
    pipe: &File,
    sender: &mpsc::Sender<GlobalActivationBatch>,
    flow: &ActivationFlow,
) -> io::Result<()> {
    let verified = match receive_verified_batch(pipe) {
        Ok(verified) => verified,
        Err(error) => {
            return write_json_frame(
                pipe,
                &ActivationResponse::Rejected {
                    reason: error.to_string(),
                },
            );
        }
    };
    let server = match ProcessInstanceId::current() {
        Ok(server) => server,
        Err(error) => {
            return write_json_frame(
                pipe,
                &ActivationResponse::Rejected {
                    reason: format!(
                        "a identidade da instância primária está indisponível: {error}"
                    ),
                },
            );
        }
    };
    write_json_frame(pipe, &ActivationResponse::Prepared { server })?;
    let commit: ActivationCommit = read_json_frame(pipe)?;
    if commit.version != PROTOCOL_VERSION || commit.server != server {
        return write_json_frame(
            pipe,
            &ActivationResponse::Rejected {
                reason: "a confirmação da ativação não corresponde ao servidor exato".into(),
            },
        );
    }
    let response = match flow.enqueue(verified.batch, sender) {
        Ok(true) => ActivationResponse::Accepted,
        Ok(false) => ActivationResponse::Retry,
        Err(_) => ActivationResponse::Rejected {
            reason: "o consumidor de ativações foi encerrado".into(),
        },
    };
    write_json_frame(pipe, &response).and_then(|()| pipe.sync_data())
}

fn receive_verified_batch(pipe: &File) -> io::Result<VerifiedActivation> {
    let mut client_process_id = 0_u32;
    let queried =
        unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(), &mut client_process_id) };
    if queried == 0 || client_process_id == 0 {
        return Err(io::Error::last_os_error());
    }

    let request: ActivationRequest = read_json_frame(pipe)?;
    if request.version != PROTOCOL_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "versão incompatível do protocolo de ativação",
        ));
    }
    if request.client.process_id() != client_process_id {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "a identidade declarada não corresponde ao cliente do pipe",
        ));
    }
    let client_guard = ProcessInstanceHandle::open(request.client, 0).map_err(|error| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("a instância exata do cliente não pôde ser validada: {error}"),
        )
    })?;
    let projects = request
        .projects
        .into_iter()
        .map(NativePathDto::into_path_buf)
        .collect::<Vec<_>>();
    validate_projects(&projects)?;
    Ok(VerifiedActivation {
        batch: GlobalActivationBatch {
            client: request.client,
            projects,
        },
        _client_guard: client_guard,
    })
}

fn forward_activation(
    pipe_name: &[u16],
    projects: &[PathBuf],
    timeout: Duration,
) -> io::Result<bool> {
    let pipe = connect_pipe(pipe_name, timeout)?;
    let mut server_process_id = 0_u32;
    let queried =
        unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &mut server_process_id) };
    if queried == 0 || server_process_id == 0 {
        return Err(io::Error::last_os_error());
    }
    let request = ActivationRequest {
        version: PROTOCOL_VERSION,
        client: ProcessInstanceId::current()?,
        projects: projects
            .iter()
            .map(|path| NativePathDto::from(path.as_path()))
            .collect(),
    };
    write_json_frame(&pipe, &request)?;
    let server = match read_json_frame::<ActivationResponse>(&pipe)? {
        ActivationResponse::Prepared { server } => {
            if server.process_id() != server_process_id {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "a identidade declarada não corresponde ao servidor do pipe",
                ));
            }
            let _server_guard = ProcessInstanceHandle::open(server, 0).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("a instância exata do servidor não pôde ser validada: {error}"),
                )
            })?;
            // This is an ephemeral UI capability, not an ownership authority.
            // Failure is non-fatal per the Win32 contract: the visible window
            // remains the fallback signal when foreground privilege is denied.
            unsafe {
                AllowSetForegroundWindow(server.process_id());
            }
            server
        }
        ActivationResponse::Retry => return Ok(false),
        ActivationResponse::Rejected { reason } => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("a ativação foi recusada pela instância primária: {reason}"),
        ))?,
        ActivationResponse::Accepted => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a instância primária omitiu a preparação da ativação",
            ));
        }
    };
    write_json_frame(
        &pipe,
        &ActivationCommit {
            version: PROTOCOL_VERSION,
            server,
        },
    )?;
    match read_json_frame::<ActivationResponse>(&pipe)? {
        ActivationResponse::Accepted => Ok(true),
        ActivationResponse::Retry => Ok(false),
        ActivationResponse::Rejected { reason } => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("a ativação foi recusada pela instância primária: {reason}"),
        )),
        ActivationResponse::Prepared { .. } => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a instância primária repetiu a preparação da ativação",
        )),
    }
}

fn connect_pipe(pipe_name: &[u16], timeout: Duration) -> io::Result<File> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let milliseconds = u32::try_from(remaining.as_millis()).unwrap_or(u32::MAX - 1);
        let available = unsafe { WaitNamedPipeW(pipe_name.as_ptr(), milliseconds.max(1)) };
        if available == 0 {
            return Err(io::Error::last_os_error());
        }
        let native_name =
            std::ffi::OsString::from_wide(&pipe_name[..pipe_name.len().saturating_sub(1)]);
        match OpenOptions::new()
            .read(true)
            .write(true)
            .open(Path::new(&native_name))
        {
            Ok(pipe) => return Ok(pipe),
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                    && Instant::now() < deadline => {}
            Err(error) => return Err(error),
        }
    }
}

fn create_server_pipe(pipe_name: &[u16]) -> io::Result<File> {
    let handle = unsafe {
        CreateNamedPipeW(
            pipe_name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

fn pipe_name(app_paths: &AppPaths) -> io::Result<Vec<u16>> {
    let scoped = scoped_name(app_paths.local_root(), "GlobalActivation", "product")
        .map_err(io::Error::other)?;
    let leaf = scoped.strip_prefix(r"Local\").unwrap_or(&scoped);
    Ok(std::ffi::OsStr::new(&format!(r"\\.\pipe\{leaf}"))
        .encode_wide()
        .chain(std::iter::once(0))
        .collect())
}

fn validate_projects(projects: &[PathBuf]) -> io::Result<()> {
    if projects.len() > MAX_PROJECTS_PER_ACTIVATION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a ativação excede o limite de Projetos",
        ));
    }
    if let Some(path) = projects.iter().find(|path| {
        !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("myalbuns"))
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a ativação contém um caminho que não é Projeto: {path:?}"),
        ));
    }
    Ok(())
}

fn write_json_frame<T: Serialize>(mut writer: &File, value: &T) -> io::Result<()> {
    let payload = serde_json::to_vec(value).map_err(io::Error::other)?;
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a mensagem de ativação excede o limite",
        ));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "mensagem inválida"))?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&payload)?;
    writer.flush()
}

fn read_json_frame<T: for<'de> Deserialize<'de>>(mut reader: &File) -> io::Result<T> {
    let mut length = [0_u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "o tamanho da mensagem de ativação é inválido",
        ));
    }
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mensagem de ativação inválida: {error}"),
        )
    })
}

#[cfg(all(test, windows))]
mod tests {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, time::Duration};

    use myalbuns_paths::{AppPaths, ProcessInstanceId};

    use super::{GlobalActivationEntry, enter_global_activation};

    const HELPER_ROOT_ENV: &str = "MYALBUNS_GLOBAL_ACTIVATION_HELPER_ROOT";
    const HELPER_PROJECT_ENV: &str = "MYALBUNS_GLOBAL_ACTIVATION_HELPER_PROJECT";

    #[test]
    fn a_later_native_activation_is_forwarded_to_the_single_primary_entry() {
        let root = tempfile::tempdir().expect("the activation fixture exists");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let initial = PathBuf::from(r"C:\Projetos\Inicial.myalbuns");
        let mut native = r"C:\Projetos\".encode_utf16().collect::<Vec<_>>();
        native.push(0xd800);
        native.extend(".myalbuns".encode_utf16());
        let forwarded = vec![
            PathBuf::from(OsString::from_wide(&native)),
            PathBuf::from(r"\\servidor\Albuns\Alias.myalbuns"),
        ];

        let GlobalActivationEntry::Primary(primary) =
            enter_global_activation(&paths, vec![initial.clone()])
                .expect("the first entry becomes primary")
        else {
            panic!("the first entry must not be forwarded");
        };
        assert_eq!(primary.initial_projects(), &[initial]);

        let forwarding_paths = paths.clone();
        let expected = forwarded.clone();
        let forwarder =
            std::thread::spawn(move || enter_global_activation(&forwarding_paths, forwarded));
        let activation = primary
            .receive_timeout(Duration::from_secs(5))
            .expect("the primary receives the later activation");

        assert_eq!(activation.client, ProcessInstanceId::current().unwrap());
        assert_eq!(activation.projects, expected);
        assert!(matches!(
            forwarder
                .join()
                .expect("the forwarder does not panic")
                .unwrap(),
            GlobalActivationEntry::Forwarded
        ));
    }

    #[test]
    fn activation_retries_as_primary_while_the_previous_entry_is_exiting() {
        let root = tempfile::tempdir().expect("the activation fixture exists");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let project = PathBuf::from(r"C:\Projetos\Continuidade.myalbuns");
        let GlobalActivationEntry::Primary(primary) =
            enter_global_activation(&paths, Vec::new()).expect("the first entry becomes primary")
        else {
            panic!("the first entry must be primary");
        };
        assert!(primary.stop_accepting());

        let retry_paths = paths.clone();
        let expected = project.clone();
        let retry =
            std::thread::spawn(move || enter_global_activation(&retry_paths, vec![project]));
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            !retry.is_finished(),
            "an activation cannot be discarded during the primary handoff"
        );
        drop(primary);

        let GlobalActivationEntry::Primary(successor) = retry
            .join()
            .expect("the retry does not panic")
            .expect("the retry becomes the successor")
        else {
            panic!("the activation must be retained by a successor primary");
        };
        assert_eq!(successor.initial_projects(), &[expected]);
    }

    #[test]
    fn a_second_executable_process_forwards_to_the_exact_primary_instance() {
        use std::{os::windows::io::AsRawHandle, process::Command};

        let root = tempfile::tempdir().expect("the activation fixture exists");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let project = root.path().join("Processo secundário.myalbuns");
        let GlobalActivationEntry::Primary(primary) =
            enter_global_activation(&paths, Vec::new()).expect("the parent becomes primary")
        else {
            panic!("the parent must be primary");
        };

        let mut child = Command::new(std::env::current_exe().expect("the test executable exists"))
            .args([
                "--exact",
                "global_activation::tests::forwarder_helper_process",
                "--ignored",
                "--nocapture",
            ])
            .env(HELPER_ROOT_ENV, root.path())
            .env(HELPER_PROJECT_ENV, &project)
            .spawn()
            .expect("the second executable process starts");
        let expected_client =
            ProcessInstanceId::from_process_handle(child.id(), child.as_raw_handle().cast())
                .expect("the second process is captured exactly");
        let activation = primary
            .receive_timeout(Duration::from_secs(5))
            .expect("the primary receives the process activation");

        assert_eq!(activation.client, expected_client);
        assert_eq!(activation.projects, vec![project]);
        assert!(child.wait().expect("the child is reaped").success());
    }

    #[test]
    fn one_primary_accepts_multiple_sequential_native_activations() {
        let root = tempfile::tempdir().expect("the activation fixture exists");
        let paths = AppPaths::from_roots(&root.path().join("roaming"), &root.path().join("local"));
        let GlobalActivationEntry::Primary(primary) =
            enter_global_activation(&paths, Vec::new()).expect("the first entry becomes primary")
        else {
            panic!("the first entry must be primary");
        };

        for index in 1..=2 {
            let project = PathBuf::from(format!(r"C:\Projetos\Ativação {index}.myalbuns"));
            let forwarding_paths = paths.clone();
            let expected = project.clone();
            let forwarder = std::thread::spawn(move || {
                enter_global_activation(&forwarding_paths, vec![project])
            });
            let batch = primary
                .receive_timeout(Duration::from_secs(5))
                .expect("the sequential activation reaches the same primary");
            assert_eq!(batch.projects, vec![expected]);
            assert!(matches!(
                forwarder
                    .join()
                    .expect("the forwarder does not panic")
                    .unwrap(),
                GlobalActivationEntry::Forwarded
            ));
            assert!(!primary.complete_activation());
        }
    }

    #[test]
    #[ignore = "spawned by the executable-process activation test"]
    fn forwarder_helper_process() {
        let root = std::env::var_os(HELPER_ROOT_ENV)
            .map(PathBuf::from)
            .expect("the helper root is supplied");
        let project = std::env::var_os(HELPER_PROJECT_ENV)
            .map(PathBuf::from)
            .expect("the helper project is supplied");
        let paths = AppPaths::from_roots(&root.join("roaming"), &root.join("local"));

        assert!(matches!(
            enter_global_activation(&paths, vec![project]).expect("the activation is forwarded"),
            GlobalActivationEntry::Forwarded
        ));
    }
}
