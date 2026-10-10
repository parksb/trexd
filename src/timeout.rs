use async_trait::async_trait;
use pingora::{
    apps::{HttpServerApp, HttpServerOptions, ReusedHttpStream},
    protocols::http::{v2::server::H2Options, ServerSession},
    proxy::HttpProxy,
    server::ShutdownWatch,
};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::watch,
    time::{sleep_until, Instant},
};

use crate::ProxyConfig;

tokio::task_local! {
    static DEADLINE: watch::Sender<Option<Instant>>;
}

pub struct BodyTimeout {
    deadline: watch::Sender<Option<Instant>>,
    duration: Duration,
}

impl BodyTimeout {
    pub fn start(duration: Duration) -> Self {
        let timer = Self {
            deadline: DEADLINE.with(Clone::clone),
            duration,
        };
        timer.progress(false);
        timer
    }

    pub fn progress(&self, complete: bool) {
        self.deadline
            .send_replace((!complete).then(|| Instant::now() + self.duration));
    }
}

pub struct TimedProxy(pub Arc<HttpProxy<ProxyConfig>>);

#[async_trait]
impl HttpServerApp for TimedProxy {
    async fn process_new_http(
        self: &Arc<Self>,
        session: ServerSession,
        shutdown: &ShutdownWatch,
    ) -> Option<ReusedHttpStream> {
        if session.as_http2().is_none() {
            return self.0.process_new_http(session, shutdown).await;
        }

        let (sender, mut receiver) = watch::channel(None);
        DEADLINE
            .scope(sender, async {
                tokio::select! {
                    result = self.0.process_new_http(session, shutdown) => result,
                    _ = expired(&mut receiver) => {
                        log::warn!("HTTP/2 request body read timed out");
                        None
                    }
                }
            })
            .await
    }

    fn h2_options(&self) -> Option<H2Options> {
        self.0.h2_options()
    }

    fn server_options(&self) -> Option<&HttpServerOptions> {
        self.0.server_options()
    }

    async fn http_cleanup(&self) {
        self.0.http_cleanup().await;
    }
}

async fn expired(receiver: &mut watch::Receiver<Option<Instant>>) {
    loop {
        let deadline = *receiver.borrow_and_update();
        if let Some(deadline) = deadline {
            tokio::select! {
                biased;
                _ = receiver.changed() => {},
                _ = sleep_until(deadline) => return,
            }
        } else if receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
