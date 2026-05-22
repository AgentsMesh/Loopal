use loopal_message::ContentBlock;

use super::{in_turn, make_cancel, make_turn_ctx, make_runner};

async fn run_ask_user_with_input(
    runner: &mut loopal_runtime::agent_loop::AgentLoopRunner,
    input: serde_json::Value,
) -> ContentBlock {
    let tool_uses = vec![("tc-bad".to_string(), "AskUser".to_string(), input)];
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(&mut turn_ctx, tool_uses, loopal_runtime::agent_loop::StreamingToolHandle::empty()))
    .await
    .unwrap();
    runner.params.store.messages()[0].content[0].clone()
}

fn assert_schema_err(block: &ContentBlock) {
    match block {
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => {
            assert_eq!(tool_use_id, "tc-bad");
            assert!(
                *is_error,
                "schema_err must emit is_error=true so the LLM retries; got is_error=false with content={content:?}"
            );
            assert!(
                content.contains("AskUser parameter validation failed"),
                "schema_err content should explain the failure: {content}"
            );
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn schema_err_when_questions_missing() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(&mut runner, serde_json::json!({})).await;
    assert_schema_err(&block);
}

#[tokio::test]
async fn schema_err_when_questions_not_array() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(
        &mut runner,
        serde_json::json!({"questions": "not an array"}),
    )
    .await;
    assert_schema_err(&block);
}

#[tokio::test]
async fn schema_err_when_questions_empty_array() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(&mut runner, serde_json::json!({"questions": []})).await;
    assert_schema_err(&block);
}

#[tokio::test]
async fn schema_err_when_question_text_missing() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(
        &mut runner,
        serde_json::json!({
            "questions": [{
                "options": [
                    {"label": "A", "description": "a"},
                    {"label": "B", "description": "b"}
                ]
            }]
        }),
    )
    .await;
    assert_schema_err(&block);
}

#[tokio::test]
async fn schema_err_when_options_missing() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(
        &mut runner,
        serde_json::json!({
            "questions": [{"question": "Pick one"}]
        }),
    )
    .await;
    assert_schema_err(&block);
}

#[tokio::test]
async fn schema_err_when_option_label_empty() {
    let (mut runner, _rx) = make_runner();
    let block = run_ask_user_with_input(
        &mut runner,
        serde_json::json!({
            "questions": [{
                "question": "Pick one",
                "options": [
                    {"label": "", "description": "empty label"},
                    {"label": "B", "description": "b"}
                ]
            }]
        }),
    )
    .await;
    assert_schema_err(&block);
}
