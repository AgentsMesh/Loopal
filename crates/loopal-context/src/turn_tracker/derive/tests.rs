use loopal_turn::{ToolExecState, TurnStep, TurnTrigger};

use super::super::TurnTracker;
use super::derive_current_tool_batch_step;
use crate::budget::ContextBudget;
use crate::turn_store::TurnStore;

fn store_with_steps(steps: Vec<TurnStep>) -> TurnStore {
    let mut store = TurnStore::new();
    store.start_turn(TurnTrigger::UserInput {
        envelope_id: "e".into(),
        content: "go".into(),
        images: Vec::new(),
    });
    for step in steps {
        store.append_step(step).unwrap();
    }
    store
}

fn llm_call_with_tools(ids: &[&str]) -> TurnStep {
    use loopal_turn::{AssistantOutput, StopReason, ToolCall, ToolCallId};
    TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            text_blocks: Vec::new(),
            tool_calls: ids
                .iter()
                .map(|id| ToolCall {
                    id: ToolCallId::new(*id),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                })
                .collect(),
            server_blocks: Vec::new(),
            stop_reason: StopReason::ToolUse,
        },
    }
}

fn tool_batch(items: Vec<(&str, ToolExecState)>) -> TurnStep {
    use loopal_turn::{OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId};
    TurnStep::ToolBatch(OrderedToolBatch {
        items: items
            .into_iter()
            .map(|(id, state)| ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new(id),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                state,
            })
            .collect(),
    })
}

fn test_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_000,
        safety_margin: 1_000,
        message_budget: 100_000,
        max_output_tokens: 16_000,
    }
}

#[test]
fn derive_returns_none_when_last_batch_is_closed_even_if_earlier_is_pending() {
    let store = store_with_steps(vec![
        llm_call_with_tools(&["a"]),
        tool_batch(vec![("a", ToolExecState::Pending)]),
        llm_call_with_tools(&["b"]),
        tool_batch(vec![(
            "b",
            ToolExecState::Done(loopal_turn::ToolResult {
                content: "ok".into(),
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }),
        )]),
    ]);
    assert_eq!(derive_current_tool_batch_step(&store), None);
}

#[test]
fn derive_returns_last_pending_batch_idx() {
    let store = store_with_steps(vec![
        llm_call_with_tools(&["a"]),
        tool_batch(vec![("a", ToolExecState::Pending)]),
    ]);
    assert_eq!(derive_current_tool_batch_step(&store), Some(1));
}

#[test]
fn derive_returns_none_when_no_batch() {
    let store = store_with_steps(vec![llm_call_with_tools(&[])]);
    assert_eq!(derive_current_tool_batch_step(&store), None);
}

#[test]
fn derive_returns_none_when_no_current_turn() {
    let store = TurnStore::new();
    assert_eq!(derive_current_tool_batch_step(&store), None);
}

#[test]
fn turn_tracker_new_recovers_open_batch_on_resume() {
    let store = store_with_steps(vec![
        llm_call_with_tools(&["a"]),
        tool_batch(vec![("a", ToolExecState::Pending)]),
    ]);
    let tracker = TurnTracker::new(store, test_budget());
    assert_eq!(tracker.current_tool_batch_step(), Some(1));
}

#[test]
fn with_wire_mut_preserves_open_batch_reference() {
    let store = store_with_steps(vec![
        llm_call_with_tools(&["a"]),
        tool_batch(vec![("a", ToolExecState::Pending)]),
    ]);
    let mut tracker = TurnTracker::new(store, test_budget());
    assert_eq!(tracker.current_tool_batch_step(), Some(1));
    tracker.with_wire_mut(|_turns| {});
    assert_eq!(tracker.current_tool_batch_step(), Some(1));
}

#[test]
fn replace_store_recovers_open_batch_via_derive() {
    let store = store_with_steps(vec![
        llm_call_with_tools(&["a"]),
        tool_batch(vec![("a", ToolExecState::Pending)]),
    ]);
    let mut tracker = TurnTracker::new(TurnStore::new(), test_budget());
    assert_eq!(tracker.current_tool_batch_step(), None);
    tracker.replace_store(store);
    assert_eq!(tracker.current_tool_batch_step(), Some(1));
}
