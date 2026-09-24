use std::{
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use myalbuns_paths::ProcessInstanceId;

use crate::ipc_contract::ProjectRecoveryDecision;

use super::{
    BootstrapIntent, BootstrapRequest, CreateWriteAuthorization, HostTerminal,
    InitialProjectCreationConfiguration, ProjectRecoveryRequest, SaveExternalCopyRequest,
    TargetAuthority, TerminalValidationError, ValidatedTerminal, validate_terminal,
};

const MAX_TERMINAL_BYTES: usize = 32 * 1024;

#[derive(Clone)]
pub(crate) struct StartupProgressReporter(
    Arc<dyn Fn(crate::ipc_contract::StartupImageProgress) + Send + Sync>,
);

impl std::fmt::Debug for StartupProgressReporter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StartupProgressReporter")
    }
}

impl StartupProgressReporter {
    pub(crate) fn new(
        publish: impl Fn(crate::ipc_contract::StartupImageProgress) + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(publish))
    }
}
#[cfg(debug_assertions)]
const HOST_WEBVIEW_DEBUG_PORT_ENV: &str = "MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT";

#[derive(Clone, Debug)]
pub(crate) struct ProjectHostBootstrap {
    executable: PathBuf,
    terminal_timeout: Duration,
    creation_timeout: Duration,
    progress: Option<StartupProgressReporter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyHost {
    pub(crate) host_pid: u32,
    pub(crate) project_id: String,
    pub(crate) revision: u64,
}

#[derive(Debug)]
pub(crate) enum BootstrapOutcome {
    Ready(ReadyHost),
    FocusExisting {
        project_id: String,
        owner_process: ProcessInstanceId,
    },
    ExternalCopyNotWritable(PendingExternalCopyProcess),
    RecoveryAvailable(PendingRecoveryProcess),
}

#[derive(Debug)]
pub(crate) struct PendingExternalCopyProcess {
    child: PendingChild,
    stdin: ChildStdin,
    terminal_receiver: mpsc::Receiver<Result<HostTerminal, BootstrapFailure>>,
    request: BootstrapRequest,
    terminal_timeout: Duration,
}

#[derive(Debug)]
pub(crate) struct PendingRecoveryProcess {
    child: PendingChild,
    stdin: ChildStdin,
    terminal_receiver: mpsc::Receiver<Result<HostTerminal, BootstrapFailure>>,
    request: BootstrapRequest,
    terminal_timeout: Duration,
}

#[derive(Debug)]
pub(crate) enum RecoveryContinuationOutcome {
    Ready(ReadyHost),
    Deferred,
}

impl PendingExternalCopyProcess {
    pub(crate) fn attempt_id(&self) -> &str {
        &self.request.attempt_id
    }

    pub(crate) fn host_process_id(&mut self) -> u32 {
        self.child.child_mut().id()
    }
}

impl PendingRecoveryProcess {
    pub(crate) fn attempt_id(&self) -> &str {
        &self.request.attempt_id
    }

