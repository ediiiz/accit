//! Verification listener: `/verify` for Traefik's `ForwardAuth`, liveness and readiness. Nothing else.
//! Traefik passes a non-2xx answer through to the client, so bodies stay generic.

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::health::Readiness;

pub fn router(readiness: Readiness) -> Router {
    Router::new()
        .route("/verify", get(verify))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .fallback(not_found)
        .with_state(readiness)
}

fn uncached(status: StatusCode, body: &'static str) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], (status, body)).into_response()
}

/// Fails closed: unavailable until ready, then deny until the gate decision exists.
async fn verify(State(readiness): State<Readiness>) -> Response {
    if readiness.is_ready() {
        uncached(StatusCode::FORBIDDEN, "Forbidden")
    } else {
        uncached(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
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
