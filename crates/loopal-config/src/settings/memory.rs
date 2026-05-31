use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub batch_window_ms: u64,
    pub channel_buffer: usize,
    pub consolidation_interval_days: u32,
    pub gc_compress_after_days: u32,
    pub gc_archive_after_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // 2s window: allows rapid-fire Memory() calls to batch (50-80% spawn reduction)
            // while keeping latency acceptable for interactive use.
            batch_window_ms: 2000,
            // 256 slots ≈ ~8s of high-frequency observations before backpressure.
            channel_buffer: 256,
            // Weekly full consolidation: balance between freshness and API cost.
            consolidation_interval_days: 7,
            gc_compress_after_days: 90,
            gc_archive_after_days: 365,
        }
    }
}