    pub(crate) fn resolve(
        self,
        decision: ProjectRecoveryDecision,
    ) -> Result<RecoveryContinuationOutcome, BootstrapFailure> {
        continue_project_recovery(self, decision)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BootstrapFailureKind {
    InvalidAuthority,
    HostUnavailable,
    Transport,
    Timeout,
    InvalidTerminal,
    CorrelationMismatch,
    HostFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BootstrapFailure {
    pub(crate) kind: BootstrapFailureKind,
    pub(crate) stage: Option<super::FailureStage>,
    pub(crate) code: Option<super::FailureCode>,
}

impl ProjectHostBootstrap {
    pub(crate) fn new(executable: PathBuf, terminal_timeout: Duration) -> Self {
        Self {
            executable,
            terminal_timeout,
            creation_timeout: terminal_timeout,
            progress: None,
        }
    }

    pub(crate) fn with_creation_timeout(mut self, timeout: Duration) -> Self {
        self.creation_timeout = timeout;
        self
    }

    pub(crate) fn with_progress(mut self, progress: StartupProgressReporter) -> Self {
        self.progress = Some(progress);
        self
    }

    pub(crate) fn open(
        &self,
        authority: TargetAuthority,
    ) -> Result<BootstrapOutcome, BootstrapFailure> {
        let request = new_open_request(authority)?;
        self.launch(request)
    }

    pub(crate) fn create(
        &self,
        authority: TargetAuthority,
        configuration: Box<InitialProjectCreationConfiguration>,
        authorization: CreateWriteAuthorization,
    ) -> Result<BootstrapOutcome, BootstrapFailure> {
        let request = new_request(
            authority,
            BootstrapIntent::CreateNew {
                configuration,
                authorization,
            },
        )?;
        self.launch(request)
    }

    pub(crate) fn save_external_copy_as(
        &self,
        pending: PendingExternalCopyProcess,
        destination: TargetAuthority,
        authorization: CreateWriteAuthorization,
    ) -> Result<BootstrapOutcome, BootstrapFailure> {
        continue_external_copy(pending, destination, authorization)
    }

    fn launch(&self, request: BootstrapRequest) -> Result<BootstrapOutcome, BootstrapFailure> {
        let child = spawn_host(&self.executable, &request.launch_nonce)?;
        let timeout = match request.intent {
            BootstrapIntent::CreateNew { .. } => self.creation_timeout,
            BootstrapIntent::OpenExisting => self.terminal_timeout,
        };
        supervise_child_with_progress(child, request, timeout, self.progress.clone())
    }
}

pub(crate) fn new_open_request(
    authority: TargetAuthority,
) -> Result<BootstrapRequest, BootstrapFailure> {
    new_request(authority, BootstrapIntent::OpenExisting)
}

fn new_request(
    authority: TargetAuthority,
    intent: BootstrapIntent,
) -> Result<BootstrapRequest, BootstrapFailure> {
    if !authority.validates_target_binding() {
        return Err(BootstrapFailure {
            kind: BootstrapFailureKind::InvalidAuthority,
            stage: None,
            code: None,
        });
    }

    Ok(BootstrapRequest {
        protocol_version: super::protocol::PROTOCOL_VERSION,
        attempt_id: uuid::Uuid::new_v4().hyphenated().to_string(),
        launch_nonce: uuid::Uuid::new_v4().simple().to_string(),
        intent,
        authority,
    })
}

fn spawn_host(executable: &Path, launch_nonce: &str) -> Result<Child, BootstrapFailure> {
    let mut command = Command::new(executable);
    command
        .arg(crate::runtime_role::PROJECT_HOST_ROLE_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(debug_assertions)]
    configure_host_webview_debugging(&mut command)?;
    #[cfg(debug_assertions)]
    let pending_host_lease =
        crate::dev_host_registration::prepare_host_command(&mut command, launch_nonce).map_err(
            |_| BootstrapFailure {
                kind: BootstrapFailureKind::HostUnavailable,
                stage: Some(super::FailureStage::Transport),
                code: Some(super::FailureCode::IoFailure),
            },
        )?;
    #[cfg(not(debug_assertions))]
    let _ = launch_nonce;
    let child = command.spawn().map_err(|_| BootstrapFailure {
        kind: BootstrapFailureKind::HostUnavailable,
        stage: None,
        code: None,
    })?;
    #[cfg(debug_assertions)]
    let mut child = child;
    #[cfg(debug_assertions)]
    if pending_host_lease
        .as_ref()
        .is_some_and(|authorization| authorization.authorize_spawned_host(&child).is_err())
    {
        reap(&mut child);
        return Err(BootstrapFailure {
            kind: BootstrapFailureKind::HostUnavailable,
            stage: Some(super::FailureStage::Transport),
            code: Some(super::FailureCode::IoFailure),
        });
    }
    Ok(child)
}

#[cfg(debug_assertions)]
fn configure_host_webview_debugging(command: &mut Command) -> Result<(), BootstrapFailure> {
    let argument = crate::desktop_webview_policy::remote_debugging_argument(std::env::var_os(
        HOST_WEBVIEW_DEBUG_PORT_ENV,
    ))
    .map_err(|_| BootstrapFailure {
        kind: BootstrapFailureKind::HostUnavailable,
        stage: None,
        code: None,
    })?;
    if let Some(argument) = argument {
        command.env("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", argument);
    }
    Ok(())
}

#[cfg(test)]
fn supervise_child(
    child: Child,
    request: BootstrapRequest,
    terminal_timeout: Duration,
) -> Result<BootstrapOutcome, BootstrapFailure> {
    supervise_child_with_progress(child, request, terminal_timeout, None)
}

fn supervise_child_with_progress(
    child: Child,
    request: BootstrapRequest,
    terminal_timeout: Duration,
    progress: Option<StartupProgressReporter>,
) -> Result<BootstrapOutcome, BootstrapFailure> {
    let mut pending = PendingChild::new(child);
    let mut stdin = pending
        .child_mut()
        .stdin
        .take()
        .ok_or_else(transport_failure)?;
    serde_json::to_writer(&mut stdin, &request).map_err(|_| transport_failure())?;
    stdin.write_all(b"\n").map_err(|_| transport_failure())?;
    stdin.flush().map_err(|_| transport_failure())?;
    let stdout = pending
        .child_mut()
        .stdout
        .take()
        .ok_or_else(transport_failure)?;
    let (terminal_sender, terminal_receiver) = mpsc::sync_channel(2);
    let reader_request = request.clone();
    let host_pid = pending.child_mut().id();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let first = read_terminal(&mut reader, &reader_request, host_pid, progress.as_ref());
        let awaits_continuation = matches!(
            first,
            Ok(HostTerminal::ExternalCopyNotWritable { .. }
                | HostTerminal::RecoveryAvailable { .. })
        );
        if terminal_sender.send(first).is_ok() && awaits_continuation {
            let _ = terminal_sender.send(read_terminal(
                &mut reader,
                &reader_request,
                host_pid,
                progress.as_ref(),
            ));
        }
    });

    let terminal = receive_terminal(&terminal_receiver, terminal_timeout)?;

    match validate_terminal(&request, pending.child_mut().id(), terminal) {
        Ok(ValidatedTerminal::Ready {
            host_pid,
            project_id,
            revision,
        }) => {
            drop(stdin);
            pending.detach();
            Ok(BootstrapOutcome::Ready(ReadyHost {
                host_pid,
                project_id,
                revision,
            }))
        }
        Ok(ValidatedTerminal::FocusExisting {
            project_id,
            owner_process,
        }) => Ok(BootstrapOutcome::FocusExisting {
            project_id,
            owner_process,
        }),
        Ok(ValidatedTerminal::ExternalCopyNotWritable { .. }) => Ok(
            BootstrapOutcome::ExternalCopyNotWritable(PendingExternalCopyProcess {
                child: pending,
                stdin,
                terminal_receiver,
                request,
                terminal_timeout,
            }),
        ),
        Ok(ValidatedTerminal::RecoveryAvailable { .. }) => Ok(BootstrapOutcome::RecoveryAvailable(
            PendingRecoveryProcess {
                child: pending,
                stdin,
                terminal_receiver,
                request,
                terminal_timeout,
            },
        )),
        Ok(ValidatedTerminal::RecoveryDeferred { .. }) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::InvalidTerminal,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::InvalidRequest),
        }),
        Ok(ValidatedTerminal::Failed { stage, code, .. }) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::HostFailed,
            stage: Some(stage),
            code: Some(code),
        }),
        Err(TerminalValidationError::CorrelationMismatch) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::CorrelationMismatch,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::CorrelationMismatch),
        }),
    }
}

