//! Fail closed (`docs/spec.md` section 7): a Revoke that was not saved makes accit unready; one whose
//! marker could not be written either stops the process with an error.

use std::time::Duration;

use server::health::{self, Readiness};
use store::Integrity;
use tokio::sync::watch;
use tokio::time::timeout;

fn ready() -> Readiness {
    let readiness = Readiness::default();
    readiness.set(true);

    readiness
}

#[tokio::test]
async fn sound_grants_keep_accit_ready() {
    let (readiness, (_integrity, following)) = (ready(), watch::channel(Integrity::Sound));

    let stopped = timeout(Duration::from_millis(50), health::follow(readiness.clone(), following)).await;

    assert!(stopped.is_err(), "follow returned");
    assert!(readiness.is_ready());
}

#[tokio::test]
async fn failing_closed_makes_accit_unready() {
    let (readiness, (integrity, following)) = (ready(), watch::channel(Integrity::Sound));
    tokio::spawn(health::follow(readiness.clone(), following));
    tokio::task::yield_now().await;

    integrity.send_replace(Integrity::FailedClosed);
    tokio::task::yield_now().await;

    assert!(!readiness.is_ready());
}

#[tokio::test]
async fn an_unrecoverable_store_stops_with_an_error() {
    let (readiness, (integrity, following)) = (ready(), watch::channel(Integrity::Sound));
    let follower = tokio::spawn(health::follow(readiness.clone(), following));

    integrity.send_replace(Integrity::Unrecoverable);
    let error = follower.await.unwrap();

    assert!(!readiness.is_ready());
    assert!(error.to_string().contains("marker"), "{error}");
}
