use clap::Parser;

use crate::auth::Credentials;
use crate::helpers::{Helpers, IntoError, Res};

/// A super basic SOCKS5 proxy.
///
/// Every option can be supplied as a CLI flag or via its `RS_*` environment variable; flags win.
#[derive(Parser, Debug, Clone)]
#[command(name = "rsocks", version, about, long_about = None)]
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

    /// Username required for SOCKS5 username/password authentication (RFC 1929).
    /// Must be set together with `--password`; when both are unset, the proxy requires no auth.
    #[arg(long, env = "RS_USERNAME")]
    pub username: Option<String>,

    /// Password required for SOCKS5 username/password authentication (RFC 1929).
    /// Must be set together with `--username`.
    #[arg(long, env = "RS_PASSWORD")]
    pub password: Option<String>,
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

    /// Resolve the configured SOCKS5 credentials.
    ///
    /// Returns `Ok(None)` when authentication is disabled (neither flag set), `Ok(Some(_))`
    /// when both are set, and an error when exactly one is set — that asymmetry almost always
    /// means the operator thinks auth is on when it isn't, so we fail fast rather than silently
    /// run open.
    pub fn credentials(&self) -> Res<Option<Credentials>> {
        match (&self.username, &self.password) {
            (None, None) => Ok(None),
            (Some(username), Some(password)) => Ok(Some(Credentials {
                username: username.clone(),
                password: password.clone(),
            })),
            _ => "Both --username and --password must be set together to enable authentication.".into_error(),
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
        let config = Config::parse_from(["rsocks", "--port", "9050", "--read-timeout", "0", "--accept-cidr", "10.0.0.0/8"]);

        assert_eq!(config.port, 9050);
        assert_eq!(config.read_timeout, 0);
        assert_eq!(config.accept_cidr, "10.0.0.0/8");
    }

    #[test]
    fn unspecified_interface_resolves_to_wildcard() {
        let config = Config::parse_from(["rsocks", "--port", "1"]);

        assert_eq!(config.listen_ip().unwrap(), "0.0.0.0");
        assert_eq!(config.endpoint_ip().unwrap(), "0.0.0.0");
    }

    #[test]
    fn credentials_none_when_neither_flag_set() {
        let config = Config::parse_from(["rsocks"]);

        assert!(config.credentials().unwrap().is_none());
    }

    #[test]
    fn credentials_some_when_both_flags_set() {
        let config = Config::parse_from(["rsocks", "--username", "bob", "--password", "s3cret"]);
        let creds = config.credentials().unwrap().unwrap();

        assert_eq!(creds.username, "bob");
        assert_eq!(creds.password, "s3cret");
    }

    #[test]
    fn credentials_error_when_only_one_flag_set() {
        assert!(Config::parse_from(["rsocks", "--username", "bob"]).credentials().is_err());
        assert!(Config::parse_from(["rsocks", "--password", "s3cret"]).credentials().is_err());
    }
}
