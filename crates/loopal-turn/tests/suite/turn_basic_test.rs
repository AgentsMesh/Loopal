use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, ToolResult, Turn, TurnOutcome, TurnStep, TurnTrigger,
};

fn make_user_turn() -> Turn {
    Turn::new(TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "test".into(),
    })
}

#[test]
fn turn_id_is_unique() {
    let a = make_user_turn();
    let b = make_user_turn();
    assert_ne!(a.id, b.id);
}

#[test]
fn turn_starts_in_progress() {
    let t = make_user_turn();
    assert_eq!(t.outcome, TurnOutcome::InProgress);
    assert!(t.body.steps.is_empty());
}

#[test]
fn ordered_tool_batch_preserves_call_order() {
    // I4 invariant: batch.items[i].call.id == prev_llm_call.tool_calls[i].id
    // 类型上 items 是 Vec<ToolBatchItem>，Vec 顺序保证此关系。
    let mut t = make_user_turn();
    let calls = vec![
        ToolCall {
            id: ToolCallId::new("a"),
            name: "Bash".into(),
            input: serde_json::json!({}),
        },
        ToolCall {
            id: ToolCallId::new("b"),
            name: "Read".into(),
            input: serde_json::json!({}),
        },
    ];
    t.body.steps.push(TurnStep::LlmCall {
        request_snapshot: snapshot(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: calls.clone(),
            server_blocks: vec![],
            stop_reason: StopReason::ToolUse,
        },
    });
    t.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![
            ToolBatchItem {
                call: calls[0].clone(),
                state: ToolExecState::Done(ok("a-result")),
            },
            ToolBatchItem {
                call: calls[1].clone(),
                state: ToolExecState::Done(ok("b-result")),
            },
        ],
    }));

    let TurnStep::ToolBatch(batch) = &t.body.steps[1] else {
        panic!()
    };
    assert_eq!(batch.items[0].call.id.as_str(), "a");
    assert_eq!(batch.items[1].call.id.as_str(), "b");
}

#[test]
fn turn_json_round_trip() {
    let t = make_user_turn();
    let json = serde_json::to_string(&t).unwrap();
    let back: Turn = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, t.id);
    assert_eq!(back.outcome, TurnOutcome::InProgress);
}

#[test]
fn turn_with_steps_round_trip() {
    let mut t = make_user_turn();
    t.body.steps.push(TurnStep::LlmCall {
        request_snapshot: snapshot(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    });
    let json = serde_json::to_string(&t).unwrap();
    let back: Turn = serde_json::from_str(&json).unwrap();
    assert_eq!(back.body.steps.len(), 1);
}

fn snapshot() -> loopal_turn::LlmRequestSnapshot {
    loopal_turn::LlmRequestSnapshot {
        model: "claude-opus-4-7".into(),
        max_tokens: 128_000,
        tool_count: 0,
        message_count: 0,
    }
}

fn ok(text: &str) -> ToolResult {
    ToolResult {
        content: text.into(),
        is_error: false,
        images: vec![],
    }
}
