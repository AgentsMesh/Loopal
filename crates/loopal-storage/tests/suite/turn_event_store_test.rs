use chrono::Utc;
use loopal_storage::{TurnEventStore, fold_events};
use loopal_turn::{
    AssistantOutput, CancelCause, CancelledCause, OrderedToolBatch, StopReason, ToolBatchItem,
    ToolCall, ToolCallId, ToolExecState, ToolResult, TurnEvent, TurnId, TurnOutcome, TurnStep,
    TurnTrigger,
};
use tempfile::TempDir;

fn store(td: &TempDir) -> TurnEventStore {
    TurnEventStore::with_base_dir(td.path().to_path_buf())
}

fn user_trigger() -> TurnTrigger {
    TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "hi".into(),
        images: Vec::new(),
    }
}

fn empty_llm_step() -> TurnStep {
    TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    }
}

fn tool_batch_step(call_id: &str, name: &str) -> TurnStep {
    TurnStep::ToolBatch(OrderedToolBatch {
        items: vec![ToolBatchItem {
            call: ToolCall {
                id: ToolCallId::new(call_id),
                name: name.into(),
                input: serde_json::json!({}),
            },
            state: ToolExecState::Pending,
        }],
    })
}

#[test]
fn append_and_load_events_roundtrip() {
    let td = TempDir::new().unwrap();
    let s = store(&td);
    let session = "sess-a";
    let turn_id = TurnId::new();

    s.append_event(
        session,
        &TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: Utc::now(),
            trigger: user_trigger(),
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    )
    .unwrap();

    let events = s.load_events(session).unwrap();
    assert_eq!(events.len(), 3);
}

#[test]
fn load_turns_folds_events_into_completed_turn() {
    let td = TempDir::new().unwrap();
    let s = store(&td);
    let session = "sess-b";
    let turn_id = TurnId::new();

    s.append_event(
        session,
        &TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: Utc::now(),
            trigger: user_trigger(),
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    )
    .unwrap();

    let turns = s.load_turns(session).unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id, turn_id);
    assert_eq!(turns[0].outcome, TurnOutcome::Complete);
    assert_eq!(turns[0].body.steps.len(), 1);
}

#[test]
fn step_update_patches_tool_state() {
    let td = TempDir::new().unwrap();
    let s = store(&td);
    let session = "sess-c";
    let turn_id = TurnId::new();

    s.append_event(
        session,
        &TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: Utc::now(),
            trigger: user_trigger(),
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: tool_batch_step("x", "Read"),
            appended_at: None,
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index: 0,
            item_index: 0,
            new_state: ToolExecState::Done(ToolResult {
                content: "OK".into(),
                is_error: false,
                images: vec![],
                metadata: None,
            }),
            updated_at: None,
        },
    )
    .unwrap();
    s.append_event(
        session,
        &TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    )
    .unwrap();

    let turns = s.load_turns(session).unwrap();
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[0] else {
        panic!("expected ToolBatch");
    };
    assert!(matches!(batch.items[0].state, ToolExecState::Done(_)));
}

#[test]
fn missing_turn_ended_becomes_crash_recovery_cancelled() {
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: TurnId::from_string("t-x"),
            started_at: Utc::now(),
            trigger: user_trigger(),
        },
        TurnEvent::StepAppended {
            turn_id: TurnId::from_string("t-x"),
            step_index: 0,
            step: tool_batch_step("y", "Bash"),
            appended_at: None,
        },
    ];
    let turns = fold_events(events);
    assert_eq!(turns.len(), 1);
    assert!(matches!(
        turns[0].outcome,
        TurnOutcome::Cancelled {
            cause: CancelledCause::CrashRecovery
        }
    ));
    let TurnStep::ToolBatch(batch) = &turns[0].body.steps[0] else {
        panic!("expected ToolBatch");
    };
    assert!(matches!(
        batch.items[0].state,
        ToolExecState::Cancelled(CancelCause::CrashRecovery)
    ));
}

#[test]
fn load_returns_empty_when_file_absent() {
    let td = TempDir::new().unwrap();
    let s = store(&td);
    assert!(s.load_events("nonexistent").unwrap().is_empty());
    assert!(s.load_turns("nonexistent").unwrap().is_empty());
}

#[test]
fn multiple_turns_load_in_order() {
    let td = TempDir::new().unwrap();
    let s = store(&td);
    let session = "sess-d";
    let t1 = TurnId::from_string("t-1");
    let t2 = TurnId::from_string("t-2");

    for tid in [&t1, &t2] {
        s.append_event(
            session,
            &TurnEvent::TurnStarted {
                turn_id: tid.clone(),
                started_at: Utc::now(),
                trigger: user_trigger(),
            },
        )
        .unwrap();
        s.append_event(
            session,
            &TurnEvent::TurnEnded {
                turn_id: tid.clone(),
                outcome: TurnOutcome::Complete,
            },
        )
        .unwrap();
    }

    let turns = s.load_turns(session).unwrap();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].id, t1);
    assert_eq!(turns[1].id, t2);
}
