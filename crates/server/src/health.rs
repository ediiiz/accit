//! Readiness: false until grant state is loaded, and again whenever accit must fail closed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::anyhow;
use store::Integrity;
use tokio::sync::watch;
use tokio::time::Instant;

const MAX_DRIFT: Duration = Duration::from_mins(5);

#[derive(Debug, Default)]
struct Flags {
    up: AtomicBool,
    /// Apart from `up`, so `accit admin recover` clears only this reason to be unready.
    failed_closed: AtomicBool,
}

#[derive(Clone, Debug, Default)]
pub struct Readiness(Arc<Flags>);

impl Readiness {
    pub fn set(&self, ready: bool) {
        self.0.up.store(ready, Ordering::SeqCst);
    }

    pub fn is_ready(&self) -> bool {
        self.0.up.load(Ordering::SeqCst) && !self.0.failed_closed.load(Ordering::SeqCst)
    }

    fn follow(&self, integrity: Integrity) {
        self.0.failed_closed.store(integrity != Integrity::Sound, Ordering::SeqCst);
    }
}

/// Every `period`, compares elapsed `wall` time with elapsed monotonic time since the call. Returns once
/// they differ by over 5 minutes, leaving accit unready until restart.
pub async fn guard_clocks(readiness: Readiness, wall: impl Fn() -> SystemTime, period: Duration) {
    let (wall_start, start) = (wall(), Instant::now());
    let mut checks = tokio::time::interval(period);

    loop {
        checks.tick().await;
        let expected = wall_start + start.elapsed();
        let drift = wall().duration_since(expected).unwrap_or_else(|behind| behind.duration());

        if drift > MAX_DRIFT {
            readiness.set(false);
            tracing::error!(drift_secs = drift.as_secs(), "wall clock jumped; unready until restart");
            return;
        }
    }
}

/// Makes accit unready while the grant store is failed closed. Returns why the process must stop: the
/// fail-closed marker could not be written, or the store is gone.
pub async fn follow(readiness: Readiness, mut integrity: watch::Receiver<Integrity>) -> anyhow::Error {
    loop {
        let current = *integrity.borrow_and_update();

        readiness.follow(current);
        if current == Integrity::Unrecoverable {
            return anyhow!("a Revoke was not saved and the fail-closed marker could not be written");
        }
        if integrity.changed().await.is_err() {
            readiness.set(false);
            return anyhow!("grant store stopped");
        }
    }
}

/// Runs `servers` until `stop` resolves, `servers` fails, or the store makes the process stop. Readiness
/// starts as the store's integrity says and follows it from then on.
pub async fn supervise(
    servers: impl Future<Output = anyhow::Result<()>>,
    readiness: Readiness,
    integrity: watch::Receiver<Integrity>,
    stop: impl Future<Output = ()>,
) -> anyhow::Result<()> {
    readiness.follow(*integrity.borrow());
    readiness.set(true);

    tokio::select! {
        result = servers => result,
        error = follow(readiness, integrity) => Err(error),
        () = stop => Ok(()),
    }
}
