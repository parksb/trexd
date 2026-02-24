use async_trait::async_trait;
use log::{info, warn};
use pingora::tls::ext;
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
    registry: HashMap<String, (Vec<X509>, PKey<Private>)>,
}

impl DynamicCert {
    pub fn new(configs: Vec<App>) -> Self {
        let mut registry = HashMap::new();

        for app in configs.into_iter().filter(|a| a.tls.is_some()) {
            let tls = app.tls.unwrap();

            info!(
                "Loading TLS key and cert from {}, {}",
                tls.cert_path, tls.key_path
            );

            let cert_bytes =
                std::fs::read(&tls.cert_path).expect("Failed to read certificate file");

            let cert_chain =
                X509::stack_from_pem(&cert_bytes).expect("Failed to parse fullchain.pem");

            if cert_chain.is_empty() {
                panic!("Certificate chain is empty");
            }

            let key_bytes = std::fs::read(&tls.key_path).expect("Failed to read private key file");

            let key = PKey::private_key_from_pem(&key_bytes).expect("Failed to parse private key");

            registry.insert(app.hostname.to_ascii_lowercase(), (cert_chain, key));
        }

        Self { registry }
    }
}

#[async_trait]
impl TlsAccept for DynamicCert {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        let sni = match ssl.servername(NameType::HOST_NAME) {
            Some(name) => name.to_ascii_lowercase(),
            None => {
                warn!("TLS handshake without SNI");
                return;
            }
        };

        if let Some((cert_chain, key)) = self.registry.get(&sni) {
            ext::ssl_use_certificate(ssl, &cert_chain[0]).expect("Failed to set leaf certificate");

            ext::ssl_use_private_key(ssl, key).expect("Failed to set private key");

            for cert in cert_chain.iter().skip(1) {
                ext::ssl_add_chain_cert(ssl, cert).expect("Failed to add intermediate cert");
            }
        } else {
            warn!("No certificate found for SNI: {}", sni);
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}
