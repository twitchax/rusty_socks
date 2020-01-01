use std::fmt::Formatter;
use std::fmt::Display;
use std::error::Error;
use rand::{self, Rng};
use rand::distributions::Alphanumeric;

use pnet::datalink;
use std::net::IpAddr;

pub struct Helpers;

impl Helpers {
    pub fn get_id() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(4)
            .collect::<String>()
    }

    pub fn bytes_to_port(data: &[u8]) -> GenericResult<u16> {
        assert!(data.len() == 2, "There must be exactly two (2) bytes for a conversion to a port.");

        Ok(((data[0] as u16) << 8) + (data[1] as u16))
    }

    pub fn port_to_bytes(port: u16) -> (u8, u8) {
        ((port >> 8) as u8, (port & 0xff) as u8)
    }

    pub fn slice_to_u32(data: &[u8]) -> GenericResult<u32> {
        assert!(data.len() == 4, "There must be exactly four (4) bytes for a conversion to an IPv4.");

        Ok(((data[0] as u32) << 24) +
                  ((data[1] as u32) << 16) +
                  ((data[2] as u32) <<  8) +
                   (data[3] as u32))
    }

    pub fn slice_to_u128(data: &[u8]) -> GenericResult<u128> {
        assert!(data.len() == 16, "There must be exactly sixteen (16) bytes for a conversion to an IPv6.");

        Ok(((data[0] as u128) << 120) +
                  ((data[1] as u128) << 112) +
                  ((data[2] as u128) << 104) +
                  ((data[3] as u128) <<  96) +
                  ((data[4] as u128) <<  88) +
                  ((data[5] as u128) <<  80) +
                  ((data[6] as u128) <<  72) +
                  ((data[7] as u128) <<  64) +
                  ((data[8] as u128) <<  56) +
                  ((data[9] as u128) <<  48) +
                 ((data[10] as u128) <<  40) +
                 ((data[11] as u128) <<  32) +
                 ((data[12] as u128) <<  24) +
                 ((data[13] as u128) <<  16) +
                 ((data[14] as u128) <<   8) +
                  (data[15] as u128))
    }

    pub fn get_socks_reply(error: i32) -> u8 {
        match error {
            0 =>                     0x00, // succeeded
            10050 | 10051 =>         0x03, // Network unreachable
            10064 | 11001 | 10065 => 0x04, // Host unreachable
            10061 =>                 0x05, // Connection refused
            10060 =>                 0x06, // TTL expired... [ARoney] Is this right?
            _ =>                     0x01  // general SOCKS server failure
        }
    }

    pub fn write_octets(buffer: &mut [u8], octets: &[u8]) {
        buffer[..octets.len()].clone_from_slice(&octets[..]);
    }

    pub fn get_interface_ip(name: &str) -> GenericResult<IpAddr> {
        for iface in datalink::interfaces() {
            if iface.name == name {
                return Ok(iface.ips[0].ip());
            }
        }

        Err(Box::new(GenericError::from(format!("Could not lookup IP for interface `{}`.", name))))
    }
}

pub type GenericResult<T> = Result<T,Box<dyn Error + 'static>>;

#[derive(Debug)]
pub struct GenericError {
    message: String
}

impl From<&str> for GenericError {
    fn from(message: &str) -> Self {
        GenericError { message: message.to_owned() }
    }
}

impl From<String> for GenericError {
    fn from(message: String) -> Self {
        GenericError { message }
    }
}

impl Display for GenericError {
    fn fmt<'a>(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.message);
    }
}

impl Error for GenericError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}