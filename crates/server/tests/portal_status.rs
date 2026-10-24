//! `GET /api/status` (docs/spec.md section 8): the state of the requesting Address; never mutates.

mod common;

use axum::http::StatusCode;
use common::harness::{COOKIE, DAY, Harness, WEEK, secs, t0};
use serde_json::json;

#[tokio::test]
async fn a_new_address_has_no_grant() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();

    let reply = browser.status().await;
    let body = reply.json();

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(body["state"], "none");
    assert_eq!(body["expiry"], json!(null));
    assert_eq!(body["server_time"], secs(t0()));
    assert_eq!(body["address"], "93.184.216.34");
    assert_eq!(body["family"], "ipv4");
    assert_eq!(body["title"], "Door");
    assert_eq!(body["default_language"], "de");
    assert_eq!(body["apps"], json!([{ "name": "Jellyfin", "url": "https://jellyfin.example.com" }]));
    assert!(body["csrf_token"].as_str().is_some_and(|token| token.len() >= 32));
}

#[tokio::test]
async fn defaults_apply_when_the_config_sets_none() {
    let harness = Harness::with(
        "profile: test
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
protection:
  mode: selected
",
    )
    .await;
    let body = harness.browser().status().await.json();

    assert_eq!(body["title"], "Access");
    assert_eq!(body["default_language"], "en");
    assert_eq!(body["apps"], json!([]));
}

#[tokio::test]
async fn an_ipv6_address_reports_its_family() {
    let harness = Harness::start().await;
    let body = harness.browser_at("2606:4700::1111").status().await.json();

    assert_eq!(body["address"], "2606:4700::1111");
    assert_eq!(body["family"], "ipv6");
}

#[tokio::test]
async fn an_ipv4_mapped_address_is_the_ipv4_address() {
    let harness = Harness::start().await;
    let body = harness.browser_at("::ffff:93.184.216.34").status().await.json();

    assert_eq!(body["address"], "93.184.216.34");
    assert_eq!(body["family"], "ipv4");
}

#[tokio::test]
async fn an_active_grant_reports_its_expiry() {
    let harness = Harness::start().await;
    harness.grants.unlock("93.184.216.34".parse().unwrap(), t0()).await.unwrap();
    harness.advance(DAY);

    let body = harness.browser().status().await.json();

    assert_eq!(body["state"], "active");
    assert_eq!(body["expiry"], secs(t0() + WEEK));
    assert_eq!(body["server_time"], secs(t0() + DAY));
}

#[tokio::test]
async fn an_ended_grant_is_expired_and_keeps_its_expiry() {
    let harness = Harness::start().await;
    harness.grants.unlock("93.184.216.34".parse().unwrap(), t0()).await.unwrap();
    harness.advance(WEEK + DAY);

    let body = harness.browser().status().await.json();

    assert_eq!(body["state"], "expired");
    assert_eq!(body["expiry"], secs(t0() + WEEK));
}

#[tokio::test]
async fn another_addresss_grant_does_not_show() {
    let harness = Harness::start().await;
    harness.grants.unlock("93.184.216.35".parse().unwrap(), t0()).await.unwrap();

    assert_eq!(harness.browser().status().await.json()["state"], "none");
}

#[tokio::test]
async fn status_never_mutates() {
    let harness = Harness::start().await;
    let client = "93.184.216.34".parse().unwrap();
    let mut browser = harness.browser();

    browser.status().await;
    browser.status().await;

    assert_eq!(harness.grants.active_until(client, t0()), None);
    assert!(harness.grants.audit_events().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_session_cookie_is_host_only_and_not_readable_by_scripts() {
    let harness = Harness::start().await;
    let reply = harness.browser().status().await;
    let cookie = reply.header("set-cookie").unwrap();
    let attributes: Vec<&str> = cookie.split("; ").collect();

    assert!(attributes[0].starts_with(&format!("{COOKIE}=")), "{cookie}");
    assert!(attributes[0].len() >= COOKIE.len() + 1 + 32, "{cookie}");
    assert_eq!(attributes[1..], ["Path=/", "Max-Age=900", "Secure", "HttpOnly", "SameSite=Strict"]);
}

#[tokio::test]
async fn a_live_session_keeps_its_cookie_and_token() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();

    let first = browser.status().await;
    let second = browser.status().await;

    assert!(first.header("set-cookie").is_some());
    assert!(second.header("set-cookie").is_none());
    assert_eq!(first.json()["csrf_token"], second.json()["csrf_token"]);
}

#[tokio::test]
async fn every_session_has_its_own_token() {
    let harness = Harness::start().await;
    let first = harness.browser().status().await.json()["csrf_token"].clone();
    let second = harness.browser().status().await.json()["csrf_token"].clone();

    assert_ne!(first, second);
}

#[tokio::test]
async fn a_session_ends_after_fifteen_minutes() {
    let harness = Harness::start().await;
    let mut browser = harness.browser();
    let first = browser.status().await.json()["csrf_token"].clone();

    harness.advance(std::time::Duration::from_mins(14));
    assert_eq!(browser.status().await.json()["csrf_token"], first);
    harness.advance(std::time::Duration::from_mins(1));
    let after = browser.status().await;

    assert!(after.header("set-cookie").is_some());
    assert_ne!(after.json()["csrf_token"], first);
}

#[tokio::test]
async fn at_most_ten_thousand_sessions_exist_and_the_oldest_goes_first() {
    let harness = Harness::start().await;
    let mut oldest = harness.browser();
    let mut newest = harness.browser();
    let token = oldest.status().await.json()["csrf_token"].clone();

    for _ in 0..9_998 {
        harness.browser().status().await;
    }

    newest.status().await;
    assert_eq!(oldest.status().await.json()["csrf_token"], token, "10,000 sessions fit");

    harness.browser().status().await;
    assert_ne!(oldest.status().await.json()["csrf_token"], token, "the 10,001st evicts the oldest");
}
