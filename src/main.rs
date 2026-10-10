use ::http::header;
use async_trait::async_trait;
use log::info;
use pingora::{
    apps::HttpServerOptions,
    http::RequestHeader,
    listeners::tls::TlsSettings,
    prelude::{HttpPeer, Opt, ProxyHttp},
    protocols::http::v2::server::default_h2_options,
    proxy::{http_proxy_service, Session},
    server::Server,
    Error, ErrorType, Result,
};
use std::{collections::HashMap, env, net::IpAddr, sync::Arc, time::Duration};

pub mod app;
pub mod cert;
mod security;

use app::App;
use cert::DynamicCert;
use security::{normalize_host, request_host, validate_request};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct ProxyConfig {
    registry: HashMap<String, App>,
    requests: Arc<Semaphore>,
}

#[derive(Default)]
pub struct RequestContext {
    host: String,
    client_ip: Option<IpAddr>,
    forwarded_host: String,
    permit: Option<OwnedSemaphorePermit>,
}

fn reject(status: u16) -> Box<Error> {
    Error::new(ErrorType::HTTPStatus(status))
}

#[async_trait]
impl ProxyHttp for ProxyConfig {
    type CTX = RequestContext;
    fn new_ctx(&self) -> Self::CTX {
        RequestContext::default()
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let req = session.req_header();
        validate_request(req).map_err(reject)?;

        let host = request_host(req).ok_or_else(|| reject(400))?;
        let app = self.registry.get(&host).ok_or_else(|| reject(404))?;

        let peer = session
            .client_addr()
            .and_then(|addr| addr.as_inet())
            .map(|addr| addr.ip().to_canonical())
            .ok_or_else(|| reject(400))?;

        ctx.client_ip = Some(match &app.client_ip {
            Some(config) => config
                .resolve(peer, &req.headers)
                .map_err(|_| reject(400))?,
            None => peer,
        });

        ctx.forwarded_host = req
            .headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .or_else(|| req.uri.authority().map(|v| v.as_str()))
            .unwrap_or(&host)
            .to_string();

        ctx.host = host;

        ctx.permit = Some(
            self.requests
                .clone()
                .try_acquire_owned()
                .map_err(|_| reject(503))?,
        );

        session.set_read_timeout(Some(Duration::from_secs(app.timeouts.io)));
        session.set_write_timeout(Some(Duration::from_secs(app.timeouts.io)));
        session.set_keepalive(Some(app.timeouts.idle));

        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        self.registry
            .get(&ctx.host)
            .map(|app| Box::new(app.http_peer()))
            .ok_or_else(|| reject(404))
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let ip = ctx.client_ip.ok_or_else(|| reject(400))?.to_string();

        if let Some(config) = self
            .registry
            .get(&ctx.host)
            .and_then(|app| app.client_ip.as_ref())
        {
            upstream_request.remove_header(&config.header);
        }

        for name in [
            "Forwarded",
            "X-Forwarded-For",
            "X-Real-IP",
            "X-Forwarded-Host",
            "X-Forwarded-Proto",
            "X-Forwarded-Port",
        ] {
            upstream_request.remove_header(name);
        }

        upstream_request.insert_header("X-Real-IP", &ip)?;
        upstream_request.insert_header("X-Forwarded-For", &ip)?;
        upstream_request.insert_header("X-Forwarded-Host", &ctx.forwarded_host)?;
        upstream_request.insert_header("X-Forwarded-Proto", "https")?;

        Ok(())
    }

    fn request_summary(&self, _session: &Session, ctx: &Self::CTX) -> String {
        format!("host={} client={:?}", ctx.host, ctx.client_ip)
    }
}

fn load_apps_from_json(path: &str) -> Vec<App> {
    let file = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&file).unwrap()
}

fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "trexd=info,pingora=error");
    }

    pretty_env_logger::init_timed();

    let options = Opt::parse_args();
    let apps_path = env::var("TREXD_APPS").unwrap_or_else(|_| "/etc/trexd/apps.json".into());
    let apps = load_apps_from_json(&apps_path);
    let mut registry = HashMap::new();

    for app in &apps {
        let host = normalize_host(&app.hostname).expect("Invalid configured hostname");
        assert!(
            registry.insert(host, app.clone()).is_none(),
            "Duplicate configured hostname"
        );
    }

    let mut server = Server::new(Some(options)).unwrap();
    server.bootstrap();

    let mut service = http_proxy_service(
        &server.configuration,
        ProxyConfig {
            registry,
            requests: Arc::new(Semaphore::new(128)),
        },
    );

    let proxy = service.app_logic_mut().unwrap();
    let mut options = HttpServerOptions::default();

    options.keepalive_request_limit = Some(1000);
    options.h2_idle_timeout = Some(Duration::from_secs(15));
    proxy.server_options = Some(options);

    let mut h2 = default_h2_options();
    h2.max_header_list_size(16_384).max_concurrent_streams(32);
    proxy.h2_options = Some(h2);

    let dynamic_cert = DynamicCert::new(apps);
    let mut tls_settings = TlsSettings::with_callbacks(Box::new(dynamic_cert)).unwrap();
    tls_settings.enable_h2();

    let listen = env::var("TREXD_LISTEN").unwrap_or_else(|_| "0.0.0.0:443".into());
    service.add_tls_with_settings(&listen, None, tls_settings);
    server.add_service(service);

    info!("Starting...");
    server.run_forever();
}
