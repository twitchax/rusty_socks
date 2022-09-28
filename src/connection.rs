use tokio::{io::AsyncReadExt, net::TcpSocket, task::JoinHandle};
use tokio::net::{TcpStream};
use tokio::io::AsyncWriteExt;

use std::iter::IntoIterator;
use std::str::FromStr;
use std::net::{SocketAddr, IpAddr, ToSocketAddrs};
use net2::TcpBuilder;
use log::{error, info, debug, warn};
use phf::{Map, phf_map};

use crate::handshake::Handshake;
use crate::helpers::{Helpers, Res, Void, IntoError};
use crate::request::{Request, Destination};
//use crate::custom_pump::CustomPump;
use crate::copy_pump::CopyPump;
use crate::buffer_pool::Buffer;

pub struct Connection {
    id: String,
    client_socket: TcpStream,
    endpoint_interface: String,
    buffer: Buffer, 
    read_timeout: u64
}

impl Connection {
    pub fn from(client_socket: TcpStream, endpoint_interface: String, buffer: Buffer, read_timeout: u64) -> Self {
        Connection { id: Helpers::get_id(), client_socket, endpoint_interface, buffer, read_timeout }
    }

    // `self` Connection is moved when the handle method is called, and ownership is given
    // fully to the thread, so `this` Connection will drop when the spawned thread ends.
    pub fn handle(self) -> JoinHandle<()> {
        debug!("[{}] Start.", self.id);

        // Move self into the spawned thread, as well.
        tokio::spawn(async move {
            match self.handle_task().await {
                Ok(_) => {},
                Err(e) => {
                    error!("{}", e);
                }
            }
        })
    }

    async fn handle_task(mut self) -> Void {
        // Get a &mut slice from the leased buffer.
        let buffer = &mut self.buffer.get().await[..];

        // Complete handshake.

        let handshake = Connection::perform_handshake(&mut self.client_socket, buffer).await?;
        let methods_string = handshake.methods.into_iter().map(|m| m.to_string()).collect::<Vec<String>>().join(",");

        debug!("[{}]   Handshake:", self.id);
        debug!("[{}]     Version: {}", self.id, handshake.version);
        debug!("[{}]     Num Methods: {}", self.id, handshake.num_methods);
        debug!("[{}]     Methods: {}", self.id, methods_string);

        // Get request from client.

        let request = Connection::perform_request_negotiation(&mut self.client_socket, buffer).await?;
        let destination = match &request.destination {
            Destination::Ipv4Addr(ipv4) => ipv4.to_string(),
            Destination::Ipv6Addr(ipv6) => ipv6.to_string(),
            Destination::Domain(s) => s.to_owned()
        };

        debug!("[{}]   Request:", self.id);
        debug!("[{}]     Version: {}", self.id, request.version);
        debug!("[{}]     Command: {}", self.id, COMMANDS[&request.command]);
        debug!("[{}]     Address Type: {}", self.id, ADDRESS_TYPES[&request.address_type]);
        debug!("[{}]     Destination: {}", self.id, destination);
        debug!("[{}]     Port: {}", self.id, request.port);

        // Perform requested action.

        let mut endpoint_socket: TcpStream;
        match request.command {
            0x01 /* CONNECT */ => endpoint_socket = Connection::establish_connect_request(&mut self.client_socket, &self.endpoint_interface, &request, buffer).await?,
            0x02 /* BIND */ => return "BIND requests not supported.".into_error(),
            0x03 /* UDP ASSOCIATE */ => return "UDP ASSOCIATE requests not supported.".into_error(),
            _ => return "Unknown command type.".into_error()
        };

        // Print the data path.

        let client_peer_addr = self.client_socket.peer_addr()?;
        let client_local_addr = self.client_socket.local_addr()?;
        let endpoint_local_addr = endpoint_socket.local_addr()?;
        let endpoint_peer_addr = endpoint_socket.peer_addr()?;

        info!("[{}] {} => {} => {} => {}", self.id, client_peer_addr, client_local_addr, endpoint_local_addr, endpoint_peer_addr);

        // Run the pump (all errors in pumps are emitted as log messages and should not disrupt the execution flow).

        //CustomPump::from(&self.id, self.client_socket, endpoint_socket, buffer, self.read_timeout).start().await;
        CopyPump::from(self.client_socket, endpoint_socket).start().await;

        debug!("[{}] End.", self.id);

        Ok(())
    }

