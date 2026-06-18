[![Build and Test](https://github.com/twitchax/rusty_socks/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/rusty_socks/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/rusty_socks/branch/master/graph/badge.svg)](https://codecov.io/gh/twitchax/rusty_socks)
[![Version](https://img.shields.io/crates/v/rsocks.svg)](https://crates.io/crates/rsocks)
[![Crates.io](https://img.shields.io/crates/d/rsocks?label=crate)](https://crates.io/crates/rsocks)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/rusty_socks/total?label=binary)](https://github.com/twitchax/rusty_socks/releases)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# rusty_socks

A super basic SOCKS5 proxy, written in Rust on `tokio`.

> Published on crates.io as [`rsocks`](https://crates.io/crates/rsocks) — the `rusty_socks` name was already taken by an unrelated crate. The repo, binary, and Docker image stay `rusty_socks`.

`rusty_socks` is a small, no-frills SOCKS5 (`CONNECT`) proxy: point a browser, an `ssh` `ProxyCommand`, or anything else SOCKS5-aware at it and it relays TCP to the requested destination. It adds a CIDR allow-list, a reset-on-activity idle timeout, and optional binding to specific network interfaces — and nothing else.

## Usage

Run with defaults (listen on `0.0.0.0:1080`, accept every client):

```bash
$ rusty_socks
```

Every option is available as a CLI flag **or** its `RS_*` environment variable (flags win):

```bash
$ rusty_socks --help
A super basic SOCKS5 proxy.

Usage: rusty_socks [OPTIONS]

Options:
      --listen-interface <LISTEN_INTERFACE>
          Network interface whose IP the proxy listens on (defaults to `0.0.0.0`) [env: RS_LISTEN_INTERFACE=]
      --endpoint-interface <ENDPOINT_INTERFACE>
          Network interface whose IP is used for outbound connections to endpoints (defaults to `0.0.0.0`) [env: RS_ENDPOINT_INTERFACE=]
      --port <PORT>
          Port to listen on [env: RS_PORT=] [default: 1080]
      --buffer-size <BUFFER_SIZE>
          Per-direction buffer size, in bytes [env: RS_BUFFER_SIZE=] [default: 2048]
      --read-timeout <READ_TIMEOUT>
          Idle timeout in milliseconds: a connection with no traffic in either direction for this long is closed. `0` disables the idle timeout entirely [env: RS_READ_TIMEOUT=] [default: 60000]
      --accept-cidr <ACCEPT_CIDR>
          CIDR of client addresses allowed to connect [env: RS_ACCEPT_CIDR=] [default: 0.0.0.0/0]
  -h, --help
          Print help
  -V, --version
          Print version
```

### Configuration

| Flag | Env var | Default | Description |
| --- | --- | --- | --- |
| `--listen-interface` | `RS_LISTEN_INTERFACE` | _(none → `0.0.0.0`)_ | Network interface whose IP the proxy listens on. |
| `--endpoint-interface` | `RS_ENDPOINT_INTERFACE` | _(none → `0.0.0.0`)_ | Network interface used for outbound connections to endpoints. |
| `--port` | `RS_PORT` | `1080` | Port to listen on. |
| `--buffer-size` | `RS_BUFFER_SIZE` | `2048` | Per-direction buffer size, in bytes. |
| `--read-timeout` | `RS_READ_TIMEOUT` | `60000` | **Idle** timeout (ms); the clock resets on every byte, so only genuinely silent connections are reaped. `0` disables it. |
| `--accept-cidr` | `RS_ACCEPT_CIDR` | `0.0.0.0/0` | CIDR of client addresses allowed to connect. |

Logging uses [`tracing`](https://docs.rs/tracing); set `RUST_LOG` to change the level (e.g. `RUST_LOG=rusty_sockslib=debug`).

### As a browser proxy

Run `rusty_socks` on a host that can reach where you want to go, then set your browser's SOCKS host to `host:1080` (SOCKS v5). All browser TCP traffic is relayed through it — the generic, any-destination case SOCKS5 is built for.

### As an `ssh` hop

Tunnel `ssh` through a machine that can't (or shouldn't) run `sshd`, using a SOCKS-aware connector such as `ncat`:

```sshconfig
Host myhost
  ProxyCommand ncat --proxy proxy-host:1080 --proxy-type socks5 %h %p
  ServerAliveInterval 15
```

> Keep `ServerAliveInterval` comfortably under `read-timeout` so an idle session is kept alive rather than reaped.

## Install

Linux:

```bash
$ curl -LO https://github.com/twitchax/rusty_socks/releases/latest/download/rusty_socks_x86_64-unknown-linux-gnu.zip
$ unzip rusty_socks_x86_64-unknown-linux-gnu.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/rusty_socks
```

macOS (Apple Silicon):

```bash
$ curl -LO https://github.com/twitchax/rusty_socks/releases/latest/download/rusty_socks_aarch64-apple-darwin.zip
$ unzip rusty_socks_aarch64-apple-darwin.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/rusty_socks
```

Windows:

```powershell
$ iwr https://github.com/twitchax/rusty_socks/releases/latest/download/rusty_socks_x86_64-pc-windows-gnu.zip -OutFile rusty_socks.zip
$ Expand-Archive rusty_socks.zip -DestinationPath C:\Users\%USERNAME%\AppData\Local\Programs\rusty_socks
```

Cargo:

```bash
$ cargo install rsocks
```

## Docker

Published as [`twitchax/rusty_socks`](https://hub.docker.com/r/twitchax/rusty_socks). Configure via flags or `RS_*` env vars:

```bash
$ docker run -d --net host \
    -e RS_PORT=1080 \
    -e RS_ACCEPT_CIDR=10.0.0.0/8 \
    twitchax/rusty_socks
```

## Testing

```bash
$ cargo nextest run
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
