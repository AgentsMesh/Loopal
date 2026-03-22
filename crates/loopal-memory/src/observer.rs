use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Default system prompt for the memory-maintainer agent, compiled into the binary.
pub const MEMORY_AGENT_PROMPT: &str = include_str!("../agent-prompts/memory-maintainer.md");

/// Trait abstracting how a memory observation is processed.
///
/// Decouples the observer loop from the concrete processing strategy
/// (raw LLM call vs full agent spawn), avoiding circular crate dependencies.
#[async_trait]
pub trait MemoryProcessor: Send + Sync {
    /// Process a single observation. Errors are logged but do not stop the observer.
    async fn process(&self, observation: &str) -> Result<(), String>;
}

/// Sidebar task that receives memory observations and delegates to a processor.
pub struct MemoryObserver {
    rx: mpsc::Receiver<String>,
    processor: Arc<dyn MemoryProcessor>,
}

impl MemoryObserver {
    pub fn new(rx: mpsc::Receiver<String>, processor: Arc<dyn MemoryProcessor>) -> Self {
        Self { rx, processor }
    }

    /// Run until the channel is closed (session ends).
    pub async fn run(mut self) {
        info!("memory observer started");
        while let Some(observation) = self.rx.recv().await {
            info!(observation = %observation, "processing memory observation");
            if let Err(e) = self.processor.process(&observation).await {
                warn!(error = %e, "memory consolidation failed");
            }
        }
        info!("memory observer stopped");
    }
}
