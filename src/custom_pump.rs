use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}}, time::sleep};
use tokio::net::tcp::{ReadHalf, WriteHalf};

use tokio::time::{Duration};
use tokio::sync::oneshot::{channel, Sender, Receiver};
use futures::{future::Either, pin_mut};

use log::trace;
use log::error;

pub struct CustomPump<'a> {
    id: &'a str,
    client_socket: TcpStream,
    endpoint_socket: TcpStream,
    buffer: &'a mut [u8],
    read_timeout: u64
}

impl<'a> CustomPump<'a> {
    pub fn from(id: &'a str, client_socket: TcpStream, endpoint_socket: TcpStream, buffer: &'a mut [u8], read_timeout: u64) -> Self {
        CustomPump { id, client_socket, endpoint_socket, buffer, read_timeout }
    }

    pub async fn start(mut self) {
        self.run_pumps_custom().await;
    }

    async fn run_pumps_custom(mut self) {
        // Split the buffer.
        let buffer_size = self.buffer.len();
        let (buffer_up, buffer_down) = self.buffer.split_at_mut(buffer_size / 2);

        // Split the sockets.
        let (client_socket_read, client_socket_write) = self.client_socket.into_split();
        let (endpoint_socket_read, endpoint_socket_write) = self.endpoint_socket.into_split();

        // Create cancellation channels.
        let (client_cancellation_sender, client_cancellation_receiver) = channel::<bool>();
        let (endpoint_cancellation_sender, endpoint_cancellation_receiver) = channel::<bool>();

        // FYI: Cancellation senders are moved because this is a one-shot channel.  The sender can only send
        // once, and the object is moved when calling the send method.

        // Run the pumps.
        let pump_up = CustomPump::run_pump(&self.id, "up", client_socket_read, endpoint_socket_write, client_cancellation_sender, endpoint_cancellation_receiver, buffer_up, self.read_timeout);
        let pump_down = CustomPump::run_pump(&self.id, "down", endpoint_socket_read, client_socket_write, endpoint_cancellation_sender, client_cancellation_receiver, buffer_down, self.read_timeout);

        futures::future::join(pump_up, pump_down).await;
    }

    async fn run_pump(
        id: &str,
        direction: &str,
        mut from: OwnedReadHalf, 
        mut to: OwnedWriteHalf, 
        cancel_sender: Sender<bool>, 
        mut cancel_receiver: Receiver<bool>, 
        mut buffer: &mut [u8], 
        read_timeout: u64
    ) {
        loop {
            let mut read_fut = from.read(buffer);
            let mut timeout_fut = sleep(Duration::from_millis(read_timeout));

            pin_mut!(read_fut);
            pin_mut!(timeout_fut);

            // Read or timeout.
            let select_future = futures::future::select(
                read_fut,
                timeout_fut
            ).await;
            
            // If we read successfully, write.
            if let Either::Left((Ok(read), _)) = select_future {
                // Reading 0 bytes is a close, and a write error is a receiver close.  Notify and return.
                if read == 0 {
                    trace!("[{}] Read {} bytes while pumping {}, closing.", id, read, direction);

                    cancel_sender.send(true).unwrap_or_default();
                    return;
                }
                
                if let Err(err) = to.write_all(&buffer[..read]).await {
                    error!("[{}] Failed to write {} {} bytes of data, closing: {}", id, direction, read, err);
                    return;
                }

                if let Err(err) = to.flush().await {
                    error!("[{}] Failed to flush {} {} bytes of data, closing: {}", id, direction, read, err);
                    return;
                }

                trace!("[{}] Pumped {} {} bytes of data: {:x?}.", id, direction, read, &buffer[0..10]);
            } else if let Either::Left((Err(err), _)) = select_future {
                error!("[{}] Failed to read {}: {}.", id, direction, err);
                return;
            }

            //Return if other thread has cancelled.
            if cancel_receiver.try_recv().unwrap_or(false) {
                return;
            }
        }
    }
}