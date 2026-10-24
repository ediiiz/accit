//! Trusted networks end to end (docs/spec.md sections 3, 5, 6 and 8): the gate, the Portal API in
//! production, and reload.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use axum::http::StatusCode;
use common::harness::Harness;
use common::{PROXY, TOKEN, TOKEN_HEADER, config, grants, ready, send, trust};
use serde_json::json;
use server::portal::{self, Clock, Portal};
use server::verify::{self, Verifier};

const HOME: &str = "192.168.1.20";
const ROUTER: &str = "192.168.1.1";
const GATED: &str = "jellyfin.example.com";
const OPEN: &str = "photos.example.com";

fn yaml(profile: &str, protection: &str) -> String {
    format!(
        "profile: {profile}
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
docker_networks: [172.30.0.0/24]
protection:
{protection}  trusted_networks: [192.168.1.0/24, fd12::/16]
  trusted_networks_exclude: [192.168.1.1]
"
    )
}

const SELECTED: &str = "  mode: selected\n  hosts: [jellyfin.example.com]\n";
const ALL: &str = "  mode: all\n  excluded_hosts: [status.example.com]\n";

async fn verify_status(yaml: &str, address: &str, host: &str) -> StatusCode {
    let app = verify::router(ready(), trust(yaml), Verifier::new(&config(yaml), grants().await));
    let headers = [(TOKEN_HEADER, TOKEN), ("x-forwarded-for", address), ("x-forwarded-host", host)];

    send(app, Some(PROXY), "/verify", &headers).await.status
}

#[tokio::test]
async fn step_5_a_trusted_address_passes_every_host_without_a_grant_in_both_modes() {
    let hosts = [GATED, OPEN, "status.example.com", "access.example.com"];

    for (protection, profile) in
        [(SELECTED, "production"), (ALL, "production"), (SELECTED, "test"), (ALL, "test")]
    {
        let yaml = yaml(profile, protection);

        for (address, host) in
            [HOME, "fd12::20"].into_iter().flat_map(|address| hosts.map(|host| (address, host)))
        {
            let status = verify_status(&yaml, address, host).await;

            assert_eq!(status, StatusCode::NO_CONTENT, "{profile} {address} {host}");
        }
    }
}

#[tokio::test]
async fn step_5_an_excluded_address_gets_no_pass() {
    let production = yaml("production", ALL);
    let test = yaml("test", ALL);

    assert_eq!(verify_status(&production, ROUTER, GATED).await, StatusCode::FORBIDDEN);
    assert_eq!(verify_status(&test, ROUTER, GATED).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn step_5_other_private_addresses_stay_rejected_in_production() {
    let yaml = yaml("production", SELECTED);

    for address in ["192.168.2.20", "10.0.0.5", "fd00::1"] {
        assert_eq!(verify_status(&yaml, address, OPEN).await, StatusCode::FORBIDDEN, "{address}");
    }
}

#[tokio::test]
async fn step_5_a_public_ipv6_source_inside_the_home_still_needs_a_grant() {
    let yaml = yaml("production", ALL);

    assert_eq!(verify_status(&yaml, "2a01:4f8::1", GATED).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn status_reports_trusted_to_a_private_home_address_in_production() {
    let harness = Harness::with(&yaml("production", SELECTED)).await;
    let mut browser = harness.browser_at(HOME);

    let reply = browser.status().await;
    let body = reply.json();

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(body["state"], "trusted");
    assert_eq!(body["expiry"], json!(null));
    assert_eq!(body["address"], HOME);
    assert_eq!(body["family"], "ipv4");
}

#[tokio::test]
async fn a_private_address_outside_the_home_networks_is_refused_in_production() {
    let harness = Harness::with(&yaml("production", SELECTED)).await;

    for address in [ROUTER, "192.168.2.20", "fd00::1"] {
        let reply = harness.browser_at(address).status().await;

        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{address}");
    }
}

#[tokio::test]
async fn unlock_from_a_trusted_address_is_refused_and_creates_no_grant() {
    let harness = Harness::with(&yaml("production", SELECTED)).await;
    let mut browser = harness.browser_at(HOME);

    let reply = browser.mutate("/api/unlock").await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    assert_eq!(harness.grants.active_until(HOME.parse().unwrap(), harness.now()), None);
    assert!(harness.grants.audit_events().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_public_address_is_unaffected_by_trusted_networks() {
    let harness = Harness::with(&yaml("production", SELECTED)).await;
    let mut browser = harness.browser();

    assert_eq!(browser.status().await.json()["state"], "none");
    assert_eq!(browser.mutate("/api/unlock").await.json()["state"], "active");
}

/// A Verifier and a Portal sharing one rule set, over `yaml`'s first config.
async fn shared(first: &str) -> (Verifier, Portal) {
    let config = config(first);
    let grants = grants().await;
    let verifier = Verifier::new(&config, grants.clone());
    let portal = Portal::new(&config, grants, ready(), Clock::system(), &verifier);

    (verifier, portal)
}

#[tokio::test]
async fn reload_moves_the_gate_and_the_portal_together() {
    let (verifier, portal) = shared(&yaml("production", SELECTED)).await;
    let dir = tempfile::tempdir().unwrap();
    let (file, token) = (dir.path().join("accit.yaml"), dir.path().join("proxy-token"));
    let reloaded = yaml("production", SELECTED)
        .replace("192.168.1.0/24", "10.0.0.0/8")
        .replace("/secrets/proxy-token", token.to_str().unwrap());
    std::fs::write(&token, TOKEN).unwrap();
    std::fs::write(&file, reloaded).unwrap();
    let api = portal::router(trust(&yaml("production", SELECTED)), portal);
    let call = |address: &'static str| async {
        let headers = [(TOKEN_HEADER, TOKEN), ("x-forwarded-for", address), ("host", "access.example.com")];

        send(api.clone(), Some(PROXY), "/api/status", &headers).await.status
    };

    assert_eq!(call(HOME).await, StatusCode::OK);
    assert_eq!(call("10.1.2.3").await, StatusCode::FORBIDDEN);

    verifier.reload(&file).unwrap();

    assert_eq!(call(HOME).await, StatusCode::FORBIDDEN);
    assert_eq!(call("10.1.2.3").await, StatusCode::OK);
}
