use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BgTaskConfig {
    pub stop_ack_timeout_secs: u64,
    pub drainers_grace_ms: u64,
    pub sigterm_grace_ms: u64,
    pub gc_interval_secs: u64,
    pub terminal_retention_secs: u64,
    pub output_sample_interval_secs: u64,
}

impl Default for BgTaskConfig {
    fn default() -> Self {
        Self {
            stop_ack_timeout_secs: 5,
            drainers_grace_ms: 50,
            sigterm_grace_ms: 500,
            gc_interval_secs: 60,
            terminal_retention_secs: 3600,
            output_sample_interval_secs: 2,
        }
    }
}

impl BgTaskConfig {
    pub fn stop_ack_timeout(&self) -> Duration {
        Duration::from_secs(self.stop_ack_timeout_secs)
    }
    pub fn drainers_grace(&self) -> Duration {
        Duration::from_millis(self.drainers_grace_ms)
    }
    pub fn sigterm_grace(&self) -> Duration {
        Duration::from_millis(self.sigterm_grace_ms)
    }
    pub fn gc_interval(&self) -> Duration {
        Duration::from_secs(self.gc_interval_secs)
    }
    pub fn terminal_retention(&self) -> Duration {
        Duration::from_secs(self.terminal_retention_secs)
    }
    pub fn output_sample_interval(&self) -> Duration {
        Duration::from_secs(self.output_sample_interval_secs)
    }
}
