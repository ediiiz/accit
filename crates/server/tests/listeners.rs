//! The two listeners: what each one answers, and that the gate fails closed until accit is ready.

mod common;

use axum::http::StatusCode;
use common::{PROXY, TOKEN, TOKEN_HEADER, portal_router, production, send, verify_router};
use server::health::Readiness;
use server::verify;

const FROM_PROXY: [(&str, &str); 1] = [(TOKEN_HEADER, TOKEN)];

#[tokio::test]
async fn liveness_answers_while_not_ready() {
    let app = verify::router(Readiness::default(), production());

    assert_eq!(send(app, Some(PROXY), "/healthz", &FROM_PROXY).await.status, StatusCode::OK);
}

#[tokio::test]
async fn readiness_reflects_state() {
    let readiness = Readiness::default();

    let app = verify::router(readiness.clone(), production());
    let before = send(app, Some(PROXY), "/readyz", &FROM_PROXY).await;
    readiness.set(true);
    let app = verify::router(readiness, production());
    let after = send(app, Some(PROXY), "/readyz", &FROM_PROXY).await;

    assert_eq!(before.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(after.status, StatusCode::OK);
}

#[tokio::test]
async fn verify_is_unavailable_until_ready() {
    let app = verify::router(Readiness::default(), production());
    let call = send(app, Some(PROXY), "/verify", &FROM_PROXY).await;

    assert_eq!(call.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(call.cache.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn verify_denies_by_default_once_ready() {
    let call = send(verify_router(production()), Some(PROXY), "/verify", &FROM_PROXY).await;

    assert_eq!(call.status, StatusCode::FORBIDDEN);
    assert_eq!(call.cache.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn verify_listener_serves_nothing_else() {
    for path in ["/", "/api/status", "/verify/x", "/index.html"] {
        let call = send(verify_router(production()), Some(PROXY), path, &FROM_PROXY).await;

        assert_eq!(call.status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn portal_listener_does_not_answer_verification_paths() {
    let headers = [(TOKEN_HEADER, TOKEN), ("host", common::PORTAL_HOST)];

    for path in ["/verify", "/healthz", "/readyz", "/api/unknown"] {
        let call = send(portal_router(production()).await, Some(PROXY), path, &headers).await;

        assert_eq!(call.status, StatusCode::NOT_FOUND, "{path}");
    }
}
