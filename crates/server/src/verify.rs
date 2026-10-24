//! Verification listener: `/verify` for Traefik's `ForwardAuth`, liveness and readiness. Nothing else.
//! Traefik passes a non-2xx answer through to the client, so bodies stay generic.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
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
use crate::trust::{self, Digested, ProxyTrust};

const GATES_NOTHING: &str = "protection: selected mode without hosts; no host requires a Grant";

/// What a reload may not change: the gate and the proxy trust were built from it at startup.
#[derive(Debug)]
struct Fixed {
    profile: Profile,
    portal_host: Option<Hostname>,
    trusted_proxy: Option<IpAddr>,
    proxy_token_file: Option<PathBuf>,
    proxy_token: Option<Digested>,
}

impl Fixed {
    fn of(config: &Config) -> Self {
        Self {
            profile: config.profile,
            portal_host: config.portal_host.clone(),
            trusted_proxy: config.trusted_proxy,
            proxy_token_file: config.proxy_token_file.clone(),
            proxy_token: config.proxy_token.as_ref().map(trust::digest_token),
        }
    }

    fn changed_keys(&self, new: &Self) -> Vec<&'static str> {
        [
            ("profile", self.profile != new.profile),
            ("public_origin", self.portal_host != new.portal_host),
            ("trusted_proxy", self.trusted_proxy != new.trusted_proxy),
            (
                "proxy_token_file",
                self.proxy_token_file != new.proxy_token_file || self.proxy_token != new.proxy_token,
            ),
        ]
        .into_iter()
        .filter_map(|(key, changed)| changed.then_some(key))
        .collect()
    }
}

#[derive(Debug)]
struct Rules {
    /// Replaced whole on reload, so each decision sees one Policy.
    policy: RwLock<Arc<Policy>>,
    fixed: Fixed,
    grants: Grants,
}

/// The gate decision's inputs: the Policy, the Portal host, the profile and the Grants.
#[derive(Clone, Debug)]
pub struct Verifier(Arc<Rules>);

impl Verifier {
    pub fn new(config: &Config, grants: Grants) -> Self {
        Self(Arc::new(Rules {
            policy: RwLock::new(Arc::new(config.protection.clone())),
            fixed: Fixed::of(config),
            grants,
        }))
    }

    /// docs/spec.md section 4: an empty selection is allowed but reported loudly.
    pub fn log_startup(&self) {
        self.log_warning();
    }

    fn log_warning(&self) -> Option<&'static str> {
        let warning = self.policy().gates_nothing().then_some(GATES_NOTHING);

        if let Some(warning) = warning {
            tracing::warn!("{warning}");
        }

        warning
    }

    /// Applies the `protection` of the config at `path` and returns its warning, also logged.
    /// An invalid file changes nothing.
    pub fn reload(&self, path: &Path) -> anyhow::Result<Option<&'static str>> {
        let config = Config::load(path)?;
        let changed = self.0.fixed.changed_keys(&Fixed::of(&config));
        // Applying these would need the gate and the proxy trust rebuilt together; a half-applied
        // token rotation would report success while the old token still passes.
        anyhow::ensure!(
            changed.is_empty(),
            "{} changed in {}; restart required",
            changed.join(", "),
            path.display()
        );

        *self.0.policy.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(config.protection);
        tracing::info!("policy reloaded");

        Ok(self.log_warning())
    }

    fn policy(&self) -> Arc<Policy> {
        Arc::clone(&self.0.policy.read().unwrap_or_else(PoisonError::into_inner))
    }

    fn decide(&self, headers: &HeaderMap) -> Verdict {
        let policy = self.policy();
        let gate = Gate {
            policy: &policy,
            portal_host: self.0.fixed.portal_host.as_ref(),
            public_only: self.0.fixed.profile == Profile::Production,
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
        return uncached(StatusCode::SERVICE_UNAVAILABLE, "");
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
