//! Counters for `accit admin status` (docs/spec.md section 11). Only totals: nothing here can name
//! an Address.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use access::Verdict;

const SLOTS: usize = 60;
const WINDOW: u64 = SLOTS as u64;
const COUNT_BITS: u32 = 32;
const COUNT_MASK: u64 = (1 << COUNT_BITS) - 1;

/// The decisions of the last 60 seconds, one slot per second of the minute. A slot packs
/// `second << 32 | count`, so a decision from an earlier minute resets it with one compare-and-swap.
#[derive(Debug)]
struct Window([AtomicU64; SLOTS]);

impl Window {
    fn new() -> Self {
        Self(std::array::from_fn(|_| AtomicU64::new(0)))
    }

    fn slot(&self, second: u64) -> &AtomicU64 {
        let index = usize::try_from(second % WINDOW).unwrap_or_default();

        &self.0[index]
    }

    fn record(&self, second: u64) {
        let _ = self.slot(second).fetch_update(Ordering::Relaxed, Ordering::Relaxed, |slot| {
            let count = if slot >> COUNT_BITS == second { (slot & COUNT_MASK).saturating_add(1) } else { 1 };

            Some(second << COUNT_BITS | count.min(COUNT_MASK))
        });
    }

    /// Decisions in the seconds `second - 59 ..= second`.
    fn count(&self, second: u64) -> u64 {
        let oldest = second.saturating_sub(WINDOW - 1);

        self.0
            .iter()
            .map(|slot| slot.load(Ordering::Relaxed))
            .filter(|slot| (oldest..=second).contains(&(slot >> COUNT_BITS)))
            .map(|slot| slot & COUNT_MASK)
            .sum()
    }
}

#[derive(Debug)]
pub struct Stats {
    started: Instant,
    window: Window,
    denials: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self { started: Instant::now(), window: Window::new(), denials: AtomicU64::new(0) }
    }

    fn second(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn record(&self, verdict: Verdict) {
        self.record_at(verdict, self.second());
    }

    fn record_at(&self, verdict: Verdict, second: u64) {
        self.window.record(second);

        if verdict == Verdict::Deny {
            self.denials.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn status(&self, active_grants: usize) -> Status {
        self.status_at(active_grants, self.second())
    }

    fn status_at(&self, active_grants: usize, second: u64) -> Status {
        Status {
            active_grants,
            verifications_per_minute: self.window.count(second),
            denials: self.denials.load(Ordering::Relaxed),
        }
    }
}

/// What `accit admin status` prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub active_grants: usize,
    /// Verifications decided in the last 60 seconds.
    pub verifications_per_minute: u64,
    /// Denied verifications since the daemon started.
    pub denials: u64,
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "active grants: {}", self.active_grants)?;
        writeln!(formatter, "verifications per minute: {}", self.verifications_per_minute)?;
        writeln!(formatter, "denials: {}", self.denials)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn per_minute(stats: &Stats, second: u64) -> u64 {
        stats.status_at(0, second).verifications_per_minute
    }

    #[test]
    fn the_rate_counts_the_last_60_seconds() {
        let stats = Stats::new();
        for second in [0, 0, 30, 61] {
            stats.record_at(Verdict::Pass, second);
        }

        assert_eq!(per_minute(&stats, 59), 3);
        assert_eq!(per_minute(&stats, 60), 1);
        assert_eq!(per_minute(&stats, 61), 2);
        assert_eq!(per_minute(&stats, 89), 2);
        assert_eq!(per_minute(&stats, 90), 1);
        assert_eq!(per_minute(&stats, 120), 1);
        assert_eq!(per_minute(&stats, 121), 0);
    }

    #[test]
    fn a_slot_from_an_earlier_minute_does_not_survive_its_reuse() {
        let stats = Stats::new();
        stats.record_at(Verdict::Pass, 5);
        stats.record_at(Verdict::Pass, 65);
        stats.record_at(Verdict::Pass, 65);

        assert_eq!(per_minute(&stats, 65), 2);
    }

    #[test]
    fn denials_are_a_total_and_do_not_age_out() {
        let stats = Stats::new();
        stats.record_at(Verdict::Pass, 0);
        stats.record_at(Verdict::Deny, 0);
        stats.record_at(Verdict::Deny, 1);

        assert_eq!(stats.status_at(0, 500).denials, 2);
        assert_eq!(per_minute(&stats, 500), 0);
    }

    #[test]
    fn status_prints_one_line_per_counter() {
        let status = Status { active_grants: 3, verifications_per_minute: 12, denials: 4 };

        assert_eq!(status.to_string(), "active grants: 3\nverifications per minute: 12\ndenials: 4\n");
    }
}
