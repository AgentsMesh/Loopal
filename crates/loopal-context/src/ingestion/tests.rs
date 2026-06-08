use super::*;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, ServerBlock, ServerToolCall, ServerToolPair,
    ServerToolResult, StopReason, ThinkingBlock, Turn, TurnStep, TurnTrigger,
};

#[test]
fn condense_server_blocks_in_turns_clears_pairs_and_appends_marker() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![ServerBlock::ToolPair(ServerToolPair {
                call: ServerToolCall {
                    id: "s1".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({"q": "x"}),
                },
                result: ServerToolResult {
                    block_type: "web_search_tool_result".into(),
                    content: serde_json::json!({"hits": []}),
                },
            })],
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

// reason: 混合 Reasoning + ToolPair 时,只有 ToolPair 产 marker;Reasoning 一并清除
// 但不产 marker(避免把 reasoning 误当 server tool result)。
#[test]
fn condense_clears_reasoning_without_marker_keeps_one_per_pair() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![
                ServerBlock::Reasoning(ThinkingBlock {
                    thinking: "r".into(),
                    signature: Some("rs_1".into()),
                }),
                ServerBlock::ToolPair(ServerToolPair {
                    call: ServerToolCall {
                        id: "ws_1".into(),
                        name: "web_search".into(),
                        input: serde_json::json!({}),
                    },
                    result: ServerToolResult {
                        block_type: "web_search_tool_result".into(),
                        content: serde_json::json!({}),
                    },
                }),
            ],
            stop_reason: StopReason::EndTurn,
        },
    });
    // 非 LlmCall step → 命中 condense 循环的 continue 臂(防御性跳过)。
    turn.body
        .steps
        .push(TurnStep::ToolBatch(OrderedToolBatch { items: vec![] }));
    let mut turns = vec![turn];
    condense_server_blocks_in_turns(&mut turns);
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!("expected LlmCall step");
    };
    assert!(
        response.server_blocks.is_empty(),
        "all server blocks cleared"
    );
    let markers = response
        .text_blocks
        .iter()
        .filter(|t| t.text.contains("condensed"))
        .count();
    assert_eq!(
        markers, 1,
        "exactly one marker (from the ToolPair, not Reasoning)"
    );
}
