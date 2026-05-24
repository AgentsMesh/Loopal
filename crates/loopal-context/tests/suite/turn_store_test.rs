use loopal_context::{ContextBudget, TurnStore, TurnStoreError};
use loopal_turn::{AssistantOutput, StopReason, TurnOutcome, TurnStep, TurnTrigger};

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

fn user_trigger() -> TurnTrigger {
    TurnTrigger::UserInput {
        envelope_id: "env-1".into(),
        content: "hi".into(),
        images: Vec::new(),
    }
}

fn llm_step() -> TurnStep {
    TurnStep::LlmCall {
        model: "claude-opus-4-7".into(),
        response: AssistantOutput {
            thinking: None,
            text_blocks: vec![],
            tool_calls: vec![],
            server_blocks: vec![],
            stop_reason: StopReason::EndTurn,
        },
    }
}

#[test]
fn empty_store_has_no_current_turn() {
    let store = TurnStore::new(budget());
    assert!(store.current_turn().is_none());
    assert_eq!(store.len(), 0);
}

#[test]
fn start_turn_creates_in_progress_turn() {
    let mut store = TurnStore::new(budget());
    let id = store.start_turn(user_trigger());
    assert_eq!(store.len(), 1);
    let cur = store.current_turn().unwrap();
    assert_eq!(cur.id, id);
    assert_eq!(cur.outcome, TurnOutcome::InProgress);
}

#[test]
fn append_step_requires_current_turn() {
    let mut store = TurnStore::new(budget());
    let err = store.append_step(llm_step()).unwrap_err();
    assert!(matches!(err, TurnStoreError::NoCurrentTurn));
}

#[test]
fn end_current_turn_clears_in_progress() {
    let mut store = TurnStore::new(budget());
    store.start_turn(user_trigger());
    store.append_step(llm_step()).unwrap();
    store.end_current_turn(TurnOutcome::Complete).unwrap();
    assert!(store.current_turn().is_none());
    assert_eq!(store.turns()[0].outcome, TurnOutcome::Complete);
}

#[test]
fn cannot_append_after_end() {
    let mut store = TurnStore::new(budget());
    store.start_turn(user_trigger());
    store.end_current_turn(TurnOutcome::Complete).unwrap();
    let err = store.append_step(llm_step()).unwrap_err();
    assert!(matches!(err, TurnStoreError::NoCurrentTurn));
}

#[test]
fn from_turns_recovers_in_progress() {
    let mut store = TurnStore::new(budget());
    let id = store.start_turn(user_trigger());
    store.append_step(llm_step()).unwrap();
    let turns = store.turns().to_vec();

    let restored = TurnStore::from_turns(turns, budget());
    assert_eq!(restored.current_turn_id().unwrap(), &id);
}

#[test]
fn from_turns_no_current_when_all_complete() {
    let mut store = TurnStore::new(budget());
    store.start_turn(user_trigger());
    store.end_current_turn(TurnOutcome::Complete).unwrap();
    let turns = store.turns().to_vec();

    let restored = TurnStore::from_turns(turns, budget());
    assert!(restored.current_turn_id().is_none());
}
