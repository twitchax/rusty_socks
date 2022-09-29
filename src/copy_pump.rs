use std::time::Duration;

use futures::{pin_mut, future::Either};
use tokio::net::TcpStream;

use crate::helpers::{IntoError, Res};

pub struct CopyPump {
    client_socket: TcpStream,
    endpoint_socket: TcpStream,
    read_timeout: u64
}

impl CopyPump {
    pub fn from(client_socket: TcpStream, endpoint_socket: TcpStream, read_timeout: u64) -> Self {
        CopyPump { client_socket, endpoint_socket, read_timeout }
    }

    pub async fn start(self) -> Res<()> {
        self.run_pumps_as_copy().await
    }

    async fn run_pumps_as_copy(self) -> Res<()> {
        let (mut client_socket_read, mut client_socket_write) = self.client_socket.into_split();
        let (mut endpoint_socket_read, mut endpoint_socket_write) = self.endpoint_socket.into_split();

        let pump_up = tokio::io::copy(&mut client_socket_read, &mut endpoint_socket_write);
        let pump_down = tokio::io::copy(&mut endpoint_socket_read, &mut client_socket_write);

        pin_mut!(pump_up);
        pin_mut!(pump_down);
        

        let pumps = futures::future::select(pump_up, pump_down);

        let timeout = tokio::time::sleep(Duration::from_millis(self.read_timeout));
        pin_mut!(timeout);

        match futures::future::select(pumps, timeout).await {
            Either::Left(_) => {},
            Either::Right((_, _)) => {
                return "Timed out.".into_error()
            }
        }

        Ok(())
    }
}