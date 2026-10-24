//! Portal listener: the embedded web app and `/api/*`. Verification paths are not served here.

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::any;

use crate::static_site;
use crate::trust::{self, ProxyTrust};

pub fn router(trust: ProxyTrust) -> Router {
    Router::new()
        .route("/api/{*rest}", any(not_found))
        .fallback(static_site::serve)
        .layer(middleware::from_fn_with_state(trust, trust::portal_gate))
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}
