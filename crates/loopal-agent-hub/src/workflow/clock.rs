use std::time::{SystemTime, UNIX_EPOCH};

pub trait WorkflowClock: Send + Sync + 'static {
    fn now_unix_ms(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWorkflowClock;

impl WorkflowClock for SystemWorkflowClock {
    fn now_unix_ms(&self) -> u64 {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        u64::try_from(millis).unwrap_or(u64::MAX)
    }
}
