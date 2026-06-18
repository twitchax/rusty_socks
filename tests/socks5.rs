//! End-to-end test: drive a real SOCKS5 `CONNECT` handshake through `serve()` to a loopback echo
//! server and assert the payload round-trips. Exercises handshake -> request -> connect -> pump.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use rusty_sockslib::config::Config;
use rusty_sockslib::serve;

/// Bind a loopback TCP echo server and return its address.
async fn spawn_echo() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if sock.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
    });

    addr
}

/// Bind the proxy on an ephemeral loopback port and serve it in the background.
async fn spawn_proxy() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = Config {
        listen_interface: None,
        endpoint_interface: None,
        port: 0, // unused: `serve` is handed an already-bound listener
        buffer_size: 2048,
        read_timeout: 60_000,
        accept_cidr: "0.0.0.0/0".to_owned(),
    };

    tokio::spawn(async move {
        let _ = serve(listener, config).await;
    });

    addr
}

#[tokio::test]
async fn socks5_connect_pumps_data_end_to_end() {
    let echo_addr = spawn_echo().await;
    let proxy_addr = spawn_proxy().await;

    let body = async {
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();

        // Greeting: VER=5, NMETHODS=1, METHODS=[NO_AUTH]; expect method selection [5, 0].
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut method = [0u8; 2];
        client.read_exact(&mut method).await.unwrap();
        assert_eq!(method, [0x05, 0x00], "server should select NO_AUTH");

        // CONNECT to the echo server (IPv4).
        let ip = match echo_addr.ip() {
            IpAddr::V4(v4) => v4.octets(),
            IpAddr::V6(_) => unreachable!("loopback bind is v4"),
        };
        let port = echo_addr.port();
        let request = [0x05, 0x01, 0x00, 0x01, ip[0], ip[1], ip[2], ip[3], (port >> 8) as u8, (port & 0xff) as u8];
        client.write_all(&request).await.unwrap();

        // Reply: VER, REP, RSV, ATYP=IPv4, BND.ADDR(4), BND.PORT(2) = 10 bytes.
        let mut reply = [0u8; 10];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply[0], 0x05, "reply version");
        assert_eq!(reply[1], 0x00, "SOCKS CONNECT should succeed");

        // Payload should echo back unchanged through the tunnel.
        let payload = b"hello rusty_socks";
        client.write_all(payload).await.unwrap();
        let mut echoed = [0u8; 17];
        client.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, payload, "payload should round-trip through the proxy");
    };

    timeout(Duration::from_secs(5), body).await.expect("end-to-end flow should complete within 5s");
}
