//! Policy reload (docs/spec.md section 4): applied whole, an invalid file keeps the previous
//! Policy, and Grants survive any change of mode or lists.

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

fn production(token_file: &Path, protection: &str) -> String {
    format!(
        "profile: production
public_origin: https://access.example.com
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
        let dir = tempfile::tempdir().unwrap();
        let (file, token) = (dir.path().join("accit.yaml"), dir.path().join("proxy-token"));
        std::fs::write(&token, TOKEN).unwrap();
        let yaml = production(&token, protection);
        std::fs::write(&file, &yaml).unwrap();
        let verifier = Verifier::new(&config(&yaml), grants);

        Self { _dir: dir, file, token, verifier, yaml }
    }

    fn write(&self, protection: &str) {
        std::fs::write(&self.file, production(&self.token, protection)).unwrap();
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
