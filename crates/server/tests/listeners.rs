//! The two listeners: what each one answers, and that the gate fails closed until accit is ready.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use server::health::Readiness;
use server::{portal, verify};
use tower::ServiceExt;

#[allow(clippy::unwrap_used, reason = "test helper: a broken request is a test bug")]
async fn call(app: Router, method: Method, path: &str) -> (StatusCode, Option<String>) {
    let request = Request::builder().method(method).uri(path).body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let cache = response.headers().get(header::CACHE_CONTROL).map(|value| value.to_str().unwrap().to_owned());

    (response.status(), cache)
}

#[tokio::test]
async fn liveness_answers_while_not_ready() {
    let (status, _) = call(verify::router(Readiness::default()), Method::GET, "/healthz").await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn readiness_reflects_state() {
    let readiness = Readiness::default();

    let (before, _) = call(verify::router(readiness.clone()), Method::GET, "/readyz").await;
    readiness.set(true);
    let (after, _) = call(verify::router(readiness), Method::GET, "/readyz").await;

    assert_eq!(before, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(after, StatusCode::OK);
}

#[tokio::test]
async fn verify_is_unavailable_until_ready() {
    let (status, cache) = call(verify::router(Readiness::default()), Method::GET, "/verify").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(cache.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn verify_denies_by_default_once_ready() {
    let readiness = Readiness::default();
    readiness.set(true);

    let (status, cache) = call(verify::router(readiness), Method::GET, "/verify").await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(cache.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn verify_listener_serves_nothing_else() {
    for path in ["/", "/api/status", "/verify/x", "/index.html"] {
        let (status, _) = call(verify::router(Readiness::default()), Method::GET, path).await;

        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn portal_listener_does_not_answer_verification_paths() {
    for path in ["/verify", "/healthz", "/readyz", "/api/unknown"] {
        let (status, _) = call(portal::router(), Method::GET, path).await;

        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}
