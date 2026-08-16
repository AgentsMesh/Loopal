use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use super::{in_turn, make_runner_with_kernel, make_turn_ctx};

struct OversizedError;

#[async_trait]
impl Tool for OversizedError {
    fn name(&self) -> &str {
        "OversizedError"
    }

    fn description(&self) -> &str {
        "returns an oversized execution error"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object", "required": ["value"], "properties": {"value": {"type": "string"}}})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        Err(LoopalError::Other(input["value"].as_str().unwrap().into()))
    }
}

#[tokio::test]
async fn oversized_execution_error_becomes_bounded_error_everywhere() {
    let kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(OversizedError));
    let (mut runner, mut event_rx, _) = make_runner_with_kernel(Arc::new(kernel));
    let huge = "z".repeat(loopal_tool_api::DEFAULT_MAX_OUTPUT_BYTES + 1);
    let mut turn_ctx = make_turn_ctx();

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![(
            "oversized-error".into(),
            "OversizedError".into(),
            json!({"value": huge}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(stats.errors, 1);
    let event_result = loop {
        let event = event_rx.recv().await.unwrap();
        if let AgentEventPayload::ToolResult {
            result,
            is_error: true,
            ..
        } = event.payload
        {
            break result;
        }
    };
    assert!(event_result.contains("final byte limit"));
    assert!(!event_result.contains(&"z".repeat(1024)));
    let ContentBlock::ToolResult {
        content,
        is_error: true,
        ..
    } = &runner.turns.view().messages()[0].content[0]
    else {
        panic!("expected error ToolResult");
    };
    assert_eq!(content, &event_result);
}
