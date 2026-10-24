//! `/verify` answers every row of the gate table (docs/spec.md section 3) and never mutates.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::time::{Duration, SystemTime};

use access::Address;
use axum::Router;
use axum::http::StatusCode;
use common::{Call, PORTAL_HOST, PROXY, TOKEN, TOKEN_HEADER, config, grants, ready, send, trust};
use server::health::Readiness;
use server::verify::{self, Verifier};
use store::{GrantStatus, Grants};

const GRANTED: &str = "81.2.69.160";
const OTHER: &str = "81.2.69.161";
const GATED: &str = "jellyfin.example.com";
const OPEN: &str = "status.example.com";

const PRODUCTION: &str = "profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
protection:
  mode: all
  excluded_hosts:
    - status.example.com
";

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

async fn granted() -> Grants {
    let grants = grants().await;
    grants.unlock(address(GRANTED), SystemTime::now()).await.unwrap();

    grants
}

fn router(yaml: &str, readiness: Readiness, grants: Grants) -> Router {
    verify::router(readiness, trust(yaml), Verifier::new(&config(yaml), grants))
}

/// From the pinned proxy with its token, plus `forwarded` headers.
async fn verify_call(app: Router, forwarded: &[(&str, &str)]) -> Call {
    let mut headers = vec![(TOKEN_HEADER, TOKEN)];
    headers.extend_from_slice(forwarded);

    send(app, Some(PROXY), "/verify", &headers).await
}

async fn verify_as(address: &str, host: &str) -> Call {
    let app = router(PRODUCTION, ready(), granted().await);

    verify_call(app, &[("x-forwarded-for", address), ("x-forwarded-host", host)]).await
}

fn assert_passes(call: &Call, context: &str) {
    assert_eq!(call.status, StatusCode::NO_CONTENT, "{context}");
    assert_eq!(call.body, "", "{context}");
    assert_eq!(call.cache.as_deref(), Some("no-store"), "{context}");
}

fn assert_forbidden(call: &Call, context: &str) {
    assert_eq!(call.status, StatusCode::FORBIDDEN, "{context}");
    assert_eq!(call.body, "Forbidden", "{context}");
    assert_eq!(call.cache.as_deref(), Some("no-store"), "{context}");
}

