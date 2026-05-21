use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{SecretError, SecretResult};

pub const DEFAULT_DEGRADE_THRESHOLD: u32 = 3;

pub struct HubHealth {
    consecutive_failures: AtomicU32,
    is_degraded: AtomicBool,
    /// Unix ms when the current degraded streak started. 0 means not
    /// degraded. Set atomically with `is_degraded=true`, cleared on
    /// recovery — readers should treat the pair as a logical snapshot
    /// (read `is_degraded` first; if true, then `degraded_at_unix_ms`).
    degraded_at_unix_ms: AtomicU64,
    degrade_threshold: u32,
}

impl HubHealth {
    pub fn new() -> Self {
        Self::with_threshold(DEFAULT_DEGRADE_THRESHOLD)
    }

    pub fn with_threshold(threshold: u32) -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            is_degraded: AtomicBool::new(false),
            degraded_at_unix_ms: AtomicU64::new(0),
            degrade_threshold: threshold.max(1),
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.is_degraded.load(Ordering::Acquire)
    }

    pub fn degraded_at_unix_ms(&self) -> Option<u64> {
        let v = self.degraded_at_unix_ms.load(Ordering::Acquire);
        (v != 0).then_some(v)
    }

    pub(crate) fn record_outcome<T>(&self, result: &SecretResult<T>) {
        match result {
            Ok(_) => self.record_success(),
            Err(SecretError::Ipc(_)) => self.record_failure(),
            Err(_) => {}
        }
    }

    fn record_failure(&self) {
        let n = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;
        if n >= self.degrade_threshold && !self.is_degraded.load(Ordering::Acquire) {
            self.degraded_at_unix_ms
                .store(now_unix_ms(), Ordering::Release);
            self.is_degraded.store(true, Ordering::Release);
        }
    }

    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.is_degraded.store(false, Ordering::Release);
        self.degraded_at_unix_ms.store(0, Ordering::Release);
    }
}

impl Default for HubHealth {
    fn default() -> Self {
        Self::new()
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fail<T>() -> SecretResult<T> {
        Err(SecretError::Ipc("test failure".into()))
    }

    fn ok() -> SecretResult<()> {
        Ok(())
    }

    #[test]
    fn starts_healthy() {
        let h = HubHealth::new();
        assert!(!h.is_degraded());
        assert_eq!(h.degraded_at_unix_ms(), None);
    }

    #[test]
    fn degrades_after_threshold() {
        let h = HubHealth::new();
        for _ in 0..DEFAULT_DEGRADE_THRESHOLD {
            h.record_outcome(&fail::<()>());
        }
        assert!(h.is_degraded());
        assert!(h.degraded_at_unix_ms().is_some());
    }

    #[test]
    fn does_not_degrade_below_threshold() {
        let h = HubHealth::new();
        for _ in 0..(DEFAULT_DEGRADE_THRESHOLD - 1) {
            h.record_outcome(&fail::<()>());
        }
        assert!(!h.is_degraded());
        assert_eq!(h.degraded_at_unix_ms(), None);
    }

    #[test]
    fn recovers_on_success() {
        let h = HubHealth::new();
        for _ in 0..DEFAULT_DEGRADE_THRESHOLD {
            h.record_outcome(&fail::<()>());
        }
        assert!(h.is_degraded());
        h.record_outcome(&ok());
        assert!(!h.is_degraded());
        assert_eq!(h.degraded_at_unix_ms(), None);
    }

    #[test]
    fn degraded_at_persists_across_repeated_failures() {
        let h = HubHealth::new();
        for _ in 0..DEFAULT_DEGRADE_THRESHOLD {
            h.record_outcome(&fail::<()>());
        }
        let first = h.degraded_at_unix_ms().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        h.record_outcome(&fail::<()>());
        assert_eq!(
            h.degraded_at_unix_ms(),
            Some(first),
            "subsequent failures must not reset the degraded_at marker"
        );
    }

    #[test]
    fn permanent_errors_do_not_affect_health() {
        let h = HubHealth::new();
        for _ in 0..(DEFAULT_DEGRADE_THRESHOLD * 2) {
            let r: SecretResult<()> = Err(SecretError::SecretNotFound("k".into()));
            h.record_outcome(&r);
        }
        assert!(!h.is_degraded());
    }

    #[test]
    fn custom_threshold_overrides_default() {
        let h = HubHealth::with_threshold(5);
        for _ in 0..4 {
            h.record_outcome(&fail::<()>());
        }
        assert!(!h.is_degraded());
        h.record_outcome(&fail::<()>());
        assert!(h.is_degraded());
    }

    #[test]
    fn threshold_zero_clamps_to_one() {
        let h = HubHealth::with_threshold(0);
        h.record_outcome(&fail::<()>());
        assert!(h.is_degraded(), "threshold should clamp to min 1");
    }
}
