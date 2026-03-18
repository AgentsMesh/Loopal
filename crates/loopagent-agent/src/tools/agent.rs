use std::sync::Arc;

use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};

use crate::config::{AgentConfig, load_agent_configs};
use crate::shared::AgentShared;
use crate::spawn::{SpawnParams, spawn_agent};

/// Tool that spawns a new sub-agent to work on a task.
pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str { "Agent" }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a task autonomously. \
         The sub-agent runs in the background and can use tools independently."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task description for the sub-agent"
                },
                "name": {
                    "type": "string",
                    "description": "A short name for this agent (e.g. 'researcher')"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Agent type from .loopagent/agents/ (optional)"
                }
            },
            "required": ["prompt", "name"]
        })
    }

    fn permission(&self) -> PermissionLevel { PermissionLevel::Supervised }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopAgentError> {
        let shared = extract_shared(ctx)?;

        let prompt = require_str(&input, "prompt")?;
        let name = require_str(&input, "name")?;
        let subagent_type = input.get("subagent_type").and_then(|v| v.as_str());

        if shared.depth >= shared.max_depth {
            return Ok(ToolResult::error(format!(
                "Maximum agent nesting depth ({}) reached", shared.max_depth,
            )));
        }

        // Check for duplicate name
        {
            let registry = shared.registry.lock().await;
            if registry.get(name).is_some() {
                return Ok(ToolResult::error(format!(
                    "Agent with name '{name}' already exists"
                )));
            }
        }

        let agent_config = if let Some(agent_type) = subagent_type {
            load_agent_configs(&shared.cwd)
                .remove(agent_type)
                .unwrap_or_default()
        } else {
            AgentConfig::default()
        };

        // Inherit model from current agent's kernel settings
        let parent_model = shared.kernel.settings().model.clone();

        let result = spawn_agent(&shared, SpawnParams {
            name: name.to_string(),
            prompt: prompt.to_string(),
            agent_config,
            parent_model,
            parent_cancel_token: None, // TODO: propagate from parent when available
        }).await;

        match result {
            Ok(spawn_result) => {
                shared.registry.lock().await.register(spawn_result.handle);
                // Block until sub-agent completes — parallel Agent tools run via JoinSet
                match spawn_result.result_rx.await {
                    Ok(Ok(output)) => Ok(ToolResult::success(output)),
                    Ok(Err(err)) => Ok(ToolResult::error(err)),
                    Err(_) => Ok(ToolResult::error("sub-agent terminated unexpectedly")),
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to spawn agent: {e}"))),
        }
    }
}

/// Extract `AgentShared` from `ToolContext.shared`.
/// The shared field stores `Arc<Arc<AgentShared>>` (outer for dyn Any erasure,
/// inner for cheap cloning), so we downcast to `Arc<AgentShared>`.
pub(crate) fn extract_shared(ctx: &ToolContext) -> Result<Arc<AgentShared>, LoopAgentError> {
    ctx.shared
        .as_ref()
        .and_then(|s| s.downcast_ref::<Arc<AgentShared>>())
        .cloned()
        .ok_or_else(|| LoopAgentError::Other(
            "AgentShared not available in ToolContext".into(),
        ))
}

fn require_str<'a>(input: &'a serde_json::Value, key: &str) -> Result<&'a str, LoopAgentError> {
    input.get(key).and_then(|v| v.as_str()).ok_or_else(|| {
        LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
            format!("missing '{key}'"),
        ))
    })
}
