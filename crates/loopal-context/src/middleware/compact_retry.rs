use std::time::Duration;

use async_trait::async_trait;

/// Structured lifecycle emitted by the compaction LLM retry loop.
///
/// The context crate stays frontend-agnostic: runtime callers translate these
/// updates into protocol events, while tests can assert the retry lifecycle
/// directly without scraping logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactRetryEvent {
    Scheduled {
        error: String,
        attempt: u32,
        max_retries: u32,
        wait: Duration,
    },
    Succeeded {
        retries: u32,
    },
    Exhausted {
        retries: u32,
    },
    Failed {
        retries: u32,
    },
    Cancelled {
        retries: u32,
    },
}

/// Async retry lifecycle sink.
///
/// Events are awaited at the point where they occur so a terminal retry event
/// is causally delivered before compaction can publish `Compacted` or `Done`.
/// This prevents an asynchronous frontend adapter from reordering an earlier
/// retry notification behind the terminal compaction state.
#[async_trait]
pub trait CompactRetryObserver: Send + Sync {
    async fn observe(&self, event: CompactRetryEvent);
}

pub(super) struct NoopCompactRetryObserver;

#[async_trait]
impl CompactRetryObserver for NoopCompactRetryObserver {
    async fn observe(&self, _event: CompactRetryEvent) {}
}
