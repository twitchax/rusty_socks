use tokio::net::TcpStream;
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::prelude::*;

use tokio::time::{delay_for, Duration};
use tokio::sync::oneshot::{channel, Sender, Receiver};
use futures::future::Either;

use crate::helpers::GenericResult;

pub struct Pump<'a> {
    client_socket: &'a mut TcpStream,
    endpoint_socket: &'a mut TcpStream,
    buffer: &'a mut [u8],
    read_timeout: u64
}

impl<'a> Pump<'a> {
    pub fn from(client_socket: &'a mut TcpStream, endpoint_socket: &'a mut TcpStream, buffer: &'a mut [u8], read_timeout: u64) -> Self {
        return Pump { client_socket: client_socket, endpoint_socket: endpoint_socket, buffer: buffer, read_timeout: read_timeout };
    }

    pub async fn start(self) -> GenericResult<()> {
        // Split the buffer.
        let buffer_size = self.buffer.len();
        let (buffer_up, buffer_down) = self.buffer.split_at_mut(buffer_size / 2);

        // Split the sockets.
        let (client_socket_read, client_socket_write) = self.client_socket.split();
        let (endpoint_socket_read, endpoint_socket_write) = self.endpoint_socket.split();

        // Create cancellation channels.
        let (client_cancellation_sender, client_cancellation_receiver) = channel::<bool>();
        let (endpoint_cancellation_sender, endpoint_cancellation_receiver) = channel::<bool>();
        
        // FYI: Cancellation senders are moved because this is a one-shot channel.  The sender can only send
        // once, and the object is moved when calling the send method.

        // Run the pumps.
        let pump_up = Pump::run_pump(client_socket_read, endpoint_socket_write, client_cancellation_sender, endpoint_cancellation_receiver, buffer_up, self.read_timeout);
        let pump_down = Pump::run_pump(endpoint_socket_read, client_socket_write, endpoint_cancellation_sender, client_cancellation_receiver, buffer_down, self.read_timeout);

        futures::future::join(pump_up, pump_down).await;

        return Ok(());
    }

    async fn run_pump(mut from: ReadHalf<'_>, mut to: WriteHalf<'_>, cancel_sender: Sender<bool>, mut cancel_receiver: Receiver<bool>, buffer: &mut [u8], read_timeout: u64) {
        loop {
            // Read or timeout.
            let select_future = futures::future::select(
                from.read(&mut buffer[..]),
                delay_for(Duration::from_millis(read_timeout))
            ).await;

            // If we read successfully, write.
            if let Either::Left((read_result, _)) = select_future {
                let read = read_result.unwrap_or(0);
                to.write(&buffer[0..read]).await.unwrap_or(0);

                // Reading 0 bytes is a close.  Notify and return.
                if read == 0 {
                    cancel_sender.send(true).unwrap_or_default();
                    return;
                }
            }

            // Return if other thread has cancelled.
            if cancel_receiver.try_recv().unwrap_or(false) {
                return;
            }
        }
    }
}