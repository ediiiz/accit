//! Unlock (`docs/spec.md` section 6): create or renew to `now + duration`, capped.

mod common;

use common::{DAY, Database, WEEK, address, limits, t0};
use store::{Limits, UnlockError};

#[tokio::test]
async fn unlock_grants_the_address_until_now_plus_duration() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");

    let expiry = grants.unlock(client, t0()).await.unwrap();

    assert_eq!(expiry, t0() + WEEK);
    assert_eq!(grants.active_until(client, t0() + WEEK - common::HOUR), Some(t0() + WEEK));
    assert_eq!(grants.active_until(client, t0() + WEEK), None);
    assert_eq!(grants.active_until(address("198.51.100.1"), t0()), None);
}

#[tokio::test]
async fn repeated_unlocks_renew_from_now_and_never_accumulate() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");

    grants.unlock(client, t0()).await.unwrap();
    let renewed = grants.unlock(client, t0() + DAY).await.unwrap();

    assert_eq!(renewed, t0() + DAY + WEEK);
    assert_eq!(grants.active_until(client, t0() + DAY), Some(t0() + DAY + WEEK));
}

fn two_at_most() -> Limits {
    Limits { max_grants: 2, ..limits() }
}

#[tokio::test]
async fn a_new_address_is_refused_at_the_cap() {
    let database = Database::new();
    let grants = database.open(two_at_most(), t0()).await;
    let newcomer = address("192.0.2.3");
    grants.unlock(address("192.0.2.1"), t0()).await.unwrap();
    grants.unlock(address("192.0.2.2"), t0()).await.unwrap();

    let refused = grants.unlock(newcomer, t0()).await;

    assert!(matches!(refused, Err(UnlockError::Full)), "{refused:?}");
    assert_eq!(grants.active_until(newcomer, t0()), None);
}

#[tokio::test]
async fn renewing_works_at_the_cap() {
    let database = Database::new();
    let grants = database.open(two_at_most(), t0()).await;
    let client = address("192.0.2.1");
    grants.unlock(client, t0()).await.unwrap();
    grants.unlock(address("192.0.2.2"), t0()).await.unwrap();

    let renewed = grants.unlock(client, t0() + DAY).await.unwrap();

    assert_eq!(renewed, t0() + DAY + WEEK);
}

#[tokio::test]
async fn expired_grants_do_not_count_towards_the_cap() {
    let database = Database::new();
    let grants = database.open(two_at_most(), t0()).await;
    grants.unlock(address("192.0.2.1"), t0()).await.unwrap();
    grants.unlock(address("192.0.2.2"), t0() + DAY).await.unwrap();
    let later = t0() + WEEK;

    let newcomer = grants.unlock(address("192.0.2.3"), later).await.unwrap();

    assert_eq!(newcomer, later + WEEK);
}
