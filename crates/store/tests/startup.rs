//! Startup (`docs/spec.md` section 7): committed Grants survive a restart; expired ones do not return.

mod common;

use common::{DAY, Database, HOUR, WEEK, address, limits, t0};

#[tokio::test]
async fn open_loads_unexpired_grants() {
    let database = Database::new();
    let client = address("2001:4860::8888");
    database.open(limits(), t0()).await.unlock(client, t0()).await.unwrap();

    let reopened = database.open(limits(), t0() + DAY).await;

    assert_eq!(reopened.active_until(client, t0() + DAY), Some(t0() + WEEK));
}

#[tokio::test]
async fn open_skips_expired_grants() {
    let database = Database::new();
    let (expired, live) = (address("203.0.113.7"), address("198.51.100.1"));
    let grants = database.open(limits(), t0()).await;
    grants.unlock(expired, t0()).await.unwrap();
    grants.unlock(live, t0() + DAY).await.unwrap();
    drop(grants);

    let reopened = database.open(limits(), t0() + WEEK + HOUR).await;

    assert_eq!(reopened.active_until(expired, t0()), None);
    assert_eq!(reopened.active_until(live, t0() + WEEK + HOUR), Some(t0() + DAY + WEEK));
}
