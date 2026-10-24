//! One serialized mutation path (`docs/spec.md` section 7): racing Unlocks, Revokes and lookups
//! never leave a Grant visible after the Revoke that came last.

#![allow(clippy::unwrap_used, reason = "task bodies of the tests below")]

mod common;

use std::collections::HashMap;

use access::Address;
use common::{Database, limits, t0};
use store::{Action, Grants};
use tokio::task::JoinSet;

const ADDRESSES: u8 = 16;
const ROUNDS: usize = 10;

fn addresses() -> Vec<Address> {
    (1..=ADDRESSES).map(|host| common::address(&format!("192.0.2.{host}"))).collect()
}

async fn look_up_all(grants: Grants) {
    for _ in 0..ROUNDS * 20 {
        for address in addresses() {
            let _ = grants.active_until(address, t0());
        }
        tokio::task::yield_now().await;
    }
}

async fn unlock_then_revoke(grants: Grants, address: Address) {
    for _ in 0..ROUNDS {
        grants.unlock(address, t0()).await.unwrap();
        grants.revoke(address, t0()).await.unwrap();
        assert_eq!(grants.active_until(address, t0()), None, "{address}");
    }
}

async fn unlock_or_revoke(grants: Grants, address: Address, unlock: bool) {
    if unlock {
        grants.unlock(address, t0()).await.unwrap();
    } else {
        grants.revoke(address, t0()).await.unwrap();
    }
}

fn hammer_lookups(grants: &Grants, tasks: &mut JoinSet<()>) {
    for _ in 0..4 {
        tasks.spawn(look_up_all(grants.clone()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_completed_revoke_hides_the_grant_from_lookups() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let mut tasks = JoinSet::new();
    hammer_lookups(&grants, &mut tasks);

    for address in addresses() {
        tasks.spawn(unlock_then_revoke(grants.clone(), address));
    }
    tasks.join_all().await;

    let reopened = database.open(limits(), t0()).await;
    assert!(addresses().into_iter().all(|address| reopened.active_until(address, t0()).is_none()));
}

/// Whatever order the race settles in, the last audited action decides, in memory and on disk.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_unlocks_and_revokes_settle_on_the_last_action() {
    let database = Database::new();
    let grants = database.open(limits(), t0()).await;
    let mut tasks = JoinSet::new();
    hammer_lookups(&grants, &mut tasks);

    for (address, round) in
        addresses().into_iter().flat_map(|address| (0..ROUNDS).map(move |round| (address, round)))
    {
        tasks.spawn(unlock_or_revoke(grants.clone(), address, round % 2 == 0));
    }
    tasks.join_all().await;

    let last: HashMap<Address, Action> =
        grants.audit_events().await.unwrap().into_iter().map(|event| (event.address, event.action)).collect();
    let reopened = database.open(limits(), t0()).await;

    for address in addresses() {
        let granted = last[&address] == Action::Unlock;

        assert_eq!(grants.active_until(address, t0()).is_some(), granted, "memory, {address}");
        assert_eq!(reopened.active_until(address, t0()).is_some(), granted, "database, {address}");
    }
}
