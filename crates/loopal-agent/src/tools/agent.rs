use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

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
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) summary of the agent's task"
                },
                "name": {
                    "type": "string",
                    "description": "A short name for this agent (e.g. 'researcher'). Auto-generated if omitted."
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Agent type from .loopal/agents/ (optional)"
                },
                "model": {
                    "type": "string",
                    "description": "LLM model override for this agent (inherits parent model if omitted)"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run in background without blocking. Default: false."
                }
            },
            "required": ["prompt", "description"]
        })
    }

    fn permission(&self) -> PermissionLevel { PermissionLevel::Supervised }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;

        let prompt = require_str(&input, "prompt")?;
        let name = match input.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => format!("agent-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x")),
        };
        let subagent_type = input.get("subagent_type").and_then(|v| v.as_str());
        let model_override = input.get("model").and_then(|v| v.as_str()).map(String::from);
        let run_in_background = input.get("run_in_background").and_then(|v| v.as_bool()).unwrap_or(false);

        if shared.depth >= shared.max_depth {
            return Ok(ToolResult::error(format!(
                "Maximum agent nesting depth ({}) reached", shared.max_depth,
            )));
        }

        // Check for duplicate name
        {
            let registry = shared.registry.lock().await;
            if registry.get(&name).is_some() {
                return Ok(ToolResult::error(format!(
                    "Agent with name '{name}' already exists"
                )));
            }
        }

        let mut agent_config = if let Some(agent_type) = subagent_type {
            load_agent_configs(&shared.cwd)
                .remove(agent_type)
                .unwrap_or_default()
        } else {
            AgentConfig::default()
        };

        // Model override: explicit param > agent config > parent model
        if let Some(ref m) = model_override {
            agent_config.model = Some(m.clone());
        }

        let parent_model = shared.kernel.settings().model.clone();

        let result = spawn_agent(&shared, SpawnParams {
            name: name.clone(),
            prompt: prompt.to_string(),
            agent_config,
            parent_model,
            parent_cancel_token: None,
        }).await;

        match result {
            Ok(spawn_result) => {
                shared.registry.lock().await.register(spawn_result.handle);
                if run_in_background {
                    // Non-blocking: return immediately with agent ID
                    Ok(ToolResult::success(format!(
                        "Agent '{name}' spawned in background.\nagentId: {}",
                        spawn_result.agent_id
                    )))
                } else {
                    // Blocking: wait for sub-agent to complete
                    match spawn_result.result_rx.await {
                        Ok(Ok(output)) => Ok(ToolResult::success(output)),
                        Ok(Err(err)) => Ok(ToolResult::error(err)),
                        Err(_) => Ok(ToolResult::error("sub-agent terminated unexpectedly")),
                    }
                }
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to spawn agent: {e}"))),
        }
    }
}

/// Extract `AgentShared` from `ToolContext.shared`.
/// The shared field stores `Arc<Arc<AgentShared>>` (outer for dyn Any erasure,
/// inner for cheap cloning), so we downcast to `Arc<AgentShared>`.
pub(crate) fn extract_shared(ctx: &ToolContext) -> Result<Arc<AgentShared>, LoopalError> {
    ctx.shared
        .as_ref()
        .and_then(|s| s.downcast_ref::<Arc<AgentShared>>())
        .cloned()
        .ok_or_else(|| LoopalError::Other(
            "AgentShared not available in ToolContext".into(),
        ))
}

fn require_str<'a>(input: &'a serde_json::Value, key: &str) -> Result<&'a str, LoopalError> {
    input.get(key).and_then(|v| v.as_str()).ok_or_else(|| {
        LoopalError::Tool(loopal_error::ToolError::InvalidInput(
            format!("missing '{key}'"),
        ))
    })
}
