//! Two clocks (`docs/spec.md` section 7): a Grant passes only while both its wall-clock expiry and
//! its monotonic deadline lie in the future. Tokio's paused clock stands in for monotonic time.

mod common;

use common::{Database, WEEK, address, limits, t0};
use std::time::Duration;
use store::Limits;
use tokio::time::advance;

const SECOND: Duration = Duration::from_secs(1);

#[tokio::test(start_paused = true)]
async fn a_backward_wall_clock_jump_never_extends_a_grant() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let client = address("203.0.113.7");
    grants.unlock(client, t0()).await.unwrap();
    // A week passes while the wall clock reads only a day later.
    let stale_wall = t0() + common::DAY;

    advance(WEEK.saturating_sub(SECOND)).await;
    assert_eq!(grants.active_until(client, stale_wall), Some(t0() + WEEK));
    advance(SECOND).await;
    assert_eq!(grants.active_until(client, stale_wall), None);
}

#[tokio::test(start_paused = true)]
async fn a_loaded_grant_gets_its_deadline_from_the_time_left_at_load() {
    let database = Database::new();
    let client = address("203.0.113.7");
    database.open(limits(), t0()).await.unlock(client, t0()).await.unwrap();
    let reopened = database.open(limits(), t0() + common::DAY).await;
    // Six days are left at load; the wall clock then stands still.
    let stale_wall = t0() + common::DAY;

    advance((6 * common::DAY).saturating_sub(SECOND)).await;
    assert_eq!(reopened.active_until(client, stale_wall), Some(t0() + WEEK));
    advance(SECOND).await;
    assert_eq!(reopened.active_until(client, stale_wall), None);
}

#[tokio::test(start_paused = true)]
async fn a_grant_past_its_deadline_frees_its_place_under_the_cap() {
    let database = Database::new();
    let grants = database.open(Limits { max_grants: 1, ..limits() }, t0()).await;
    grants.unlock(address("203.0.113.7"), t0()).await.unwrap();

    advance(WEEK).await;
    let result = grants.unlock(address("198.51.100.1"), t0() + common::HOUR).await;

    assert!(result.is_ok(), "{result:?}");
}
