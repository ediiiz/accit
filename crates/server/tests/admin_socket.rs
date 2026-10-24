//! Admin socket (docs/spec.md section 11): owner-only, and `reload` runs the same code as SIGHUP.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use axum::http::StatusCode;
use server::admin;
use server::verify::{self, Verifier};
use store::Grants;

#[tokio::test]
async fn the_socket_is_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("admin.sock");

    let _listener = admin::bind(&path).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();

    assert!(metadata.file_type().is_socket());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn a_stale_socket_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("admin.sock");
    drop(admin::bind(&path).unwrap());

    let _listener = admin::bind(&path).unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn a_staging_directory_left_by_a_crash_does_not_block_the_bind() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("admin.sock");
    std::fs::create_dir(dir.path().join(".admin.sock.staging")).unwrap();

    let _listener = admin::bind(&path).unwrap();

    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn a_file_that_is_not_a_socket_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.db");
    std::fs::write(&path, "grants").unwrap();

    assert!(admin::bind(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "grants");
}

const TOKEN: &str = "s3cr3t-proxy-token-value";
const SELECTED: &str = "  mode: selected\n  hosts: [jellyfin.example.com]\n";
const ALL: &str = "  mode: all\n";

struct Daemon {
    dir: tempfile::TempDir,
    verifier: Verifier,
    grants: Grants,
}

impl Daemon {
    async fn start(protection: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("token"), TOKEN).unwrap();
        let config = common::config(&yaml(dir.path(), protection));
        let grants = common::grants().await;
        let daemon = Self { verifier: Verifier::new(&config, grants.clone()), grants, dir };
        daemon.write(protection);
        let listener = admin::bind(&daemon.socket()).unwrap();
        tokio::spawn(admin::serve(listener, daemon.verifier.clone(), daemon.dir.path().join("accit.yaml")));

        daemon
    }

    fn socket(&self) -> PathBuf {
        self.dir.path().join("admin.sock")
    }

    fn write(&self, protection: &str) {
        std::fs::write(self.dir.path().join("accit.yaml"), yaml(self.dir.path(), protection)).unwrap();
    }

    async fn status(&self, host: &str) -> StatusCode {
        self.verify_as("81.2.69.160", host).await
    }

    async fn verify_as(&self, address: &str, host: &str) -> StatusCode {
        let app = verify::router(common::ready(), common::production(), self.verifier.clone());
        let headers =
            [(common::TOKEN_HEADER, common::TOKEN), ("x-forwarded-for", address), ("x-forwarded-host", host)];

        common::send(app, Some(common::PROXY), "/verify", &headers).await.status
    }
}

fn yaml(dir: &Path, protection: &str) -> String {
    format!(
        "profile: production
public_origin: https://access.example.com
trusted_proxy: 172.30.0.2
proxy_token_file: {}
database: /data/accit.db
protection:
{protection}",
        dir.join("token").display()
    )
}

#[tokio::test]
async fn reload_applies_the_policy_through_the_socket() {
    let daemon = Daemon::start(SELECTED).await;
    daemon.write(ALL);

    admin::request(&daemon.socket(), "reload").await.unwrap();

    assert_eq!(daemon.status("photos.example.com").await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_invalid_config_comes_back_as_the_daemons_error_and_changes_nothing() {
    let daemon = Daemon::start(SELECTED).await;
    daemon.write("  mode: everything\n");

    let error = format!("{:#}", admin::request(&daemon.socket(), "reload").await.unwrap_err());

    assert!(error.contains("everything"), "{error}");
    assert_eq!(daemon.status("photos.example.com").await, StatusCode::NO_CONTENT);
    assert_eq!(daemon.status("jellyfin.example.com").await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_reload_to_an_empty_selection_answers_with_the_warning() {
    let daemon = Daemon::start(SELECTED).await;
    daemon.write("  mode: selected\n");

    let output = admin::request(&daemon.socket(), "reload").await.unwrap();

    assert!(output.contains("no host requires a Grant"), "{output}");
}

#[tokio::test]
async fn an_unknown_command_is_an_error() {
    let daemon = Daemon::start(SELECTED).await;

    let error = format!("{:#}", admin::request(&daemon.socket(), "launch").await.unwrap_err());

    assert!(error.contains("unknown command"), "{error}");
}

const GATED: &str = "jellyfin.example.com";
const OPEN: &str = "photos.example.com";

async fn unlock(daemon: &Daemon, address: &str) {
    daemon.grants.unlock(address.parse().unwrap(), SystemTime::now()).await.unwrap();
}

#[tokio::test]
async fn status_reports_grants_verifications_per_minute_and_denials() {
    let daemon = Daemon::start(SELECTED).await;
    unlock(&daemon, "81.2.69.160").await;
    unlock(&daemon, "81.2.69.161").await;
    daemon.verify_as("81.2.69.160", GATED).await;
    daemon.verify_as("81.2.69.160", OPEN).await;
    daemon.verify_as("81.2.69.162", GATED).await;
    daemon.verify_as("2001:db8::1", GATED).await;

    let output = admin::request(&daemon.socket(), "status").await.unwrap();

    assert_eq!(output, "active grants: 2\nverifications per minute: 4\ndenials: 2\n");
}

#[tokio::test]
async fn status_before_any_verification_is_all_zero() {
    let daemon = Daemon::start(SELECTED).await;

    let output = admin::request(&daemon.socket(), "status").await.unwrap();

    assert_eq!(output, "active grants: 0\nverifications per minute: 0\ndenials: 0\n");
}

#[tokio::test]
async fn status_never_shows_an_address() {
    let daemon = Daemon::start(SELECTED).await;
    for address in ["81.2.69.160", "81.2.69.162", "2001:db8::1"] {
        unlock(&daemon, address).await;
        daemon.verify_as(address, GATED).await;
        daemon.verify_as("81.2.69.163", GATED).await;
    }

    let output = admin::request(&daemon.socket(), "status").await.unwrap();

    for fragment in ["81.2.69", "2001", "db8", "::"] {
        assert!(!output.contains(fragment), "{fragment} in {output}");
    }
    let labels: Vec<&str> =
        output.lines().filter_map(|line| line.split_once(':')).map(|(label, _)| label).collect();
    assert_eq!(labels, ["active grants", "verifications per minute", "denials"]);
}
