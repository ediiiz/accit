//! Traefik is the only caller (ADR 0002): its pinned address, the proxy token and, on the Portal,
//! the exact Portal host. Anything else is `Forbidden`; so is any doubt.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use access::Hostname;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use config::{Config, Profile, ProxyToken};
use subtle::ConstantTimeEq;

const TOKEN_HEADER: &str = "x-accit-proxy-token";

#[derive(Debug)]
struct Rules {
    /// `None`: no proxy is pinned, which only the `test` profile allows; loopback stands in.
    proxy: Option<IpAddr>,
    token: Option<ProxyToken>,
    portal_host: Option<Hostname>,
    /// Production without the keys it requires. Config loading rejects that; this is the backstop.
    locked: bool,
}

#[derive(Clone, Debug)]
pub struct ProxyTrust(Arc<Rules>);

impl ProxyTrust {
    pub fn from_config(config: &Config) -> Self {
        let production = config.profile == Profile::Production;
        let complete =
            config.trusted_proxy.is_some() && config.proxy_token.is_some() && config.portal_host.is_some();

        Self(Arc::new(Rules {
            proxy: config.trusted_proxy,
            token: config.proxy_token.clone(),
            portal_host: config.portal_host.clone(),
            locked: production && !complete,
        }))
    }

    /// Without a pinned proxy, a loopback peer is trusted. For local development only.
    pub fn log_startup(&self) {
        if !self.0.locked && self.0.proxy.is_none() {
            tracing::warn!("no trusted_proxy configured: trusting loopback peers (profile test)");
        }
    }

    fn peer_is_proxy(&self, peer: IpAddr) -> bool {
        match self.0.proxy {
            Some(proxy) => peer == proxy,
            None => peer.is_loopback(),
        }
    }

    fn token_matches(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = &self.0.token else {
            return true;
        };
        let mut values = headers.get_all(TOKEN_HEADER).iter();

        match (values.next(), values.next()) {
            (Some(sent), None) => sent.as_bytes().ct_eq(expected.expose().as_bytes()).into(),
            _ => false,
        }
    }

    fn host_matches(&self, request: &Request) -> bool {
        let Some(expected) = &self.0.portal_host else {
            return true;
        };
        let mut values = request.headers().get_all(header::HOST).iter();
        let sent = match (values.next(), values.next()) {
            (Some(value), None) => value.as_bytes(),
            (None, None) => {
                request.uri().authority().map_or(&[][..], |authority| authority.as_str().as_bytes())
            }
            _ => return false,
        };

        Hostname::from_forwarded_host(sent).is_ok_and(|host| host == *expected)
    }

    fn admits(&self, peer: Option<IpAddr>, headers: &HeaderMap) -> bool {
        !self.0.locked && peer.is_some_and(|peer| self.peer_is_proxy(peer)) && self.token_matches(headers)
    }
}

fn peer(request: &Request) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| address.ip().to_canonical())
}

fn forbidden() -> Response {
    ([(header::CACHE_CONTROL, "no-store")], (StatusCode::FORBIDDEN, "Forbidden")).into_response()
}

/// Verification listener: `/healthz` from loopback answers without the token (the image healthcheck).
pub async fn verify_gate(State(trust): State<ProxyTrust>, request: Request, next: Next) -> Response {
    let peer = peer(&request);
    let healthcheck = request.uri().path() == "/healthz" && peer.is_some_and(|peer| peer.is_loopback());

    if healthcheck || trust.admits(peer, request.headers()) { next.run(request).await } else { forbidden() }
}

/// Portal listener: additionally the exact Portal host.
pub async fn portal_gate(State(trust): State<ProxyTrust>, request: Request, next: Next) -> Response {
    if trust.admits(peer(&request), request.headers()) && trust.host_matches(&request) {
        next.run(request).await
    } else {
        forbidden()
    }
}
