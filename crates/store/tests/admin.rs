//! Grant administration (`docs/spec.md` sections 7 and 11): list, revoke every Grant, and recover from
//! the fail-closed state.

#![allow(clippy::unwrap_used, reason = "test fixtures: a failure here is a test bug")]

mod common;

use access::Address;
use common::{DAY, Database, HOUR, WEEK, address, limits, t0};
use store::{Action, FAIL_CLOSED_MARKER, Grants, Integrity, Recovery};

#[tokio::test]
async fn list_shows_active_grants_by_address_with_their_expiry() {
    let grants = Database::new().open(limits(), t0()).await;
    let (first, second, lapsed) = (address("203.0.113.7"), address("198.51.100.1"), address("192.0.2.9"));
    grants.unlock(lapsed, t0() - WEEK).await.unwrap();
    grants.unlock(first, t0()).await.unwrap();
    grants.unlock(second, t0() + DAY).await.unwrap();

    let listed = grants.list(t0() + HOUR);

    assert_eq!(listed, [(second, t0() + DAY + WEEK), (first, t0() + WEEK)]);
}

#[tokio::test]
async fn revoke_all_removes_every_grant_durably_and_audits_each() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let (first, second) = (address("203.0.113.7"), address("198.51.100.1"));
    grants.unlock(first, t0()).await.unwrap();
    grants.unlock(second, t0()).await.unwrap();

    let revoked = grants.revoke_all(t0() + HOUR).await.unwrap();

    assert_eq!(revoked, 2);
    assert_eq!(grants.list(t0() + HOUR), []);
    let events: Vec<_> = grants
        .audit_events()
        .await
        .unwrap()
        .into_iter()
        .filter(|event| event.action == Action::Revoke)
        .map(|event| (event.address, event.at))
        .collect();
    assert_eq!(events.len(), 2);
    assert!(events.contains(&(first, t0() + HOUR)) && events.contains(&(second, t0() + HOUR)));
    drop(grants);
    assert_eq!(database.open(limits(), t0() + DAY).await.list(t0() + DAY), []);
}

#[tokio::test]
async fn a_revoke_all_whose_write_fails_fails_closed() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    database.fail("DELETE", "grants").await;

    assert!(grants.revoke_all(t0()).await.is_err());

    assert_eq!(grants.active_until(client, t0()), None);
    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
    assert!(database.path.with_file_name(FAIL_CLOSED_MARKER).exists());
}

/// A store reopened after a Revoke of `revoked` was not saved; `kept` was never revoked.
async fn failed_closed(database: &Database, revoked: Address, kept: Address) -> Grants {
    let grants = database.open(limits(), t0()).await;
    grants.unlock(revoked, t0()).await.unwrap();
    grants.unlock(kept, t0()).await.unwrap();
    database.fail("DELETE", "grants").await;
    assert!(grants.revoke(revoked, t0()).await.is_err());
    drop(grants);
    database.heal("DELETE", "grants").await;

    database.open(limits(), t0()).await
}

#[tokio::test]
async fn recover_keeping_grants_is_sound_again_with_the_stored_grants() {
    let database = Database::new();
    let (revoked, kept) = (address("203.0.113.7"), address("198.51.100.1"));
    let grants = failed_closed(&database, revoked, kept).await;

    grants.recover(Recovery::KeepGrants, t0() + HOUR).await.unwrap();

    assert_eq!(*grants.integrity().borrow(), Integrity::Sound);
    assert!(!database.path.with_file_name(FAIL_CLOSED_MARKER).exists());
    assert_eq!(grants.list(t0() + HOUR), [(kept, t0() + WEEK), (revoked, t0() + WEEK)]);
}

#[tokio::test]
async fn recover_discarding_grants_is_sound_again_without_them() {
    let database = Database::new();
    let (revoked, kept) = (address("203.0.113.7"), address("198.51.100.1"));
    let grants = failed_closed(&database, revoked, kept).await;

    grants.recover(Recovery::DiscardGrants, t0() + HOUR).await.unwrap();

    assert_eq!(*grants.integrity().borrow(), Integrity::Sound);
    assert!(!database.path.with_file_name(FAIL_CLOSED_MARKER).exists());
    assert_eq!(grants.list(t0() + HOUR), []);
    let last = grants.audit_events().await.unwrap().pop().unwrap();
    assert_eq!(last.action, Action::Revoke);
    drop(grants);
    let reopened = database.open(limits(), t0() + HOUR).await;
    assert_eq!(*reopened.integrity().borrow(), Integrity::Sound);
    assert_eq!(reopened.list(t0() + HOUR), []);
}

#[tokio::test]
async fn recover_without_a_restart_keeps_the_grant_whose_revoke_was_not_saved() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    database.fail("DELETE", "grants").await;
    assert!(grants.revoke(client, t0()).await.is_err());

    grants.recover(Recovery::KeepGrants, t0()).await.unwrap();

    assert_eq!(grants.list(t0()), [(client, t0() + WEEK)]);
}

#[tokio::test]
async fn a_recover_whose_write_fails_stays_failed_closed() {
    let database = Database::new();
    let grants = failed_closed(&database, address("203.0.113.7"), address("198.51.100.1")).await;
    database.fail("DELETE", "grants").await;

    assert!(grants.recover(Recovery::DiscardGrants, t0()).await.is_err());

    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
    assert!(database.path.with_file_name(FAIL_CLOSED_MARKER).exists());
}

#[tokio::test]
async fn a_marker_that_cannot_be_removed_stays_failed_closed() {
    let database = Database::new();
    // A directory in its place cannot be removed as a file.
    std::fs::create_dir(database.path.with_file_name(FAIL_CLOSED_MARKER)).unwrap();
    let grants = database.open(limits(), t0()).await;

    assert!(grants.recover(Recovery::KeepGrants, t0()).await.is_err());

    assert_eq!(*grants.integrity().borrow(), Integrity::FailedClosed);
}

#[tokio::test]
async fn recover_while_sound_is_refused_and_keeps_the_grants() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();

    assert!(grants.recover(Recovery::DiscardGrants, t0()).await.is_err());

    assert_eq!(grants.list(t0()), [(client, t0() + WEEK)]);
}
