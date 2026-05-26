use super::*;
use loopal_turn::{
    AssistantOutput, ServerToolCall, ServerToolPair, ServerToolResult, StopReason, Turn, TurnStep,
    TurnTrigger,
};

#[test]
fn condense_server_blocks_in_turns_clears_pairs_and_appends_marker() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![ServerToolPair {
                call: ServerToolCall {
                    id: "s1".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({"q": "x"}),
                },
                result: ServerToolResult {
                    block_type: "web_search_tool_result".into(),
                    content: serde_json::json!({"hits": []}),
                },
            }],
            stop_reason: StopReason::EndTurn,
        },
    });
    let mut turns = vec![turn];
    condense_server_blocks_in_turns(&mut turns);
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!("expected LlmCall step");
    };
    assert!(response.server_blocks.is_empty());
    assert_eq!(response.text_blocks.len(), 1);
    assert!(response.text_blocks[0].text.contains("web_search"));
    assert!(response.text_blocks[0].text.contains("condensed"));
}

#[test]
fn condense_server_blocks_in_turns_noop_when_empty() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    });
    let mut turns = vec![turn];
    condense_server_blocks_in_turns(&mut turns);
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!("expected LlmCall step");
    };
    assert!(response.text_blocks.is_empty());
}
