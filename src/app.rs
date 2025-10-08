use pingora::prelude::HttpPeer;
use serde::Deserialize;
use std::net::IpAddr;

use crate::cert::TlsConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct App {
    #[serde(deserialize_with = "deserialize_addr")]
    pub addr: IpAddr,
    pub port: u16,
    pub hostname: String,
    pub tls: Option<TlsConfig>,
}

impl App {
    pub fn http_peer(&self) -> HttpPeer {
        HttpPeer::new(
            (self.addr, self.port),
            false, // Assumes no services use TLS internally.
            self.hostname.clone(),
        )
    }

    pub fn entry(&self) -> (String, Self) {
        (self.hostname.clone(), self.clone())
    }
}

fn deserialize_addr<'de, D>(deserializer: D) -> Result<IpAddr, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let p = s
        .split('.')
        .map(|s| s.parse::<u8>())
        .collect::<Result<Vec<u8>, _>>()
        .map_err(serde::de::Error::custom)?;
    Ok(IpAddr::V4(std::net::Ipv4Addr::new(p[0], p[1], p[2], p[3])))
}
