use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_scheduler::CronScheduler;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::collaboration::shared_extract::extract_shared;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn extract_scheduler(ctx: &ToolContext) -> Result<Arc<CronScheduler>, LoopalError> {
    Ok(extract_shared(ctx)?.scheduler_handle.scheduler.clone())
}

// ---------------------------------------------------------------------------
// CronCreate
// ---------------------------------------------------------------------------

pub struct CronCreateTool;

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> &str {
        "CronCreate"
    }

    fn description(&self) -> &str {
        "Schedule a prompt to be enqueued at a future time. \
         Uses standard 5-field cron (minute hour dom month dow). \
         Set recurring=false for one-shot tasks that auto-delete after firing. \
         Jobs only fire while the agent is idle. \
         Recurring jobs auto-expire after 3 days. Max 50 concurrent jobs."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["cron", "prompt"],
            "properties": {
                "cron": {
                    "type": "string",
                    "description": "5-field cron expression, e.g. \"*/5 * * * *\""
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to enqueue at each fire time"
                },
                "recurring": {
                    "type": "boolean",
                    "description": "true (default) = recurring; false = one-shot"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let cron_expr = input["cron"].as_str().ok_or(LoopalError::Tool(
            loopal_error::ToolError::InvalidInput("cron is required".into()),
        ))?;
        let prompt = input["prompt"].as_str().ok_or(LoopalError::Tool(
            loopal_error::ToolError::InvalidInput("prompt is required".into()),
        ))?;
        if prompt.len() > 4096 {
            return Ok(ToolResult::error("prompt exceeds 4096 character limit"));
        }
        if prompt.is_empty() {
            return Ok(ToolResult::error("prompt cannot be empty"));
        }
        let recurring = input["recurring"].as_bool().unwrap_or(true);
        let scheduler = extract_scheduler(ctx)?;
        match scheduler.add(cron_expr, prompt, recurring).await {
            Ok(id) => {
                let kind = if recurring { "recurring" } else { "one-shot" };
                Ok(ToolResult::success(format!(
                    "Scheduled {kind} job {id} with cron \"{cron_expr}\""
                )))
            }
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// CronDelete
// ---------------------------------------------------------------------------

pub struct CronDeleteTool;

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> &str {
        "CronDelete"
    }

    fn description(&self) -> &str {
        "Cancel a scheduled cron job by its ID."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The 8-char job ID returned by CronCreate"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let id = input["id"].as_str().ok_or(LoopalError::Tool(
            loopal_error::ToolError::InvalidInput("id is required".into()),
        ))?;
        let scheduler = extract_scheduler(ctx)?;
        if scheduler.remove(id).await {
            Ok(ToolResult::success(format!("Cancelled job {id}")))
        } else {
            Ok(ToolResult::error(format!("No job found with id {id}")))
        }
    }
}

// ---------------------------------------------------------------------------
// CronList
// ---------------------------------------------------------------------------

pub struct CronListTool;

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str {
        "CronList"
    }

    fn description(&self) -> &str {
        "List all active cron jobs scheduled in this session."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let scheduler = extract_scheduler(ctx)?;
        let tasks = scheduler.list().await;
        if tasks.is_empty() {
            return Ok(ToolResult::success("No scheduled jobs."));
        }
        let json = serde_json::to_string_pretty(&tasks).unwrap_or_else(|_| "[]".to_string());
        Ok(ToolResult::success(json))
    }
}
