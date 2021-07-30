#![warn(rust_2018_idioms)]
#![warn(clippy::all)]
//#![feature(test)]

mod connection;
mod handshake;
mod helpers;
mod request;
//mod custom_pump;
mod copy_pump;
mod buffer_pool;
mod config;

use tokio::{io::AsyncWriteExt, net::TcpListener};
use log::{info, debug, warn, LevelFilter};

use connection::Connection;
use helpers::Helpers;
use buffer_pool::BufferPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    // Compute config.

    let config_file: Option<&str> = if args.len() == 2 {
        Some(&args[1])
    } else {
        None
    };

    let config = config::from_file_and_env(config_file).await?;
    
    // Set the log level.
    simple_logger::init().unwrap();
    log::set_max_level(LevelFilter::Info);
    
    info!("Listen IP:    {}", config.listen_ip);
    info!("Endpoint IP:  {}", config.endpoint_ip);
    info!("Port:         {}", config.port);
    info!("Buffer Size:  {}", config.buffer_size);
    info!("Read Timeout: {}", config.read_timeout);
    info!("Accept CIDR:  {}", config.accept_cidr);

    // Calculate the CIDR prefix and mask.
    let cidr = Helpers::parse_cidr(&config.accept_cidr)?;
    let cidr_is_trivial = cidr.is_trivial();

    // Create a buffer pool (doubled so that each half of the connection achieves the desired size).
    let mut pool = BufferPool::new(2 * config.buffer_size);

    // Start the server.
    let listener = TcpListener::bind(format!("{}:{}", config.listen_ip, config.port)).await?;
    info!("Listening on tcp://{}:{} ... ", config.listen_ip, config.port);

    // Server loop.
    loop {
        debug!("Buffer pool: {} leased / {} total.", pool.leased_count(), pool.total_count());

        // Accept new connections.
        let (mut stream, _) = listener.accept().await?;
        let remote_ip = stream.peer_addr()?.ip();
        
        // Drop connections that do not match the accept CIDR.
        if !cidr_is_trivial && !Helpers::is_ip_in_cidr(&remote_ip, &cidr)? {
            warn!("Request from {} does not match {}: dropping connection.", remote_ip, config.accept_cidr);
            stream.shutdown().await.unwrap_or_default();
            continue;
        }
        
        Connection::from(stream, config.endpoint_ip.to_owned(), pool.lease(), config.read_timeout).handle();
    }
}