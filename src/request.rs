use std::fmt::Display;
use std::net::{Ipv4Addr, Ipv6Addr};

use crate::helpers::{Helpers, Res, IntoError};

pub struct Request {
    pub version: u8,
    pub command: u8,
    pub reserved: u8,
    pub address_type: u8,
    pub port: u16,
    pub destination: Destination
}

pub enum Destination {
    Ipv4Addr(Ipv4Addr),
    Ipv6Addr(Ipv6Addr),
    Domain(String)
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

        if address_type == 0x01 /* IPv4 */ {
            let address = Ipv4Addr::from(Helpers::slice_to_u32(&data[4..8])?);
            let port = Helpers::bytes_to_port(&data[8..10])?;
            
            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Ipv4Addr(address)
            });
        }

        if address_type == 0x03 /* Domain Name */ {
            let name_length = data[4] as usize;
            let name = std::str::from_utf8(&data[5..(5 + name_length)])?.to_owned();
            let port = Helpers::bytes_to_port(&data[(5 + name_length)..(5 + name_length + 2)])?;
            
            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Domain(name)
            });
        }

        if address_type == 0x04 /* IPv6 */ {
            let address = Ipv6Addr::from(Helpers::slice_to_u128(&data[4..20])?);
            let port = Helpers::bytes_to_port(&data[20..22])?;

            return Ok(Request {
                version,
                command,
                reserved,
                address_type,
                port,
                destination: Destination::Ipv6Addr(address)
            });
        }

        "Unknown request type, or data corrupt.".into_error()
    }
}