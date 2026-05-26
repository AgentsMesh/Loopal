use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

// wait() returns true iff every spawn outstanding at any point during the wait
// has completed — fast tasks cannot prematurely declare "settled" on behalf
// of still-running peers.
pub(crate) struct SettleSignal {
    running: AtomicU64,
    notify: Notify,
}

impl SettleSignal {
    pub(crate) fn new() -> Self {
        Self {
            running: AtomicU64::new(0),
            notify: Notify::new(),
        }
    }

    pub(crate) fn mark_running(&self) {
        self.running.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn mark_settled(&self) {
        if self.running.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notify.notify_waiters();
        }
    }

    pub(crate) async fn wait(&self, timeout: Duration) -> bool {
        if self.running.load(Ordering::Acquire) == 0 {
            return true;
        }
        let deadline = Instant::now() + timeout;
        loop {
            if self.running.load(Ordering::Acquire) == 0 {
                return true;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            if tokio::time::timeout(remaining, self.notify.notified())
                .await
                .is_err()
            {
                return false;
            }
        }
    }

    pub(crate) async fn wait_forever(&self) {
        loop {
            if self.running.load(Ordering::Acquire) == 0 {
                return;
            }
            self.notify.notified().await;
        }
    }
}
