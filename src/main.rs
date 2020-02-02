#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

mod connection;
mod handshake;
mod helpers;
mod request;
//mod custom_pump;
mod copy_pump;
mod buffer_pool;

use tokio::net::TcpListener;
use std::net::Shutdown;
use toml::from_str;
use serde::Deserialize;
use log::{info, debug, warn, LevelFilter};
use simple_logger;

use connection::Connection;
use helpers::Helpers;
use buffer_pool::BufferPool;

#[derive(Deserialize)]
struct Config {
    listen_interface: Option<String>,
    endpoint_interface: Option<String>,
    port: Option<u16>,
    buffer_size: Option<usize>,
    read_timeout: Option<u64>,
    accept_cidr: Option<String>
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    // Compute options.

    let config: Option<Config> = if args.len() == 2 {
        let config_file = args[1].to_owned();
        let config_file_data = tokio::fs::read(config_file).await?;
        let config_text = std::str::from_utf8(&config_file_data)?;

        Some(from_str::<Config>(config_text)?)
    } else {
        None
    };

    let mut listen_interface: Option<String> = None;
    let mut endpoint_interface: Option<String> = None;
    let mut port = 1080u16;
    let mut buffer_size = 2048usize;
    let mut read_timeout = 5000u64;
    let mut accept_cidr = "0.0.0.0/0".to_owned();
    
    if let Some(c) = config {
        listen_interface = c.listen_interface;
        endpoint_interface = c.endpoint_interface;
        port = c.port.unwrap_or(port);
        buffer_size = c.buffer_size.unwrap_or(buffer_size);
        read_timeout = c.read_timeout.unwrap_or(read_timeout);
        accept_cidr = c.accept_cidr.unwrap_or(accept_cidr);
    }

    let listen_ip = match &listen_interface {
        Some(i) => Helpers::get_interface_ip(i)?.to_string(),
        None => "0.0.0.0".to_owned()
    };

    let endpoint_ip = match &endpoint_interface {
        Some(i) => Helpers::get_interface_ip(i)?.to_string(),
        None => "0.0.0.0".to_owned()
    };

    // Set the log level.
    simple_logger::init().unwrap();
    log::set_max_level(LevelFilter::Info);
    
    info!("Listen IP:    {}", listen_ip);
    info!("Endpoint IP:  {}", endpoint_ip);
    info!("Port:         {}", port);
    info!("Buffer Size:  {}", buffer_size);
    info!("Read Timeout: {}", read_timeout);
    info!("Accept CIDR:  {}", accept_cidr);

    // Calculate the CIDR prefix and mask.
    let cidr = Helpers::parse_cidr(&accept_cidr)?;
    let cidr_is_trivial = cidr.is_trivial();

    // Create a buffer pool (doubled so that each half of the connection achieves the desired size).
    let mut pool = BufferPool::new(2 * buffer_size);

    // Start the server.
    let mut listener = TcpListener::bind(format!("{}:{}", listen_ip, port)).await?;
    info!("Listening on tcp://{}:{} ... ", listen_ip, port);

    // Server loop.
    loop {
        debug!("Buffer pool: {} leased / {} total.", pool.leased_count(), pool.total_count());

        // Accept new connections.
        let (stream, _) = listener.accept().await?;
        let remote_ip = stream.peer_addr()?.ip();
        
        // Drop connections that do not match the accept CIDR.
        if !cidr_is_trivial && !Helpers::is_ip_in_cidr(&remote_ip, &cidr)? {
            warn!("Request from {} does not match {}: dropping connection.", remote_ip, accept_cidr);
            stream.shutdown(Shutdown::Both).unwrap_or(());
            continue;
        }
        
        Connection::from(stream, endpoint_ip.to_owned(), pool.lease(), read_timeout).handle();
    }
}