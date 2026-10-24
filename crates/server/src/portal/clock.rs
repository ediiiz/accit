use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

/// The wall clock, replaceable so tests can move time.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> SystemTime + Send + Sync>);

impl Clock {
    pub fn system() -> Self {
        Self::new(SystemTime::now)
    }

    pub fn new(now: impl Fn() -> SystemTime + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    pub(super) fn now(&self) -> SystemTime {
        (self.0)()
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Clock")
    }
}
