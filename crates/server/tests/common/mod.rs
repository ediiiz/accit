//! Fixtures: routers behind a pinned proxy, and requests from chosen peers.

#![allow(dead_code, reason = "each test binary uses a different subset")]
#![allow(clippy::unwrap_used, reason = "test helpers: a broken fixture is a test bug")]

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use config::Config;
use server::health::Readiness;
use server::trust::ProxyTrust;
use server::{portal, verify};
use store::{Grants, Limits};
use tower::ServiceExt;

pub mod harness;

pub const PROXY: &str = "172.30.0.2";
pub const TOKEN: &str = "s3cr3t-proxy-token-value";
pub const PORTAL_HOST: &str = "access.example.com";
pub const TOKEN_HEADER: &str = "x-accit-proxy-token";

const PRODUCTION: &str = "profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
protection:
  mode: selected
";

pub fn trust(yaml: &str) -> ProxyTrust {
    let config = Config::parse(yaml, |_| Ok(TOKEN.to_owned())).unwrap();

    ProxyTrust::from_config(&config)
}

pub fn production() -> ProxyTrust {
    trust(PRODUCTION)
}

pub fn verify_router(trust: ProxyTrust) -> Router {
    let readiness = Readiness::default();
    readiness.set(true);

    verify::router(readiness, trust)
}

/// A Portal over an empty in-memory store, ready.
pub async fn portal_router(trust: ProxyTrust) -> Router {
    let config = Config::parse(PRODUCTION, |_| Ok(TOKEN.to_owned())).unwrap();
    let limits = Limits {
        grant_duration: config.grant_duration,
        max_grants: config.max_grants,
        audit_retention: config.audit_retention,
    };
    let grants = Grants::open(None, limits, std::time::SystemTime::now()).await.unwrap();
    let readiness = Readiness::default();
    readiness.set(true);

    portal::router(trust, portal::Portal::new(&config, grants, readiness, portal::Clock::system()))
}

pub struct Call {
    pub status: StatusCode,
    pub body: String,
    pub cache: Option<String>,
}

/// A request from `peer` (`None`: no connection info) with extra `headers`.
pub async fn send(app: Router, peer: Option<&str>, path: &str, headers: &[(&str, &str)]) -> Call {
    let mut request = Request::builder().uri(path);

    for (name, value) in headers {
        request = request.header(*name, *value);
    }

    let mut request = request.body(Body::empty()).unwrap();

    if let Some(peer) = peer {
        let address: SocketAddr = format!("{}:40000", bracketed(peer)).parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(address));
    }

    let response = app.oneshot(request).await.unwrap();
    let cache = response.headers().get(header::CACHE_CONTROL).map(|value| value.to_str().unwrap().to_owned());
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 16).await.unwrap();

    Call { status, body: String::from_utf8(bytes.to_vec()).unwrap(), cache }
}

fn bracketed(peer: &str) -> String {
    if peer.contains(':') { format!("[{peer}]") } else { peer.to_owned() }
}
