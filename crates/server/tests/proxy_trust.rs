//! Both listeners answer only Traefik: its pinned address, the proxy token, and on the Portal
//! its exact host (docs/spec.md sections 3 and 8, ADR 0002).

mod common;

use axum::http::StatusCode;
use common::{
    PORTAL_HOST, PROXY, TOKEN, TOKEN_HEADER, portal_router, production, send, trust, verifier, verify_router,
};
use server::health::Readiness;
use server::verify;

const FROM_PROXY: [(&str, &str); 1] = [(TOKEN_HEADER, TOKEN)];
const PORTAL_REQUEST: [(&str, &str); 2] = [(TOKEN_HEADER, TOKEN), ("host", PORTAL_HOST)];
const TEST_PROFILE: &str = "profile: test\nprotection:\n  mode: selected\n";

/// `/readyz` is 200 once ready, so anything else is the gate saying no.
async fn verify_status(peer: Option<&str>, path: &str, headers: &[(&str, &str)]) -> StatusCode {
    send(verify_router(production()).await, peer, path, headers).await.status
}

/// The Portal answers 404 for unknown paths, so anything else is the gate saying no.
async fn portal_status(peer: Option<&str>, headers: &[(&str, &str)]) -> StatusCode {
    send(portal_router(production()), peer, "/api/unknown", headers).await.status
}

#[tokio::test]
async fn the_pinned_proxy_with_the_token_passes_the_verification_gate() {
    assert_eq!(verify_status(Some(PROXY), "/readyz", &FROM_PROXY).await, StatusCode::OK);
}

#[tokio::test]
async fn the_ipv4_mapped_form_of_the_proxy_address_passes() {
    let peer = "::ffff:172.30.0.2";

    assert_eq!(verify_status(Some(peer), "/readyz", &FROM_PROXY).await, StatusCode::OK);
}

#[tokio::test]
async fn other_peers_are_forbidden_on_every_verification_path() {
    for peer in ["172.30.0.3", "10.0.0.1", "127.0.0.1", "203.0.113.9", "2001:db8::2", "::ffff:172.30.0.3"] {
        for path in ["/verify", "/readyz", "/nope"] {
            let call = send(verify_router(production()).await, Some(peer), path, &FROM_PROXY).await;

            assert_eq!(call.status, StatusCode::FORBIDDEN, "{peer} {path}");
            assert_eq!(call.body, "Forbidden", "{peer} {path}");
            assert_eq!(call.cache.as_deref(), Some("no-store"), "{peer} {path}");
        }
    }
}

