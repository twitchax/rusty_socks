use std::{str::FromStr, ffi::OsStr};
use serde::Deserialize;
use toml::from_str;

use crate::helpers::{Res, Helpers};

#[derive(Deserialize)]
struct OptionalConfig {
    listen_interface: Option<String>,
    endpoint_interface: Option<String>,
    port: Option<u16>,
    buffer_size: Option<usize>,
    read_timeout: Option<u64>,
    accept_cidr: Option<String>
}

pub struct Config {
    pub listen_ip: String,
    pub endpoint_ip: String,
    pub port: u16,
    pub buffer_size: usize,
    pub read_timeout: u64,
    pub accept_cidr: String
}

pub async fn from_file_and_env(file: Option<&str>) -> Res<Config> {
    let config: Option<OptionalConfig> = if let Some(f) = file {
        let config_file_data = tokio::fs::read(f).await?;
        let config_text = std::str::from_utf8(&config_file_data)?;

        Some(from_str::<OptionalConfig>(config_text)?)
    } else {
        None
    };

    let mut listen_interface: Option<String> = None;
    let mut endpoint_interface: Option<String> = None;
    let mut port = 1080u16;
    let mut buffer_size = 2048usize;
    let mut read_timeout = 5000u64;
    let mut accept_cidr = "0.0.0.0/0".to_owned();

    // Compute the config values: file > env > default.
    if let Some(c) = config {
        listen_interface = c.listen_interface.or_else(|| std::env::var("RS_LISTEN_INTERFACE").ok());
        endpoint_interface = c.endpoint_interface.or_else(|| std::env::var("RS_ENDPOINT_INTERFACE").ok());
        port = c.port.unwrap_or_else(|| get_env_or("RS_PORT", port));
        buffer_size = c.buffer_size.unwrap_or_else(|| get_env_or("RS_BUFFER_SIZE", buffer_size));
        read_timeout = c.read_timeout.unwrap_or_else(|| get_env_or("RS_READ_TIMEOUT", read_timeout));
        accept_cidr = c.accept_cidr.unwrap_or_else(|| get_env_or("RS_ACCEPT_CIDR", accept_cidr));
    }

    let listen_ip = match &listen_interface {
        Some(i) => Helpers::get_interface_ip(i)?.to_string(),
        None => "0.0.0.0".to_owned()
    };

    let endpoint_ip = match &endpoint_interface {
        Some(i) => Helpers::get_interface_ip(i)?.to_string(),
        None => "0.0.0.0".to_owned()
    };

    Ok(Config { 
        listen_ip,
        endpoint_ip,
        port,
        buffer_size,
        read_timeout,
        accept_cidr
    })
}

fn get_env_or<S: AsRef<OsStr>, T: FromStr>(s: S, d: T) -> T {
    match std::env::var(s) {
        Ok(s) => match s.parse() {
            Ok(v) => v,
            _ => d
        },
        _ => d
    }
}