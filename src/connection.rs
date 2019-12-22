use tokio::task::JoinHandle;
use tokio::net::{TcpStream};
use tokio::prelude::*;

use std::net::Shutdown;
use std::iter::IntoIterator;
use std::str::FromStr;
use std::net::{SocketAddr, IpAddr, ToSocketAddrs};
use net2::TcpBuilder;

use crate::handshake::Handshake;
use crate::helpers::{Helpers, GenericResult, GenericError};
use crate::request::{Request, Destination};
use crate::pump::Pump;
use crate::buffer_pool::Buffer;

pub struct Connection {
    id: String,
    client_socket: TcpStream,
    endpoint_interface: String,
    buffer: Buffer
}

impl Connection {
    pub fn from(stream: TcpStream, endpoint_interface: String, buffer: Buffer) -> Self {
        return Connection { id: Helpers::get_id().to_owned(), client_socket: stream, endpoint_interface: endpoint_interface, buffer: buffer };
    }

    // `self` Connection is moved when the handle method is called, and ownership is given
    // fully to the thread, so `this` Connection will drop when the spawned thread ends.
    pub fn handle(self) -> JoinHandle<()> {
        println!("[{}] Start.", self.id);

        // Move self into the spawned thread, as well.
        return tokio::spawn(async move {
            match self.handle_task().await {
                Ok(_) => return,
                Err(e) => {
                    
                    eprintln!("{}", e);
                }
            }
        });
    }

    async fn handle_task(mut self) -> GenericResult<()> {

        let buffer = &mut self.buffer.get().await[..];

        // Complete handshake.

        let handshake = Connection::perform_handshake(&mut self.client_socket, buffer).await?;
        let methods_string = handshake.methods.into_iter().map(|m| m.to_string()).collect::<Vec<String>>().join(",");

        println!("[{}]   Handshake:", self.id);
        println!("[{}]     Version: {}", self.id, handshake.version);
        println!("[{}]     Num Methods: {}", self.id, handshake.num_methods);
        println!("[{}]     Methods: {}", self.id, methods_string);

        // Get request from client.

        let request = Connection::perform_request_negotiation(&mut self.client_socket, buffer).await?;
        let destination = match &request.destination {
            Destination::Ipv4Addr(ipv4) => ipv4.to_string(),
            Destination::Ipv6Addr(ipv6) => ipv6.to_string(),
            Destination::Domain(s) => s.to_owned()
        };

        println!("[{}]   Request:", self.id);
        println!("[{}]     Version: {}", self.id, request.version);
        println!("[{}]     Command: {}", self.id, request.command);
        println!("[{}]     Address Type: {}", self.id, request.address_type);
        println!("[{}]     Destination: {}", self.id, destination);
        println!("[{}]     Port: {}", self.id, request.port);

        // Perform requested action.

        let mut endpoint_socket: TcpStream;
        match request.command {
            0x01 /* CONNECT */ => endpoint_socket = Connection::establish_connect_request(&mut self.client_socket, &self.endpoint_interface, &request, buffer).await?,
            0x02 /* BIND */ => return Err(Box::new(GenericError::from("BIND requests not supported.")) /* hack */),
            0x03 /* UDP ASSOCIATE */ => return Err(Box::new(GenericError::from("UDP ASSOCIATE requests not supported.")) as Box<dyn std::error::Error> /* hack */),
            _ => return Err(Box::new(GenericError::from("Unknown command type.")) as Box<dyn std::error::Error> /* hack */)
        };

        let client_peer_addr = self.client_socket.peer_addr()?;
        let client_local_addr = self.client_socket.local_addr()?;
        let endpoint_local_addr = endpoint_socket.local_addr()?;
        let endpoint_peer_addr = endpoint_socket.peer_addr()?;

        println!("[{}]   Data Path: {} => {} => {} => {}", self.id, client_peer_addr, client_local_addr, endpoint_local_addr, endpoint_peer_addr);

        Pump::from(&mut self.client_socket, &mut endpoint_socket, buffer).start().await?;

        self.client_socket.shutdown(Shutdown::Both)?;
        endpoint_socket.shutdown(Shutdown::Both)?;

        println!("[{}] End.", self.id);

        return Ok(());
    }

