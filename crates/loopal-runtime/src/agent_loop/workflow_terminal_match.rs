use loopal_protocol::WorkflowTerminalDeliveryId;
use loopal_turn::{Turn, TurnEvent, TurnTrigger};

pub(super) enum PersistedDelivery {
    Absent,
    Exact { should_execute: bool },
    Conflict,
}

pub(super) fn classify(
    events: &[TurnEvent],
    turns: &[Turn],
    delivery_id: &WorkflowTerminalDeliveryId,
    payload_digest: &str,
) -> PersistedDelivery {
    let persisted: Vec<_> = events
        .iter()
        .filter_map(|event| event_delivery(event, delivery_id))
        .collect();
    if persisted.is_empty() {
        return PersistedDelivery::Absent;
    }
    if persisted
        .iter()
        .any(|(_, digest)| *digest != payload_digest)
    {
        return PersistedDelivery::Conflict;
    }
    let is_completed = |turn_id: &loopal_turn::TurnId| {
        events.iter().any(|event| {
            matches!(event, TurnEvent::TurnEnded { turn_id: ended, .. } if ended == turn_id)
        })
    };
    let completed = persisted.iter().any(|(turn_id, _)| is_completed(turn_id));
    let unfinished = persisted.iter().any(|(turn_id, _)| !is_completed(turn_id));
    let should_execute = !completed
        && unfinished
        && turns.iter().any(|turn| {
            trigger_digest(&turn.trigger, delivery_id) == Some(payload_digest)
                && matches!(
                    &turn.outcome,
                    loopal_turn::TurnOutcome::Cancelled {
                        cause: loopal_turn::CancelledCause::CrashRecovery
                    }
                )
        });
    PersistedDelivery::Exact { should_execute }
}

pub(super) fn contains_exact(
    turns: &[Turn],
    delivery_id: &WorkflowTerminalDeliveryId,
    payload_digest: &str,
) -> bool {
    turns
        .iter()
        .any(|turn| trigger_digest(&turn.trigger, delivery_id) == Some(payload_digest))
}

pub(super) fn resume_trigger(turns: &[Turn]) -> Option<TurnTrigger> {
    let last = turns.last().filter(|turn| {
        matches!(
            turn.outcome,
            loopal_turn::TurnOutcome::Cancelled {
                cause: loopal_turn::CancelledCause::CrashRecovery
            }
        )
    })?;
    let trigger @ TurnTrigger::WorkflowResult {
        session_id,
        run_id,
        terminal_revision,
        payload_digest,
        ..
    } = &last.trigger
    else {
        return None;
    };
    if turns.iter().any(|turn| {
        !matches!(
            turn.outcome,
            loopal_turn::TurnOutcome::Cancelled {
                cause: loopal_turn::CancelledCause::CrashRecovery
            }
        ) && matches!(
            &turn.trigger,
            TurnTrigger::WorkflowResult {
                session_id: other_session,
                run_id: other_run,
                terminal_revision: other_revision,
                payload_digest: other_digest,
                ..
            } if other_session == session_id
                && other_run == run_id
                && other_revision == terminal_revision
                && other_digest == payload_digest
        )
    }) {
        return None;
    }
    Some(trigger.clone())
}

fn event_delivery<'a>(
    event: &'a TurnEvent,
    delivery_id: &WorkflowTerminalDeliveryId,
) -> Option<(&'a loopal_turn::TurnId, &'a str)> {
    match event {
        TurnEvent::TurnStarted {
            turn_id, trigger, ..
        } => trigger_digest(trigger, delivery_id).map(|digest| (turn_id, digest)),
        _ => None,
    }
}

fn trigger_digest<'a>(
    trigger: &'a TurnTrigger,
    delivery_id: &WorkflowTerminalDeliveryId,
) -> Option<&'a str> {
    match trigger {
        TurnTrigger::WorkflowResult {
            session_id,
            run_id,
            terminal_revision,
            payload_digest,
            ..
        } if session_id == &delivery_id.session_id
            && run_id == delivery_id.run_id.as_str()
            && terminal_revision == &delivery_id.terminal_revision =>
        {
            Some(payload_digest)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "workflow_terminal_match_tests.rs"]
mod tests;
