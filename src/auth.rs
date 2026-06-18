use crate::helpers::{IntoError, Res};

/// SOCKS5 authentication method: no authentication required.
pub const METHOD_NO_AUTH: u8 = 0x00;
/// SOCKS5 authentication method: username/password (RFC 1929).
pub const METHOD_USER_PASS: u8 = 0x02;
/// SOCKS5 method-selection sentinel: none of the client's offered methods are acceptable.
pub const METHOD_NONE_ACCEPTABLE: u8 = 0xFF;

/// Choose the authentication method to reply with during the greeting.
///
/// With no credentials configured we always select no-auth, preserving the proxy's original
/// behavior byte-for-byte. With credentials configured we require the client to offer
/// username/password; if it doesn't, no offered method is acceptable.
pub fn select_method(offered: &[u8], creds: Option<&Credentials>) -> u8 {
    match creds {
        None => METHOD_NO_AUTH,
        Some(_) if offered.contains(&METHOD_USER_PASS) => METHOD_USER_PASS,
        Some(_) => METHOD_NONE_ACCEPTABLE,
    }
}

/// SOCKS5 username/password credentials (RFC 1929).
///
/// The same type serves two roles: parsing a client's authentication sub-negotiation
/// (via [`Credentials::from_data`]) and holding the server's configured expected
/// credentials. Authentication succeeds when the two compare equal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    /// Parse an RFC 1929 username/password request: `VER(0x01) | ULEN | UNAME | PLEN | PASSWD`.
    ///
    /// Lengths are taken from the embedded `ULEN`/`PLEN` fields (bounded by `data`), mirroring
    /// how [`crate::handshake::Handshake`] and [`crate::request::Request`] parse their buffers.
    pub fn from_data(data: &[u8]) -> Res<Credentials> {
        // VER, ULEN.
        if data.len() < 2 {
            return "Auth too short: need at least a version and a username length.".into_error();
        }

        if data[0] != 0x01 {
            return "Bad auth sub-negotiation version.".into_error();
        }

        // Username: ULEN bytes following the header, then the PLEN byte.
        let ulen = usize::from(data[1]);
        let uname_end = 2 + ulen;

        if data.len() < uname_end + 1 {
            return "Auth too short for the stated username length.".into_error();
        }

        // Password: PLEN bytes following the username.
        let plen = usize::from(data[uname_end]);
        let passwd_end = uname_end + 1 + plen;

        if data.len() < passwd_end {
            return "Auth too short for the stated password length.".into_error();
        }

        let username = match String::from_utf8(data[2..uname_end].to_vec()) {
            Ok(s) => s,
            Err(_) => return "Username is not valid UTF-8.".into_error(),
        };

        let password = match String::from_utf8(data[uname_end + 1..passwd_end].to_vec()) {
            Ok(s) => s,
            Err(_) => return "Password is not valid UTF-8.".into_error(),
        };

        Ok(Credentials { username, password })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_username_and_password() {
        // VER=1, ULEN=3, "bob", PLEN=4, "pass".
        let data = [0x01, 0x03, b'b', b'o', b'b', 0x04, b'p', b'a', b's', b's'];
        let creds = Credentials::from_data(&data).unwrap();

        assert_eq!(creds.username, "bob");
        assert_eq!(creds.password, "pass");
    }

    #[test]
    fn parses_empty_username_and_password() {
        // ULEN=0, PLEN=0 is structurally valid; the handshake comparison rejects it, not the parser.
        let creds = Credentials::from_data(&[0x01, 0x00, 0x00]).unwrap();

        assert_eq!(creds.username, "");
        assert_eq!(creds.password, "");
    }

    #[test]
    fn rejects_wrong_subnegotiation_version() {
        // The auth sub-negotiation version is 0x01, distinct from the 0x05 SOCKS version.
        assert!(Credentials::from_data(&[0x05, 0x01, b'a', 0x01, b'b']).is_err());
    }

    #[test]
    fn rejects_truncated_header() {
        // Nothing past the version byte (and the empty case).
        assert!(Credentials::from_data(&[0x01]).is_err());
        assert!(Credentials::from_data(&[]).is_err());
    }

    #[test]
    fn rejects_truncated_username() {
        // Claims ULEN=5 but only two username bytes follow.
        assert!(Credentials::from_data(&[0x01, 0x05, b'a', b'b']).is_err());
    }

    #[test]
    fn rejects_truncated_password() {
        // ULEN=3 "bob", PLEN=4 but only two password bytes follow.
        assert!(Credentials::from_data(&[0x01, 0x03, b'b', b'o', b'b', 0x04, b'p', b'a']).is_err());
    }

    #[test]
    fn rejects_non_utf8_username() {
        // 0xFF is not a valid UTF-8 byte.
        assert!(Credentials::from_data(&[0x01, 0x01, 0xFF, 0x01, b'p']).is_err());
    }

    #[test]
    fn selects_no_auth_when_unconfigured() {
        assert_eq!(select_method(&[METHOD_NO_AUTH], None), METHOD_NO_AUTH);
        // Even a client offering only user/pass stays no-auth when the proxy has no creds (legacy behavior).
        assert_eq!(select_method(&[METHOD_USER_PASS], None), METHOD_NO_AUTH);
    }

    #[test]
    fn selects_user_pass_when_configured_and_offered() {
        let creds = Credentials {
            username: "u".to_owned(),
            password: "p".to_owned(),
        };
        assert_eq!(select_method(&[METHOD_NO_AUTH, METHOD_USER_PASS], Some(&creds)), METHOD_USER_PASS);
    }

    #[test]
    fn rejects_when_configured_but_user_pass_not_offered() {
        let creds = Credentials {
            username: "u".to_owned(),
            password: "p".to_owned(),
        };
        assert_eq!(select_method(&[METHOD_NO_AUTH], Some(&creds)), METHOD_NONE_ACCEPTABLE);
    }

    #[test]
    fn credentials_compare_on_both_fields() {
        let a = Credentials {
            username: "u".to_owned(),
            password: "p".to_owned(),
        };
        assert_eq!(
            a,
            Credentials {
                username: "u".to_owned(),
                password: "p".to_owned()
            }
        );
        assert_ne!(
            a,
            Credentials {
                username: "u".to_owned(),
                password: "x".to_owned()
            }
        );
        assert_ne!(
            a,
            Credentials {
                username: "x".to_owned(),
                password: "p".to_owned()
            }
        );
    }
}
