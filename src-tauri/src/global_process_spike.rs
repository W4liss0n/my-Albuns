//! Transporte descartável do spike para observar o processo global real.
//!
//! Este módulo não define o transporte normativo do produto. O socket TCP
//! loopback existe somente para medir singleton, disponibilidade e queda do
//! processo global durante a comparação de topologias.

use std::{
    ffi::OsStr,
    fmt,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use myalbuns_logging::{
    ProcessRole, init_local_logging, safe_log_identifier, sidecar_log_directory,
};
use myalbuns_paths::AppPaths;
use serde::{Deserialize, Serialize};
use tauri::{WebviewUrl, WebviewWindowBuilder};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, SetPriorityClass,
};

use crate::topology_spike::TOPOLOGY_ENV;

pub(crate) const PROCESS_ROLE_ENV: &str = "MYALBUNS_PROCESS_ROLE";
pub(crate) const GLOBAL_ENDPOINT_ENV: &str = "MYALBUNS_GLOBAL_SPIKE_ENDPOINT";
pub(crate) const GLOBAL_RUN_ID_ENV: &str = "MYALBUNS_TOPOLOGY_RUN_ID";
pub(crate) const GLOBAL_PROCESS_ROLE: &str = "global";
pub(crate) const GLOBAL_SINGLETON_REJECTED_EXIT_CODE: i32 = 73;

const GLOBAL_PROCESS_FAILED_EXIT_CODE: i32 = 74;
const MAX_WIRE_MESSAGE_BYTES: usize = 4 * 1024;
const SERVER_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalProcessSpikeConfig {
    endpoint: SocketAddr,
    run_id: String,
    topology: String,
}

impl GlobalProcessSpikeConfig {
    pub(crate) fn new(
        endpoint: SocketAddr,
        run_id: impl Into<String>,
        topology: impl Into<String>,
    ) -> Result<Self, GlobalProcessSpikeError> {
        let run_id = run_id.into();
        let topology = topology.into();
        validate_endpoint(endpoint).map_err(GlobalProcessSpikeError::InvalidConfiguration)?;
        validate_identifier("run id", &run_id)
            .map_err(GlobalProcessSpikeError::InvalidConfiguration)?;
        validate_topology(&topology).map_err(GlobalProcessSpikeError::InvalidConfiguration)?;
        Ok(Self {
            endpoint,
            run_id,
            topology,
        })
    }

