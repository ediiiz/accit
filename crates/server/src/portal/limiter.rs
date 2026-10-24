//! Mutations per Address: [`BUDGET`] in any [`WINDOW`], with at most [`CAPACITY`] Addresses tracked.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use access::Address;

const BUDGET: usize = 5;
const WINDOW: Duration = Duration::from_mins(1);
const CAPACITY: usize = 10_000;

#[derive(Debug, Default)]
pub(super) struct Limiter(Mutex<HashMap<Address, VecDeque<SystemTime>>>);

fn within(window_start: SystemTime, now: SystemTime) -> bool {
    window_start + WINDOW > now
}

/// Drops Addresses with no recent mutation; if all are recent, the one idle longest.
fn make_room(hits: &mut HashMap<Address, VecDeque<SystemTime>>, now: SystemTime) {
    hits.retain(|_, log| log.back().is_some_and(|last| within(*last, now)));

    if hits.len() >= CAPACITY {
        let idlest = hits.iter().min_by_key(|(_, log)| log.back().copied()).map(|(address, _)| *address);

        if let Some(address) = idlest {
            hits.remove(&address);
        }
    }
}

impl Limiter {
    /// Counts a mutation, or says how many seconds until the Address may mutate again.
    pub(super) fn admit(&self, address: Address, now: SystemTime) -> Result<(), u64> {
        let mut hits = self.0.lock().unwrap_or_else(PoisonError::into_inner);

        if !hits.contains_key(&address) && hits.len() >= CAPACITY {
            make_room(&mut hits, now);
        }

        let log = hits.entry(address).or_default();
        log.retain(|hit| within(*hit, now));

        if let Some(oldest) = log.front().filter(|_| log.len() >= BUDGET) {
            let wait = (*oldest + WINDOW).duration_since(now).unwrap_or_default();

            return Err((wait.as_secs() + u64::from(wait.subsec_nanos() > 0)).max(1));
        }

        log.push_back(now);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    fn address(number: u32) -> Address {
        Address::from(IpAddr::V4(Ipv4Addr::from(0x5db8_0000 + number)))
    }

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn five_a_minute_then_the_wait_for_the_oldest() {
        let limiter = Limiter::default();

        for second in 0..5 {
            assert_eq!(limiter.admit(address(0), at(1_000 + second)), Ok(()));
        }

        assert_eq!(limiter.admit(address(0), at(1_010)), Err(50));
        assert_eq!(limiter.admit(address(0), at(1_059)), Err(1));
        assert_eq!(limiter.admit(address(0), at(1_060)), Ok(()));
        assert_eq!(limiter.admit(address(0), at(1_060)), Err(1));
    }

    #[test]
    fn a_fraction_of_a_second_waits_a_whole_one() {
        let limiter = Limiter::default();

        for _ in 0..5 {
            assert_eq!(limiter.admit(address(0), at(1_000)), Ok(()));
        }

        assert_eq!(limiter.admit(address(0), at(1_000) + Duration::from_millis(500)), Err(60));
        assert_eq!(limiter.admit(address(0), at(1_059) + Duration::from_millis(500)), Err(1));
    }

    #[test]
    fn tracking_is_bounded_and_drops_the_address_idle_longest() {
        let limiter = Limiter::default();

        for _ in 0..5 {
            assert_eq!(limiter.admit(address(0), at(1_000)), Ok(()));
        }

        assert_eq!(limiter.admit(address(0), at(1_001)), Err(59));

        for number in 1..=10_000 {
            assert_eq!(limiter.admit(address(number), at(1_002)), Ok(()));
        }

        assert_eq!(limiter.0.lock().map(|hits| hits.len()).unwrap_or_default(), 10_000);
        assert_eq!(limiter.admit(address(0), at(1_003)), Ok(()), "address 0 was dropped");
    }
}
