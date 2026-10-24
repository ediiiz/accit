//! Fail-closed recovery (`docs/spec.md` section 7): a Revoke that is not durable leaves a marker in
//! the data directory, and accit stays failed closed across restarts until recovered.

mod common;

use common::{Database, address, limits, t0};
use store::{FAIL_CLOSED_MARKER, Integrity};

async fn failed_revoke(database: &Database) -> store::Grants {
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    assert!(grants.unlock(client, t0()).await.is_ok());
    database.fail("DELETE", "grants").await;
    assert!(grants.revoke(client, t0()).await.is_err());

    grants
}

#[tokio::test]
async fn a_fresh_store_is_sound() {
    let grants = Database::new().open(limits(), t0()).await;

    assert_eq!(*grants.integrity().borrow(), Integrity::Sound);
}

#[tokio::test]
async fn a_failed_revoke_write_fails_closed() {
    let grants = failed_revoke(&Database::new()).await;

    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
}

#[tokio::test]
async fn after_a_failed_revoke_a_restart_stays_failed_closed() {
    let database = Database::new();
    drop(failed_revoke(&database).await);

    let reopened = database.open(limits(), t0()).await;

    assert_eq!(*reopened.integrity().borrow(), Integrity::FailedClosed);
}

#[tokio::test]
async fn an_unwritable_marker_is_unrecoverable() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    database.fail("DELETE", "grants").await;
    // A directory in its place makes the marker unwritable, even for root.
    std::fs::create_dir(database.path.with_file_name(FAIL_CLOSED_MARKER)).unwrap();

    assert!(grants.revoke(client, t0()).await.is_err());

    assert_eq!(*grants.integrity().borrow(), Integrity::Unrecoverable);
}
