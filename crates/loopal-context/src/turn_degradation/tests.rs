use super::*;
use loopal_tool_invocation as _;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, TextBlock, ThinkingBlock, ToolBatchItem,
    ToolCall, ToolCallId, ToolResult, Turn, TurnTrigger,
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
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            thinking: if thinking {
                Some(ThinkingBlock {
                    thinking: "deep thoughts".into(),
                    signature: None,
                })
            } else {
                None
            },
            text_blocks: vec![TextBlock {
                text: "reply".into(),
            }],
            tool_calls: vec![],
            server_blocks: vec![],
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
            }),
        }],
    }));
    turn
}

#[test]
fn strips_thinking_from_old_turns_only() {
    let mut turns = vec![make_turn_with_llm_call(true), make_turn_with_llm_call(true)];
    degrade_turns_for_wire(&mut turns, &budget());
    let TurnStep::LlmCall { response, .. } = &turns[0].body.steps[0] else {
        panic!();
    };
    assert!(response.thinking.is_none());
    let TurnStep::LlmCall { response, .. } = &turns[1].body.steps[0] else {
        panic!();
    };
    assert!(response.thinking.is_some());
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
