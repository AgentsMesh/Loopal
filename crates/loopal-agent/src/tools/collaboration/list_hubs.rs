//! ListHubs tool — discover other hubs in the MetaHub cluster.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::json;

use super::shared_extract::extract_shared;

pub struct ListHubsTool;

#[async_trait]
impl Tool for ListHubsTool {
    fn name(&self) -> &str {
        "ListHubs"
    }
    fn description(&self) -> &str {
        "List all hubs connected to the MetaHub cluster. Returns hub names, \
         agent counts, and capabilities. Only works when connected to a MetaHub."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({"type": "object", "properties": {}})
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;

        let status = match shared
            .hub_connection
            .send_request(methods::HUB_STATUS.name, json!({}))
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(ToolResult::error(format!("hub/status failed: {e}"))),
        };

        if status["uplink"].is_null() {
            return Ok(ToolResult::success(
                "Not connected to a MetaHub cluster. \
                 Use --join-hub to connect this instance to a MetaHub.",
            ));
        }

        let hub_name = status["uplink"]["hub_name"].as_str().unwrap_or("unknown");

        let resp = shared
            .hub_connection
            .send_request(methods::META_LIST_HUBS.name, json!({}))
            .await;

        match resp {
            Ok(data) => {
                let mut out = format!("Connected to MetaHub as '{hub_name}'\n\nHubs:\n");
                if let Some(hubs) = data["hubs"].as_array() {
                    for h in hubs {
                        let n = h["name"].as_str().unwrap_or("?");
                        let a = h["agent_count"].as_u64().unwrap_or(0);
                        let s = h["status"].as_str().unwrap_or("?");
                        out.push_str(&format!("  - {n} ({s}, {a} agents)\n"));
                    }
                }
                Ok(ToolResult::success(out))
            }
            Err(e) => Ok(ToolResult::error(format!("meta/list_hubs failed: {e}"))),
        }
    }
}
