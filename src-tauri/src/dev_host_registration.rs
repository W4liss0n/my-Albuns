use std::{
    env,
    io::{self, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use crate::dev_supervisor_protocol::{HOST_LEASE_ENDPOINT_ENV, HOST_LEASE_TOKEN_ENV};

const HOST_REGISTRATION_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn register_from_environment() -> io::Result<()> {
    let endpoint = env::var_os(HOST_LEASE_ENDPOINT_ENV);
    let token = env::var_os(HOST_LEASE_TOKEN_ENV);
    let (endpoint, token) = match (endpoint, token) {
        (None, None) => return Ok(()),
        (Some(endpoint), Some(token)) => (endpoint, token),
        _ => {
            return Err(io::Error::other(
                "o contrato do supervisor de desenvolvimento está incompleto",
            ));
        }
    };
    let endpoint = endpoint
        .to_str()
        .ok_or_else(|| io::Error::other("o endpoint do supervisor não é Unicode"))?
        .parse::<SocketAddr>()
        .map_err(|_| io::Error::other("o endpoint do supervisor não é válido"))?;
    let token = token
        .to_str()
        .ok_or_else(|| io::Error::other("o token do supervisor não é Unicode"))?;

    let process_id = std::process::id();
    let mut connection = TcpStream::connect_timeout(&endpoint, HOST_REGISTRATION_CONNECT_TIMEOUT)?;
    connection.set_nodelay(true)?;
    connection.write_all(format!("{token} {process_id}\n").as_bytes())?;
    connection.flush()?;
    eprintln!(r#"{{"event":"dev_host_registered","processId":{process_id}}}"#);
    Ok(())
}
