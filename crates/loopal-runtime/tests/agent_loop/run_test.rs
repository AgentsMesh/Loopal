use async_trait::async_trait;
use loopal_error::{LoopalError, TerminateReason};
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{COMPLETION_PREFIX, Tool, ToolContext, ToolResult};

use super::mock_provider::{make_interactive_multi_runner, make_runner_with_mock_provider};

#[tokio::test]
async fn test_full_run_stream_error_recovery_with_close() {
    // Tests stream_error && tool_uses.is_empty() && assistant_text.is_empty()
    // Then the wait_for_input channel is closed, so it breaks.
    let chunks = vec![Err(LoopalError::Provider(
        loopal_error::ProviderError::StreamEnded,
    ))];
    let (mut runner, mut event_rx, input_tx, ctrl_tx) = make_runner_with_mock_provider(chunks);

    drop(input_tx);
    drop(ctrl_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let result = runner.run().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_full_run_max_turns_with_messages_present() {
    // Tests turn_count >= max_turns with messages already present
    let chunks = vec![];
    let (mut runner, mut event_rx, input_tx, ctrl_tx) = make_runner_with_mock_provider(chunks);
    runner.params.config.max_turns = 0;

    drop(input_tx);
    drop(ctrl_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let result = runner.run().await;
    let output = result.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::MaxTurns);
}

struct FakeCompletionTool;
#[async_trait]
impl Tool for FakeCompletionTool {
    fn name(&self) -> &str {
        "AttemptCompletion"
    }
    fn description(&self) -> &str {
        "test"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let r = input
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("done");
        Ok(ToolResult::success(format!("{COMPLETION_PREFIX}{r}")))
    }
}

/// Interactive agent must NOT exit after AttemptCompletion — it should
/// proceed to wait_for_input (emitting AwaitingInput) before finishing.
#[tokio::test]
async fn test_interactive_completion_emits_awaiting_input() {
    let calls = vec![vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "all done"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx, mbox_tx, ctrl_tx) = make_interactive_multi_runner(calls, |k| {
        k.register_tool(Box::new(FakeCompletionTool));
    });

    // Drop senders: after AttemptCompletion, wait_for_input sees closed channels → exits
    drop(mbox_tx);
    drop(ctrl_tx);

    // Drain events in background
    let events = tokio::spawn(async move {
        let mut payloads = vec![];
        while let Some(e) = event_rx.recv().await {
            payloads.push(e.payload);
        }
        payloads
    });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    drop(runner); // Close event channel so the collector finishes
    let payloads = events.await.unwrap();

    // Key assertion: AwaitingInput was emitted AFTER completion (proves loop didn't break)
    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, AgentEventPayload::AwaitingInput)),
        "interactive agent should emit AwaitingInput after AttemptCompletion"
    );
}
