//! Memory adapter for the agent server process.
//!
//! Bridges the generic memory traits (loopal-memory) with concrete agent spawning
//! and filesystem operations.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::{info, warn};

use loopal_agent::shared::AgentShared;
use loopal_agent::spawn::{SpawnParams, SpawnTarget, spawn_agent, wait_agent};
use loopal_memory::{MEMORY_AGENT_PROMPT, MemoryProcessor};
use loopal_tool_api::MemoryChannel;

// ---------------------------------------------------------------------------
// Channel adapter
// ---------------------------------------------------------------------------

/// Adapts `mpsc::Sender<String>` to the `MemoryChannel` trait.
pub(crate) struct ServerMemoryChannel(mpsc::Sender<String>);

impl MemoryChannel for ServerMemoryChannel {
    fn try_send(&self, observation: String) -> Result<(), String> {
        self.0.try_send(observation).map_err(|e| {
            warn!("memory observation dropped: channel full");
            e.to_string()
        })
    }
}

// ---------------------------------------------------------------------------
// Observation processor
// ---------------------------------------------------------------------------

/// Processes memory observations by spawning a memory-maintainer agent via Hub.
pub(crate) struct ServerMemoryProcessor {
    shared: Arc<AgentShared>,
    model: String,
}

impl ServerMemoryProcessor {
    pub fn new(shared: Arc<AgentShared>, model: String) -> Self {
        Self { shared, model }
    }

    pub(crate) fn make_agent_name(prefix: &str) -> String {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        format!("{prefix}-{ts:08x}")
    }

    async fn spawn_and_wait(&self, name: &str, prompt: String) -> Result<(), String> {
        let params = SpawnParams {
            name: name.to_string(),
            prompt,
            model: Some(self.model.clone()),
            permission_mode: None,
            agent_type: None,
            depth: self.shared.depth + 1,
            target: SpawnTarget::InHub {
                cwd_override: None,
                fork_context: None,
            },
        };
        spawn_agent(&self.shared, params).await?;
        match wait_agent(&self.shared, name).await {
            Ok(output) => {
                info!(output = %output, "memory-maintainer done");
                Ok(())
            }
            Err(e) => Err(format!("memory-maintainer error: {e}")),
        }
    }
}

#[async_trait]
impl MemoryProcessor for ServerMemoryProcessor {
    async fn process(&self, observation: &str) -> Result<(), String> {
        let name = Self::make_agent_name("memory");
        let today = loopal_memory::date::today_str();
        let prompt = format!(
            "{MEMORY_AGENT_PROMPT}\n\n\
             Today: {today}\n\n\
             ## Observations to incorporate\n\n\
             1. {observation}"
        );
        info!("spawning memory-maintainer agent (single observation)");
        self.spawn_and_wait(&name, prompt).await
    }

    async fn process_batch(&self, observations: &[String]) -> Result<(), String> {
        let name = Self::make_agent_name("memory");
        let today = loopal_memory::date::today_str();
        let numbered: String = observations
            .iter()
            .enumerate()
            .map(|(i, obs)| format!("{}. {}", i + 1, obs))
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "{MEMORY_AGENT_PROMPT}\n\n\
             Today: {today}\n\n\
             ## Observations to incorporate\n\n\
             {numbered}"
        );
        info!(
            count = observations.len(),
            "spawning memory-maintainer agent (batch)"
        );
        self.spawn_and_wait(&name, prompt).await
    }
}

// ---------------------------------------------------------------------------
// Pipeline builder (entry point)
// ---------------------------------------------------------------------------

/// Build the optional memory channel + observer sidebar.
///
/// Also checks if memory consolidation is due and triggers it in the background.
pub fn build_memory_channel(
    long_lived: bool,
    settings: &loopal_config::Settings,
    shared: &Arc<AgentShared>,
    model: &str,
) -> Option<Arc<dyn MemoryChannel>> {
    if !(long_lived && settings.memory.enabled) {
        return None;
    }

    // Check if consolidation is due
    if settings.memory.consolidation_interval_days > 0
        && loopal_memory::consolidation::needs_consolidation(
            &shared.cwd.join(".loopal/memory"),
            settings.memory.consolidation_interval_days,
        )
    {
        info!("memory consolidation due — triggering in background");
        crate::memory_consolidation::trigger_consolidation(shared, model);
    }

    // Channel capacity from config (default: 256)
    let buffer = settings.memory.channel_buffer;
    // Debounce window from config (default: 2000ms)
    let batch_window = Duration::from_millis(settings.memory.batch_window_ms);

    let (tx, rx) = mpsc::channel::<String>(buffer);
    let processor = Arc::new(ServerMemoryProcessor::new(
        shared.clone(),
        model.to_string(),
    ));
    tokio::spawn(loopal_memory::MemoryObserver::new(rx, processor, batch_window).run());
    Some(Arc::new(ServerMemoryChannel(tx)))
}
