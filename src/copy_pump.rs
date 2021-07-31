use futures::pin_mut;
use log::info;
use tokio::net::TcpStream;

pub struct CopyPump {
    client_socket: TcpStream,
    endpoint_socket: TcpStream,
}

impl CopyPump {
    pub fn from(client_socket: TcpStream, endpoint_socket: TcpStream) -> Self {
        CopyPump { client_socket, endpoint_socket }
    }

    pub async fn start(mut self) {
        self.run_pumps_as_copy().await;
    }

    async fn run_pumps_as_copy(self) {
        let (mut client_socket_read, mut client_socket_write) = self.client_socket.into_split();
        let (mut endpoint_socket_read, mut endpoint_socket_write) = self.endpoint_socket.into_split();

        let pump_up = tokio::io::copy(&mut client_socket_read, &mut endpoint_socket_write);
        let pump_down = tokio::io::copy(&mut endpoint_socket_read, &mut client_socket_write);

        pin_mut!(pump_up);
        pin_mut!(pump_down);

        futures::future::select(
            pump_up,
            pump_down
        ).await;
    }
}