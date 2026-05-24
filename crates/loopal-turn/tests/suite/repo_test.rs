use loopal_turn::{
    AssistantOutput, InMemoryTurnRepo, LlmRequestSnapshot, OrderedToolBatch, StopReason,
    ToolBatchItem, ToolCall, ToolCallId, ToolExecState, ToolResult, TurnEvent, TurnOutcome,
    TurnRepo, TurnRepoError, TurnStep, TurnTrigger,
};

fn snapshot() -> LlmRequestSnapshot {
    LlmRequestSnapshot {
        model: "claude-opus-4-7".into(),
        max_tokens: 64_000,
        tool_count: 1,
        message_count: 1,
    }
}

fn user_trigger() -> TurnTrigger {
    TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "hello".into(),
        images: Vec::new(),
    }
}

#[test]
fn start_then_end_turn() {
    let repo = InMemoryTurnRepo::new();
    let id = repo.start_turn(user_trigger()).unwrap();
    repo.end_turn(&id, TurnOutcome::Complete).unwrap();

    let snap = repo.snapshot_turn(&id).unwrap();
    assert_eq!(snap.outcome, TurnOutcome::Complete);
    assert!(snap.body.steps.is_empty());
}

#[test]
fn append_step_emits_event_and_grows_body() {
    let repo = InMemoryTurnRepo::new();
    let id = repo.start_turn(user_trigger()).unwrap();
    let idx = repo
        .append_step(
            &id,
            TurnStep::LlmCall {
                request_snapshot: snapshot(),
                response: AssistantOutput {
                    thinking: None,
                    text_blocks: vec![],
                    tool_calls: vec![],
                    server_blocks: vec![],
                    stop_reason: StopReason::EndTurn,
                },
            },
        )
        .unwrap();
    assert_eq!(idx, 0);

    let events = repo.events();
    assert!(matches!(events[0], TurnEvent::TurnStarted { .. }));
    assert!(matches!(events[1], TurnEvent::StepAppended { .. }));
}

#[test]
fn update_tool_state_round_trip() {
    let repo = InMemoryTurnRepo::new();
    let id = repo.start_turn(user_trigger()).unwrap();
    let call = ToolCall {
        id: ToolCallId::new("c1"),
        name: "Bash".into(),
        input: serde_json::json!({"command":"ls"}),
    };
    repo.append_step(
        &id,
        TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call: call.clone(),
                state: ToolExecState::Pending,
            }],
        }),
    )
    .unwrap();

    repo.update_tool_state(
        &id,
        0,
        0,
        ToolExecState::Done(ToolResult {
            content: "ok".into(),
            is_error: false,
            images: vec![],
        }),
    )
    .unwrap();

    let snap = repo.snapshot_turn(&id).unwrap();
    let TurnStep::ToolBatch(batch) = &snap.body.steps[0] else {
        panic!()
    };
    assert!(matches!(batch.items[0].state, ToolExecState::Done(_)));
}

#[test]
fn cannot_append_after_end() {
    let repo = InMemoryTurnRepo::new();
    let id = repo.start_turn(user_trigger()).unwrap();
    repo.end_turn(&id, TurnOutcome::Complete).unwrap();
    let err = repo
        .append_step(
            &id,
            TurnStep::LlmCall {
                request_snapshot: snapshot(),
                response: AssistantOutput {
                    thinking: None,
                    text_blocks: vec![],
                    tool_calls: vec![],
                    server_blocks: vec![],
                    stop_reason: StopReason::EndTurn,
                },
            },
        )
        .unwrap_err();
    assert!(matches!(err, TurnRepoError::TurnAlreadyEnded(_)));
}

#[test]
fn update_state_rejects_non_tool_batch_step() {
    let repo = InMemoryTurnRepo::new();
    let id = repo.start_turn(user_trigger()).unwrap();
    repo.append_step(
        &id,
        TurnStep::LlmCall {
            request_snapshot: snapshot(),
            response: AssistantOutput {
                thinking: None,
                text_blocks: vec![],
                tool_calls: vec![],
                server_blocks: vec![],
                stop_reason: StopReason::EndTurn,
            },
        },
    )
    .unwrap();
    let err = repo
        .update_tool_state(&id, 0, 0, ToolExecState::Pending)
        .unwrap_err();
    assert!(matches!(err, TurnRepoError::StepNotToolBatch));
}

#[test]
fn load_turns_after_multiple_turns() {
    let repo = InMemoryTurnRepo::new();
    let _ = repo.start_turn(user_trigger()).unwrap();
    let _ = repo.start_turn(TurnTrigger::Resume).unwrap();
    let turns = repo.load_turns().unwrap();
    assert_eq!(turns.len(), 2);
}
