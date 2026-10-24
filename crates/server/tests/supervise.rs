//! The wiring of `serve::run` (`docs/spec.md` section 7): readiness follows the grant store's
//! integrity from the first request, and an unrecoverable store ends the run with an error.

use std::future::{pending, ready};
use std::time::{Duration, UNIX_EPOCH};

use server::health::{self, Readiness};
use store::{FAIL_CLOSED_MARKER, Grants, Integrity, Limits};
use tokio::sync::watch;
use tokio::time::timeout;

const LIMITS: Limits = Limits {
    grant_duration: Duration::from_hours(7 * 24),
    max_grants: 1_000,
    audit_retention: Duration::from_hours(7 * 24),
};

#[tokio::test]
async fn a_sound_store_makes_accit_ready_while_serving() {
    let readiness = Readiness::default();
    let (_integrity, following) = watch::channel(Integrity::Sound);

    let run = timeout(
        Duration::from_millis(50),
        health::supervise(pending(), readiness.clone(), following, pending()),
    )
    .await;

    assert!(run.is_err(), "run returned");
    assert!(readiness.is_ready());
}

#[tokio::test]
#[allow(clippy::unwrap_used, reason = "test fixture")]
async fn a_store_opened_with_a_marker_stays_unready() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accit.db");
    std::fs::write(path.with_file_name(FAIL_CLOSED_MARKER), "").unwrap();
    let grants = Grants::open(Some(&path), LIMITS, UNIX_EPOCH).await.unwrap();
    let readiness = Readiness::default();

    let run = timeout(
        Duration::from_millis(50),
        health::supervise(pending(), readiness.clone(), grants.integrity(), pending()),
    )
    .await;

    assert!(run.is_err(), "run returned");
    assert!(!readiness.is_ready());
}

#[tokio::test]
async fn a_store_that_fails_closed_while_serving_turns_unready() {
    let readiness = Readiness::default();
    let (integrity, following) = watch::channel(Integrity::Sound);
    tokio::spawn(health::supervise(pending(), readiness.clone(), following, pending()));
    tokio::task::yield_now().await;
    assert!(readiness.is_ready());

    integrity.send_replace(Integrity::FailedClosed);
    tokio::task::yield_now().await;

    assert!(!readiness.is_ready());
}

#[tokio::test]
async fn an_unrecoverable_store_ends_the_run_with_an_error() {
    let readiness = Readiness::default();
    let (_integrity, following) = watch::channel(Integrity::Unrecoverable);

    let run = health::supervise(pending(), readiness.clone(), following, pending()).await;

    assert!(!readiness.is_ready());
    assert!(run.is_err_and(|error| error.to_string().contains("marker")));
}

#[tokio::test]
async fn a_stop_signal_ends_the_run_cleanly() {
    let (_integrity, following) = watch::channel(Integrity::Sound);

    let run = health::supervise(pending(), Readiness::default(), following, ready(())).await;

    assert!(run.is_ok());
}
