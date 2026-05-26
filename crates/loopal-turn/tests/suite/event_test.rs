use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, ToolResult, TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger,
};

#[test]
fn turn_started_event_round_trip() {
    let id = TurnId::new();
    let event = TurnEvent::TurnStarted {
        turn_id: id.clone(),
        started_at: chrono::Utc::now(),
        trigger: TurnTrigger::UserInput {
            envelope_id: "env-1".into(),
            content: "hi".into(),
            images: Vec::new(),
        },
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: TurnEvent = serde_json::from_str(&json).unwrap();
    matches!(back, TurnEvent::TurnStarted { .. });
}

#[test]
fn step_updated_event_carries_state_transition() {
    let event = TurnEvent::StepUpdated {
        turn_id: TurnId::from_string("t-fixed"),
        step_index: 1,
        item_index: 0,
        new_state: ToolExecState::Done(ToolResult {
            content: "ok".into(),
            is_error: false,
            images: vec![],
        }),
        updated_at: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: TurnEvent = serde_json::from_str(&json).unwrap();
    if let TurnEvent::StepUpdated {
        step_index,
        item_index,
        ..
    } = back
    {
        assert_eq!(step_index, 1);
        assert_eq!(item_index, 0);
    } else {
        panic!("expected StepUpdated");
    }
}

#[test]
fn turn_ended_event_with_outcome() {
    let event = TurnEvent::TurnEnded {
        turn_id: TurnId::from_string("t-1"),
        outcome: TurnOutcome::Complete,
    };
    let json = serde_json::to_string(&event).unwrap();
    let back: TurnEvent = serde_json::from_str(&json).unwrap();
    if let TurnEvent::TurnEnded { outcome, .. } = back {
        assert_eq!(outcome, TurnOutcome::Complete);
    } else {
        panic!("expected TurnEnded");
    }
}

#[test]
fn step_appended_carries_tool_batch() {
    let batch = OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new("c1"),
                name: "Bash".into(),
                input: serde_json::json!({}),
            },
            state: ToolExecState::Pending,
        }],
    };
    let event = TurnEvent::StepAppended {
        turn_id: TurnId::from_string("t-1"),
        step_index: 0,
        step: TurnStep::ToolBatch(batch),
        appended_at: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    let _back: TurnEvent = serde_json::from_str(&json).unwrap();
}

#[test]
fn assistant_output_with_thinking_serialization() {
    // 单 LLM call 即使 stop_reason=EndTurn 也合法（empty response 不算 protocol violation）
    let output = AssistantOutput {
        thinking: None,
        text_blocks: vec![],
        tool_calls: vec![],
        server_blocks: vec![],
        stop_reason: StopReason::EndTurn,
    };
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("EndTurn"));
}
