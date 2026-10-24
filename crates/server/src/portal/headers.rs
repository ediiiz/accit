//! Headers on every Portal response, the Traefik-level refusals included.

use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

const CSP: &str = "default-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; \
                   frame-ancestors 'none'; form-action 'self'";

pub(super) async fn secure(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    // Hashed assets and the other files set their own policy; everything else is never stored.
    headers.entry(header::CACHE_CONTROL).or_insert(HeaderValue::from_static("no-store"));

    response
}