    pub(crate) fn from_environment() -> Result<Self, GlobalProcessSpikeError> {
        let endpoint = required_environment(GLOBAL_ENDPOINT_ENV)?
            .parse::<SocketAddr>()
            .map_err(|_| {
                GlobalProcessSpikeError::InvalidConfiguration(format!(
                    "{GLOBAL_ENDPOINT_ENV} deve conter um endpoint TCP válido."
                ))
            })?;
        Self::new(
            endpoint,
            required_environment(GLOBAL_RUN_ID_ENV)?,
            required_environment(TOPOLOGY_ENV)?,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum GlobalProcessSpikeError {
    InvalidConfiguration(String),
    SingletonRejected {
        endpoint: SocketAddr,
    },
    BindFailed {
        endpoint: SocketAddr,
        reason: String,
    },
    Io(String),
    Protocol(String),
}

impl GlobalProcessSpikeError {
    pub(crate) const fn exit_code(&self) -> i32 {
        match self {
            Self::SingletonRejected { .. } => GLOBAL_SINGLETON_REJECTED_EXIT_CODE,
            _ => GLOBAL_PROCESS_FAILED_EXIT_CODE,
        }
    }
}

impl fmt::Display for GlobalProcessSpikeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(reason) => write!(formatter, "{reason}"),
            Self::SingletonRejected { endpoint } => {
                write!(
                    formatter,
                    "já existe um processo global do spike em {endpoint}"
                )
            }
            Self::BindFailed { endpoint, reason } => {
                write!(
                    formatter,
                    "não foi possível reservar o endpoint {endpoint}: {reason}"
                )
            }
            Self::Io(reason) => write!(formatter, "falha de I/O no processo global: {reason}"),
            Self::Protocol(reason) => {
                write!(
                    formatter,
                    "mensagem inválida no processo global do spike: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for GlobalProcessSpikeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalStatusResponse {
    pub(crate) process_id: u32,
    pub(crate) run_id: String,
    pub(crate) topology: String,
    pub(crate) probe_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GlobalStatusErrorCode {
    RunIdMismatch,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum GlobalStatusProbeError {
    InvalidRequest(String),
    Unavailable(String),
    InvalidResponse(String),
    Rejected(GlobalStatusErrorCode),
}

impl fmt::Display for GlobalStatusProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(reason) => write!(formatter, "probe global inválido: {reason}"),
            Self::Unavailable(reason) => {
                write!(formatter, "processo global indisponível: {reason}")
            }
            Self::InvalidResponse(reason) => {
                write!(formatter, "resposta global inválida: {reason}")
            }
            Self::Rejected(code) => write!(formatter, "probe global rejeitado: {code:?}"),
        }
    }
}

impl std::error::Error for GlobalStatusProbeError {}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum GlobalSpikeRequest {
    Status {
        #[serde(rename = "runId")]
        run_id: String,
        #[serde(rename = "probeId")]
        probe_id: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum GlobalSpikeWireResponse {
    Status {
        #[serde(rename = "processId")]
        process_id: u32,
        #[serde(rename = "runId")]
        run_id: String,
        topology: String,
        #[serde(rename = "probeId")]
        probe_id: String,
    },
    Error {
        code: GlobalStatusErrorCode,
        #[serde(rename = "probeId")]
        probe_id: String,
    },
}

#[derive(Debug)]
pub(crate) struct GlobalProcessSpike {
    listener: TcpListener,
    config: GlobalProcessSpikeConfig,
}

impl GlobalProcessSpike {
    #[cfg(test)]
    pub(crate) fn serve_one(&self) -> Result<(), GlobalProcessSpikeError> {
        let (stream, _) = self
            .listener
            .accept()
            .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))?;
        self.handle_stream(stream)
    }

    pub(crate) fn run(self) -> Result<(), GlobalProcessSpikeError> {
        tracing::info!(
            target: "myalbuns.global",
            process_role = ProcessRole::GlobalShell.as_str(),
            process_id = std::process::id(),
            run_id = self.config.run_id.as_str(),
            topology = self.config.topology.as_str(),
            endpoint = %self.config.endpoint,
            event = "start",
        );

        loop {
            let (stream, _) = self
                .listener
                .accept()
                .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))?;
            if let Err(error) = self.handle_stream(stream) {
                tracing::warn!(
                    target: "myalbuns.global",
                    process_role = ProcessRole::GlobalShell.as_str(),
                    process_id = std::process::id(),
                    run_id = self.config.run_id.as_str(),
                    topology = self.config.topology.as_str(),
                    endpoint = %self.config.endpoint,
                    reason = %error,
                    event = "status_request_rejected",
                );
            }
        }
    }

    fn handle_stream(&self, mut stream: TcpStream) -> Result<(), GlobalProcessSpikeError> {
        stream
            .set_read_timeout(Some(SERVER_IO_TIMEOUT))
            .and_then(|_| stream.set_write_timeout(Some(SERVER_IO_TIMEOUT)))
            .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))?;
        let request: GlobalSpikeRequest =
            read_wire_message(&mut stream).map_err(GlobalProcessSpikeError::Protocol)?;

        let GlobalSpikeRequest::Status { run_id, probe_id } = request;
        validate_identifier("probe id", &probe_id).map_err(GlobalProcessSpikeError::Protocol)?;
        let response = if run_id == self.config.run_id {
            GlobalSpikeWireResponse::Status {
                process_id: std::process::id(),
                run_id: self.config.run_id.clone(),
                topology: self.config.topology.clone(),
                probe_id: probe_id.clone(),
            }
        } else {
            GlobalSpikeWireResponse::Error {
                code: GlobalStatusErrorCode::RunIdMismatch,
                probe_id: probe_id.clone(),
            }
        };
        write_wire_message(&mut stream, &response)
            .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))?;

        tracing::info!(
            target: "myalbuns.global",
            process_role = ProcessRole::GlobalShell.as_str(),
            process_id = std::process::id(),
            run_id = self.config.run_id.as_str(),
            topology = self.config.topology.as_str(),
            endpoint = %self.config.endpoint,
            probe_id,
            event = "status",
        );
        Ok(())
    }
}

