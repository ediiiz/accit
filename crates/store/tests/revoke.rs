//! Revoke (`docs/spec.md` section 6): removes the Grant of one Address, durably.

mod common;

use common::{DAY, Database, HOUR, address, limits, t0};

#[tokio::test]
async fn revoke_removes_only_that_grant() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let (client, other) = (address("203.0.113.7"), address("198.51.100.1"));
    grants.unlock(client, t0()).await.unwrap();
    let kept = grants.unlock(other, t0()).await.unwrap();

    grants.revoke(client, t0() + HOUR).await.unwrap();

    assert_eq!(grants.active_until(client, t0() + HOUR), None);
    assert_eq!(grants.active_until(other, t0() + HOUR), Some(kept));
}

#[tokio::test]
async fn a_revoked_grant_stays_gone_after_restart() {
    let database = Database::new();
    let client = address("203.0.113.7");
    let grants = database.open(limits(), t0()).await;
    grants.unlock(client, t0()).await.unwrap();
    grants.revoke(client, t0() + HOUR).await.unwrap();
    drop(grants);

    let reopened = database.open(limits(), t0() + DAY).await;

    assert_eq!(reopened.active_until(client, t0() + DAY), None);
}