#[tokio::test]
async fn a_request_without_connection_info_is_forbidden() {
    assert_eq!(verify_status(None, "/readyz", &FROM_PROXY).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_missing_or_wrong_token_is_forbidden() {
    let wrong = [
        vec![],
        vec![(TOKEN_HEADER, "")],
        vec![(TOKEN_HEADER, "s3cr3t-proxy-token-valuf")],
        vec![(TOKEN_HEADER, "s3cr3t")],
        vec![(TOKEN_HEADER, "s3cr3t-proxy-token-value-and-more")],
        vec![(TOKEN_HEADER, " s3cr3t-proxy-token-value")],
        vec![(TOKEN_HEADER, TOKEN), (TOKEN_HEADER, TOKEN)],
        vec![(TOKEN_HEADER, TOKEN), (TOKEN_HEADER, "other")],
        vec![("x-accit-proxy-token-2", TOKEN)],
    ];

    for headers in wrong {
        assert_eq!(
            verify_status(Some(PROXY), "/readyz", &headers).await,
            StatusCode::FORBIDDEN,
            "{headers:?}"
        );
    }
}

#[tokio::test]
async fn not_ready_answers_503_before_the_peer_is_checked() {
    let app = verify::router(Readiness::default(), production(), verifier().await);
    let call = send(app, Some("203.0.113.9"), "/verify", &[]).await;

    assert_eq!(call.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn the_image_healthcheck_from_loopback_needs_no_token() {
    for peer in ["127.0.0.1", "::1", "127.0.0.2"] {
        assert_eq!(verify_status(Some(peer), "/healthz", &[]).await, StatusCode::OK, "{peer}");
    }
}

#[tokio::test]
async fn the_loopback_exemption_covers_only_healthz() {
    for path in ["/verify", "/readyz", "/healthz/x", "/nope"] {
        assert_eq!(verify_status(Some("127.0.0.1"), path, &[]).await, StatusCode::FORBIDDEN, "{path}");
    }
}

#[tokio::test]
async fn healthz_from_elsewhere_still_needs_the_proxy() {
    assert_eq!(verify_status(Some("172.30.0.3"), "/healthz", &[]).await, StatusCode::FORBIDDEN);
    assert_eq!(verify_status(Some(PROXY), "/healthz", &[]).await, StatusCode::FORBIDDEN);
    assert_eq!(verify_status(Some(PROXY), "/healthz", &FROM_PROXY).await, StatusCode::OK);
}

#[tokio::test]
async fn the_portal_needs_the_proxy_the_token_and_the_exact_host() {
    assert_eq!(portal_status(Some(PROXY), &PORTAL_REQUEST).await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_portal_forbids_other_peers() {
    for peer in ["172.30.0.3", "127.0.0.1", "203.0.113.9"] {
        assert_eq!(portal_status(Some(peer), &PORTAL_REQUEST).await, StatusCode::FORBIDDEN, "{peer}");
    }

    assert_eq!(portal_status(None, &PORTAL_REQUEST).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_portal_forbids_a_missing_or_wrong_token() {
    let host = ("host", PORTAL_HOST);

    assert_eq!(portal_status(Some(PROXY), &[host]).await, StatusCode::FORBIDDEN);
    assert_eq!(portal_status(Some(PROXY), &[host, (TOKEN_HEADER, "wrong")]).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_portal_forbids_any_other_host() {
    let hosts = [
        "",
        "example.com",
        "evil.example.com",
        "access.example.com.evil.com",
        "evil.com.access.example.com",
        "xaccess.example.com",
        "203.0.113.9",
        "access.example.com@evil.com",
        "access.example.com:abc",
        "access.example.com, evil.com",
    ];

    for host in hosts {
        let headers = [(TOKEN_HEADER, TOKEN), ("host", host)];

        assert_eq!(portal_status(Some(PROXY), &headers).await, StatusCode::FORBIDDEN, "{host:?}");
    }
}

#[tokio::test]
async fn the_portal_forbids_a_missing_or_repeated_host() {
    assert_eq!(portal_status(Some(PROXY), &FROM_PROXY).await, StatusCode::FORBIDDEN);

    let twice = [(TOKEN_HEADER, TOKEN), ("host", PORTAL_HOST), ("host", "evil.com")];

    assert_eq!(portal_status(Some(PROXY), &twice).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_portal_host_is_compared_canonically() {
    for host in ["ACCESS.example.com", "access.example.com.", "access.example.com:8443"] {
        let headers = [(TOKEN_HEADER, TOKEN), ("host", host)];

        assert_eq!(portal_status(Some(PROXY), &headers).await, StatusCode::NOT_FOUND, "{host}");
    }
}

#[tokio::test]
async fn the_portal_forbids_every_path_to_other_peers() {
    for path in ["/", "/api/status", "/verify", "/healthz", "/_app/x.js"] {
        let call = send(portal_router(production()), Some("127.0.0.1"), path, &PORTAL_REQUEST).await;

        assert_eq!(call.status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(call.body, "Forbidden", "{path}");
    }
}

#[tokio::test]
async fn the_test_profile_without_a_pinned_proxy_trusts_loopback_only() {
    let app = async || verify_router(trust(TEST_PROFILE)).await;

    assert_eq!(send(app().await, Some("127.0.0.1"), "/readyz", &[]).await.status, StatusCode::OK);
    assert_eq!(send(app().await, Some("::1"), "/readyz", &[]).await.status, StatusCode::OK);
    assert_eq!(send(app().await, Some("172.30.0.2"), "/readyz", &[]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(app().await, None, "/readyz", &[]).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_test_profile_portal_accepts_loopback_without_a_host_to_match() {
    let app = portal_router(trust(TEST_PROFILE));

    assert_eq!(
        send(app, Some("127.0.0.1"), "/api/x", &[("host", "localhost:5173")]).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn the_test_profile_still_enforces_what_is_configured() {
    let pinned = format!("{TEST_PROFILE}trusted_proxy: {PROXY}\nproxy_token_file: /t\n");
    let app = async || verify_router(trust(&pinned)).await;

    assert_eq!(send(app().await, Some("127.0.0.1"), "/readyz", &[]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(app().await, Some(PROXY), "/readyz", &[]).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(app().await, Some(PROXY), "/readyz", &FROM_PROXY).await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_test_profile_portal_enforces_a_configured_host() {
    let configured = format!("{TEST_PROFILE}public_origin: https://{PORTAL_HOST}\n");
    let app = || portal_router(trust(&configured));

    assert_eq!(
        send(app(), Some("127.0.0.1"), "/api/x", &[("host", "evil.com")]).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(app(), Some("127.0.0.1"), "/api/x", &[("host", PORTAL_HOST)]).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn the_portal_host_may_come_from_the_request_authority() {
    let headers = [(TOKEN_HEADER, TOKEN)];
    let own = format!("https://{PORTAL_HOST}/api/x");

    assert_eq!(
        send(portal_router(production()), Some(PROXY), &own, &headers).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(portal_router(production()), Some(PROXY), "https://evil.com/api/x", &headers).await.status,
        StatusCode::FORBIDDEN
    );
}
