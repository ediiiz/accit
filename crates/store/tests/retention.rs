//! Retention (`docs/spec.md` section 7): expired Grants stay reportable as `expired` until audit
//! retention passes; cleanup then removes them and old audit events.

mod common;

use common::{DAY, Database, HOUR, WEEK, address, limits, t0};
use store::{Action, AuditEvent, GrantStatus};

#[tokio::test]
async fn status_tells_none_active_and_expired_apart() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();

    assert_eq!(grants.status(address("198.51.100.1"), t0()).await.unwrap(), GrantStatus::None);
    assert_eq!(grants.status(client, t0() + DAY).await.unwrap(), GrantStatus::Active(t0() + WEEK));
    assert_eq!(grants.status(client, t0() + WEEK).await.unwrap(), GrantStatus::Expired(t0() + WEEK));
}

#[tokio::test]
async fn an_expired_grant_is_still_reported_after_restart() {
    let database = Database::new();
    let client = address("203.0.113.7");
    database.open(limits(), t0()).await.unlock(client, t0()).await.unwrap();

    let reopened = database.open(limits(), t0() + WEEK + DAY).await;

    assert_eq!(reopened.status(client, t0() + WEEK + DAY).await.unwrap(), GrantStatus::Expired(t0() + WEEK));
}

#[tokio::test]
async fn a_revoked_grant_reports_none() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();

    grants.revoke(client, t0() + HOUR).await.unwrap();

    assert_eq!(grants.status(client, t0() + HOUR).await.unwrap(), GrantStatus::None);
}

#[tokio::test]
async fn cleanup_removes_expired_grants_only_once_retention_has_passed() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    let expired_at = t0() + WEEK;
    grants.unlock(client, t0()).await.unwrap();

    grants.clean_up(expired_at + WEEK - HOUR).await.unwrap();
    let kept = grants.status(client, expired_at + WEEK - HOUR).await.unwrap();
    grants.clean_up(expired_at + WEEK).await.unwrap();
    let removed = grants.status(client, expired_at + WEEK).await.unwrap();

    assert_eq!(kept, GrantStatus::Expired(expired_at));
    assert_eq!(removed, GrantStatus::None);
}

#[tokio::test]
async fn cleanup_removes_audit_events_older_than_retention() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let (old, recent) = (address("203.0.113.7"), address("198.51.100.1"));
    grants.unlock(old, t0()).await.unwrap();
    grants.unlock(recent, t0() + DAY).await.unwrap();

    grants.clean_up(t0() + WEEK).await.unwrap();

    assert_eq!(
        grants.audit_events().await.unwrap(),
        [AuditEvent { action: Action::Unlock, address: recent, at: t0() + DAY }]
    );
}
