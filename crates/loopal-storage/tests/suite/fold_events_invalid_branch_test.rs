use chrono::Utc;
use loopal_storage::fold_events;
use loopal_turn::{
    AssistantOutput, InjectionKind, OrderedToolBatch, StopReason, ToolExecState, TurnEvent, TurnId,
    TurnOutcome, TurnStep, TurnTrigger,
};

fn empty_llm_step() -> TurnStep {
    TurnStep::LlmCall {
        model: "m".into(),
        response: AssistantOutput {
            text_blocks: Vec::new(),
            tool_calls: Vec::new(),
            server_blocks: Vec::new(),
            stop_reason: StopReason::EndTurn,
        },
    }
}

#[test]
fn ignores_unknown_turns_and_invalid_tool_updates() {
    let turn_id = TurnId::from_string("known");
    let missing = TurnId::from_string("missing");
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: Utc::now(),
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: missing.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
        TurnEvent::StepUpdated {
            turn_id: missing.clone(),
            step_index: 0,
            item_index: 0,
            new_state: ToolExecState::Pending,
            updated_at: None,
        },
        TurnEvent::TurnEnded {
            turn_id: missing,
            outcome: TurnOutcome::Complete,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: TurnStep::Injection {
                kind: InjectionKind::SystemNote,
                text: "note".into(),
            },
            appended_at: None,
        },
        TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index: 1,
            item_index: 0,
            new_state: ToolExecState::Pending,
            updated_at: None,
        },
        TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index: 0,
            item_index: 0,
            new_state: ToolExecState::Pending,
            updated_at: None,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: TurnStep::ToolBatch(OrderedToolBatch { items: Vec::new() }),
            appended_at: None,
        },
        TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index: 0,
            item_index: 1,
            new_state: ToolExecState::Pending,
            updated_at: None,
        },
        TurnEvent::TurnEnded {
            turn_id,
            outcome: TurnOutcome::Complete,
        },
    ];

    let turns = fold_events(events);
    assert_eq!(turns.len(), 1);
    assert!(matches!(turns[0].body.steps[0], TurnStep::ToolBatch(_)));
    assert_eq!(turns[0].outcome, TurnOutcome::Complete);
}

#[test]
fn replaces_existing_steps_and_fills_sparse_indices() {
    let turn_id = TurnId::from_string("sparse");
    let events = vec![
        TurnEvent::TurnStarted {
            turn_id: turn_id.clone(),
            started_at: Utc::now(),
            trigger: TurnTrigger::Resume,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: empty_llm_step(),
            appended_at: None,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 0,
            step: TurnStep::Injection {
                kind: InjectionKind::SystemNote,
                text: "replacement".into(),
            },
            appended_at: None,
        },
        TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: 2,
            step: empty_llm_step(),
            appended_at: None,
        },
        TurnEvent::TurnEnded {
            turn_id,
            outcome: TurnOutcome::Complete,
        },
    ];

    let turns = fold_events(events);
    assert_eq!(turns[0].body.steps.len(), 3);
    assert!(matches!(turns[0].body.steps[0], TurnStep::Injection { .. }));
    assert!(matches!(turns[0].body.steps[1], TurnStep::Injection { .. }));
    assert!(matches!(turns[0].body.steps[2], TurnStep::LlmCall { .. }));
}
