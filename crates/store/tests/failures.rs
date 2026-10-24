//! Failed writes (`docs/spec.md` section 7): an Unlock counts only once committed; a Revoke takes
//! effect in memory even when its write fails.

mod common;

use common::{DAY, Database, WEEK, address, limits, t0};
use store::UnlockError;

#[tokio::test]
async fn a_failed_unlock_commit_reports_failure_and_grants_nothing() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    database.fail("INSERT", "audit_events").await;

    let result = grants.unlock(client, t0()).await;

    assert!(matches!(result, Err(UnlockError::Store(_))), "{result:?}");
    assert_eq!(grants.active_until(client, t0()), None);
    drop(grants);
    assert_eq!(database.open(limits(), t0()).await.active_until(client, t0()), None);
}

#[tokio::test]
async fn a_failed_renewal_keeps_the_previous_expiry() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    database.fail("INSERT", "audit_events").await;

    let result = grants.unlock(client, t0() + DAY).await;

    assert!(result.is_err());
    assert_eq!(grants.active_until(client, t0() + DAY), Some(t0() + WEEK));
}

#[tokio::test]
async fn a_failed_revoke_write_still_removes_the_grant_from_memory() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    database.fail("DELETE", "grants").await;

    let result = grants.revoke(client, t0()).await;

    assert!(result.is_err());
    assert_eq!(grants.active_until(client, t0()), None);
}
