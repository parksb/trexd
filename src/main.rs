use ::http::header;
use async_trait::async_trait;
use log::{info, warn};
use pingora::{
    listeners::tls::TlsSettings,
    prelude::{HttpPeer, Opt, ProxyHttp},
    proxy::{http_proxy_service, Session},
    server::Server,
    Error, ErrorType, Result,
};
use std::{collections::HashMap, env};

pub mod app;
pub mod cert;

use app::App;
use cert::DynamicCert;

pub struct ProxyConfig {
    registry: HashMap<String, App>,
}

#[async_trait]
impl ProxyHttp for ProxyConfig {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let uri = &session.req_header().uri;
        let host = if let Some(host) = session.get_header(header::HOST) {
            host.to_str().unwrap()
        } else {
            uri.host().unwrap()
        };

        let client_addr = session
            .client_addr()
            .map(|addr| addr.to_string())
            .unwrap_or("unknown".to_string());

        info!("[{host}] Request from {client_addr} for {uri}");

        match self.registry.get(host) {
            Some(app) => Ok(Box::new(app.http_peer())),
            None => {
                warn!("No app found for host: {}, responding with 404", host);
                Err(Error::new(ErrorType::HTTPStatus(404)))
            }
        }
    }
}

fn load_apps_from_json(path: &str) -> Vec<App> {
    let file = std::fs::read_to_string(path).unwrap();
    let reader = std::io::BufReader::new(file.as_bytes());
    serde_json::from_reader(reader).unwrap()
}

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trexd=debug,pingora=error");
        info!("RUST_LOG not set, using default value: trexd=debug,pingora=error");
    }

    if env::var("TREXD_APPS").is_err() {
        env::set_var("TREXD_APPS", "/etc/trexd/apps.json");
        info!("TREXD_APPS not set, using default path: /etc/trexd/apps.json");
    }

    pretty_env_logger::init_timed();

    let apps_path = env::var("TREXD_APPS").unwrap();
    let apps = load_apps_from_json(&apps_path);

    let mut server = Server::new(Some(Opt::parse_args())).unwrap();
    server.bootstrap();

    let mut service = http_proxy_service(
        &server.configuration,
        ProxyConfig {
            registry: apps
                .iter()
                .map(|app| app.entry())
                .collect::<HashMap<String, App>>(),
        },
    );

    let dynamic_cert = DynamicCert::new(apps);
    let mut tls_settings = TlsSettings::with_callbacks(Box::new(dynamic_cert)).unwrap();
    tls_settings.enable_h2();
    service.add_tls_with_settings("0.0.0.0:443", None, tls_settings);
    server.add_service(service);

    info!("Starting...");
    server.run_forever();
}
