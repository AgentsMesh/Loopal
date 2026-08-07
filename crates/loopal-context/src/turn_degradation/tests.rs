use super::*;
use loopal_tool_invocation as _;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, ServerBlock, ServerToolCall, ServerToolPair,
    ServerToolResult, StopReason, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolCallId,
    ToolResult, Turn, TurnTrigger,
};

fn budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 80_000,
        max_output_tokens: 64_000,
    }
}

fn make_turn_with_llm_call(thinking: bool) -> Turn {
    let mut turn = Turn::new(TurnTrigger::Resume);
    let server_blocks = if thinking {
        vec![ServerBlock::Reasoning(ThinkingBlock {
            thinking: "deep thoughts".into(),
            signature: Some("sig-1".into()),
        })]
    } else {
        vec![]
    };
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![TextBlock {
                text: "reply".into(),
            }],
            tool_calls: vec![],
            server_blocks,
            stop_reason: StopReason::EndTurn,
        },
    });
    turn
}

fn make_turn_with_tool_result(body: String) -> Turn {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new("tc-1"),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
            state: ToolExecState::Done(ToolResult {
                content: body,
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }),
        }],
    }));
    turn
}

#[test]
fn strips_reasoning_from_old_turns_only() {
    let mut turns = vec![make_turn_with_llm_call(true), make_turn_with_llm_call(true)];
    degrade_turns_for_wire(&mut turns, &budget());
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!();
    };
    assert!(
        response.server_blocks.is_empty(),
        "old turn reasoning must be dropped"
    );
    let TurnStep::LlmCall { response, .. } = &turns[1].body.steps[0] else {
        panic!();
    };
    assert!(
        response
            .server_blocks
            .iter()
            .any(|b| matches!(b, ServerBlock::Reasoning(_))),
        "current turn reasoning must be kept"
    );
}

// reason: 旧 turn 的 ToolPair 必须被 condense 成 marker text(web_search_call 已不
// 存在,其 reasoning 锚点亦无需保留),覆盖 degrade 的 ToolPair 臂。
#[test]
fn condenses_tool_pair_in_old_turn_to_marker() {
    let mut old = Turn::new(TurnTrigger::Resume);
    old.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![ServerBlock::ToolPair(ServerToolPair {
                call: ServerToolCall {
                    id: "ws_1".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({}),
                },
                result: ServerToolResult {
                    block_type: "web_search_tool_result".into(),
                    content: serde_json::json!({}),
                },
            })],
            stop_reason: StopReason::EndTurn,
        },
    });
    let mut turns = vec![old, make_turn_with_llm_call(false)];
    degrade_turns_for_wire(&mut turns, &budget());
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!();
    };
    assert!(response.server_blocks.is_empty());
    assert!(
        response
            .text_blocks
            .iter()
            .any(|t| t.text.contains("web_search") && t.text.contains("condensed")),
        "old-turn ToolPair must condense to a marker"
    );
}

#[test]
fn caps_oversized_tool_results() {
    let huge = "x".repeat(200_000);
    let mut turns = vec![make_turn_with_tool_result(huge)];
    degrade_turns_for_wire(&mut turns, &budget());
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[0] else {
        panic!();
    };
    let ToolExecState::Done(r) = &batch.items[0].state else {
        panic!();
    };
    assert!(r.content.len() < 200_000);
    assert!(r.content.contains("Truncated:"));
}

#[test]
fn empty_turns_noop() {
    let mut turns: Vec<Turn> = Vec::new();
    degrade_turns_for_wire(&mut turns, &budget());
}

// reason: reasoning token 必须计入预算,否则删 thinking 字段后 reasoning 体量从
// compaction 预算消失、触发时机偏移。
#[test]
fn estimate_counts_reasoning_text() {
    let with = estimate_turns_tokens(&[make_turn_with_llm_call(true)]);
    let without = estimate_turns_tokens(&[make_turn_with_llm_call(false)]);
    assert!(
        with > without,
        "reasoning text must contribute to the token estimate"
    );
}
