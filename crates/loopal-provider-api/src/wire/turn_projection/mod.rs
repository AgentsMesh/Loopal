mod blocks;
mod compaction;
mod step;
mod trigger;

use loopal_turn::{Turn, TurnStep};

use self::step::project_step;
use self::trigger::project_trigger;
use super::message::Message;

pub fn project_turns_to_messages(turns: &[Turn]) -> Vec<Message> {
    let drop_before = find_compaction_drop_index(turns);
    turns
        .iter()
        .skip(drop_before)
        .flat_map(project_turn_to_messages)
        .collect()
}

pub fn project_turn_to_messages(turn: &Turn) -> Vec<Message> {
    let mut out = Vec::new();
    // CompactionSummary represents pre-boundary history — it must project
    // BEFORE the turn's own trigger + steps, or an in-progress UserInput turn
    // with appended summary would emit [user X, summary, ack] (summary AFTER
    // user query). debug_assert pins the at-most-one invariant; release-mode
    // recovery: keep only the latest summary, dropping any surplus.
    let summary_steps: Vec<&TurnStep> = turn
        .body
        .steps
        .iter()
        .filter(|s| matches!(s, TurnStep::CompactionSummary(_)))
        .collect();
    debug_assert!(
        summary_steps.len() <= 1,
        "turn {} has {} CompactionSummary steps; hoist loses causal ordering",
        turn.id.as_str(),
        summary_steps.len()
    );
    if let Some(latest) = summary_steps.last() {
        out.extend(project_step(latest));
    }
    if let Some(msg) = project_trigger(&turn.trigger) {
        out.push(msg);
    }
    for step in &turn.body.steps {
        if !matches!(step, TurnStep::CompactionSummary(_)) {
            out.extend(project_step(step));
        }
    }
    out
}

fn find_compaction_drop_index(turns: &[Turn]) -> usize {
    for (i, turn) in turns.iter().enumerate().rev() {
        if turn
            .body
            .steps
            .iter()
            .any(|s| matches!(s, TurnStep::CompactionSummary(_)))
        {
            return i;
        }
    }
    0
}

#[cfg(test)]
mod tests;
