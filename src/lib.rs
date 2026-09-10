#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod auth;
pub mod buffer_pool;
pub mod config;
pub mod connection;
pub mod copy_pump;
pub mod handshake;
pub mod helpers;
pub mod request;

use std::io::{Error, ErrorKind};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::buffer_pool::BufferPool;
use crate::config::Config;
use crate::connection::Connection;
use crate::helpers::{Helpers, Res};

/// Resolve the listen address from `config`, bind it, log the configuration, and serve forever.
pub async fn run(config: Config) -> Res<()> {
    let listen_ip = config.listen_ip()?;

    // Validate the auth configuration up front so a half-configured proxy fails before binding.
    let auth_enabled = config.credentials()?.is_some();

    info!("Version:      {}", env!("CARGO_PKG_VERSION"));
    info!("Listen IP:    {}", listen_ip);
    info!("Endpoint IP:  {}", config.endpoint_ip()?);
    info!("Port:         {}", config.port);
    info!("Buffer Size:  {}", config.buffer_size);
    info!("Read Timeout: {}", config.read_timeout);
    info!("Accept CIDR:  {}", config.accept_cidr);
    info!("Auth:         {}", if auth_enabled { "user/pass" } else { "none" });

    let listener = TcpListener::bind(format!("{}:{}", listen_ip, config.port)).await?;
    info!("Listening on tcp://{}:{} ...", listen_ip, config.port);

    serve(listener, config).await
}

/// Serve SOCKS5 connections on an already-bound `listener`. Split out from [`run`] so tests can
/// drive the proxy against an ephemeral loopback port.
pub async fn serve(listener: TcpListener, config: Config) -> Res<()> {
    let endpoint_ip = config.endpoint_ip()?;
    let cidr = Helpers::parse_cidr(&config.accept_cidr)?;
    let cidr_is_trivial = cidr.is_trivial();
    let credentials = config.credentials()?;

    // Create a buffer pool (doubled so that each half of the connection achieves the desired size).
    let mut pool = BufferPool::new(2 * config.buffer_size);

    loop {
        // Nothing that goes wrong with a single connection may take the listener down. Descriptor
        // exhaustion is the case that used to kill the proxy outright: `accept` returns `EMFILE`,
        // and propagating it ended `serve`.
        let (stream, _) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(error) => {
                let backoff = accept_backoff(&error);
                warn!("Accept failed ({}); retrying in {}ms.", error, backoff.as_millis());
                sleep(backoff).await;
                continue;
            }
        };

        // A peer can vanish between `accept` and `peer_addr`, which is not fatal either.
        let remote_ip = match stream.peer_addr() {
            Ok(address) => address.ip(),
            Err(error) => {
                warn!("Could not read peer address ({}): dropping connection.", error);
                continue;
            }
        };

        // Drop connections that do not match the accept CIDR.
        match Helpers::is_ip_in_cidr(&remote_ip, &cidr) {
            Ok(true) => {}
            Ok(false) if cidr_is_trivial => {}
            Ok(false) => {
                warn!("Request from {} does not match {}: dropping connection.", remote_ip, config.accept_cidr);
                drop(stream);
                continue;
            }
            Err(error) => {
                warn!("Could not match {} against {} ({}): dropping connection.", remote_ip, config.accept_cidr, error);
                drop(stream);
                continue;
            }
        }

        Connection::from(stream, endpoint_ip.clone(), pool.lease(), config.read_timeout, credentials.clone()).handle();
    }
}

/// Pause before the next `accept` after `error`.
///
/// Errors that concern only the connection being accepted are safe to retry immediately: that
/// connection is already gone and the listener is healthy. Everything else is treated as a resource
/// problem, and those need a pause. Descriptor exhaustion in particular leaves the pending
/// connection sitting in the accept queue, so retrying with no delay fails on the same connection
/// and spins the loop at full speed.
fn accept_backoff(error: &Error) -> Duration {
    if is_connection_error(error) { Duration::ZERO } else { ACCEPT_ERROR_BACKOFF }
}

/// Whether `error` concerns only the connection being accepted, rather than the listener.
fn is_connection_error(error: &Error) -> bool {
    matches!(error.kind(), ErrorKind::ConnectionRefused | ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset)
}

/// How long to wait before accepting again after a resource error, such as running out of file
/// descriptors.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn connection_errors_retry_immediately() {
        for kind in [ErrorKind::ConnectionRefused, ErrorKind::ConnectionAborted, ErrorKind::ConnectionReset] {
            assert_eq!(accept_backoff(&Error::new(kind, "peer went away")), Duration::ZERO);
        }
    }

    #[test]
    fn resource_errors_back_off() {
        // EMFILE, the case that used to end `serve`.
        let emfile = Error::from_raw_os_error(24);
        assert_eq!(accept_backoff(&emfile), ACCEPT_ERROR_BACKOFF);

        assert_eq!(accept_backoff(&Error::other("unknown")), ACCEPT_ERROR_BACKOFF);
    }
}
