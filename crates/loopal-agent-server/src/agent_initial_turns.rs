use loopal_runtime::SessionManager;
use loopal_turn::{Turn, TurnEvent};

use crate::agent_setup_helpers::build_fork_synthetic_turn;
use crate::params::StartParams;

pub fn initial_turns_for_start(
    start: &StartParams,
    session_manager: &SessionManager,
    session_id: &str,
    mut resume_turns: Vec<Turn>,
) -> anyhow::Result<Vec<Turn>> {
    let Some(fork_turn) = build_fork_synthetic_turn(start) else {
        return Ok(resume_turns);
    };
    persist_fork_turn(session_manager, session_id, &fork_turn)?;
    resume_turns.insert(0, fork_turn);
    Ok(resume_turns)
}

fn persist_fork_turn(
    session_manager: &SessionManager,
    session_id: &str,
    turn: &Turn,
) -> anyhow::Result<()> {
    session_manager.record_turn_event(
        session_id,
        &TurnEvent::TurnStarted {
            turn_id: turn.id.clone(),
            started_at: turn.started_at,
            trigger: turn.trigger.clone(),
        },
    )?;
    for (step_index, step) in turn.body.steps.iter().enumerate() {
        session_manager.record_turn_event(
            session_id,
            &TurnEvent::StepAppended {
                turn_id: turn.id.clone(),
                step_index: step_index as u32,
                step: step.clone(),
                appended_at: None,
            },
        )?;
    }
    Ok(())
}
