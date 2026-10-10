use pingora::prelude::HttpPeer;
use serde::Deserialize;
use std::{net::IpAddr, time::Duration};

use crate::security::ClientIp;

use crate::cert::TlsConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct App {
    pub addr: IpAddr,
    pub port: u16,
    pub hostname: String,
    pub tls: Option<TlsConfig>,
    pub client_ip: Option<ClientIp>,
    #[serde(default)]
    pub timeouts: Timeouts,
}

impl App {
    pub fn http_peer(&self) -> HttpPeer {
        let mut peer = HttpPeer::new(
            (self.addr, self.port),
            false, // Assumes no services use TLS internally.
            self.hostname.clone(),
        );

        peer.options.connection_timeout = Some(Duration::from_secs(self.timeouts.connect));
        peer.options.total_connection_timeout = Some(Duration::from_secs(self.timeouts.connect));
        peer.options.read_timeout = Some(Duration::from_secs(self.timeouts.io));
        peer.options.write_timeout = Some(Duration::from_secs(self.timeouts.io));
        peer.options.idle_timeout = Some(Duration::from_secs(self.timeouts.idle));

        peer
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Timeouts {
    pub connect: u64,
    pub io: u64,
    pub idle: u64,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: 5,
            io: 30,
            idle: 15,
        }
    }
}
