//! Verification listener: `/verify` for Traefik's `ForwardAuth`, liveness and readiness. Nothing else.
//! Traefik passes a non-2xx answer through to the client, so bodies stay generic.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::health::Readiness;
use crate::trust::{self, ProxyTrust};

pub fn router(readiness: Readiness, trust: ProxyTrust) -> Router {
    Router::new()
        .route("/verify", get(verify))
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

/// Denies until the gate decision exists.
async fn verify() -> Response {
    uncached(StatusCode::FORBIDDEN, "Forbidden")
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