pub(crate) fn bind_global_process_spike(
    config: GlobalProcessSpikeConfig,
) -> Result<GlobalProcessSpike, GlobalProcessSpikeError> {
    match TcpListener::bind(config.endpoint) {
        Ok(listener) => Ok(GlobalProcessSpike { listener, config }),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::warn!(
                target: "myalbuns.global",
                process_role = ProcessRole::GlobalShell.as_str(),
                process_id = std::process::id(),
                run_id = config.run_id.as_str(),
                topology = config.topology.as_str(),
                endpoint = %config.endpoint,
                event = "singleton_rejected",
            );
            Err(GlobalProcessSpikeError::SingletonRejected {
                endpoint: config.endpoint,
            })
        }
        Err(error) => Err(GlobalProcessSpikeError::BindFailed {
            endpoint: config.endpoint,
            reason: error.to_string(),
        }),
    }
}

pub(crate) fn probe_global_status(
    endpoint: SocketAddr,
    run_id: &str,
    probe_id: &str,
    timeout: Duration,
) -> Result<GlobalStatusResponse, GlobalStatusProbeError> {
    validate_endpoint(endpoint).map_err(GlobalStatusProbeError::InvalidRequest)?;
    validate_identifier("run id", run_id).map_err(GlobalStatusProbeError::InvalidRequest)?;
    validate_identifier("probe id", probe_id).map_err(GlobalStatusProbeError::InvalidRequest)?;
    if timeout.is_zero() {
        return Err(GlobalStatusProbeError::InvalidRequest(
            "o timeout precisa ser positivo.".into(),
        ));
    }

    let mut stream = TcpStream::connect_timeout(&endpoint, timeout)
        .map_err(|error| GlobalStatusProbeError::Unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|_| stream.set_write_timeout(Some(timeout)))
        .map_err(|error| GlobalStatusProbeError::Unavailable(error.to_string()))?;
    write_wire_message(
        &mut stream,
        &GlobalSpikeRequest::Status {
            run_id: run_id.into(),
            probe_id: probe_id.into(),
        },
    )
    .map_err(|error| GlobalStatusProbeError::Unavailable(error.to_string()))?;

    let response: GlobalSpikeWireResponse =
        read_wire_message(&mut stream).map_err(GlobalStatusProbeError::InvalidResponse)?;
    match response {
        GlobalSpikeWireResponse::Status {
            process_id,
            run_id: response_run_id,
            topology,
            probe_id: response_probe_id,
        } if process_id > 0
            && response_run_id == run_id
            && response_probe_id == probe_id
            && validate_topology(&topology).is_ok() =>
        {
            Ok(GlobalStatusResponse {
                process_id,
                run_id: response_run_id,
                topology,
                probe_id: response_probe_id,
            })
        }
        GlobalSpikeWireResponse::Status { .. } => Err(GlobalStatusProbeError::InvalidResponse(
            "a resposta não corresponde ao probe solicitado.".into(),
        )),
        GlobalSpikeWireResponse::Error {
            code,
            probe_id: response_probe_id,
        } if response_probe_id == probe_id => Err(GlobalStatusProbeError::Rejected(code)),
        GlobalSpikeWireResponse::Error { .. } => Err(GlobalStatusProbeError::InvalidResponse(
            "a rejeição não corresponde ao probe solicitado.".into(),
        )),
    }
}

pub(crate) fn global_process_requested() -> bool {
    is_global_process_role(std::env::var_os(PROCESS_ROLE_ENV).as_deref())
}

