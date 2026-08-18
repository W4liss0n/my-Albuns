use std::{
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use super::{
    BootstrapIntent, BootstrapRequest, CreateWriteAuthorization, HostTerminal,
    InitialProjectCreationConfiguration, SaveExternalCopyRequest, TargetAuthority,
    TerminalValidationError, ValidatedTerminal, validate_terminal,
};

const MAX_TERMINAL_BYTES: usize = 32 * 1024;
#[cfg(debug_assertions)]
const HOST_WEBVIEW_DEBUG_PORT_ENV: &str = "MYALBUNS_DEV_HOST_WEBVIEW_DEBUG_PORT";

#[derive(Clone, Debug)]
pub(crate) struct ProjectHostBootstrap {
    executable: PathBuf,
    terminal_timeout: Duration,
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
        owner_process_id: u32,
    },
    ExternalCopyNotWritable(PendingExternalCopyProcess),
}

#[derive(Debug)]
pub(crate) struct PendingExternalCopyProcess {
    child: PendingChild,
    stdin: ChildStdin,
    terminal_receiver: mpsc::Receiver<Result<HostTerminal, BootstrapFailure>>,
    request: BootstrapRequest,
    terminal_timeout: Duration,
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
        }
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
        supervise_child(child, request, self.terminal_timeout)
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
    if authority.root_bindings.validate().is_err()
        || !authority
            .root_bindings
            .covers(authority.logical_target.as_path())
    {
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
        let _ = child.kill();
        let _ = child.wait();
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

fn supervise_child(
    child: Child,
    request: BootstrapRequest,
    terminal_timeout: Duration,
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
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let first = read_terminal(&mut reader);
        let awaits_continuation = matches!(first, Ok(HostTerminal::ExternalCopyNotWritable { .. }));
        if terminal_sender.send(first).is_ok() && awaits_continuation {
            let _ = terminal_sender.send(read_terminal(&mut reader));
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
            owner_process_id,
        }) => Ok(BootstrapOutcome::FocusExisting {
            project_id,
            owner_process_id,
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
    if destination.root_bindings.validate().is_err()
        || !destination
            .root_bindings
            .covers(destination.logical_target.as_path())
    {
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
            owner_process_id,
        }) => Ok(BootstrapOutcome::FocusExisting {
            project_id,
            owner_process_id,
        }),
        Ok(ValidatedTerminal::ExternalCopyNotWritable { .. }) => Err(BootstrapFailure {
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

fn read_terminal(reader: &mut impl Read) -> Result<HostTerminal, BootstrapFailure> {
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
            let _ = child.kill();
            let _ = child.wait();
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

        let ready = supervise_child(child, request, Duration::from_secs(2))
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
              ownerProcessId = 4242
            }
            [Console]::Out.WriteLine(($terminal | ConvertTo-Json -Compress))
            [Console]::Out.Flush()
            [Threading.ManualResetEventSlim]::new($false).Wait()
            "#,
        );
        let spawned_pid = child.id();

        let outcome = supervise_child(child, request, Duration::from_secs(2))
            .expect("the correlated focus terminal is accepted");

        let BootstrapOutcome::FocusExisting {
            project_id,
            owner_process_id,
        } = outcome
        else {
            panic!("the fixture must return FocusExisting");
        };
        assert_eq!(project_id, "c4495826-fdf6-43ac-bbf9-92f068e6a704");
        assert_eq!(owner_process_id, 4242);
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
        let pending = match supervise_child(child, request, Duration::from_secs(2))
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
        let pending = match supervise_child(child, request, Duration::from_secs(2))
            .expect("the actionable terminal is accepted")
        {
            BootstrapOutcome::ExternalCopyNotWritable(pending) => pending,
            _ => panic!("the fixture must remain pending"),
        };

        drop(pending);

        assert!(
            !process_is_alive(spawned_pid),
            "cancellation leaves no source Host or editable Session"
        );
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

        let error = supervise_child(child, request, Duration::from_secs(2))
            .expect_err("a mismatched nonce is rejected");

        assert_eq!(error.kind, BootstrapFailureKind::CorrelationMismatch);
        assert!(!process_is_alive(spawned_pid));
    }

    #[cfg(windows)]
    #[test]
    fn timeout_and_invalid_json_kill_and_reap_the_spawned_process() {
        let request = fixture_request();
        let timeout_child = powershell_host(
            r#"
            $null = [Console]::In.ReadLine()
            Start-Sleep -Seconds 10
            "#,
        );
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
        let invalid = supervise_child(invalid_child, request, Duration::from_secs(2))
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

        let error = supervise_child(child, request, Duration::from_secs(2))
            .expect_err("a Failed terminal never releases its host");

        assert_eq!(error.kind, BootstrapFailureKind::HostFailed);
        assert_eq!(error.stage, Some(super::super::FailureStage::Open));
        assert_eq!(error.code, Some(super::super::FailureCode::ProjectInUse));
        assert!(!process_is_alive(spawned_pid));
    }
}