fn continue_external_copy(
    mut pending: PendingExternalCopyProcess,
    destination: TargetAuthority,
    authorization: CreateWriteAuthorization,
) -> Result<BootstrapOutcome, BootstrapFailure> {
    if !destination.validates_target_binding() {
        return Err(BootstrapFailure {
            kind: BootstrapFailureKind::InvalidAuthority,
            stage: None,
            code: None,
        });
    }
    let continuation = SaveExternalCopyRequest {
        protocol_version: super::protocol::PROTOCOL_VERSION,
        attempt_id: pending.request.attempt_id.clone(),
        launch_nonce: pending.request.launch_nonce.clone(),
        authority: destination,
        authorization,
    };
    serde_json::to_writer(&mut pending.stdin, &continuation).map_err(|_| transport_failure())?;
    pending
        .stdin
        .write_all(b"\n")
        .map_err(|_| transport_failure())?;
    pending.stdin.flush().map_err(|_| transport_failure())?;
    drop(pending.stdin);
    let terminal = receive_terminal(&pending.terminal_receiver, pending.terminal_timeout)?;
    match validate_terminal(&pending.request, pending.child.child_mut().id(), terminal) {
        Ok(ValidatedTerminal::Ready {
            host_pid,
            project_id,
            revision,
        }) => {
            pending.child.detach();
            Ok(BootstrapOutcome::Ready(ReadyHost {
                host_pid,
                project_id,
                revision,
            }))
        }
        Ok(ValidatedTerminal::FocusExisting {
            project_id,
            owner_process,
        }) => Ok(BootstrapOutcome::FocusExisting {
            project_id,
            owner_process,
        }),
        Ok(
            ValidatedTerminal::ExternalCopyNotWritable { .. }
            | ValidatedTerminal::RecoveryAvailable { .. }
            | ValidatedTerminal::RecoveryDeferred { .. },
        ) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::InvalidTerminal,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::InvalidRequest),
        }),
        Ok(ValidatedTerminal::Failed { stage, code, .. }) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::HostFailed,
            stage: Some(stage),
            code: Some(code),
        }),
        Err(TerminalValidationError::CorrelationMismatch) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::CorrelationMismatch,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::CorrelationMismatch),
        }),
    }
}