pub(crate) fn run_global_process_spike_from_environment() -> Result<(), GlobalProcessSpikeError> {
    let config = GlobalProcessSpikeConfig::from_environment()?;
    let app_paths = AppPaths::discover()
        .map_err(|error| GlobalProcessSpikeError::InvalidConfiguration(error.to_string()))?;
    let log_directory = sidecar_log_directory(&app_paths);
    let _logging_guard = match init_local_logging(&log_directory, ProcessRole::GlobalShell) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("logging do processo global indisponível: {error}");
            None
        }
    };
    let server = bind_global_process_spike(config)?;
    if !global_welcome_window_visible() {
        configure_headless_process_priority()?;
        return server.run();
    }

    let webview_data_directory = app_paths
        .webview_data_directory("global-shell")
        .map_err(|error| GlobalProcessSpikeError::InvalidConfiguration(error.to_string()))?;
    std::thread::Builder::new()
        .name("global-status-spike".into())
        .spawn(move || {
            if let Err(error) = server.run() {
                tracing::error!(
                    target: "myalbuns.global",
                    process_role = ProcessRole::GlobalShell.as_str(),
                    reason = %error,
                    event = "status_server_failed",
                );
            }
        })
        .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))?;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "global", WebviewUrl::App("global.html".into()))
                .title("MyAlbuns")
                .inner_size(980.0, 680.0)
                .min_inner_size(720.0, 520.0)
                .data_directory(webview_data_directory)
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![crate::logging::frontend_log])
        .run(tauri::generate_context!())
        .map_err(|error| GlobalProcessSpikeError::Io(error.to_string()))
}

fn global_welcome_window_visible() -> bool {
    std::env::var_os("MYALBUNS_GLOBAL_SPIKE_WELCOME_VISIBLE").is_some()
}

#[cfg(windows)]
fn configure_headless_process_priority() -> Result<(), GlobalProcessSpikeError> {
    let configured = unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) };
    if configured == 0 {
        let error = std::io::Error::last_os_error();
        tracing::error!(
            target: "myalbuns.global",
            process_role = ProcessRole::GlobalShell.as_str(),
            process_id = std::process::id(),
            reason = %error,
            event = "process_priority_configuration_failed",
        );
        return Err(GlobalProcessSpikeError::Io(format!(
            "não foi possível priorizar os hosts interativos: {}",
            error
        )));
    }
    tracing::info!(
        target: "myalbuns.global",
        process_role = ProcessRole::GlobalShell.as_str(),
        process_id = std::process::id(),
        priority = "below_normal",
        event = "process_priority_configured",
    );
    Ok(())
}

#[cfg(not(windows))]
fn configure_headless_process_priority() -> Result<(), GlobalProcessSpikeError> {
    Ok(())
}

fn is_global_process_role(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new(GLOBAL_PROCESS_ROLE))
}

fn required_environment(name: &str) -> Result<String, GlobalProcessSpikeError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GlobalProcessSpikeError::InvalidConfiguration(format!(
                "{name} é obrigatório para o processo global do spike."
            ))
        })
}

fn validate_endpoint(endpoint: SocketAddr) -> Result<(), String> {
    if endpoint.ip().is_loopback() && endpoint.port() != 0 {
        Ok(())
    } else {
        Err("o endpoint global do spike deve usar loopback e porta não zero.".into())
    }
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if safe_log_identifier(value).is_some() {
        Ok(())
    } else {
        Err(format!("{label} inválido."))
    }
}

fn validate_topology(topology: &str) -> Result<(), String> {
    match topology {
        "independent" | "multiwindow" => Ok(()),
        _ => Err("a topologia deve ser independent ou multiwindow.".into()),
    }
}

