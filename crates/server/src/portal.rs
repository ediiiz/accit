//! Portal listener: the embedded web app and `/api/*` (docs/spec.md section 8). Verification paths
//! are not served here.

mod api;
mod clock;
mod error;
mod guard;
mod headers;
mod limiter;
mod session;

use std::sync::Arc;

use access::Address;
use axum::Router;
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware;
use axum::routing::{any, get, post};
use config::{Config, Language, Profile};
use serde::Serialize;
use store::Grants;

pub use clock::Clock;

use crate::health::Readiness;
use crate::static_site;
use crate::trust::{self, ProxyTrust};
use error::ApiError;
use limiter::Limiter;
use session::Sessions;

#[derive(Debug, Serialize)]
struct App {
    name: String,
    url: String,
}

#[derive(Debug)]
struct Inner {
    grants: Grants,
    readiness: Readiness,
    clock: Clock,
    /// The only `Origin` a mutation may carry; without it, every mutation is refused.
    origin: Option<String>,
    production: bool,
    title: String,
    default_language: &'static str,
    apps: Vec<App>,
    sessions: Sessions,
    limiter: Limiter,
}

/// What the Portal API needs from the process. Cheap to clone; all clones share one state.
#[derive(Clone, Debug)]
pub struct Portal(Arc<Inner>);

impl Portal {
    pub fn new(config: &Config, grants: Grants, readiness: Readiness, clock: Clock) -> Self {
        Self(Arc::new(Inner {
            grants,
            readiness,
            clock,
            origin: config.public_origin.clone(),
            production: config.profile == Profile::Production,
            title: config.title.clone(),
            default_language: match config.default_language {
                Language::En => "en",
                Language::De => "de",
            },
            apps: config
                .apps
                .iter()
                .map(|app| App { name: app.name.clone(), url: app.url.clone() })
                .collect(),
            sessions: Sessions::default(),
            limiter: Limiter::default(),
        }))
    }

    /// Fails closed: not ready is 503, and so is any doubt about the Address, 403.
    fn requester(&self, headers: &HeaderMap) -> Result<Address, ApiError> {
        if !self.0.readiness.is_ready() {
            return Err(ApiError::Unavailable);
        }

        let values = headers.get_all("x-forwarded-for").iter().map(HeaderValue::as_bytes);
        let address = Address::from_forwarded_for(values).map_err(|_| ApiError::Forbidden)?;

        if self.0.production && !address.is_public() {
            return Err(ApiError::Forbidden);
        }

        Ok(address)
    }
}

pub fn router(trust: ProxyTrust, portal: Portal) -> Router {
    Router::new()
        .route("/api/status", get(api::status))
        .route("/api/unlock", post(api::unlock))
        .route("/api/revoke", post(api::revoke))
        .route("/api/{*rest}", any(error::not_found))
        .fallback(static_site::serve)
        .with_state(portal)
        .layer(middleware::from_fn_with_state(trust, trust::portal_gate))
        .layer(middleware::from_fn(headers::secure))
}