fn continue_project_recovery(
    mut pending: PendingRecoveryProcess,
    decision: ProjectRecoveryDecision,
) -> Result<RecoveryContinuationOutcome, BootstrapFailure> {
    let continuation = ProjectRecoveryRequest {
        protocol_version: super::protocol::PROTOCOL_VERSION,
        attempt_id: pending.request.attempt_id.clone(),
        launch_nonce: pending.request.launch_nonce.clone(),
        decision,
    };
    serde_json::to_writer(&mut pending.stdin, &continuation).map_err(|_| transport_failure())?;
    pending
        .stdin
        .write_all(b"\n")
        .map_err(|_| transport_failure())?;
    pending.stdin.flush().map_err(|_| transport_failure())?;
    drop(pending.stdin);
    let terminal = receive_terminal(&pending.terminal_receiver, pending.terminal_timeout)?;
    match validate_terminal(&pending.request, pending.child.child_mut().id(), terminal) {
        Ok(ValidatedTerminal::Ready {
            host_pid,
            project_id,
            revision,
        }) => {
            pending.child.detach();
            Ok(RecoveryContinuationOutcome::Ready(ReadyHost {
                host_pid,
                project_id,
                revision,
            }))
        }
        Ok(ValidatedTerminal::RecoveryDeferred { .. }) => Ok(RecoveryContinuationOutcome::Deferred),
        Ok(ValidatedTerminal::Failed { stage, code, .. }) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::HostFailed,
            stage: Some(stage),
            code: Some(code),
        }),
        Ok(
            ValidatedTerminal::FocusExisting { .. }
            | ValidatedTerminal::ExternalCopyNotWritable { .. }
            | ValidatedTerminal::RecoveryAvailable { .. },
        ) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::InvalidTerminal,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::InvalidRequest),
        }),
        Err(TerminalValidationError::CorrelationMismatch) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::CorrelationMismatch,
            stage: Some(super::FailureStage::Protocol),
            code: Some(super::FailureCode::CorrelationMismatch),
        }),
    }
}

fn receive_terminal(
    receiver: &mpsc::Receiver<Result<HostTerminal, BootstrapFailure>>,
    terminal_timeout: Duration,
) -> Result<HostTerminal, BootstrapFailure> {
    match receiver.recv_timeout(terminal_timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(BootstrapFailure {
            kind: BootstrapFailureKind::Timeout,
            stage: Some(super::FailureStage::Transport),
            code: Some(super::FailureCode::HostExitedBeforeReady),
        }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(transport_failure()),
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum HostMessage {
    Terminal(HostTerminal),
    Progress(super::HostProgress),
}

fn read_terminal(
    reader: &mut impl Read,
    request: &BootstrapRequest,
    host_pid: u32,
    reporter: Option<&StartupProgressReporter>,
) -> Result<HostTerminal, BootstrapFailure> {
    loop {
        match read_host_message(reader)? {
            HostMessage::Terminal(terminal) => return Ok(terminal),
            HostMessage::Progress(message) => {
                let progress =
                    message
                        .validate(request, host_pid)
                        .map_err(|_| BootstrapFailure {
                            kind: BootstrapFailureKind::CorrelationMismatch,
                            stage: Some(super::FailureStage::Protocol),
                            code: Some(super::FailureCode::CorrelationMismatch),
                        })?;
                if progress.total_files == 0 || progress.completed_files > progress.total_files {
                    return Err(BootstrapFailure {
                        kind: BootstrapFailureKind::InvalidTerminal,
                        stage: Some(super::FailureStage::Protocol),
                        code: Some(super::FailureCode::InvalidRequest),
                    });
                }
                if let Some(reporter) = reporter {
                    (reporter.0)(progress);
                }
            }
        }
    }
}

fn read_host_message(reader: &mut impl Read) -> Result<HostMessage, BootstrapFailure> {
    let mut bytes = Vec::new();
    for _ in 0..=MAX_TERMINAL_BYTES {
        let mut byte = [0_u8; 1];
        match reader.read(&mut byte) {
            Ok(0) => return Err(transport_failure()),
            Ok(_) if byte[0] == b'\n' => {
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                return serde_json::from_slice(&bytes).map_err(|_| BootstrapFailure {
                    kind: BootstrapFailureKind::InvalidTerminal,
                    stage: Some(super::FailureStage::Protocol),
                    code: Some(super::FailureCode::InvalidRequest),
                });
            }
            Ok(_) => bytes.push(byte[0]),
            Err(_) => return Err(transport_failure()),
        }
    }

    Err(BootstrapFailure {
        kind: BootstrapFailureKind::InvalidTerminal,
        stage: Some(super::FailureStage::Protocol),
        code: Some(super::FailureCode::InvalidRequest),
    })
}

fn transport_failure() -> BootstrapFailure {
    BootstrapFailure {
        kind: BootstrapFailureKind::Transport,
        stage: Some(super::FailureStage::Transport),
        code: Some(super::FailureCode::HostExitedBeforeReady),
    }
}

#[derive(Debug)]
struct PendingChild(Option<Child>);

impl PendingChild {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("the pending Host is present")
    }

    fn detach(mut self) {
        self.0.take();
    }
}

impl Drop for PendingChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            reap(&mut child);
        }
    }
}

/// How long a killed Host may take to finish terminating. A process whose
/// termination is held in the kernel (for instance by a filter driver) may
/// never exit; the supervisor must not wait for it forever.
const REAP_TIMEOUT: Duration = Duration::from_secs(10);
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Terminates a Host and collects it, waiting at most `REAP_TIMEOUT`. A Host
/// that does not exit in time is left to the operating system and logged.
fn reap(child: &mut Child) {
    let _ = child.kill();
    if !wait_bounded(child, REAP_TIMEOUT) {
        tracing::warn!(
            target: "myalbuns.desktop",
            event = "project_host_termination_unconfirmed",
            host_pid = child.id(),
            timeout_ms = REAP_TIMEOUT.as_millis() as u64,
        );
    }
}

