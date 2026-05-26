use chrono::Utc;
use loopal_storage::fold_events;
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger,
};

fn empty_llm_step() -> TurnStep {
    TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: Vec::new(),
            tool_calls: Vec::new(),
            server_blocks: Vec::new(),
            stop_reason: StopReason::EndTurn,
        },
    }
}

#[test]
fn fold_events_restores_last_step_at_from_step_appended() {
    let turn_id = TurnId::from_string("t-1");
    let started = Utc::now() - chrono::Duration::seconds(120);
    let appended = Utc::now() - chrono::Duration::seconds(30);
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: started,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: Some(appended),
        },
        TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    ];
    let turns = fold_events(events);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].last_step_at, Some(appended));
}

#[test]
fn fold_events_restores_last_step_at_from_step_updated() {
    let turn_id = TurnId::from_string("t-1");
    let started = Utc::now() - chrono::Duration::seconds(120);
    let appended = Utc::now() - chrono::Duration::seconds(60);
    let updated = Utc::now() - chrono::Duration::seconds(10);
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: started,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: TurnStep::ToolBatch(OrderedToolBatch {
                items: vec![ToolBatchItem {
                    call: ToolCall {
                        id: ToolCallId::new("x"),
                        name: "Read".into(),
                        input: serde_json::json!({}),
                    },
                    state: ToolExecState::Pending,
                }],
            }),
            appended_at: Some(appended),
        },
        TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index: 0,
            item_index: 0,
            new_state: ToolExecState::Done(loopal_turn::ToolResult {
                content: "ok".into(),
                images: Vec::new(),
                is_error: false,
            }),
            updated_at: Some(updated),
        },
    ];
    let turns = fold_events(events);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].last_step_at, Some(updated));
}

#[test]
fn fold_events_legacy_no_timestamp_falls_back_to_resume_now() {
    let turn_id = TurnId::from_string("t-1");
    let started = Utc::now() - chrono::Duration::hours(8);
    let before_fold = Utc::now();
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: started,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
        TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    ];
    let turns = fold_events(events);
    let last = turns[0].last_step_at.expect("legacy fallback must set");
    assert!(last >= before_fold);
}

#[test]
fn fold_events_fallback_uses_max_resume_now_or_started_at_for_clock_backwards() {
    let future_started = Utc::now() + chrono::Duration::hours(1);
    let turn_id = TurnId::from_string("t-1");
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: future_started,
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
        TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: TurnOutcome::Complete,
        },
    ];
    let turns = fold_events(events);
    let last = turns[0].last_step_at.expect("fallback must set");
    assert!(last >= future_started);
}
