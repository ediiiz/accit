//! Policy reload (docs/spec.md section 4): applied whole, an invalid file keeps the previous
//! Policy, and Grants survive any change of mode or lists. A changed profile or Portal host needs
//! a restart, so such a file is invalid for reload.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use axum::http::StatusCode;
use common::{PROXY, TOKEN, TOKEN_HEADER, config, grants, ready, send, trust};
use server::verify::{self, Verifier};
use store::Grants;
use tempfile::TempDir;

const GRANTED: &str = "81.2.69.160";
const OTHER: &str = "81.2.69.161";
const PRIVATE: &str = "192.168.1.20";

const PRODUCTION: &str = "profile: production\npublic_origin: https://access.example.com";
const TEST: &str = "profile: test\npublic_origin: https://access.example.com";
const MOVED_PORTAL: &str = "profile: production\npublic_origin: https://portal.example.com";

fn yaml(head: &str, token_file: &Path, protection: &str) -> String {
    format!(
        "{head}
trusted_proxy: 172.30.0.2
proxy_token_file: {}
database: /data/accit.db
protection:
{protection}",
        token_file.display()
    )
}

const SELECTED: &str = "  mode: selected\n  hosts: [jellyfin.example.com]\n";
const ALL: &str = "  mode: all\n  excluded_hosts: [status.example.com]\n";

struct Setup {
    _dir: TempDir,
    file: PathBuf,
    token: PathBuf,
    verifier: Verifier,
    yaml: String,
}

impl Setup {
    fn new(protection: &str, grants: Grants) -> Self {
        Self::with_head(PRODUCTION, protection, grants)
    }

    fn with_head(head: &str, protection: &str, grants: Grants) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (file, token) = (dir.path().join("accit.yaml"), dir.path().join("proxy-token"));
        std::fs::write(&token, TOKEN).unwrap();
        let yaml = yaml(head, &token, protection);
        std::fs::write(&file, &yaml).unwrap();
        let verifier = Verifier::new(&config(&yaml), grants);

        Self { _dir: dir, file, token, verifier, yaml }
    }

    fn write(&self, protection: &str) {
        self.write_with_head(PRODUCTION, protection);
    }

    fn write_with_head(&self, head: &str, protection: &str) {
        std::fs::write(&self.file, yaml(head, &self.token, protection)).unwrap();
    }

    async fn status(&self, address: &str, host: &str) -> StatusCode {
        let app = verify::router(ready(), trust(&self.yaml), self.verifier.clone());
        let headers = [(TOKEN_HEADER, TOKEN), ("x-forwarded-for", address), ("x-forwarded-host", host)];

        send(app, Some(PROXY), "/verify", &headers).await.status
    }
}

async fn granted() -> Grants {
    let grants = grants().await;
    grants.unlock(GRANTED.parse().unwrap(), SystemTime::now()).await.unwrap();

    grants
}

#[tokio::test]
async fn a_reload_applies_the_new_policy() {
    let setup = Setup::new(SELECTED, grants().await);
    assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::NO_CONTENT);

    setup.write(ALL);
    setup.verifier.reload(&setup.file).unwrap();

    assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::FORBIDDEN);
    assert_eq!(setup.status(OTHER, "status.example.com").await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn an_invalid_file_keeps_the_previous_policy_and_returns_the_error() {
    let setup = Setup::new(SELECTED, grants().await);
    let invalid = [
        "  mode: everything\n",
        "  mode: all\n  hosts: [jellyfin.example.com]\n",
        "  mode: selected\n  hosts: [\"*.example.com\"]\n",
        "  mode: selected\n  hosts: [access.example.com]\n",
        "  mode: selected\n  unknown: true\n",
    ];

    for protection in invalid {
        setup.write(protection);

        assert!(setup.verifier.reload(&setup.file).is_err(), "{protection}");
        assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::NO_CONTENT, "{protection}");
        assert_eq!(setup.status(OTHER, "jellyfin.example.com").await, StatusCode::FORBIDDEN, "{protection}");
    }

    std::fs::remove_file(&setup.file).unwrap();

    assert!(setup.verifier.reload(&setup.file).is_err());
    assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn grants_survive_a_mode_switch() {
    let setup = Setup::new(SELECTED, granted().await);
    assert_eq!(setup.status(GRANTED, "jellyfin.example.com").await, StatusCode::NO_CONTENT);

    setup.write(ALL);
    setup.verifier.reload(&setup.file).unwrap();
    assert_eq!(setup.status(GRANTED, "photos.example.com").await, StatusCode::NO_CONTENT);

    setup.write(SELECTED);
    setup.verifier.reload(&setup.file).unwrap();
    assert_eq!(setup.status(GRANTED, "jellyfin.example.com").await, StatusCode::NO_CONTENT);
    assert_eq!(setup.status(OTHER, "jellyfin.example.com").await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_changed_profile_is_refused_and_keeps_the_previous_policy() {
    let setup = Setup::with_head(TEST, SELECTED, grants().await);
    assert_eq!(setup.status(PRIVATE, "photos.example.com").await, StatusCode::NO_CONTENT);

    setup.write_with_head(PRODUCTION, ALL);

    assert!(setup.verifier.reload(&setup.file).is_err());
    assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::NO_CONTENT);
    assert_eq!(setup.status(OTHER, "jellyfin.example.com").await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_changed_portal_host_is_refused_and_keeps_the_previous_policy() {
    let setup = Setup::new(SELECTED, grants().await);

    setup.write_with_head(MOVED_PORTAL, "  mode: selected\n  hosts: [access.example.com]\n");

    assert!(setup.verifier.reload(&setup.file).is_err());
    assert_eq!(setup.status(OTHER, "jellyfin.example.com").await, StatusCode::FORBIDDEN);
    assert_eq!(setup.status(OTHER, "photos.example.com").await, StatusCode::NO_CONTENT);
}
