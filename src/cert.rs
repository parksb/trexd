use async_trait::async_trait;
use log::info;
use pingora::tls::pkey::{PKey, Private};
use pingora::tls::x509::X509;
use pingora::{
    listeners::TlsAccept,
    tls::ssl::{NameType, SslRef},
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::app::App;

pub struct DynamicCert {
    registry: HashMap<String, (X509, PKey<Private>)>,
}

impl DynamicCert {
    pub fn new(configs: Vec<App>) -> Self {
        Self {
            registry: configs
                .into_iter()
                .filter(|app| app.tls.is_some())
                .map(|app| {
                    let tls = app.tls.unwrap();
                    info!(
                        "Loading TLS key and cert from {}, {}",
                        tls.cert_path, tls.key_path
                    );

                    let cert_bytes = std::fs::read(tls.cert_path).unwrap();
                    let cert = X509::from_pem(&cert_bytes).unwrap();

                    let key_bytes = std::fs::read(tls.key_path).unwrap();
                    let key = PKey::private_key_from_pem(&key_bytes).unwrap();

                    (app.hostname, (cert, key))
                })
                .collect::<HashMap<String, (X509, PKey<Private>)>>(),
        }
    }
}

#[async_trait]
impl TlsAccept for DynamicCert {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        use pingora::tls::ext;

        let sni = ssl.servername(NameType::HOST_NAME).unwrap();
        if let Some((cert, key)) = self.registry.get(sni) {
            ext::ssl_use_certificate(ssl, cert).unwrap();
            ext::ssl_use_private_key(ssl, key).unwrap();
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}
