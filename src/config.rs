use clap::Parser;

use crate::helpers::{Helpers, Res};

/// A super basic SOCKS5 proxy.
///
/// Every option can be supplied as a CLI flag or via its `RS_*` environment variable; flags win.
#[derive(Parser, Debug, Clone)]
#[command(name = "rusty_socks", version, about, long_about = None)]
pub struct Config {
    /// Network interface whose IP the proxy listens on (defaults to `0.0.0.0`).
    #[arg(long, env = "RS_LISTEN_INTERFACE")]
    pub listen_interface: Option<String>,

    /// Network interface whose IP is used for outbound connections to endpoints (defaults to `0.0.0.0`).
    #[arg(long, env = "RS_ENDPOINT_INTERFACE")]
    pub endpoint_interface: Option<String>,

    /// Port to listen on.
    #[arg(long, env = "RS_PORT", default_value_t = 1080)]
    pub port: u16,

    /// Per-direction buffer size, in bytes.
    #[arg(long, env = "RS_BUFFER_SIZE", default_value_t = 2048)]
    pub buffer_size: usize,

    /// Idle timeout in milliseconds: a connection with no traffic in either direction for this long
    /// is closed. `0` disables the idle timeout entirely.
    #[arg(long, env = "RS_READ_TIMEOUT", default_value_t = 60_000)]
    pub read_timeout: u64,

    /// CIDR of client addresses allowed to connect.
    #[arg(long, env = "RS_ACCEPT_CIDR", default_value = "0.0.0.0/0")]
    pub accept_cidr: String,
}

impl Config {
    /// Resolve the listen IP from the configured interface, defaulting to `0.0.0.0`.
    pub fn listen_ip(&self) -> Res<String> {
        Self::interface_ip(self.listen_interface.as_deref())
    }

    /// Resolve the endpoint (outbound) IP from the configured interface, defaulting to `0.0.0.0`.
    pub fn endpoint_ip(&self) -> Res<String> {
        Self::interface_ip(self.endpoint_interface.as_deref())
    }

    fn interface_ip(interface: Option<&str>) -> Res<String> {
        match interface {
            Some(name) => Ok(Helpers::get_interface_ip(name)?.to_string()),
            None => Ok("0.0.0.0".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use pretty_assertions::assert_eq;

    #[test]
    fn cli_flags_override_defaults() {
        let config = Config::parse_from(["rusty_socks", "--port", "9050", "--read-timeout", "0", "--accept-cidr", "10.0.0.0/8"]);

        assert_eq!(config.port, 9050);
        assert_eq!(config.read_timeout, 0);
        assert_eq!(config.accept_cidr, "10.0.0.0/8");
    }

    #[test]
    fn unspecified_interface_resolves_to_wildcard() {
        let config = Config::parse_from(["rusty_socks", "--port", "1"]);

        assert_eq!(config.listen_ip().unwrap(), "0.0.0.0");
        assert_eq!(config.endpoint_ip().unwrap(), "0.0.0.0");
    }
}
