use loopal_context::{ContextBudget, PersistError, TurnEventLogger, TurnStore, TurnTracker};
use loopal_turn::{
    CancelCause, OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnEvent,
    TurnStep, TurnTrigger,
};

struct InMemoryLogger;
impl TurnEventLogger for InMemoryLogger {
    fn persist(&self, _event: &TurnEvent) -> Result<(), PersistError> {
        Ok(())
    }
}

fn budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
        max_output_tokens: 64_000,
    }
}

fn pending_item(id: &str) -> ToolBatchItem {
    ToolBatchItem {
        call: ToolCall {
            id: ToolCallId::new(id),
            name: "Bash".into(),
            input: serde_json::Value::Null,
        },
        state: ToolExecState::Pending,
    }
}

#[test]
fn cancel_open_tool_batch_marks_all_pending_as_cancelled() {
    let mut t = TurnTracker::new(TurnStore::new(), budget());
    t.try_start_turn(
        TurnTrigger::UserInput {
            envelope_id: "e".into(),
            content: "hi".into(),
            images: Vec::new(),
        },
        &InMemoryLogger,
    )
    .unwrap();
    let step_index = t
        .try_append_step(
            TurnStep::ToolBatch(OrderedToolBatch {
                items: vec![pending_item("t1"), pending_item("t2")],
            }),
            &InMemoryLogger,
        )
        .unwrap();
    t.mark_tool_batch_open(step_index);

    t.cancel_open_tool_batch(CancelCause::CrashRecovery, &InMemoryLogger);

    let turn = t.store().current_turn().unwrap();
    let TurnStep::ToolBatch(b) = &turn.body.steps[step_index as usize] else {
        panic!("expected ToolBatch step");
    };
    for item in &b.items {
        assert!(
            matches!(
                item.state,
                ToolExecState::Cancelled(CancelCause::CrashRecovery)
            ),
            "pending item must become Cancelled, got {:?}",
            item.state
        );
    }
}

#[test]
fn cancel_open_tool_batch_is_noop_without_open_batch() {
    let mut t = TurnTracker::new(TurnStore::new(), budget());
    // no open batch → must not panic
    t.cancel_open_tool_batch(CancelCause::UserInterrupt, &InMemoryLogger);
}
