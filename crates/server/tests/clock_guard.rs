//! Clock guard (`docs/spec.md` section 7): when elapsed wall time and elapsed monotonic time differ by
//! over 5 minutes, accit is unready until restart. Tokio's paused clock stands in for monotonic time.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use server::health::{self, Readiness};
use tokio::time::{Instant, sleep};

const PERIOD: Duration = Duration::from_secs(30);
const FIVE_MINUTES: i64 = 5 * 60;

/// A wall clock that follows monotonic time, shifted by a settable number of seconds.
#[derive(Clone)]
struct Wall {
    start: Instant,
    shift: Arc<AtomicI64>,
}

impl Wall {
    fn new() -> Self {
        Self { start: Instant::now(), shift: Arc::default() }
    }

    fn jump(&self, seconds: i64) {
        self.shift.fetch_add(seconds, Ordering::SeqCst);
    }

    fn reader(&self) -> impl Fn() -> SystemTime + Send + 'static {
        let wall = self.clone();

        move || {
            let followed = UNIX_EPOCH + Duration::from_hours(500_000) + wall.start.elapsed();
            let shift = wall.shift.load(Ordering::SeqCst);
            let by = Duration::from_secs(shift.unsigned_abs());

            if shift < 0 { followed - by } else { followed + by }
        }
    }
}

/// Returns once the guard has taken its starting point.
async fn guarded() -> (Readiness, Wall) {
    let (readiness, wall) = (Readiness::default(), Wall::new());
    readiness.set(true);
    tokio::spawn(health::guard_clocks(readiness.clone(), wall.reader(), PERIOD));
    tokio::task::yield_now().await;

    (readiness, wall)
}

/// Lands between two checks, after the one due within it.
async fn next_check() {
    sleep(PERIOD + PERIOD / 2).await;
}

#[tokio::test(start_paused = true)]
async fn a_backward_jump_over_five_minutes_is_unready_until_restart() {
    let (readiness, wall) = guarded().await;
    next_check().await;
    assert!(readiness.is_ready());

    wall.jump(-(FIVE_MINUTES + 1));
    next_check().await;
    assert!(!readiness.is_ready());

    wall.jump(FIVE_MINUTES + 1);
    next_check().await;
    assert!(!readiness.is_ready());
}

#[tokio::test(start_paused = true)]
async fn a_forward_jump_over_five_minutes_is_unready() {
    let (readiness, wall) = guarded().await;

    wall.jump(FIVE_MINUTES + 1);
    next_check().await;

    assert!(!readiness.is_ready());
}

#[tokio::test(start_paused = true)]
async fn a_drift_of_exactly_five_minutes_stays_ready() {
    let (readiness, wall) = guarded().await;

    wall.jump(-FIVE_MINUTES);
    next_check().await;

    assert!(readiness.is_ready());
}