/// Returns whether the process exited within `timeout`.
fn wait_bounded(child: &mut Child, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if std::time::Instant::now() < deadline => {
                thread::sleep(REAP_POLL_INTERVAL);
            }
            Ok(None) | Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process::{Child, Command, Stdio},
        time::Duration,
    };

    use myalbuns_paths::{NativePathDto, OperationPathContext, RootBindingPlan};

    use super::super::configuration::{
        InitialBackground, InitialBackgroundContent, InitialDisplayUnit,
        InitialDocumentConfiguration, InitialFrameBorder, InitialOverlay,
        InitialProjectCreationConfiguration, InitialSheetFormat, InitialStructureConfiguration,
        InitialVisualDefaults,
    };

    use super::*;

    fn authority(path: PathBuf) -> TargetAuthority {
        let mut context = OperationPathContext::new();
        context
            .capture(&path)
            .expect("the fixture path has a supported root");
        TargetAuthority {
            logical_target: NativePathDto::from(path),
            root_bindings: context.freeze(),
        }
    }

    fn fixture_request() -> BootstrapRequest {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Projeto.myalbuns");
        new_open_request(authority(target)).expect("valid bootstrap fixture")
    }

    #[test]
    fn startup_progress_stream_preserves_every_count_until_the_actual_terminal() {
        use crate::ipc_contract::StartupImageProgress;
        let request = fixture_request();
        let mut stream = Vec::new();
        for completed_files in 0..=3 {
            super::super::write_host_progress(
                &mut stream,
                &super::super::HostProgress::preparing_images(
                    &request,
                    StartupImageProgress {
                        completed_files,
                        total_files: 3,
                    },
                ),
            )
            .unwrap();
        }
        let terminal = HostTerminal::ready(&request, "project".into(), 1);
        super::super::write_host_terminal(&mut stream, &terminal).unwrap();
        let (sender, receiver) = mpsc::channel();
        let reporter = StartupProgressReporter::new(move |progress| {
            sender.send(progress).unwrap();
        });
        let actual = read_terminal(
            &mut stream.as_slice(),
            &request,
            std::process::id(),
            Some(&reporter),
        )
        .unwrap();
        assert_eq!(actual, terminal);
        assert_eq!(
            receiver
                .try_iter()
                .map(|p| p.completed_files)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn startup_progress_rejects_other_attempts_and_invalid_counts() {
        use crate::ipc_contract::StartupImageProgress;
        let request = fixture_request();
        for (host_pid, completed_files, total_files, expected) in [
            (
                std::process::id() + 1,
                0,
                3,
                BootstrapFailureKind::CorrelationMismatch,
            ),
            (
                std::process::id(),
                4,
                3,
                BootstrapFailureKind::InvalidTerminal,
            ),
            (
                std::process::id(),
                0,
                0,
                BootstrapFailureKind::InvalidTerminal,
            ),
        ] {
            let mut stream = Vec::new();
            super::super::write_host_progress(
                &mut stream,
                &super::super::HostProgress::preparing_images(
                    &request,
                    StartupImageProgress {
                        completed_files,
                        total_files,
                    },
                ),
            )
            .unwrap();
            let error =
                read_terminal(&mut stream.as_slice(), &request, host_pid, None).unwrap_err();
            assert_eq!(error.kind, expected);
        }
    }

    /// Upper bound for a PowerShell fixture that does answer. A cold PowerShell
    /// start on a loaded machine can take several seconds; tests that assert
    /// the timeout itself use their own short deadline with a silent fixture.
    #[cfg(windows)]
    const FIXTURE_TERMINAL_TIMEOUT: Duration = Duration::from_secs(30);

    /// A host that starts at once and never answers. Killing PowerShell while
    /// it is still starting can leave it stuck in process termination, so the
    /// timeout path uses a native executable instead. Each fixture waits on
    /// its own signal: a second `waitfor` on a busy name exits at once.
    #[cfg(windows)]
    fn silent_host() -> Child {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let signal = format!(
            "MyAlbunsSilentHost{}n{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        Command::new("waitfor.exe")
            .args(["/t", "10", &signal])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the silent host fixture starts")
    }

    #[cfg(windows)]
    fn powershell_host(script: &str) -> Child {
        Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the PowerShell host fixture starts")
    }

    #[cfg(windows)]
    fn pending_external_copy_host() -> Child {
        powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $pending = @{
              state = 'externalCopyNotWritable'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
            }
            [Console]::Out.WriteLine(($pending | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            $continuation = [Console]::In.ReadLine() | ConvertFrom-Json
            if ($null -ne $continuation.source `
                -or $continuation.attemptId -ne $request.attemptId `
                -or $continuation.launchNonce -ne $request.launchNonce `
                -or $null -eq $continuation.authority) {
              exit 2
            }
            $ready = @{
              state = 'ready'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
              projectId = '8dfdb57a-918b-4280-9969-88b31b635f57'
              revision = 7
            }
            [Console]::Out.WriteLine(($ready | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            "#,
        )
    }

    #[cfg(windows)]
    fn pending_recovery_host() -> Child {
        powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $pending = @{
              state = 'recoveryAvailable'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
            }
            [Console]::Out.WriteLine(($pending | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            $continuation = [Console]::In.ReadLine() | ConvertFrom-Json
            if ($continuation.attemptId -ne $request.attemptId `
                -or $continuation.launchNonce -ne $request.launchNonce `
                -or $continuation.protocolVersion -ne $request.protocolVersion) {
              exit 2
            }
            if ($continuation.decision -eq 'nowNot') {
              $terminal = @{
                state = 'recoveryDeferred'
                attemptId = $request.attemptId
                launchNonce = $request.launchNonce
                hostPid = $PID
              }
            } else {
              $terminal = @{
                state = 'ready'
                attemptId = $request.attemptId
                launchNonce = $request.launchNonce
                hostPid = $PID
                projectId = 'a230216b-197c-424e-b146-2a95fc7dfbf7'
                revision = 11
              }
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            if ($continuation.decision -ne 'nowNot') { Start-Sleep -Seconds 2 }
            "#,
        )
    }

    #[cfg(windows)]
    fn process_is_alive(process_id: u32) -> bool {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };

        const STILL_ACTIVE: u32 = 259;
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

    #[test]
    fn each_open_request_has_fresh_correlation_and_frozen_native_authority() {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Projeto.myalbuns");
        let first = new_open_request(authority(target.clone())).expect("valid request");
        let second = new_open_request(authority(target.clone())).expect("valid request");

        assert_eq!(
            first.protocol_version,
            super::super::protocol::PROTOCOL_VERSION
        );
        assert_eq!(first.intent, BootstrapIntent::OpenExisting);
        assert_eq!(first.authority.logical_target.as_path(), target);
        assert!(first.authority.root_bindings.covers(&target));
        assert!(!first.attempt_id.is_empty());
        assert!(!first.launch_nonce.is_empty());
        assert_ne!(first.attempt_id, second.attempt_id);
        assert_ne!(first.launch_nonce, second.launch_nonce);
    }

    #[test]
    fn development_host_debugging_accepts_only_a_nonzero_port() {
        assert_eq!(
            crate::desktop_webview_policy::remote_debugging_argument(Some(
                std::ffi::OsString::from("9222"),
            ))
            .expect("valid debug port"),
            Some(std::ffi::OsString::from("--remote-debugging-port=9222"))
        );
        assert!(
            crate::desktop_webview_policy::remote_debugging_argument(None)
                .expect("absent port")
                .is_none()
        );
        assert!(
            crate::desktop_webview_policy::remote_debugging_argument(Some(
                std::ffi::OsString::from("0"),
            ))
            .is_err()
        );
        assert!(
            crate::desktop_webview_policy::remote_debugging_argument(Some(
                std::ffi::OsString::from("invalid"),
            ))
            .is_err()
        );
    }

    #[test]
    fn an_unbound_target_is_rejected_before_any_process_is_started() {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Projeto.myalbuns");
        let error = new_open_request(TargetAuthority {
            logical_target: NativePathDto::from(target),
            root_bindings: RootBindingPlan::default(),
        })
        .expect_err("authority without its root is invalid");

        assert_eq!(error.kind, BootstrapFailureKind::InvalidAuthority);
    }

    #[test]
    fn create_request_freezes_its_configuration_and_write_authorization() {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Novo.myalbuns");
        let configuration = InitialProjectCreationConfiguration {
            frame_gap_um: 5_000,
            document: InitialDocumentConfiguration {
                display_unit: InitialDisplayUnit::Cm,
                sheet_width_um: 508_000,
                sheet_height_um: 254_000,
                dpi: 240,
                bleed_um: 4_000,
                safety_um: 7_500,
            },
            structure: InitialStructureConfiguration {
                sheet_count: 3,
                first_sheet: InitialSheetFormat::SinglePage,
                last_sheet: InitialSheetFormat::Double,
            },
            visual_defaults: InitialVisualDefaults {
                background: InitialBackground::BothSides {
                    both: InitialBackgroundContent::Color {
                        rgb: "#FFFFFF".into(),
                    },
                },
                overlay: InitialOverlay::BothSides { both: None },
                frame_border: InitialFrameBorder::None,
            },
        };
        let request = new_request(
            authority(target.clone()),
            BootstrapIntent::CreateNew {
                configuration: Box::new(configuration.clone()),
                authorization: CreateWriteAuthorization::ReplaceConfirmed,
            },
        )
        .expect("valid create bootstrap fixture");

        assert_eq!(
            request.intent,
            BootstrapIntent::CreateNew {
                configuration: Box::new(configuration),
                authorization: CreateWriteAuthorization::ReplaceConfirmed,
            }
        );
        assert_eq!(request.authority.logical_target.as_path(), target);
    }

    #[test]
    fn spawn_failure_is_structured_without_falling_back_to_another_host() {
        let bootstrap = ProjectHostBootstrap::new(
            PathBuf::from(r"Z:\definitely-missing\myalbuns.exe"),
            Duration::from_secs(1),
        );
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Projeto.myalbuns");

        let error = bootstrap
            .open(authority(target))
            .expect_err("a missing executable cannot spawn");

        assert_eq!(error.kind, BootstrapFailureKind::HostUnavailable);
    }

    #[cfg(windows)]
    #[test]
    fn a_correlated_ready_detaches_the_still_running_host() {
        let request = fixture_request();
        let child = powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $terminal = @{
              state = 'ready'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
              projectId = 'c4495826-fdf6-43ac-bbf9-92f068e6a704'
              revision = 4
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            Start-Sleep -Seconds 2
            "#,
        );
        let spawned_pid = child.id();

        let ready = supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the correlated terminal is accepted");

        let BootstrapOutcome::Ready(ready) = ready else {
            panic!("the fixture must return Ready");
        };
        assert_eq!(ready.host_pid, spawned_pid);
        assert_eq!(ready.project_id, "c4495826-fdf6-43ac-bbf9-92f068e6a704");
        assert_eq!(ready.revision, 4);
        assert!(
            process_is_alive(spawned_pid),
            "dropping the global-side process handle must not terminate a Ready host"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_correlated_focus_terminal_reaps_the_ephemeral_host() {
        let request = fixture_request();
        let child = powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $terminal = @{
              state = 'focusExisting'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
              projectId = 'c4495826-fdf6-43ac-bbf9-92f068e6a704'
              ownerProcess = @{
                processId = 4242
                creationTime = 123
              }
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            [Threading.ManualResetEventSlim]::new($false).Wait()
            "#,
        );
        let spawned_pid = child.id();

        let outcome = supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the correlated focus terminal is accepted");

        let BootstrapOutcome::FocusExisting {
            project_id,
            owner_process,
        } = outcome
        else {
            panic!("the fixture must return FocusExisting");
        };
        assert_eq!(project_id, "c4495826-fdf6-43ac-bbf9-92f068e6a704");
        assert_eq!(
            owner_process,
            ProcessInstanceId::from_wire(4242, 123).expect("the owner process instance is valid")
        );
        assert!(
            !process_is_alive(spawned_pid),
            "the probing Host never survives as a duplicate Project process"
        );
    }

    #[cfg(windows)]
    #[test]
    fn save_copy_as_continues_in_the_same_pending_host_without_resending_the_source() {
        let request = fixture_request();
        let child = pending_external_copy_host();
        let spawned_pid = child.id();
        let pending = match supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the actionable terminal is accepted")
        {
            BootstrapOutcome::ExternalCopyNotWritable(pending) => pending,
            _ => panic!("the fixture must remain pending"),
        };
        assert!(
            process_is_alive(spawned_pid),
            "the source-owning Host remains alive while Global asks for a destination"
        );
        let destination = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cópia editável.myalbuns");

        let outcome = continue_external_copy(
            pending,
            authority(destination),
            CreateWriteAuthorization::CreateOnly,
        )
        .expect("the correlated continuation reaches Ready");
        let BootstrapOutcome::Ready(ready) = outcome else {
            panic!("the continuation must become Ready");
        };
        assert_eq!(ready.host_pid, spawned_pid);
        assert_eq!(ready.project_id, "8dfdb57a-918b-4280-9969-88b31b635f57");
        assert_eq!(ready.revision, 7);
    }

    #[cfg(windows)]
    #[test]
    fn cancelling_the_destination_reaps_the_pending_source_host() {
        let request = fixture_request();
        let child = pending_external_copy_host();
        let spawned_pid = child.id();
        let mut pending = match supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the actionable terminal is accepted")
        {
            BootstrapOutcome::ExternalCopyNotWritable(pending) => pending,
            _ => panic!("the fixture must remain pending"),
        };
        assert_eq!(pending.host_process_id(), spawned_pid);

        drop(pending);

        assert!(
            !process_is_alive(spawned_pid),
            "cancellation leaves no source Host or editable Sessão"
        );
    }

    #[cfg(windows)]
    #[test]
    fn recovery_decision_continues_in_the_exact_pending_host_before_ready() {
        let request = fixture_request();
        let expected_attempt_id = request.attempt_id.clone();
        let child = pending_recovery_host();
        let spawned_pid = child.id();
        let pending = match supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the correlated Recovery signal is accepted")
        {
            BootstrapOutcome::RecoveryAvailable(pending) => pending,
            _ => panic!("the fixture must wait for the Recovery decision"),
        };
        assert_eq!(pending.attempt_id(), expected_attempt_id);
        assert!(process_is_alive(spawned_pid));

        let outcome = pending
            .resolve(ProjectRecoveryDecision::ReopenAndRecover)
            .expect("the same Host reaches Ready after Recovery");
        let RecoveryContinuationOutcome::Ready(ready) = outcome else {
            panic!("Recovery must continue into the same ready Host")
        };
        assert_eq!(ready.host_pid, spawned_pid);
        assert_eq!(ready.project_id, "a230216b-197c-424e-b146-2a95fc7dfbf7");
        assert_eq!(ready.revision, 11);
        assert!(process_is_alive(spawned_pid));
    }

    #[cfg(windows)]
    #[test]
    fn deferring_or_abandoning_recovery_reaps_the_pending_host() {
        let request = fixture_request();
        let child = pending_recovery_host();
        let deferred_pid = child.id();
        let pending = match supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the correlated Recovery signal is accepted")
        {
            BootstrapOutcome::RecoveryAvailable(pending) => pending,
            _ => panic!("the fixture must wait for the Recovery decision"),
        };
        assert!(matches!(
            pending
                .resolve(ProjectRecoveryDecision::NowNot)
                .expect("Agora não is a closed Recovery terminal"),
            RecoveryContinuationOutcome::Deferred
        ));
        assert!(!process_is_alive(deferred_pid));

        let request = fixture_request();
        let child = pending_recovery_host();
        let abandoned_pid = child.id();
        let pending = match supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect("the correlated Recovery signal is accepted")
        {
            BootstrapOutcome::RecoveryAvailable(pending) => pending,
            _ => panic!("the fixture must wait for the Recovery decision"),
        };
        drop(pending);
        assert!(!process_is_alive(abandoned_pid));
    }

    #[cfg(windows)]
    #[test]
    fn a_correlation_mismatch_kills_the_spawned_process() {
        let request = fixture_request();
        let child = powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $terminal = @{
              state = 'ready'
              attemptId = $request.attemptId
              launchNonce = 'wrong-nonce'
              hostPid = $PID
              projectId = 'project'
              revision = 0
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            Start-Sleep -Seconds 10
            "#,
        );
        let spawned_pid = child.id();

        let error = supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect_err("a mismatched nonce is rejected");

        assert_eq!(error.kind, BootstrapFailureKind::CorrelationMismatch);
        assert!(!process_is_alive(spawned_pid));
    }

    #[cfg(windows)]
    #[test]
    fn waiting_for_a_host_is_bounded_and_reaping_collects_it() {
        let mut host = silent_host();
        let pid = host.id();
        let started = std::time::Instant::now();
        assert!(!wait_bounded(&mut host, Duration::from_millis(200)));
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(200));
        assert!(
            waited < Duration::from_secs(5),
            "the wait stopped at its bound"
        );
        assert!(process_is_alive(pid));

        reap(&mut host);
        assert!(!process_is_alive(pid));
        assert!(matches!(host.try_wait(), Ok(Some(_))));
    }

    #[cfg(windows)]
    #[test]
    fn timeout_and_invalid_json_kill_and_reap_the_spawned_process() {
        let request = fixture_request();
        let timeout_child = silent_host();
        let timeout_pid = timeout_child.id();

        let timeout = supervise_child(timeout_child, request.clone(), Duration::from_millis(150))
            .expect_err("silence until the deadline times out");
        assert_eq!(timeout.kind, BootstrapFailureKind::Timeout);
        assert!(!process_is_alive(timeout_pid));

        let invalid_child = powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $null = [Console]::In.ReadLine()
            [Console]::Out.WriteLine('{not-json')
            [Console]::Out.Flush()
            Start-Sleep -Seconds 10
            "#,
        );
        let invalid_pid = invalid_child.id();
        let invalid = supervise_child(invalid_child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect_err("invalid JSON is rejected");
        assert_eq!(invalid.kind, BootstrapFailureKind::InvalidTerminal);
        assert!(!process_is_alive(invalid_pid));
    }

    #[cfg(windows)]
    #[test]
    fn a_correlated_failed_terminal_preserves_stage_and_code_then_reaps_the_host() {
        let request = fixture_request();
        let child = powershell_host(
            r#"
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            $request = [Console]::In.ReadLine() | ConvertFrom-Json
            $terminal = @{
              state = 'failed'
              attemptId = $request.attemptId
              launchNonce = $request.launchNonce
              hostPid = $PID
              stage = 'open'
              code = 'projectInUse'
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            Start-Sleep -Seconds 10
            "#,
        );
        let spawned_pid = child.id();

        let error = supervise_child(child, request, FIXTURE_TERMINAL_TIMEOUT)
            .expect_err("a Failed terminal never releases its host");

        assert_eq!(error.kind, BootstrapFailureKind::HostFailed);
        assert_eq!(error.stage, Some(super::super::FailureStage::Open));
        assert_eq!(error.code, Some(super::super::FailureCode::ProjectInUse));
        assert!(!process_is_alive(spawned_pid));
    }
}
