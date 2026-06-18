use std::time::Duration;

use futures::{pin_mut, future::Either};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

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

        // `read_timeout` is an *idle* timeout, not a connection lifetime cap: the clock resets
        // every time bytes move (see `pump`), so an active connection stays open indefinitely and
        // only a connection that is genuinely silent for `read_timeout` ms is torn down. `0`
        // disables the idle timeout entirely.
        let idle = match self.read_timeout {
            0 => None,
            ms => Some(Duration::from_millis(ms))
        };

        let pump_up = Self::pump(&mut client_socket_read, &mut endpoint_socket_write, idle);
        let pump_down = Self::pump(&mut endpoint_socket_read, &mut client_socket_write, idle);

        pin_mut!(pump_up);
        pin_mut!(pump_down);

        // The connection is finished as soon as either direction ends (EOF, error, or idle);
        // dropping the surviving pump closes the other half of the connection.
        match futures::future::select(pump_up, pump_down).await {
            Either::Left((result, _)) | Either::Right((result, _)) => result
        }
    }

    /// Copy bytes from `from` to `to` until EOF, an I/O error, or — when `idle` is set — a window
    /// of `idle` with no data at all. Because each iteration arms a *fresh* timeout around a single
    /// read, any byte that arrives resets the clock; the timeout only trips on true silence.
    async fn pump<R, W>(from: &mut R, to: &mut W, idle: Option<Duration>) -> Res<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin
    {
        let mut buffer = [0u8; 16 * 1024];

        loop {
            let read = match idle {
                Some(duration) => match timeout(duration, from.read(&mut buffer)).await {
                    Ok(result) => result?,
                    Err(_) => return "Idle timeout.".into_error()
                },
                None => from.read(&mut buffer).await?
            };

            // A zero-length read is a clean half-close from the peer.
            if read == 0 {
                return Ok(());
            }

            to.write_all(&buffer[..read]).await?;
            to.flush().await?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CopyPump;
    use std::time::Duration;
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};
    use tokio::time::{sleep, timeout};

    // The regression guard for the 60s hard-cap bug: a connection that keeps moving data must
    // survive a span far longer than the idle window, because each chunk resets the clock.
    #[tokio::test]
    async fn idle_timeout_resets_on_activity() {
        let idle = Some(Duration::from_millis(250));

        // `src` feeds the pump's reader (`from`); the pump writes into `to`, drained via `drain`.
        let (mut src, mut from) = duplex(256);
        let (mut to, mut drain) = duplex(256);

        // One byte every 50ms for 500ms: each 50ms gap is well inside the 250ms window, but the
        // 500ms total span is twice the window — the old absolute deadline would have killed it.
        let writer = async move {
            for _ in 0..10 {
                src.write_all(b"x").await.unwrap();
                src.flush().await.unwrap();
                sleep(Duration::from_millis(50)).await;
            }
            // Closing the source triggers a clean EOF, so the pump returns Ok.
            drop(src);
        };

        let reader = async move {
            let mut buf = [0u8; 16];
            let mut total = 0;
            while total < 10 {
                match drain.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => total += n
                }
            }
            total
        };

        let pump = CopyPump::pump(&mut from, &mut to, idle);

        // Run the pump and its driver concurrently on one task (a boxed-error `Result` isn't
        // `Send`, so it can't cross a `tokio::spawn` boundary — `join!` keeps it local).
        let (pump_result, (), received) =
            timeout(Duration::from_secs(5), async { tokio::join!(pump, writer, reader) })
                .await
                .expect("pump + driver should finish well within 5s");

        assert!(pump_result.is_ok(), "active connection was killed: {:?}", pump_result.err());
        assert_eq!(received, 10, "all bytes should have been pumped through");
    }

    // A silent connection must be reaped once the idle window elapses.
    #[tokio::test]
    async fn idle_timeout_fires_when_silent() {
        let idle = Some(Duration::from_millis(100));

        let (_src, mut from) = duplex(64); // hold the source open but never write to it
        let (mut to, _drain) = duplex(64);

        let result = timeout(Duration::from_secs(2), CopyPump::pump(&mut from, &mut to, idle))
            .await
            .expect("pump should give up around the idle window, well before 2s");

        assert!(result.is_err(), "silent connection should have hit the idle timeout");
    }

    // `0`/`None` disables the idle timeout: a silent connection is NOT reaped (it waits for EOF).
    #[tokio::test]
    async fn disabled_idle_timeout_never_fires() {
        let (_src, mut from) = duplex(64);
        let (mut to, _drain) = duplex(64);

        // With the timeout disabled the pump stays blocked on the read, so the *outer* bound is
        // what trips — i.e. the pump itself never returned.
        let outcome = timeout(
            Duration::from_millis(300),
            CopyPump::pump(&mut from, &mut to, None)
        )
        .await;

        assert!(outcome.is_err(), "with idle disabled the pump must keep waiting, not return");
    }
}
