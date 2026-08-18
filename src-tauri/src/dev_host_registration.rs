use std::{
    env,
    ffi::OsStr,
    io::{self, BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command},
    time::Duration,
};

use crate::dev_supervisor_protocol::{
    AUTHORIZE_HOST_LEASE_REQUEST, HOST_LEASE_AUTHORITY_ENV, HOST_LEASE_AUTHORIZED_RESPONSE,
    HOST_LEASE_ENDPOINT_ENV, HOST_LEASE_REGISTERED_RESPONSE, REGISTER_HOST_LEASE_REQUEST,
};
use myalbuns_paths::ProcessInstanceId as HostProcessInstanceId;
use windows_sys::Win32::System::Threading::CREATE_BREAKAWAY_FROM_JOB;

const HOST_REGISTRATION_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HOST_LEASE_CREDENTIAL_ENV: &str = "MYALBUNS_DEV_HOST_LEASE_CREDENTIAL";

pub(crate) struct PendingHostLeaseAuthorization {
    endpoint: SocketAddr,
    authority: String,
    credential: String,
}

impl PendingHostLeaseAuthorization {
    pub(crate) fn authorize_spawned_host(&self, child: &Child) -> io::Result<()> {
        let process_instance =
            HostProcessInstanceId::from_process_handle(child.id(), child.as_raw_handle().cast())?;
        let response = exchange(
            self.endpoint,
            &format!(
                "{AUTHORIZE_HOST_LEASE_REQUEST} {} {} {} {}\n",
                self.authority,
                self.credential,
                process_instance.process_id(),
                process_instance.creation_time_wire(),
            ),
        )?;
        if response != HOST_LEASE_AUTHORIZED_RESPONSE {
            return Err(io::Error::other(
                "the supervisor rejected the Host registration credential",
            ));
        }
        Ok(())
    }
}

pub(crate) fn prepare_host_command(
    command: &mut Command,
    launch_nonce: &str,
) -> io::Result<Option<PendingHostLeaseAuthorization>> {
    let endpoint = env::var_os(HOST_LEASE_ENDPOINT_ENV);
    let authority = env::var_os(HOST_LEASE_AUTHORITY_ENV);
    let (endpoint, authority) = match (endpoint, authority) {
        (None, None) => {
            command.env_remove(HOST_LEASE_CREDENTIAL_ENV);
            return Ok(None);
        }
        (Some(endpoint), Some(authority)) => (endpoint, authority),
        _ => {
            return Err(io::Error::other(
                "the development supervisor contract is incomplete",
            ));
        }
    };
    let endpoint = parse_endpoint(&endpoint)?;
    let authority = authority
        .to_str()
        .ok_or_else(|| io::Error::other("the supervisor authority is not Unicode"))?;
    validate_launch_nonce(launch_nonce)?;

    command
        .creation_flags(CREATE_BREAKAWAY_FROM_JOB)
        .env(HOST_LEASE_ENDPOINT_ENV, endpoint.to_string())
        .env_remove(HOST_LEASE_AUTHORITY_ENV)
        .env(HOST_LEASE_CREDENTIAL_ENV, launch_nonce);
    Ok(Some(PendingHostLeaseAuthorization {
        endpoint,
        authority: authority.to_owned(),
        credential: launch_nonce.to_owned(),
    }))
}

pub(crate) fn register_from_environment(launch_nonce: &str) -> io::Result<()> {
    let endpoint = env::var_os(HOST_LEASE_ENDPOINT_ENV);
    let credential = env::var_os(HOST_LEASE_CREDENTIAL_ENV);
    let (endpoint, credential) = match (endpoint, credential) {
        (None, None) => return Ok(()),
        (Some(endpoint), Some(credential)) => (endpoint, credential),
        _ => {
            return Err(io::Error::other(
                "the development Host registration contract is incomplete",
            ));
        }
    };
    let endpoint = parse_endpoint(&endpoint)?;
    let credential = credential
        .to_str()
        .ok_or_else(|| io::Error::other("the Host credential is not Unicode"))?;
    validate_launch_nonce(credential)?;
    if credential != launch_nonce {
        return Err(io::Error::other(
            "the Host credential does not match the bootstrap nonce",
        ));
    }

    let process_id = std::process::id();
    let response = exchange(
        endpoint,
        &format!("{REGISTER_HOST_LEASE_REQUEST} {credential} {process_id}\n"),
    )?;
    if response != HOST_LEASE_REGISTERED_RESPONSE {
        return Err(io::Error::other(
            "the supervisor rejected the Host registration",
        ));
    }
    eprintln!(r#"{{"event":"dev_host_registered","processId":{process_id}}}"#);
    Ok(())
}

fn parse_endpoint(value: &OsStr) -> io::Result<SocketAddr> {
    value
        .to_str()
        .ok_or_else(|| io::Error::other("the supervisor endpoint is not Unicode"))?
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::other("the supervisor endpoint is invalid"))
}

fn validate_launch_nonce(value: &str) -> io::Result<()> {
    uuid::Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| io::Error::other("the Host registration nonce is invalid"))
}

fn exchange(endpoint: SocketAddr, request: &str) -> io::Result<String> {
    let mut connection = TcpStream::connect_timeout(&endpoint, HOST_REGISTRATION_CONNECT_TIMEOUT)?;
    connection.set_nodelay(true)?;
    connection.set_read_timeout(Some(HOST_REGISTRATION_CONNECT_TIMEOUT))?;
    connection.set_write_timeout(Some(HOST_REGISTRATION_CONNECT_TIMEOUT))?;
    connection.write_all(request.as_bytes())?;
    connection.flush()?;

    let mut response = String::new();
    BufReader::new(connection).read_line(&mut response)?;
    Ok(response)
}
