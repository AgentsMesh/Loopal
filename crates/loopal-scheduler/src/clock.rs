//! Clock abstraction for deterministic time control in tests.

use chrono::{DateTime, Utc};

/// Provides the current time. Inject into `CronScheduler` for testability.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
}

/// Production clock — delegates to `Utc::now()`.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Test clock with manually controlled time.
pub struct ManualClock {
    now: std::sync::Mutex<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(initial: DateTime<Utc>) -> Self {
        Self {
            now: std::sync::Mutex::new(initial),
        }
    }

    /// Set the current time to an absolute value.
    pub fn set(&self, time: DateTime<Utc>) {
        *self.now.lock().unwrap() = time;
    }

    /// Advance the current time by `duration`.
    pub fn advance(&self, duration: chrono::Duration) {
        let mut now = self.now.lock().unwrap();
        *now += duration;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}