    async fn perform_handshake(client_socket: &mut TcpStream, buffer: &mut [u8]) -> GenericResult<Handshake> {
        let read = client_socket.read(buffer).await?;

        if read == 0 {
            return Err(Box::new(GenericError::from("Read 0 bytes during handshake.")) as Box<dyn std::error::Error> /* This is a hack to fix a bug with async/await in rust. */);
        }

        let handshake = Handshake::from_data(buffer);

        if handshake.version != 5 {
            return Err(Box::new(GenericError::from("Bad SOCKS version.")) as Box<dyn std::error::Error> /* hack */);
        }

        // Reuse the buffer since we are borrowing it anyway.

        buffer[0] = 0x05; // VERSION.
        buffer[1] = 0x00; // NO AUTH.

        client_socket.write(&buffer[..2]).await?;

        return Ok(handshake);
    }

    async fn perform_request_negotiation(client_socket: &mut TcpStream, buffer: &mut [u8]) -> GenericResult<Request> {
        let read = client_socket.read(buffer).await?;

        if read == 0 {
            return Err(Box::new(GenericError::from("Read 0 bytes during connection negotiation.")) as Box<dyn std::error::Error> /* hack */);
        }

        let request = Request::from_data(buffer)?;

        return Ok(request);
    }

    async fn establish_connect_request(client_socket: &mut TcpStream, endpoint_interface: &str, request: &Request, buffer: &mut [u8]) -> GenericResult<TcpStream> {
        let string_to_bind = format!("{}:{}", endpoint_interface, 0); // Have to split into two statements due to Rust bug: https://github.com/rust-lang/rust/issues/64960.
        let local_addr = SocketAddr::from_str(&string_to_bind)?;
        
        let string_to_connect = format!("{}:{}", request.destination, request.port);
        let endpoint_addr: SocketAddr = string_to_connect
            .to_socket_addrs()?
            .collect::<Vec<SocketAddr>>()[0];
        
        // [ARoney] TODO: Don't hardcode this to ipv4...
        let standard_stream = TcpBuilder::new_v4()?.bind(local_addr)?.to_tcp_stream()?;
        let mut error: i32 = 0x00;

        let mut endpoint_socket: Option<TcpStream> = None;
        match TcpStream::connect_std(standard_stream, &endpoint_addr).await {
            Ok(s) => endpoint_socket = Some(s),
            Err(e) => match e.raw_os_error() {
                Some(i) => error = i,
                None => ()
            }
        }
        
        // Get the local IP and port.

        let local_ip = local_addr.ip();
        let (port_high, port_low) = Helpers::port_to_bytes(local_addr.port());

        // Compute correct reply field.

        let reply_field = Helpers::get_socks_reply(error);

        // Prepare reply.

        let mut reply_length = 0;

        buffer[0] = 0x05; // VERSION.local_addr
        buffer[1] = reply_field;
        buffer[2] = 0x0; // RESERVED.

        if let IpAddr::V4(ipv4) = local_ip {
            let octets = ipv4.octets();

            buffer[3] = 0x01; // ADDRESS TYPE (IPv4).
            buffer[4] = octets[0]; buffer[5] = octets[1]; buffer[6] = octets[2]; buffer[7] = octets[3];
            Helpers::write_octets(&mut buffer[4..8], &octets);

            buffer[8] = port_high;
            buffer[9] = port_low;

            reply_length = 10;
        } else if let IpAddr::V6(ipv6) = local_ip {
            let octets = ipv6.octets();

            buffer[3] = 0x04; // ADDRESS TYPE (IPv6).
            Helpers::write_octets(&mut buffer[4..20], &octets);

            buffer[20] = port_high;
            buffer[21] = port_low;

            reply_length = 22;
        }

        // Send a response to the client, even if there is a failure.

        client_socket.write(&buffer[0..reply_length]).await?;

        // In a failure scenario, ensure the SOCKS process does not continue.

        if error != 0 {
            let err_string = format!("The connection failed gracefully with `{}`.", error);
            return Err(Box::new(GenericError::from(err_string)) as Box<dyn std::error::Error> /* hack */);
        }

        return Ok(endpoint_socket.unwrap());
    }
}