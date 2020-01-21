use tokio::net::TcpStream;

pub struct CopyPump<'a> {
    client_socket: &'a mut TcpStream,
    endpoint_socket: &'a mut TcpStream,
}

impl<'a> CopyPump<'a> {
    pub fn from(client_socket: &'a mut TcpStream, endpoint_socket: &'a mut TcpStream) -> Self {
        CopyPump { client_socket, endpoint_socket }
    }

    pub async fn start(mut self) {
        self.run_pumps_as_copy().await;
    }

    async fn run_pumps_as_copy(&mut self) {
        let (mut client_socket_read, mut client_socket_write) = self.client_socket.split();
        let (mut endpoint_socket_read, mut endpoint_socket_write) = self.endpoint_socket.split();

        let pump_up = tokio::io::copy(&mut client_socket_read, &mut endpoint_socket_write);
        let pump_down = tokio::io::copy(&mut endpoint_socket_read, &mut client_socket_write);

        futures::future::select(
            pump_up,
            pump_down
        ).await;
    }
}