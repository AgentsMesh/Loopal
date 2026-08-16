use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use loopal_provider_api::ContentBlock;
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_secret_client::{IpcBudget, SecretClient, SecretResult};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use secrecy::SecretString;
use serde_json::{Value, json};

use super::{in_turn, make_runner_with_kernel, make_runner_with_question_channel, make_turn_ctx};

const PLAINTEXT: &str = "tool-error-plaintext-canary";

struct OneSecret;

#[async_trait]
impl SecretClient for OneSecret {
    async fn get(&self, _: &str, _: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(PLAINTEXT))
    }

    async fn list_names(&self, _: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(vec!["token".into()])
    }

    async fn expand_author(&self, value: &str, _: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(value.to_string()))
    }

    async fn expand_wire(&self, value: &str, _: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(value.to_string()))
    }
}

struct SecretError;

#[async_trait]
impl Tool for SecretError {
    fn name(&self) -> &str {
        "SecretError"
    }

    fn description(&self) -> &str {
        "returns an error containing its resolved input"
    }

    fn parameters_schema(&self) -> Value {
        json!({"type": "object", "required": ["value"], "properties": {"value": {"type": "string"}}})
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &["value"]
    }

    async fn execute(&self, input: Value, _: &ToolContext) -> Result<ToolResult, LoopalError> {
        Err(LoopalError::Other(format!(
            "consumer failed with {}",
            input["value"].as_str().unwrap()
        )))
    }
}

#[tokio::test]
async fn tool_error_keeps_secret_seed_through_event_and_persistence() {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.set_secret_client(Arc::new(OneSecret));
    kernel.register_tool(Box::new(SecretError));
    let (mut runner, mut event_rx, _) = make_runner_with_kernel(Arc::new(kernel));
    let mut turn_ctx = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![(
            "secret-error".into(),
            "SecretError".into(),
            json!({"value": "<secret_ref:token>"}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let event_result = loop {
        let event = event_rx.recv().await.unwrap();
        if let AgentEventPayload::ToolResult { result, .. } = event.payload {
            break result;
        }
    };
    assert!(event_result.contains("<secret_ref:token>"));
    assert!(!event_result.contains(PLAINTEXT));
    let ContentBlock::ToolResult { content, .. } = &runner.turns.view().messages()[0].content[0]
    else {
        panic!("expected ToolResult");
    };
    assert_eq!(content, &event_result);
}

#[tokio::test]
async fn oversized_runner_direct_result_becomes_bounded_error_everywhere() {
    let (mut runner, mut event_rx, question_tx) = make_runner_with_question_channel();
    let huge = "x".repeat(loopal_tool_api::DEFAULT_MAX_OUTPUT_BYTES + 1);
    question_tx
        .send(UserQuestionResponse::answered("", vec![huge.clone()]))
        .await
        .unwrap();
    let mut turn_ctx = make_turn_ctx();

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![(
            "ask-large".into(),
            "AskUser".into(),
            json!({"questions": [{
                "question": "Large answer?",
                "options": [{"label": "A", "description": "A"}, {"label": "B", "description": "B"}]
            }]}),
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
    assert!(!event_result.contains(&huge));
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
