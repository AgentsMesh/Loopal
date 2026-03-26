//! Memory adapter for the agent server process.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::info;

use loopal_agent::config::AgentConfig;
use loopal_agent::shared::AgentShared;
use loopal_agent::spawn::{SpawnParams, spawn_agent};
use loopal_memory::{MEMORY_AGENT_PROMPT, MemoryProcessor};
use loopal_tool_api::MemoryChannel;

/// Adapts `mpsc::Sender<String>` to the `MemoryChannel` trait.
pub struct ServerMemoryChannel(pub mpsc::Sender<String>);

impl MemoryChannel for ServerMemoryChannel {
    fn try_send(&self, observation: String) -> Result<(), String> {
        self.0.try_send(observation).map_err(|e| e.to_string())
    }
}

/// Processes memory observations by spawning a memory-maintainer agent.
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
        let config = AgentConfig {
            name: "memory-maintainer".to_string(),
            description: "Maintains project memory files".to_string(),
            system_prompt: MEMORY_AGENT_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "Read".into(),
                "Write".into(),
                "Edit".into(),
                "Grep".into(),
                "Glob".into(),
                "Ls".into(),
            ]),
            max_turns: 10,
            ..Default::default()
        };
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let params = SpawnParams {
            name: format!("memory-{ts:08x}"),
            prompt: format!("New observation to incorporate:\n\n{observation}"),
            agent_config: config,
            parent_model: self.model.clone(),
            parent_cancel_token: None,
            cwd_override: None,
        };
        let result = spawn_agent(&self.shared, params).await?;
        info!("memory-maintainer agent spawned");
        match result.result_rx.await {
            Ok(Ok(output)) => {
                info!(output = %output, "memory-maintainer done");
                Ok(())
            }
            Ok(Err(e)) => Err(format!("memory-maintainer error: {e}")),
            Err(_) => Err("memory-maintainer channel dropped".into()),
        }
    }
}
