use std::fmt::Formatter;
use std::fmt::Display;
use std::error::Error;
use rand::{self, Rng};
use rand::distributions::Alphanumeric;

use pnet::datalink;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub enum Cidr {
    V4(u32, u32),
    V6(u128, u128)
}

impl Cidr {
    pub fn is_trivial(&self) -> bool {
        match self {
            Cidr::V4(_, mask) => *mask == 0,
            Cidr::V6(_, mask) => *mask == 0
        }
    }
}

pub struct Helpers;

impl Helpers {
    pub fn get_id() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(4)
            .collect::<String>()
    }

    pub fn bytes_to_port(data: &[u8]) -> Res<u16> {
        if data.len() != 2 {
            return "There must be exactly two (2) bytes for a conversion to a port.".into_error();
        }

        Ok(((data[0] as u16) << 8) + (data[1] as u16))
    }

    pub fn port_to_bytes(port: u16) -> (u8, u8) {
        ((port >> 8) as u8, (port & 0xff) as u8)
    }

    pub fn slice_to_u32(data: &[u8]) -> Res<u32> {
        if data.len() != 4 {
            return "There must be exactly four (4) bytes for a conversion to an IPv4.".into_error();
        }

        Ok(((data[0] as u32) << 24) +
                  ((data[1] as u32) << 16) +
                  ((data[2] as u32) <<  8) +
                   (data[3] as u32))
    }

    pub fn slice_to_u128(data: &[u8]) -> Res<u128> {
        if data.len() != 16 {
            return "There must be exactly sixteen (16) bytes for a conversion to an IPv6.".into_error();
        }

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

    pub fn get_interface_ip(name: &str) -> Res<IpAddr> {
        for iface in datalink::interfaces() {
            if iface.name == name {
                if iface.ips.is_empty() {
                    return format!("Found interface `{}`, but could not find an assigned IP for that interface.", name).into_error();
                }   

                return Ok(iface.ips[0].ip());
            }
        }

        format!("Could not lookup IP for interface `{}`.", name).into_error()
    }

    pub fn mask_ipv4(ip: &Ipv4Addr, mask: u32) -> Res<u32> {
        Ok(Helpers::slice_to_u32(&ip.octets())? & mask) 
    }

    pub fn mask_ipv6(ip: &Ipv6Addr, mask: u128) -> Res<u128> {
        Ok(Helpers::slice_to_u128(&ip.octets())? & mask) 
    }

    pub fn is_ip_in_cidr(ip_addr: &IpAddr, cidr: &Cidr) -> Res<bool> {
        match cidr {
            Cidr::V4(prefix, mask) => {
                match &ip_addr {
                    IpAddr::V4(ip) => Ok(Helpers::mask_ipv4(ip, *mask)? == *prefix),
                    _ => Err(Box::new(GenericError::from("Cannot check IPv6 addresses against IPv4 CIDRs.")))
                }
            },
            Cidr::V6(prefix, mask) => {
                match &ip_addr {
                    IpAddr::V6(ip) => Ok(Helpers::mask_ipv6(ip, *mask)? == *prefix),
                    _ => Err(Box::new(GenericError::from("Cannot check IPv4 addresses against IPv6 CIDRs."))),
                }
            }
        }
    }

    pub fn parse_cidr(s: &str) -> Res<Cidr> {
        let splits = s.split('/').collect::<Vec<&str>>();

        let ip_addr = splits[0].parse::<IpAddr>()?;
        let num_mask_bits = splits[1].parse::<u32>()?;

        match ip_addr {
            IpAddr::V4(ip) => {
                if num_mask_bits > 32 {
                    return Err(Box::new(GenericError::from("An IPv4 CIDR prefix must have a mask bit length less than or equal to 32.")));
                }

                let mask = !(2u32.overflowing_pow(32 - num_mask_bits).0.overflowing_sub(1).0);
                let prefix = Helpers::slice_to_u32(&ip.octets())? & mask;

                Ok(Cidr::V4(prefix, mask))
            },
            IpAddr::V6(ip) => {
                if num_mask_bits > 128 {
                    return Err(Box::new(GenericError::from("An IPv4 CIDR prefix must have a mask bit length less than or equal to 128.")));
                }

                let mask = !(2u128.overflowing_pow(128 - num_mask_bits).0.overflowing_sub(1).0);
                let prefix = Helpers::slice_to_u128(&ip.octets())? & mask;

                Ok(Cidr::V6(prefix, mask))
            }
        }
    }
}

pub type Void = Result<(), Box<dyn std::error::Error>>;
pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

pub trait IntoError<T> {
    fn into_error(self) -> Res<T>;
}

impl<T, S> IntoError<T> for S 
    where S: AsRef<str> + ToString
{
    fn into_error(self) -> Res<T> {
        Err(Box::new(GenericError::from(self)))
    }
}

#[derive(Debug)]
pub struct GenericError {
    message: String
}

impl<T> From<T> for GenericError 
    where T: AsRef<str> + ToString 
{
    fn from(message: T) -> Self {
        GenericError { message: message.to_string() }
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