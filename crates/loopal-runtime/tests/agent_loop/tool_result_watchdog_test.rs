use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use loopal_tool_invocation::{StaleReason, ToolResultMetadata};
use serde_json::{Value, json};

use super::{in_turn, make_runner_with_kernel, make_turn_ctx};

struct StuckRead;

#[async_trait]
impl Tool for StuckRead {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "never returns"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, _: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn watchdog_timeout_flows_through_final_sink() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(StuckRead));
    let (mut runner, mut events, _) = make_runner_with_kernel(Arc::new(kernel));
    let mut turn_ctx = make_turn_ctx();

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![("stuck".into(), "Read".into(), json!({}))],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(stats.errors, 1);
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event.payload,
            AgentEventPayload::ToolResult {
                metadata: Some(ToolResultMetadata::Stale {
                    reason: StaleReason::WatchdogTimeout
                }),
                ..
            }
        ))
    );
}
