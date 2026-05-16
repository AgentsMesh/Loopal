use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use loopal_tool_background::BackgroundTaskStore;
use schemars::JsonSchema;
use serde::Deserialize;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT: Duration = Duration::from_secs(600);

pub struct BashProcessTool {
    store: Arc<BackgroundTaskStore>,
}

impl BashProcessTool {
    pub fn new(store: Arc<BackgroundTaskStore>) -> Self {
        Self { store }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct BashProcessParams {
    pub process_id: String,
    #[serde(default)]
    pub block: Option<bool>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub stop: Option<bool>,
}

#[async_trait]
impl TypedTool<BashProcessParams> for BashProcessTool {
    fn name(&self) -> &str {
        "BashProcess"
    }

    fn description(&self) -> &str {
        "Manage a background process started by the Bash tool.\n\n\
         Use this tool to check output or stop a running background process.\n\
         - Provide `process_id` to identify the process.\n\
         - Set `stop: true` to terminate the process.\n\
         - Set `block: true` (default) to wait for completion, or `false` for a non-blocking check.\n\
         - Optionally set `timeout` in seconds (max 600s)."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: BashProcessParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        if input.stop.unwrap_or(false) {
            return Ok(loopal_tool_background::ops::bg_stop(
                &self.store,
                &input.process_id,
            ));
        }

        let block = input.block.unwrap_or(true);
        let timeout_secs = input.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);
        let timeout = Duration::from_secs(timeout_secs).min(MAX_TIMEOUT);

        Ok(
            loopal_tool_background::ops::bg_output(&self.store, &input.process_id, block, timeout)
                .await,
        )
    }
}
