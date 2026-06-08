use loopal_storage::{finalize_incomplete_turns, synthesize_missing_tool_batches};
use loopal_turn::{
    AssistantOutput, CancelCause, CancelledCause, OrderedToolBatch, StopReason, ToolBatchItem,
    ToolCall, ToolCallId, ToolExecState, Turn, TurnOutcome, TurnStep, TurnTrigger,
};

fn turn_with_llm_call_and_tools(tool_ids: &[&str]) -> Turn {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: tool_ids
                .iter()
                .map(|id| ToolCall {
                    id: ToolCallId::new(*id),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                })
                .collect(),
            server_blocks: vec![],
            stop_reason: StopReason::ToolUse,
        },
    });
    turn
}

fn llm_call_with_tool(id: &str) -> TurnStep {
    TurnStep::LlmCall {
        model: "test".into(),
        response: AssistantOutput {
            text_blocks: Vec::new(),
            tool_calls: vec![ToolCall {
                id: ToolCallId::new(id),
                name: "Read".into(),
                input: serde_json::json!({}),
            }],
            server_blocks: Vec::new(),
            stop_reason: StopReason::ToolUse,
        },
    }
}

fn tool_batch_pending(id: &str) -> TurnStep {
    TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new(id),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
            state: ToolExecState::Pending,
        }],
    })
}

#[test]
fn synthesize_creates_cancelled_batch() {
    let mut turn = turn_with_llm_call_and_tools(&["tc-1", "tc-2"]);
    synthesize_missing_tool_batches(&mut turn);
    assert_eq!(turn.body.steps.len(), 2);
    let TurnStep::ToolBatch(batch) = &turn.body.steps[1] else {
        panic!("expected ToolBatch step");
    };
    assert_eq!(batch.items.len(), 2);
    for item in &batch.items {
        assert!(matches!(
            item.state,
            ToolExecState::Cancelled(CancelCause::CrashRecovery)
        ));
    }
}

#[test]
fn synthesize_noop_when_paired() {
    let mut turn = turn_with_llm_call_and_tools(&["tc-1"]);
    turn.body.steps.push(tool_batch_pending("tc-1"));
    let before = turn.body.steps.len();
    synthesize_missing_tool_batches(&mut turn);
    assert_eq!(turn.body.steps.len(), before);
}

#[test]
fn synthesize_noop_when_no_tool_calls() {
    let mut turn = turn_with_llm_call_and_tools(&[]);
    synthesize_missing_tool_batches(&mut turn);
    assert_eq!(turn.body.steps.len(), 1);
}

#[test]
fn synthesize_no_cross_pairing_between_llmcalls() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(llm_call_with_tool("X"));
    turn.body.steps.push(llm_call_with_tool("Y"));
    turn.body.steps.push(tool_batch_pending("Y"));
    synthesize_missing_tool_batches(&mut turn);
    assert_eq!(turn.body.steps.len(), 4);
    let TurnStep::ToolBatch(b1) = &turn.body.steps[1] else {
        panic!()
    };
    assert_eq!(b1.items.len(), 1);
    assert_eq!(b1.items[0].call.id.as_str(), "X");
    assert!(matches!(
        b1.items[0].state,
        ToolExecState::Cancelled(CancelCause::CrashRecovery)
    ));
    let TurnStep::ToolBatch(b2) = &turn.body.steps[3] else {
        panic!()
    };
    assert_eq!(b2.items.len(), 1);
    assert_eq!(b2.items[0].call.id.as_str(), "Y");
}

#[test]
fn synthesize_handles_legacy_pre_merge_batch() {
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(llm_call_with_tool("X"));
    turn.body.steps.push(llm_call_with_tool("Y"));
    turn.body.steps.push(TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![
            ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new("X"),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                state: ToolExecState::Pending,
            },
            ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new("Y"),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                state: ToolExecState::Pending,
            },
        ],
    }));
    synthesize_missing_tool_batches(&mut turn);
    let mut id_count: std::collections::HashMap<String, usize> = Default::default();
    for step in &turn.body.steps {
        if let TurnStep::ToolBatch(b) = step {
            for item in &b.items {
                *id_count
                    .entry(item.call.id.as_str().to_string())
                    .or_insert(0) += 1;
            }
        }
    }
    assert_eq!(id_count.get("X").copied().unwrap_or(0), 1);
    assert_eq!(id_count.get("Y").copied().unwrap_or(0), 1);
}

#[test]
fn synthesize_runs_for_complete_turn_missing_batch() {
    let mut turn = turn_with_llm_call_and_tools(&["tc-1"]);
    turn.outcome = TurnOutcome::Complete;
    let mut turns = vec![turn];
    finalize_incomplete_turns(&mut turns);
    assert!(matches!(turns[0].outcome, TurnOutcome::Complete));
    let has_batch = turns[0]
        .body
        .steps
        .iter()
        .any(|s| matches!(s, TurnStep::ToolBatch(_)));
    assert!(has_batch);
}

#[test]
fn finalize_incomplete_turns_synthesizes_for_inprogress() {
    let turn = turn_with_llm_call_and_tools(&["tc-1"]);
    let mut turns = vec![turn];
    finalize_incomplete_turns(&mut turns);
    assert!(matches!(
        turns[0].outcome,
        TurnOutcome::Cancelled {
            cause: CancelledCause::CrashRecovery
        }
    ));
    assert_eq!(turns[0].body.steps.len(), 2);
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[1] else {
        panic!()
    };
    assert!(matches!(
        batch.items[0].state,
        ToolExecState::Cancelled(CancelCause::CrashRecovery)
    ));
}
