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
pub struct MpscMemoryChannel(pub mpsc::Sender<String>);

impl MemoryChannel for MpscMemoryChannel {
    fn try_send(&self, observation: String) -> Result<(), String> {
        self.0.try_send(observation).map_err(|e| e.to_string())
    }
}

/// Processes memory observations by spawning a full memory-maintainer agent.
pub struct AgentMemoryProcessor {
    shared: Arc<AgentShared>,
    model: String,
}

impl AgentMemoryProcessor {
    pub fn new(shared: Arc<AgentShared>, model: String) -> Self {
        Self { shared, model }
    }
}

#[async_trait]
impl MemoryProcessor for AgentMemoryProcessor {
    async fn process(&self, observation: &str) -> Result<(), String> {
        let config = AgentConfig {
            name: "memory-maintainer".to_string(),
            description: "Maintains project memory files".to_string(),
            system_prompt: MEMORY_AGENT_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "Read".into(), "Write".into(), "Edit".into(),
                "Grep".into(), "Glob".into(), "Ls".into(),
                "AttemptCompletion".into(),
            ]),
            max_turns: 10,
            ..Default::default()
        };

        let agent_name = format!("memory-{:08x}", rand_id());
        let params = SpawnParams {
            name: agent_name,
            prompt: format!("New observation to incorporate:\n\n{observation}"),
            agent_config: config,
            parent_model: self.model.clone(),
            parent_cancel_token: None,
            cwd_override: None,
        };

        let result = spawn_agent(&self.shared, params).await?;
        info!("memory-maintainer agent spawned, waiting for completion");

        match result.result_rx.await {
            Ok(Ok(output)) => {
                info!(output = %output, "memory-maintainer completed");
                Ok(())
            }
            Ok(Err(e)) => Err(format!("memory-maintainer error: {e}")),
            Err(_) => Err("memory-maintainer channel dropped".into()),
        }
    }
}

fn rand_id() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}
