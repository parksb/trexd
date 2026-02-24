use async_trait::async_trait;
use log::{info, warn};
use pingora::{
    listeners::TlsAccept,
    tls::{
        pkey::{PKey, Private},
        ssl::{NameType, SslRef},
        x509::X509,
    },
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use crate::app::App;

pub struct TlsMaterial {
    leaf: X509,
    chain: Vec<X509>,
    key: PKey<Private>,
}

pub struct DynamicCert {
    registry: HashMap<String, Arc<TlsMaterial>>,
}

impl DynamicCert {
    pub fn new(configs: Vec<App>) -> Self {
        let mut registry = HashMap::new();

        for app in configs.into_iter().filter(|a| a.tls.is_some()) {
            let tls = app.tls.unwrap();

            info!("Loading TLS for host={}", app.hostname);

            let cert_bytes = std::fs::read(&tls.cert_path).expect("read fullchain failed");

            let mut certs = X509::stack_from_pem(&cert_bytes).expect("parse fullchain failed");

            assert!(
                !certs.is_empty(),
                "fullchain must contain at least one cert"
            );

            let leaf = certs.remove(0);
            let chain = certs;

            let key_bytes = std::fs::read(&tls.key_path).expect("read privkey failed");

            let key = PKey::private_key_from_pem(&key_bytes).expect("parse key failed");

            registry.insert(
                app.hostname.to_ascii_lowercase(),
                Arc::new(TlsMaterial { leaf, chain, key }),
            );
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
                warn!("Handshake without SNI");
                return;
            }
        };

        let Some(material) = self.registry.get(&sni) else {
            warn!("No TLS cert for {}", sni);
            return;
        };

        if let Err(e) = ssl.set_certificate(&material.leaf) {
            warn!("set_certificate failed: {}", e);
            return;
        }

        if let Err(e) = ssl.set_private_key(&material.key) {
            warn!("set_private_key failed: {}", e);
            return;
        }

        for cert in &material.chain {
            if let Err(e) = ssl.add_chain_cert(cert.clone()) {
                warn!("add_chain_cert failed: {}", e);
                return;
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}