fn write_wire_message<T: Serialize>(
    stream: &mut TcpStream,
    message: &T,
) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *stream, message).map_err(std::io::Error::other)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn read_wire_message<T: for<'de> Deserialize<'de>>(stream: &mut TcpStream) -> Result<T, String> {
    let reader = BufReader::new(stream);
    let mut limited = reader.take((MAX_WIRE_MESSAGE_BYTES + 1) as u64);
    let mut payload = Vec::new();
    limited
        .read_until(b'\n', &mut payload)
        .map_err(|error| error.to_string())?;
    if payload.is_empty() || payload.len() > MAX_WIRE_MESSAGE_BYTES || !payload.ends_with(b"\n") {
        return Err("a mensagem deve ser uma linha JSON limitada.".into());
    }
    serde_json::from_slice(&payload).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        net::{Ipv4Addr, SocketAddrV4, TcpListener},
        thread,
        time::Duration,
    };

    use super::{
        GLOBAL_SINGLETON_REJECTED_EXIT_CODE, GlobalProcessSpikeConfig, GlobalProcessSpikeError,
        GlobalStatusProbeError, GlobalStatusResponse, bind_global_process_spike,
        is_global_process_role, probe_global_status,
    };

    fn available_loopback_endpoint() -> std::net::SocketAddr {
        let reservation = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .expect("the OS assigns a loopback test endpoint");
        let endpoint = reservation
            .local_addr()
            .expect("the assigned endpoint is observable");
        drop(reservation);
        endpoint
    }

    fn config(endpoint: std::net::SocketAddr) -> GlobalProcessSpikeConfig {
        GlobalProcessSpikeConfig::new(endpoint, "run-001", "independent")
            .expect("the test configuration is valid")
    }

    #[test]
    fn singleton_bind_rejects_a_second_instance_and_releases_after_drop() {
        let endpoint = available_loopback_endpoint();
        let first = bind_global_process_spike(config(endpoint))
            .expect("the first global spike instance owns the endpoint");

        let rejection = bind_global_process_spike(config(endpoint))
            .expect_err("a second global spike instance is rejected");
        assert_eq!(
            rejection,
            GlobalProcessSpikeError::SingletonRejected { endpoint }
        );
        assert_eq!(rejection.exit_code(), GLOBAL_SINGLETON_REJECTED_EXIT_CODE);

        drop(first);
        bind_global_process_spike(config(endpoint))
            .expect("dropping the owner releases the singleton endpoint");
    }

    #[test]
    fn status_round_trip_is_typed_and_correlated() {
        let endpoint = available_loopback_endpoint();
        let server = bind_global_process_spike(config(endpoint))
            .expect("the global spike owns its singleton endpoint");
        let serving = thread::spawn(move || {
            server
                .serve_one()
                .expect("the global spike serves one valid status request");
        });

        let response =
            probe_global_status(endpoint, "run-001", "probe-007", Duration::from_secs(2))
                .expect("the host receives a typed global status");

        assert_eq!(
            response,
            GlobalStatusResponse {
                process_id: std::process::id(),
                run_id: "run-001".into(),
                topology: "independent".into(),
                probe_id: "probe-007".into(),
            }
        );
        serving
            .join()
            .expect("the status server terminates cleanly");
    }

    #[test]
    fn status_probe_rejects_a_response_from_another_run() {
        let endpoint = available_loopback_endpoint();
        let server = bind_global_process_spike(config(endpoint))
            .expect("the global spike owns its singleton endpoint");
        let serving = thread::spawn(move || {
            server
                .serve_one()
                .expect("the global spike answers the mismatched status request");
        });

        assert_eq!(
            probe_global_status(endpoint, "run-002", "probe-008", Duration::from_secs(2)),
            Err(GlobalStatusProbeError::Rejected(
                super::GlobalStatusErrorCode::RunIdMismatch
            ))
        );
        serving
            .join()
            .expect("the status server terminates cleanly");
    }

    #[test]
    fn only_the_exact_global_role_selects_the_lightweight_process() {
        assert!(is_global_process_role(Some(OsStr::new("global"))));
        assert!(!is_global_process_role(None));
        assert!(!is_global_process_role(Some(OsStr::new("desktop"))));
        assert!(!is_global_process_role(Some(OsStr::new("GLOBAL"))));
    }

    #[test]
    fn configuration_rejects_non_loopback_endpoints_and_unknown_topologies() {
        assert!(
            GlobalProcessSpikeConfig::new(
                SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 31_337).into(),
                "run-001",
                "independent",
            )
            .is_err()
        );
        assert!(
            GlobalProcessSpikeConfig::new(
                SocketAddrV4::new(Ipv4Addr::LOCALHOST, 31_337).into(),
                "run-001",
                "future-topology",
            )
            .is_err()
        );
    }
}
