use std::{
    env,
    io::{self, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use crate::dev_supervisor_protocol::{HOST_LEASE_ENDPOINT_ENV, HOST_LEASE_TOKEN_ENV};

const HOST_LEASE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct DevHostLease {
    _connection: TcpStream,
}

impl DevHostLease {
    pub(crate) fn connect_from_environment() -> io::Result<Option<Self>> {
        let endpoint = env::var_os(HOST_LEASE_ENDPOINT_ENV);
        let token = env::var_os(HOST_LEASE_TOKEN_ENV);
        let (endpoint, token) = match (endpoint, token) {
            (None, None) => return Ok(None),
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

        let mut connection = TcpStream::connect_timeout(&endpoint, HOST_LEASE_CONNECT_TIMEOUT)?;
        connection.set_nodelay(true)?;
        connection.write_all(format!("{token} {}\n", std::process::id()).as_bytes())?;
        connection.flush()?;
        eprintln!(
            "{{\"event\":\"dev_host_lease_acquired\",\"processId\":{}}}",
            std::process::id()
        );
        Ok(Some(Self {
            _connection: connection,
        }))
    }
}

impl Drop for DevHostLease {
    fn drop(&mut self) {
        eprintln!(
            "{{\"event\":\"dev_host_lease_released\",\"processId\":{}}}",
            std::process::id()
        );
    }
}
