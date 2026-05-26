use loopal_turn::{Turn, TurnBody, TurnEvent, TurnOutcome, TurnStep};

use super::synthesize::finalize_incomplete_turns;

pub fn fold_events(events: Vec<TurnEvent>) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for event in events {
        match event {
            TurnEvent::TurnStarted {
                turn_id,
                started_at,
                trigger,
            } => {
                turns.push(Turn {
                    id: turn_id,
                    started_at,
                    trigger,
                    body: TurnBody::default(),
                    outcome: TurnOutcome::InProgress,
                    last_step_at: None,
                });
            }
            TurnEvent::StepAppended {
                turn_id,
                step_index,
                step,
                appended_at,
            } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    apply_step_append(&mut turn.body.steps, step_index, step);
                    if let Some(ts) = appended_at {
                        turn.last_step_at = Some(ts);
                    }
                }
            }
            TurnEvent::StepUpdated {
                turn_id,
                step_index,
                item_index,
                new_state,
                updated_at,
            } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    apply_step_update(&mut turn.body.steps, step_index, item_index, new_state);
                    if let Some(ts) = updated_at {
                        turn.last_step_at = Some(ts);
                    }
                }
            }
            TurnEvent::TurnEnded { turn_id, outcome } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    turn.outcome = outcome;
                }
            }
            TurnEvent::Cleared {
                cancel_in_progress, ..
            } => {
                apply_cancel_in_progress(&mut turns, cancel_in_progress.as_ref(), "Cleared");
                turns.clear();
            }
            TurnEvent::Rewound {
                keep,
                cancel_in_progress,
                ..
            } => {
                apply_cancel_in_progress(&mut turns, cancel_in_progress.as_ref(), "Rewound");
                let n = keep as usize;
                if n < turns.len() {
                    turns.truncate(n);
                }
            }
        }
    }
    finalize_incomplete_turns(&mut turns);
    apply_legacy_last_step_at_fallback(&mut turns);
    turns
}

fn apply_cancel_in_progress(
    turns: &mut [Turn],
    turn_id: Option<&loopal_turn::TurnId>,
    event_kind: &str,
) {
    let Some(turn_id) = turn_id else { return };
    let Some(turn) = turns.iter_mut().find(|t| &t.id == turn_id) else {
        return;
    };
    if matches!(turn.outcome, TurnOutcome::InProgress) {
        turn.outcome = TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::ParentTurnAborted,
        };
    } else {
        super::tracing_warn(&format!(
            "{event_kind} cancel_in_progress={} target already non-InProgress; skipping cancel overwrite",
            turn_id.as_str()
        ));
    }
}

fn apply_legacy_last_step_at_fallback(turns: &mut [Turn]) {
    let resume_now = chrono::Utc::now();
    for turn in turns.iter_mut() {
        if turn.last_step_at.is_none()
            && turn
                .body
                .steps
                .iter()
                .any(|s| matches!(s, TurnStep::LlmCall { .. }))
        {
            turn.last_step_at = Some(resume_now.max(turn.started_at));
        }
    }
}

fn apply_step_append(steps: &mut Vec<TurnStep>, step_index: u32, step: TurnStep) {
    let idx = step_index as usize;
    if idx == steps.len() {
        steps.push(step);
    } else if idx < steps.len() {
        steps[idx] = step;
    } else {
        steps.resize_with(idx, || TurnStep::Injection {
            kind: loopal_turn::InjectionKind::SystemNote,
            text: String::new(),
        });
        steps.push(step);
    }
}

fn apply_step_update(
    steps: &mut [TurnStep],
    step_index: u32,
    item_index: u32,
    new_state: loopal_turn::ToolExecState,
) {
    let Some(step) = steps.get_mut(step_index as usize) else {
        return;
    };
    if let TurnStep::ToolBatch(batch) = step
        && let Some(item) = batch.items.get_mut(item_index as usize)
    {
        item.state = new_state;
    }
}