    async fn perform_handshake(client_socket: &mut TcpStream, buffer: &mut [u8]) -> Res<Handshake> {
        let read = client_socket.read(buffer).await?;

        if read == 0 {
            return "Read 0 bytes during handshake.".into_error();
        }

        let handshake = Handshake::from_data(buffer);

        if handshake.version != 5 {
            return "Bad SOCKS version.".into_error();
        }

        // Reuse the buffer since we are borrowing it anyway.

        buffer[0] = 0x05; // VERSION.
        buffer[1] = 0x00; // NO AUTH.

        client_socket.write_all(&buffer[..2]).await?;
        client_socket.flush().await?;

        Ok(handshake)
    }

    async fn perform_request_negotiation(client_socket: &mut TcpStream, buffer: &mut [u8]) -> Res<Request> {
        let read = client_socket.read(buffer).await?;

        if read == 0 {
            return "Read 0 bytes during connection negotiation.".into_error();
        }

        let request = Request::from_data(buffer)?;

        Ok(request)
    }

    async fn establish_connect_request(client_socket: &mut TcpStream, endpoint_interface: &str, request: &Request, buffer: &mut [u8]) -> Res<TcpStream> {
        let mut reply = 0u8;

        // Get requested local interface.
        let local_addr = SocketAddr::from_str(&format!("{}:{}", endpoint_interface, 0))?;
        
        // Get endpoint address.
        let string_to_connect = format!("{}:{}", request.destination, request.port);
        let endpoint_addr_iterator = string_to_connect.to_socket_addrs();

        // Bind to requested local address.
        // [ARoney] TODO: Don't hardcode this to ipv4...
        let socket = TcpSocket::new_v4()?;
        socket.bind(local_addr)?;

        // Compute valid endpoint addresses, and connect to endpoint.
        
        let endpoint_socket = match endpoint_addr_iterator {
            Ok(addresses) => {
                // [ARoney] TODO: Don't hardcode this to ipv4...
                let endpoint_addr = addresses.into_iter().find(|a| a.is_ipv4()).unwrap();

                match socket.connect(endpoint_addr).await {
                    Ok(s) => Some(s),
                    Err(e) => {
                        warn!("Could not connect to `{}` (`{}`).", string_to_connect, endpoint_addr);
                        
                        reply = match e.raw_os_error() {
                            Some(i) => Helpers::get_socks_reply(i),
                            _ => 5u8 // Connection refused?.
                        };

                        None
                    }
                }
            },
            Err(e) => {
                warn!("Could not compute an endpoint address for `{}`.", string_to_connect);
                
                reply = match e.raw_os_error() {
                    Some(i) => Helpers::get_socks_reply(i),
                    _ => 8u8 // Address type not supported.
                };

                None
            }
        };
        
        // Get the local IP and port.
        let local_ip = local_addr.ip();
        let (port_high, port_low) = Helpers::port_to_bytes(local_addr.port());

        // Prepare reply.

        buffer[0] = 0x05; // VERSION.
        buffer[1] = reply;
        buffer[2] = 0x0; // RESERVED.

        let reply_length = match local_ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();

                buffer[3] = 0x01; // ADDRESS TYPE (IPv4).
                buffer[4] = octets[0]; buffer[5] = octets[1]; buffer[6] = octets[2]; buffer[7] = octets[3];
                Helpers::write_octets(&mut buffer[4..8], &octets);

                buffer[8] = port_high;
                buffer[9] = port_low;

                10
            },
            IpAddr::V6(ipv6) => {
                let octets = ipv6.octets();

                buffer[3] = 0x04; // ADDRESS TYPE (IPv6).
                Helpers::write_octets(&mut buffer[4..20], &octets);

                buffer[20] = port_high;
                buffer[21] = port_low;

                22
            }
        };

        // Send a response to the client, even if there is a failure.

        client_socket.write_all(&buffer[0..reply_length]).await?;
        client_socket.flush().await?;

        // In a failure scenario, ensure the SOCKS process does not continue.
        
        if reply != 0 {
            return format!("The connection to `{}` failed gracefully with `{}`.", string_to_connect, ERRORS[&reply]).into_error();
        }
        
        // This should only be `None` if there is an error, which aborts above.
        Ok(endpoint_socket.unwrap())
    }
}

static COMMANDS: Map<u8, &'static str> = phf_map! {
    1u8 => "Connect",
    2u8 => "Bind",
    3u8 => "UDP Associate",
};

static ADDRESS_TYPES: Map<u8, &'static str> = phf_map! {
    1u8 => "Ipv4",
    3u8 => "Domain",
    4u8 => "Ipv6",
};

static ERRORS: Map<u8, &'static str> = phf_map! {
    0u8 => "Succeeded",
    1u8 => "General SOCKS Server Failure",
    3u8 => "Network Unreachable",
    4u8 => "Host Unreachable",
    5u8 => "Connection Refused",
    6u8 => "TTL Expired",
    8u8 => "Address type not supported"
};