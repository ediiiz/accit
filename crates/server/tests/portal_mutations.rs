//! `POST /api/unlock` and `POST /api/revoke` (docs/spec.md sections 6, 7 and 8): they act on the
//! requesting Address only and answer with the committed state.

mod common;

use axum::http::StatusCode;
use common::harness::{DAY, Harness, WEEK, address, secs, t0};
use serde_json::json;
use store::Action;

const CLIENT: &str = "93.184.216.34";
const OTHER: &str = "93.184.216.35";

#[tokio::test]
async fn unlock_grants_the_requesting_address_for_a_full_duration() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();

    let reply = browser.mutate("/api/unlock").await;
    let body = reply.json();

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(body["state"], "active");
    assert_eq!(body["expiry"], secs(t0() + WEEK));
    assert_eq!(body["server_time"], secs(t0()));
    assert_eq!(body["address"], CLIENT);
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), Some(t0() + WEEK));
    assert_eq!(browser.status().await.json()["state"], "active");
}

#[tokio::test]
async fn unlock_touches_no_other_address() {
    let harness = Harness::start().await;
    harness.browser().mutate("/api/unlock").await;

    assert_eq!(harness.grants.active_until(address(OTHER), t0()), None);
    assert_eq!(harness.browser_at(OTHER).status().await.json()["state"], "none");
}

#[tokio::test]
async fn unlock_again_renews_to_a_full_duration_from_now() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.mutate("/api/unlock").await;
    harness.advance(DAY);
    browser.status().await;

    let body = browser.mutate("/api/unlock").await.json();

    assert_eq!(body["expiry"], secs(t0() + DAY + WEEK));
}

#[tokio::test]
async fn a_body_cannot_name_another_address() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.status().await;

    let reply = browser.post("/api/unlock", browser.form(), r#"{"address":"93.184.216.35"}"#).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(harness.grants.active_until(address(OTHER), t0()), None);
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), None);
}

#[tokio::test]
async fn revoke_removes_the_requesting_addresss_grant_only() {
    let harness = Harness::start().await;
    harness.grants.unlock(address(OTHER), t0()).await.unwrap();
    let mut browser = harness.browser();
    browser.mutate("/api/unlock").await;

    let reply = browser.mutate("/api/revoke").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["state"], "none");
    assert_eq!(reply.json()["expiry"], json!(null));
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), None);
    assert_eq!(harness.grants.active_until(address(OTHER), t0()), Some(t0() + WEEK));
    assert!(harness.readiness.is_ready());
}

#[tokio::test]
async fn revoke_without_a_grant_succeeds() {
    let harness = Harness::start().await;
    let reply = harness.browser().mutate("/api/revoke").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["state"], "none");
}

#[tokio::test]
async fn a_mutation_is_audited_with_the_requesting_address() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.mutate("/api/unlock").await;
    browser.mutate("/api/revoke").await;

    let events: Vec<_> = harness
        .grants
        .audit_events()
        .await
        .unwrap()
        .into_iter()
        .map(|event| (event.action, event.address))
        .collect();

    assert_eq!(events, [(Action::Unlock, address(CLIENT)), (Action::Revoke, address(CLIENT))]);
}

#[tokio::test]
async fn a_revoke_whose_write_fails_makes_accit_unready() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.mutate("/api/unlock").await;
    harness.fail("DELETE", "grants").await;

    let reply = browser.mutate("/api/revoke").await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!harness.readiness.is_ready());
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), None);
}

#[tokio::test]
async fn an_unlock_whose_commit_fails_grants_nothing_and_stays_ready() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.status().await;
    harness.fail("INSERT", "audit_events").await;

    let reply = browser.mutate("/api/unlock").await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(harness.readiness.is_ready());
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), None);
}

#[tokio::test]
async fn a_new_address_is_refused_at_the_cap_but_a_renewal_is_not() {
    let harness = Harness::replacing("title: Door", "title: Door\nmax_grants: 1").await;
    let mut holder = harness.browser();
    let mut newcomer = harness.browser_at(OTHER);

    assert_eq!(holder.mutate("/api/unlock").await.status, StatusCode::OK);
    assert_eq!(newcomer.mutate("/api/unlock").await.status, StatusCode::CONFLICT);
    assert_eq!(holder.mutate("/api/unlock").await.status, StatusCode::OK);
}

#[tokio::test]
async fn nothing_answers_while_not_ready() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    browser.status().await;
    harness.readiness.set(false);

    assert_eq!(browser.status().await.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        browser.post("/api/unlock", browser.form(), "{}").await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        browser.post("/api/revoke", browser.form(), "{}").await.status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(harness.grants.active_until(address(CLIENT), t0()), None);
}

#[tokio::test]
async fn five_mutations_a_minute_then_429_with_retry_after() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();

    for _ in 0..5 {
        assert_eq!(browser.mutate("/api/unlock").await.status, StatusCode::OK);
    }

    harness.advance(std::time::Duration::from_secs(20));
    let limited = browser.mutate("/api/revoke").await;

    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.header("retry-after"), Some("40"));
    assert!(
        harness.grants.active_until(address(CLIENT), harness.now()).is_some(),
        "the refused Revoke did nothing"
    );

    harness.advance(std::time::Duration::from_secs(40));

    assert_eq!(browser.mutate("/api/revoke").await.status, StatusCode::OK);
}

#[tokio::test]
async fn the_budget_is_per_address() {
    let harness = Harness::start().await;
    let (mut first, mut second) = (harness.browser(), harness.browser_at(OTHER));

    for _ in 0..5 {
        first.mutate("/api/unlock").await;
    }

    assert_eq!(first.mutate("/api/unlock").await.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(second.mutate("/api/unlock").await.status, StatusCode::OK);
}

#[tokio::test]
async fn status_does_not_spend_the_budget() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();

    for _ in 0..10 {
        assert_eq!(browser.status().await.status, StatusCode::OK);
    }

    assert_eq!(browser.mutate("/api/unlock").await.status, StatusCode::OK);
}
