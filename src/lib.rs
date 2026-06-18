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

use tokio::net::TcpListener;
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
        let (stream, _) = listener.accept().await?;
        let remote_ip = stream.peer_addr()?.ip();

        // Drop connections that do not match the accept CIDR.
        if !cidr_is_trivial && !Helpers::is_ip_in_cidr(&remote_ip, &cidr)? {
            warn!("Request from {} does not match {}: dropping connection.", remote_ip, config.accept_cidr);
            drop(stream);
            continue;
        }

        Connection::from(stream, endpoint_ip.clone(), pool.lease(), config.read_timeout, credentials.clone()).handle();
    }
}
