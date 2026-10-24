//! Verification listener: `/verify` for Traefik's `ForwardAuth`, liveness and readiness. Nothing else.
//! Traefik passes a non-2xx answer through to the client, so bodies stay generic.

use std::path::Path;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::SystemTime;

use access::{Gate, Hostname, Policy, Verdict};
use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use config::{Config, Profile};
use store::Grants;

use crate::health::Readiness;
use crate::trust::{self, ProxyTrust};

#[derive(Debug)]
struct Rules {
    /// Replaced whole on reload, so each decision sees one Policy.
    policy: RwLock<Arc<Policy>>,
    portal_host: Option<Hostname>,
    public_only: bool,
    grants: Grants,
}

/// The gate decision's inputs: the Policy, the Portal host, the profile and the Grants.
#[derive(Clone, Debug)]
pub struct Verifier(Arc<Rules>);

impl Verifier {
    pub fn new(config: &Config, grants: Grants) -> Self {
        Self(Arc::new(Rules {
            policy: RwLock::new(Arc::new(config.protection.clone())),
            portal_host: config.portal_host.clone(),
            public_only: config.profile == Profile::Production,
            grants,
        }))
    }

    /// Docs/spec.md section 4: an empty selection is allowed but reported loudly.
    pub fn log_startup(&self) {
        if self.policy().gates_nothing() {
            tracing::warn!("protection: selected mode without hosts; the Policy gates nothing");
        }
    }

    /// Applies the `protection` of the config at `path`. An invalid file changes nothing.
    pub fn reload(&self, path: &Path) -> anyhow::Result<()> {
        let policy = Config::load(path)?.protection;

        *self.0.policy.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(policy);
        tracing::info!("policy reloaded");

        Ok(())
    }

    fn policy(&self) -> Arc<Policy> {
        Arc::clone(&self.0.policy.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn decide(&self, headers: &HeaderMap) -> Verdict {
        let policy = self.policy();
        let gate = Gate {
            policy: &policy,
            portal_host: self.0.portal_host.as_ref(),
            public_only: self.0.public_only,
        };
        let values = |name| headers.get_all(name).into_iter().map(HeaderValue::as_bytes);

        gate.decide(values("x-forwarded-for"), values("x-forwarded-host"), |address| {
            self.0.grants.active_until(address, SystemTime::now()).is_some()
        })
    }
}

pub fn router(readiness: Readiness, trust: ProxyTrust, verifier: Verifier) -> Router {
    Router::new()
        .route("/verify", get(verify).with_state(verifier))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(trust, trust::verify_gate))
        .layer(middleware::from_fn_with_state(readiness.clone(), unready_first))
        .with_state(readiness)
}

fn uncached(status: StatusCode, body: &'static str) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], (status, body)).into_response()
}

/// Spec section 3 step 1: not ready is 503 before the caller is even looked at.
async fn unready_first(State(readiness): State<Readiness>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/verify" && !readiness.is_ready() {
        return uncached(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable");
    }

    next.run(request).await
}

/// Steps 3 to 8; steps 1 and 2 are the layers above.
async fn verify(State(verifier): State<Verifier>, headers: HeaderMap) -> Response {
    match verifier.decide(&headers) {
        Verdict::Pass => uncached(StatusCode::NO_CONTENT, ""),
        Verdict::Deny => uncached(StatusCode::FORBIDDEN, "Forbidden"),
    }
}

async fn healthz() -> Response {
    uncached(StatusCode::OK, "ok")
}

async fn readyz(State(readiness): State<Readiness>) -> Response {
    if readiness.is_ready() {
        uncached(StatusCode::OK, "ready")
    } else {
        uncached(StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}
