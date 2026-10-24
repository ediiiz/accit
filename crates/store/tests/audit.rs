//! Audit events (`docs/spec.md` section 7): action, Address, time.

mod common;

use common::{Database, HOUR, address, limits, t0};
use store::{Action, AuditEvent};

#[tokio::test]
async fn unlock_and_revoke_are_recorded_in_order() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("2001:4860::8888");

    grants.unlock(client, t0()).await.unwrap();
    grants.revoke(client, t0() + HOUR).await.unwrap();

    assert_eq!(
        grants.audit_events().await.unwrap(),
        [
            AuditEvent { action: Action::Unlock, address: client, at: t0() },
            AuditEvent { action: Action::Revoke, address: client, at: t0() + HOUR },
        ]
    );
}