#[tokio::test]
async fn step_1_not_ready_is_503_even_for_the_portal_host() {
    let app = router(PRODUCTION, Readiness::default(), granted().await);
    let call = verify_call(app, &[("x-forwarded-for", GRANTED), ("x-forwarded-host", PORTAL_HOST)]).await;

    assert_eq!(call.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(call.body, "");
    assert_eq!(call.cache.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn step_2_a_request_not_from_the_proxy_is_forbidden_even_for_the_portal_host() {
    let forwarded = [("x-forwarded-for", GRANTED), ("x-forwarded-host", PORTAL_HOST)];
    let with_token = [(TOKEN_HEADER, TOKEN), forwarded[0], forwarded[1]];
    let grants = granted().await;
    let app = || router(PRODUCTION, ready(), grants.clone());

    assert_forbidden(&send(app(), Some(PROXY), "/verify", &forwarded).await, "no token");
    assert_forbidden(&send(app(), Some("172.30.0.3"), "/verify", &with_token).await, "other peer");
}

#[tokio::test]
async fn step_3_an_unusable_forwarded_for_is_forbidden() {
    let cases: [&[(&str, &str)]; 5] = [
        &[("x-forwarded-host", PORTAL_HOST)],
        &[("x-forwarded-for", GRANTED), ("x-forwarded-for", GRANTED), ("x-forwarded-host", PORTAL_HOST)],
        &[("x-forwarded-for", "81.2.69.160, 81.2.69.161"), ("x-forwarded-host", PORTAL_HOST)],
        &[("x-forwarded-for", "unknown"), ("x-forwarded-host", PORTAL_HOST)],
        &[("x-forwarded-for", "81.2.69.160:51234"), ("x-forwarded-host", PORTAL_HOST)],
    ];

    for forwarded in cases {
        let app = router(PRODUCTION, ready(), granted().await);

        assert_forbidden(&verify_call(app, forwarded).await, &format!("{forwarded:?}"));
    }
}

#[tokio::test]
async fn step_3_an_invalid_forwarded_host_is_forbidden_even_with_a_grant() {
    for host in
        ["", "*.example.com", "81.2.69.1", "user@jellyfin.example.com", "a.example.com, b.example.com"]
    {
        assert_forbidden(&verify_as(GRANTED, host).await, host);
    }

    let app = router(PRODUCTION, ready(), granted().await);
    let missing = verify_call(app, &[("x-forwarded-for", GRANTED)]).await;
    let app = router(PRODUCTION, ready(), granted().await);
    let twice = [("x-forwarded-for", GRANTED), ("x-forwarded-host", OPEN), ("x-forwarded-host", OPEN)];

    assert_forbidden(&missing, "missing host");
    assert_forbidden(&verify_call(app, &twice).await, "repeated host");
}

#[tokio::test]
async fn step_3_production_forbids_a_non_public_address_on_every_host() {
    for host in [PORTAL_HOST, OPEN, GATED] {
        for private in ["192.168.1.20", "10.1.2.3", "fd00::1", "127.0.0.1"] {
            assert_forbidden(&verify_as(private, host).await, &format!("{private} {host}"));
        }
    }
}

#[tokio::test]
async fn the_test_profile_lets_a_private_address_through() {
    let yaml = PRODUCTION.replace("profile: production", "profile: test");
    let grants = grants().await;
    grants.unlock(address("192.168.1.20"), SystemTime::now()).await.unwrap();
    let app = router(&yaml, ready(), grants);
    let call = verify_call(app, &[("x-forwarded-for", "192.168.1.20"), ("x-forwarded-host", GATED)]).await;

    assert_passes(&call, "private address with a grant");
}

#[tokio::test]
async fn step_4_the_portal_host_passes_without_a_grant() {
    assert_passes(&verify_as(OTHER, PORTAL_HOST).await, "portal");
    assert_passes(&verify_as(OTHER, "ACCESS.example.com:443").await, "portal, other spelling");
}

#[tokio::test]
async fn step_6_an_open_host_passes_without_a_grant() {
    assert_passes(&verify_as(OTHER, OPEN).await, OPEN);
}

#[tokio::test]
async fn step_7_a_gated_host_passes_with_a_grant() {
    assert_passes(&verify_as(GRANTED, GATED).await, GATED);
    assert_passes(&verify_as(GRANTED, "newly-seen.example.com").await, "newly seen host, all mode");
}

#[tokio::test]
async fn step_8_a_gated_host_without_a_grant_is_forbidden() {
    assert_forbidden(&verify_as(OTHER, GATED).await, GATED);
    assert_forbidden(&verify_as(OTHER, "newly-seen.example.com").await, "newly seen host, all mode");
}

#[tokio::test]
async fn step_8_an_expired_grant_is_forbidden() {
    let grants = grants().await;
    let eight_days_ago = SystemTime::now() - Duration::from_hours(8 * 24);
    grants.unlock(address(GRANTED), eight_days_ago).await.unwrap();
    let app = router(PRODUCTION, ready(), grants);
    let call = verify_call(app, &[("x-forwarded-for", GRANTED), ("x-forwarded-host", GATED)]).await;

    assert_forbidden(&call, "expired");
}

#[tokio::test]
async fn selected_mode_opens_newly_seen_hosts() {
    let yaml = PRODUCTION.replace("  mode: all\n  excluded_hosts:\n", "  mode: selected\n  hosts:\n");
    let yaml = yaml.replace("    - status.example.com", "    - jellyfin.example.com");
    let call = |host| {
        let yaml = yaml.clone();
        async move {
            let app = router(&yaml, ready(), grants().await);
            verify_call(app, &[("x-forwarded-for", OTHER), ("x-forwarded-host", host)]).await
        }
    };

    assert_passes(&call("newly-seen.example.com").await, "newly seen host, selected mode");
    assert_forbidden(&call(GATED).await, GATED);
}

#[tokio::test]
async fn verification_never_mutates_or_renews_a_grant() {
    let grants = grants().await;
    let unlocked_at = SystemTime::now() - Duration::from_hours(24);
    let expiry = grants.unlock(address(GRANTED), unlocked_at).await.unwrap();
    let app = router(PRODUCTION, ready(), grants.clone());

    for host in [GATED, OPEN, PORTAL_HOST] {
        let call =
            verify_call(app.clone(), &[("x-forwarded-for", GRANTED), ("x-forwarded-host", host)]).await;
        assert_passes(&call, host);
    }

    let status = grants.status(address(GRANTED), SystemTime::now()).await.unwrap();

    assert_eq!(status, GrantStatus::Active(expiry));
    assert_eq!(grants.audit_events().await.unwrap().len(), 1);
}
