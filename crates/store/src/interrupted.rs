//! A Revoke whose task dies before its write finished fails closed (`docs/spec.md` section 7).

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use crate::{FAIL_CLOSED_MARKER, Grants, Inner, Integrity, Limits, StoreError};

#[tokio::test]
#[allow(clippy::panic, clippy::unwrap_used, reason = "the panic is the injected fault")]
async fn a_revoke_task_that_panics_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.db");
    let limits = Limits {
        grant_duration: Duration::from_hours(7 * 24),
        max_grants: 1_000,
        audit_retention: Duration::from_hours(7 * 24),
    };
    let grants = Grants::open(Some(&path), limits, UNIX_EPOCH).await.unwrap();

    let result = grants.revoke_task(|_: Arc<Inner>| async { panic!("task died mid-revoke") }).await;

    assert!(matches!(result, Err(StoreError::Interrupted)));
    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
    assert!(path.with_file_name(FAIL_CLOSED_MARKER).exists());
}
