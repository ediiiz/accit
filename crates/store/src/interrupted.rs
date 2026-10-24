//! A Revoke whose task dies before its write finished fails closed (`docs/spec.md` section 7), even
//! with no caller left to see it.

#![allow(clippy::unwrap_used, reason = "a failure here is a test bug")]

use std::future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use access::Address;
use tokio::sync::oneshot;

use crate::{FAIL_CLOSED_MARKER, Grants, Integrity, Limits};

fn limits() -> Limits {
    Limits {
        grant_duration: Duration::from_hours(7 * 24),
        max_grants: 1_000,
        audit_retention: Duration::from_hours(7 * 24),
    }
}

fn address() -> Address {
    "192.0.2.1".parse().unwrap()
}

fn marker(database: &Path) -> PathBuf {
    database.with_file_name(FAIL_CLOSED_MARKER)
}

#[test]
fn a_revoke_cut_off_by_runtime_shutdown_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.db");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    let grants = runtime.block_on(async {
        let grants = Grants::open(Some(&path), limits(), UNIX_EPOCH).await.unwrap();
        let (reached, writing) = oneshot::channel();
        let inner = Arc::clone(&grants.0);
        tokio::spawn(async move {
            inner
                .revoke_by(address(), async move |_| {
                    let _ = reached.send(());
                    future::pending().await
                })
                .await
        });
        writing.await.unwrap();

        grants
    });
    drop(runtime);

    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
    assert!(marker(&path).exists());
}

#[tokio::test]
#[allow(clippy::panic, reason = "the panic is the injected fault")]
async fn a_revoke_task_that_panics_with_no_caller_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.db");
    let grants = Grants::open(Some(&path), limits(), UNIX_EPOCH).await.unwrap();
    let mut integrity = grants.integrity();

    let inner = Arc::clone(&grants.0);
    drop(tokio::spawn(
        async move { inner.revoke_by(address(), async |_| panic!("task died mid-revoke")).await },
    ));

    let failed =
        tokio::time::timeout(Duration::from_secs(5), integrity.wait_for(|state| *state != Integrity::Sound));
    assert_eq!(*failed.await.unwrap().unwrap(), Integrity::FailedClosed);
    assert!(marker(&path).exists());
}
