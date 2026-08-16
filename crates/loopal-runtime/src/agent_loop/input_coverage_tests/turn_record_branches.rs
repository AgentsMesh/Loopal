use chrono::Utc;
use loopal_turn::{ToolExecState, ToolResult, Turn, TurnOutcome, TurnTrigger};

use super::support::make_fixture;

fn turn(id: &str, outcome: TurnOutcome) -> Turn {
    Turn {
        id: loopal_turn::TurnId::from_string(id),
        started_at: Utc::now(),
        trigger: TurnTrigger::Resume,
        body: Default::default(),
        outcome,
        last_step_at: None,
    }
}

#[test]
fn invalid_batch_update_and_turn_end_leave_the_empty_store_unchanged() {
    let mut fixture = make_fixture();
    fixture.runner.update_tool_batch_item_state(
        0,
        ToolExecState::Done(ToolResult {
            content: "orphan".into(),
            is_error: true,
            images: Vec::new(),
            metadata: None,
        }),
    );
    fixture.runner.end_turn_record(TurnOutcome::Complete);
    assert!(fixture.runner.recorded_turns().is_empty());
}

#[test]
#[should_panic(expected = "InProgress turn at index 0 is not the last")]
fn seeded_in_progress_turn_must_be_the_trailing_turn() {
    let mut fixture = make_fixture();
    fixture.runner.seed_test_turns(vec![
        turn("in-progress", TurnOutcome::InProgress),
        turn("complete", TurnOutcome::Complete),
    ]);
}
