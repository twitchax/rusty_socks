use std::fmt::Display;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::helpers::{Helpers, IntoError, Res};

pub struct Request {
    pub version: u8,
    pub command: u8,
    pub reserved: u8,
    pub address_type: u8,
    pub port: u16,
    pub destination: Destination,
}

pub enum Destination {
    Ipv4Addr(Ipv4Addr),
    Ipv6Addr(Ipv6Addr),
    Domain(String),
}

impl Display for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Ipv4Addr(ipv4) => write!(f, "{}", ipv4),
            Self::Ipv6Addr(ipv6) => write!(f, "{}", ipv6),
            Self::Domain(domain) => write!(f, "{}", domain),
        }
    }
}

impl Request {
    pub fn from_data(data: &[u8]) -> Res<Self> {
        // VER, CMD, RSV, ATYP.
        if data.len() < 4 {
            return "Request too short: need at least the four-byte header.".into_error();
        }

        let version = data[0];
        let command = data[1];
        let reserved = data[2];
        let address_type = data[3];

        match address_type {
            0x01 => {
                // IPv4: four address bytes followed by a two-byte port.
                if data.len() < 10 {
                    return "Request too short for an IPv4 address.".into_error();
                }

                let address = Ipv4Addr::from(Helpers::slice_to_u32(&data[4..8])?);
                let port = Helpers::bytes_to_port(&data[8..10])?;

                Ok(Request {
                    version,
                    command,
                    reserved,
                    address_type,
                    port,
                    destination: Destination::Ipv4Addr(address),
                })
            }
            0x03 => {
                // Domain: a length byte, that many name bytes, then a two-byte port.
                if data.len() < 5 {
                    return "Request too short for a domain name.".into_error();
                }

                let name_length = data[4] as usize;
                let port_start = 5 + name_length;

                if data.len() < port_start + 2 {
                    return "Request too short for the stated domain length.".into_error();
                }

                let name = std::str::from_utf8(&data[5..port_start])?.to_owned();
                let port = Helpers::bytes_to_port(&data[port_start..port_start + 2])?;

                Ok(Request {
                    version,
                    command,
                    reserved,
                    address_type,
                    port,
                    destination: Destination::Domain(name),
                })
            }
            0x04 => {
                // IPv6: sixteen address bytes followed by a two-byte port.
                if data.len() < 22 {
                    return "Request too short for an IPv6 address.".into_error();
                }

                let address = Ipv6Addr::from(Helpers::slice_to_u128(&data[4..20])?);
                let port = Helpers::bytes_to_port(&data[20..22])?;

                Ok(Request {
                    version,
                    command,
                    reserved,
                    address_type,
                    port,
                    destination: Destination::Ipv6Addr(address),
                })
            }
            _ => "Unknown request type, or data corrupt.".into_error(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn parses_ipv4_connect() {
        // VER, CMD=CONNECT, RSV, ATYP=IPv4, 93.184.216.34, port 443.
        let data = [0x05, 0x01, 0x00, 0x01, 93, 184, 216, 34, 0x01, 0xBB];
        let req = Request::from_data(&data).unwrap();

        assert_eq!(req.version, 5);
        assert_eq!(req.command, 1);
        assert_eq!(req.address_type, 1);
        assert_eq!(req.port, 443);
        match req.destination {
            Destination::Ipv4Addr(ip) => assert_eq!(ip, Ipv4Addr::new(93, 184, 216, 34)),
            other => panic!("expected ipv4 destination, got {other}"),
        }
    }

    #[test]
    fn parses_domain_connect() {
        let domain = b"example.com";
        let mut data = vec![0x05, 0x01, 0x00, 0x03, domain.len() as u8];
        data.extend_from_slice(domain);
        data.extend_from_slice(&[0x00, 0x50]); // port 80

        let req = Request::from_data(&data).unwrap();

        assert_eq!(req.address_type, 3);
        assert_eq!(req.port, 80);
        match req.destination {
            Destination::Domain(name) => assert_eq!(name, "example.com"),
            other => panic!("expected domain destination, got {other}"),
        }
    }

    #[test]
    fn parses_ipv6_connect() {
        let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let mut data = vec![0x05, 0x01, 0x00, 0x04];
        data.extend_from_slice(&ip.octets());
        data.extend_from_slice(&[0x1F, 0x90]); // port 8080

        let req = Request::from_data(&data).unwrap();

        assert_eq!(req.address_type, 4);
        assert_eq!(req.port, 8080);
        match req.destination {
            Destination::Ipv6Addr(parsed) => assert_eq!(parsed, ip),
            other => panic!("expected ipv6 destination, got {other}"),
        }
    }

    #[test]
    fn rejects_unknown_address_type() {
        let data = [0x05, 0x01, 0x00, 0x09, 0, 0, 0, 0, 0, 0];
        assert!(Request::from_data(&data).is_err());
    }

    #[test]
    fn rejects_truncated_header() {
        assert!(Request::from_data(&[0x05, 0x01]).is_err());
    }

    #[test]
    fn rejects_truncated_ipv4() {
        // ATYP IPv4 but only part of the address is present.
        assert!(Request::from_data(&[0x05, 0x01, 0x00, 0x01, 127, 0, 0]).is_err());
    }

    #[test]
    fn rejects_domain_length_overrun() {
        // Claims a 50-byte domain but provides only a couple of bytes.
        assert!(Request::from_data(&[0x05, 0x01, 0x00, 0x03, 50, b'a', b'b']).is_err());
    }
}
