use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{info, warn};

/// Default system prompt for the memory-maintainer agent, compiled into the binary.
pub const MEMORY_AGENT_PROMPT: &str = include_str!("../agent-prompts/memory-maintainer.md");

/// Default system prompt for memory consolidation, compiled into the binary.
pub const MEMORY_CONSOLIDATION_PROMPT: &str =
    include_str!("../agent-prompts/memory-consolidation.md");

/// Trait abstracting how a memory observation is processed.
///
/// Decouples the observer loop from the concrete processing strategy
/// (raw LLM call vs full agent spawn), avoiding circular crate dependencies.
#[async_trait]
pub trait MemoryProcessor: Send + Sync {
    /// Process a single observation. Errors are logged but do not stop the observer.
    async fn process(&self, observation: &str) -> Result<(), String>;

    /// Process a batch of observations in a single call.
    /// Default implementation calls `process` for each observation sequentially.
    async fn process_batch(&self, observations: &[String]) -> Result<(), String> {
        for obs in observations {
            self.process(obs).await?;
        }
        Ok(())
    }
}

/// Sidebar task that receives memory observations, batches them within a
/// configurable debounce window, and delegates to a processor.
pub struct MemoryObserver {
    rx: mpsc::Receiver<String>,
    processor: Arc<dyn MemoryProcessor>,
    batch_window: Duration,
}

impl MemoryObserver {
    /// Create a new observer with the given channel receiver, processor, and debounce window.
    pub fn new(
        rx: mpsc::Receiver<String>,
        processor: Arc<dyn MemoryProcessor>,
        batch_window: Duration,
    ) -> Self {
        Self {
            rx,
            processor,
            batch_window,
        }
    }

    /// Run until the channel is closed (session ends).
    ///
    /// Observations are collected in a debounce window: after the first observation
    /// arrives, the observer waits up to `batch_window` for additional observations
    /// before sending the batch to the processor.
    pub async fn run(mut self) {
        info!(
            "memory observer started (batch_window={}ms)",
            self.batch_window.as_millis()
        );
        loop {
            // Block until the first observation arrives (or channel closes).
            let first = match self.rx.recv().await {
                Some(obs) => obs,
                None => break, // channel closed
            };

            // Collect additional observations within the debounce window.
            let mut batch = vec![first];
            let deadline = sleep(self.batch_window);
            tokio::pin!(deadline);

            // Collect more observations until the debounce window expires or channel closes.
            // tokio::select! uses fair scheduling — either branch can fire when both are ready,
            // which is correct here: we want whichever event comes first.
            loop {
                tokio::select! {
                    maybe_obs = self.rx.recv() => {
                        match maybe_obs {
                            Some(obs) => batch.push(obs),
                            None => break, // channel closed, process remaining batch
                        }
                    }
                    _ = &mut deadline => break, // debounce window expired
                }
            }

            let count = batch.len();
            info!(count, "processing memory observation batch");

            let result = if count == 1 {
                self.processor.process(&batch[0]).await
            } else {
                self.processor.process_batch(&batch).await
            };

            if let Err(e) = result {
                warn!(error = %e, "memory consolidation failed");
            }
        }
        info!("memory observer stopped");
    }
}
