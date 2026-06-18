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
        let version = data[0];
        let command = data[1];
        let reserved = data[2];
        let address_type = data[3];

        if address_type == 0x01
        /* IPv4 */
        {
            let address = Ipv4Addr::from(Helpers::slice_to_u32(&data[4..8])?);
            let port = Helpers::bytes_to_port(&data[8..10])?;

            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Ipv4Addr(address),
            });
        }

        if address_type == 0x03
        /* Domain Name */
        {
            let name_length = data[4] as usize;
            let name = std::str::from_utf8(&data[5..(5 + name_length)])?.to_owned();
            let port = Helpers::bytes_to_port(&data[(5 + name_length)..(5 + name_length + 2)])?;

            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Domain(name),
            });
        }

        if address_type == 0x04
        /* IPv6 */
        {
            let address = Ipv6Addr::from(Helpers::slice_to_u128(&data[4..20])?);
            let port = Helpers::bytes_to_port(&data[20..22])?;

            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Ipv6Addr(address),
            });
        }

        "Unknown request type, or data corrupt.".into_error()
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
}
