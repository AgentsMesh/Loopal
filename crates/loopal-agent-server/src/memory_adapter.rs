//! Memory adapter for the agent server process.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use loopal_agent::shared::AgentShared;
use loopal_agent::spawn::{SpawnParams, spawn_agent, wait_agent};
use loopal_memory::{MEMORY_AGENT_PROMPT, MemoryProcessor};
use loopal_tool_api::MemoryChannel;

/// Adapts `mpsc::Sender<String>` to the `MemoryChannel` trait.
pub struct ServerMemoryChannel(pub mpsc::Sender<String>);

impl MemoryChannel for ServerMemoryChannel {
    fn try_send(&self, observation: String) -> Result<(), String> {
        self.0.try_send(observation).map_err(|e| e.to_string())
    }
}

/// Processes memory observations by spawning a memory-maintainer agent via Hub.
pub struct ServerMemoryProcessor {
    shared: Arc<AgentShared>,
    model: String,
}

impl ServerMemoryProcessor {
    pub fn new(shared: Arc<AgentShared>, model: String) -> Self {
        Self { shared, model }
    }
}

#[async_trait]
impl MemoryProcessor for ServerMemoryProcessor {
    async fn process(&self, observation: &str) -> Result<(), String> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let name = format!("memory-{ts:08x}");
        let params = SpawnParams {
            name: name.clone(),
            prompt: format!(
                "{MEMORY_AGENT_PROMPT}\n\nNew observation to incorporate:\n\n{observation}"
            ),
            model: Some(self.model.clone()),
            cwd_override: None,
            permission_mode: None,
        };
        spawn_agent(&self.shared, params).await?;
        info!("memory-maintainer agent spawned via Hub");

        // Wait for completion
        match wait_agent(&self.shared, &name).await {
            Ok(output) => {
                info!(output = %output, "memory-maintainer done");
                Ok(())
            }
            Err(e) => Err(format!("memory-maintainer error: {e}")),
        }
    }
}

/// Build the optional memory channel + observer sidebar.
pub fn build_memory_channel(
    long_lived: bool,
    settings: &loopal_config::Settings,
    shared: &Arc<AgentShared>,
    model: &str,
) -> Option<Arc<dyn MemoryChannel>> {
    if !(long_lived && settings.memory.enabled) {
        return None;
    }
    let (tx, rx) = mpsc::channel::<String>(64);
    let processor = Arc::new(ServerMemoryProcessor::new(
        shared.clone(),
        model.to_string(),
    ));
    tokio::spawn(loopal_memory::MemoryObserver::new(rx, processor).run());
    Some(Arc::new(ServerMemoryChannel(tx)))
}
