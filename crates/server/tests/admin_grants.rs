//! Grant commands over the admin socket (`docs/spec.md` sections 7 and 11): they go through the
//! daemon's Grant store, so `/verify` sees them at once.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use common::harness::{Harness, address};
use server::admin;
use server::verify::{self, Verifier};
use store::{Action, FAIL_CLOSED_MARKER};
use tempfile::TempDir;

const GATED: &str = "profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: /secrets/proxy-token
database: /data/accit.db
protection:
  mode: all
";
const CLIENT: &str = "81.2.69.160";
const OTHER: &str = "2a00:1450:4001::1";

/// 2030-01-01T00:00:00Z: Grants unlocked then are still active today.
fn unlocked_at() -> SystemTime {
    UNIX_EPOCH + Duration::from_hours(525_960)
}

struct Daemon {
    harness: Harness,
    verifier: Verifier,
    dir: TempDir,
}

impl Daemon {
    /// Serving, with Grants for `CLIENT` and `OTHER`.
    async fn start() -> Self {
        let harness = Harness::start().await;
        let verifier = Verifier::new(&common::config(GATED), harness.grants.clone());
        let dir = tempfile::tempdir().unwrap();
        let listener = admin::bind(&dir.path().join("admin.sock")).unwrap();
        tokio::spawn(admin::serve(listener, verifier.clone(), dir.path().join("accit.yaml")));
        for client in [CLIENT, OTHER] {
            harness.grants.unlock(address(client), unlocked_at()).await.unwrap();
        }

        Self { harness, verifier, dir }
    }

    async fn admin(&self, command: &str) -> anyhow::Result<String> {
        admin::request(&self.dir.path().join("admin.sock"), command).await
    }

    async fn verify(&self, client: &str) -> StatusCode {
        let app = verify::router(self.harness.readiness.clone(), common::production(), self.verifier.clone());
        let headers = [
            (common::TOKEN_HEADER, common::TOKEN),
            ("x-forwarded-for", client),
            ("x-forwarded-host", "photos.example.com"),
        ];

        common::send(app, Some(common::PROXY), "/verify", &headers).await.status
    }

    fn marker(&self) -> PathBuf {
        self.harness.database.with_file_name(FAIL_CLOSED_MARKER)
    }

    /// A Revoke of `CLIENT` whose write fails, as a broken disk would make it; the disk then recovers.
    async fn fail_closed(&self) {
        self.harness.fail("DELETE", "grants").await;
        assert!(self.admin(&format!("revoke {CLIENT}")).await.is_err());
        self.harness.settle().await;
        assert_eq!(self.verify(OTHER).await, StatusCode::SERVICE_UNAVAILABLE);
        self.harness.heal("DELETE", "grants").await;
    }
}

#[tokio::test]
async fn list_shows_each_active_grant_with_its_expiry() {
    let daemon = Daemon::start().await;

    let listed = daemon.admin("list").await.unwrap();

    assert_eq!(
        listed,
        "81.2.69.160 Tue, 08 Jan 2030 00:00:00 GMT\n2a00:1450:4001::1 Tue, 08 Jan 2030 00:00:00 GMT\n"
    );
}

#[tokio::test]
async fn a_revoked_grant_no_longer_passes_verify() {
    let daemon = Daemon::start().await;
    assert_eq!(daemon.verify(CLIENT).await, StatusCode::NO_CONTENT);

    let answer = daemon.admin(&format!("revoke {CLIENT}")).await.unwrap();

    assert_eq!(answer, "revoked 81.2.69.160\n");
    assert_eq!(daemon.verify(CLIENT).await, StatusCode::FORBIDDEN);
    assert_eq!(daemon.verify(OTHER).await, StatusCode::NO_CONTENT);
    let last = daemon.harness.grants.audit_events().await.unwrap().pop().unwrap();
    assert_eq!((last.action, last.address), (Action::Revoke, address(CLIENT)));
}

#[tokio::test]
async fn revoke_takes_the_canonical_address() {
    let daemon = Daemon::start().await;

    daemon.admin("revoke ::ffff:81.2.69.160").await.unwrap();

    assert_eq!(daemon.verify(CLIENT).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_invalid_address_is_rejected_and_revokes_nothing() {
    let daemon = Daemon::start().await;

    for request in
        ["revoke 81.2.69", "revoke 81.2.69.160/32", "revoke", "revoke 81.2.69.160 2a00:1450:4001::1"]
    {
        assert!(daemon.admin(request).await.is_err(), "{request}");
    }

    assert_eq!(daemon.verify(CLIENT).await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn revoke_all_stops_every_grant_passing_verify() {
    let daemon = Daemon::start().await;

    let answer = daemon.admin("revoke-all").await.unwrap();

    assert_eq!(answer, "revoked: 2\n");
    assert_eq!(daemon.verify(CLIENT).await, StatusCode::FORBIDDEN);
    assert_eq!(daemon.verify(OTHER).await, StatusCode::FORBIDDEN);
    assert_eq!(daemon.admin("list").await.unwrap(), "");
}

#[tokio::test]
async fn recover_discarding_grants_makes_accit_ready_without_them() {
    let daemon = Daemon::start().await;
    daemon.fail_closed().await;

    daemon.admin("recover --discard-grants").await.unwrap();
    daemon.harness.settle().await;

    assert!(daemon.harness.readiness.is_ready());
    assert!(!daemon.marker().exists());
    assert_eq!(daemon.verify(CLIENT).await, StatusCode::FORBIDDEN);
    assert_eq!(daemon.verify(OTHER).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn recover_keeping_grants_makes_accit_ready_with_them() {
    let daemon = Daemon::start().await;
    daemon.fail_closed().await;

    daemon.admin("recover --keep-grants").await.unwrap();
    daemon.harness.settle().await;

    assert!(daemon.harness.readiness.is_ready());
    assert!(!daemon.marker().exists());
    assert_eq!(daemon.verify(CLIENT).await, StatusCode::NO_CONTENT);
    assert_eq!(daemon.verify(OTHER).await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn recover_needs_exactly_one_flag() {
    let daemon = Daemon::start().await;
    daemon.fail_closed().await;

    for request in ["recover", "recover --discard-grants --keep-grants", "recover --all"] {
        let error = format!("{:#}", daemon.admin(request).await.unwrap_err());
        assert!(error.contains("--discard-grants"), "{request}: {error}");
    }

    assert!(!daemon.harness.readiness.is_ready());
    assert!(daemon.marker().exists());
}
